// RobloxGameSettings.cs: an account's own Roblox in-game settings, written just before it opens.
//! Roblox keeps its in-game settings (graphics, FPS limit, volume…) in one file shared by every account:
//! %LOCALAPPDATA%\Roblox\GlobalBasicSettings_*.xml, read as a client starts. Writing values just before opening an
//! account gives it its own setup; only those values change (the rest stays byte for byte), and the person's own are
//! put back once that client has started.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

use crate::core::base::{Extra, Stamp};
use crate::core::roblox_api::{contains_ci, lock, starts_with_ci};
use crate::core::{durable_file, log, vault};

/// %LOCALAPPDATA%\Roblox; the checks point it elsewhere (`set_roblox_dir`).
static ROBLOX_DIR: RwLock<Option<String>> = RwLock::new(None);

fn roblox_dir() -> String {
    if let Some(d) = ROBLOX_DIR.read().unwrap_or_else(|e| e.into_inner()).clone() {
        return d;
    }
    crate::core::base::local_app_data().join("Roblox").to_string_lossy().into_owned()
}

/// Tests: Roblox's folder in place of the real one.
pub(crate) fn set_roblox_dir(dir: Option<String>) {
    *ROBLOX_DIR.write().unwrap_or_else(|e| e.into_inner()) = dir;
}

fn baseline_path() -> PathBuf {
    vault::data_dir().join("roblox-settings.json")
}

/// The values Bloxkeep manages, as (element type, name) in the file.
const KEYS: [(&str, &str); 5] =
    [("token", "SavedQualityLevel"), ("int", "GraphicsQualityLevel"), ("int", "FramerateCap"), ("float", "MasterVolume"), ("bool", "Fullscreen")];

struct FileLookup {
    file: Option<String>,
    next: Option<Instant>,
}

static SETTINGS_FILE: Mutex<FileLookup> = Mutex::new(FileLookup { file: None, next: None });
const FILE_LOOKUP_INTERVAL: Duration = Duration::from_secs(10);

/// `cached`: the file found last time is fine while it still exists; the folder is listed again every 10 s at most.
fn settings_file(cached: bool) -> Option<String> {
    let mut look = lock(&SETTINGS_FILE);
    if cached
        && let Some(file) = &look.file
        && look.next.is_some_and(|n| Instant::now() < n)
        && Path::new(file).is_file()
    {
        return Some(file.clone());
    }
    look.next = Some(Instant::now() + FILE_LOOKUP_INTERVAL);
    let dir = roblox_dir();
    let found = if Path::new(&dir).is_dir() {
        match std::fs::read_dir(&dir) {
            Ok(entries) => entries
                .flatten()
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    e.path().is_file() && starts_with_ci(&name, "GlobalBasicSettings_") && name.to_lowercase().ends_with(".xml") && !contains_ci(&name, "Studio")
                })
                .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH), e.path().to_string_lossy().into_owned()))
                // OrderByDescending keeps the first of equal times.
                .fold(None::<(SystemTime, String)>, |best, f| match best {
                    Some(b) if b.0 >= f.0 => Some(b),
                    _ => Some(f),
                })
                .map(|f| f.1),
            Err(_) => None,
        }
    } else {
        None
    };
    look.file = found.clone();
    found
}

/// Each of KEYS' elements, made once: `restore` looks every couple of seconds.
static ELEMENTS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    KEYS.iter().map(|(kind, name)| Regex::new(&format!("(<{kind} name=\"{}\">)([^<]*)(</{kind}>)", regex::escape(name))).unwrap()).collect()
});

/// The element of one of KEYS.
fn element(kind: &str, name: &str) -> &'static Regex {
    let i = KEYS.iter().position(|k| *k == (kind, name)).expect("one of KEYS");
    &ELEMENTS[i]
}

/// ToString("0.##", InvariantCulture).
fn two_places(v: f64) -> String {
    let text = format!("{:.2}", (v * 100.0).round_ties_even() / 100.0);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".into() } else { text.to_string() }
}

/// The values the given setup asks for (zero / -1 fields are left alone).
fn wanted(c: &RobloxConfig) -> HashMap<String, String> {
    let mut values = HashMap::new();
    if (1..=10).contains(&c.graphics) {
        values.insert("SavedQualityLevel".into(), c.graphics.to_string());
        values.insert("GraphicsQualityLevel".into(), engine_quality(c.graphics).to_string());
    }
    if c.fps > 0 {
        values.insert("FramerateCap".into(), c.fps.to_string());
    }
    if (0..=100).contains(&c.volume) {
        values.insert("MasterVolume".into(), two_places(c.volume as f64 / 100.0));
    }
    values
}

/// The graphics slider (1–10) as Roblox's internal quality level (1–21) in the same file.
pub fn engine_quality(slider: i32) -> i32 {
    if slider >= 10 { 21 } else { slider * 2 - 1 }
}

/// The frame rate limits offered, for the shared setup and each account (written like Roblox's own menu writes
/// its 30, 60, 144 and 240).
pub const FPS_CHOICES: [i32; 8] = [15, 20, 30, 45, 60, 120, 144, 240];

/// Any whole limit from 5 to 360 is taken, not only the ones offered (a user asked for 17 and 18, 2026-11-15): Roblox
/// reads FramerateCap as a plain number.
pub fn fps_allowed(fps: i32) -> bool {
    (5..=360).contains(&fps) || fps == FPS_UNLIMITED
}

/// No limit: Roblox's own menu stops at 240, and a FramerateCap above that lifts the cap. 9999 is what Voidstrap's FPS
/// unlocker writes (its Bootstrapper.cs, read 2026-11-15); the owner asked for it as Lunar Client has it.
pub const FPS_UNLIMITED: i32 = 9999;

/// What a client opens with; None: nothing of Bloxkeep's. An account's own limit (Account.fps_cap) goes over
/// the shared setup's and works without it; light AFK stays light, so only a lower limit goes over its own.
pub fn for_launch(shared: &RobloxConfig, afk: bool, uses_shared: bool, account_fps: i32) -> Option<RobloxConfig> {
    let mut config = if afk {
        Some(light_afk())
    } else if uses_shared {
        Some(RobloxConfig { graphics: shared.graphics, fps: shared.fps, volume: shared.volume, ..RobloxConfig::default() })
    } else {
        None
    };
    if account_fps <= 0 {
        return config;
    }
    let c = config.get_or_insert_with(RobloxConfig::default);
    if !afk || account_fps < c.fps {
        c.fps = account_fps;
    }
    config
}

/// A preset's light AFK profile: lowest graphics, the lowest FPS limit Roblox's menu offers (30; lower is
/// untested) and no sound.
pub fn light_afk() -> RobloxConfig {
    RobloxConfig { mode: "all".into(), graphics: 1, fps: 30, volume: 0, extra: Extra::new() }
}

/// Before opening an account: the shared setup if it uses it, the person's own values for everything else managed
/// (so an account never inherits an earlier one's, such as a light-AFK preset's 30 FPS). `windowed`:
/// fullscreen off too, since every client reads the one switch and saves it as it joins a game. The person's values
/// are what the file holds, except a value still equal to what Bloxkeep last wrote (as in `restore`).
/// False when there was nothing to write or the write failed.
pub fn prepare_for(config: &RobloxConfig, use_config: bool, windowed: bool) -> bool {
    let Some(file) = settings_file(false) else { return false };
    // The copy can't be read just now: nothing written, so the person's own in it is never replaced.
    let Some(baseline) = try_load_baseline() else { return false };
    if !use_config && !windowed && baseline.is_none() {
        return false; // nothing of ours was ever written: leave the file alone
    }
    let Some(text) = read(&file).filter(|t| !t.is_empty()) else { return false };
    let mut baseline = match baseline {
        None => {
            // A copy found damaged, the file holding Light AFK's values: those are Bloxkeep's, never taken for the person's
            // own (they'd be "put back" for good). The file is left as it is until the person's own are in it again.
            if holds_light_afk(&current(&text)) && had_damaged_copy() {
                return false;
            }
            let b = Baseline { values: Some(current(&text)), written: Some(HashMap::new()) };
            // No copy on disk, no change: one made without it could never be put back.
            if !save_baseline(&b) {
                return false;
            }
            b
        }
        Some(mut b) => {
            // What the file holds now is the person's own unless it is still what Bloxkeep wrote: changed in a Roblox
            // window since the copy, or missing from the copy (an older version didn't manage it).
            let mut refreshed = false;
            for (name, value) in current(&text) {
                if b.written().get(&name) == Some(&value) {
                    continue;
                }
                if b.values().get(&name) != Some(&value) {
                    b.values_mut().insert(name, value);
                    refreshed = true;
                }
            }
            // PORT: the C# goes on when this fails; then the person's newer values would be only in memory while
            // Bloxkeep's go into the file, and a restart would put back the older ones. Nothing written instead.
            if refreshed && !save_baseline(&b) {
                return false;
            }
            b
        }
    };
    // Bloxkeep's values for this client over the person's own, for everything it manages.
    let mut ours = if use_config { wanted(config) } else { HashMap::new() };
    if windowed {
        ours.insert("Fullscreen".into(), "false".into());
    }
    let mut target = baseline.values().clone();
    for (name, value) in &ours {
        target.insert(name.clone(), value.clone());
    }
    if target.is_empty() {
        return false;
    }
    let mut updated = text.clone();
    for (kind, name) in KEYS {
        if let Some(value) = target.get(name) {
            updated = element(kind, name).replacen(&updated, 1, |m: &Captures| format!("{}{value}{}", &m[1], &m[3])).into_owned();
        }
    }
    if updated == text {
        return true;
    }
    // What is written noted first: a value of Bloxkeep's left in the file without the note would pass for the person's.
    let written_before = baseline.written().clone();
    if !ours.is_empty() {
        for (name, value) in &ours {
            baseline.written_mut().insert(name.clone(), value.clone());
        }
        if !save_baseline(&baseline) {
            return false;
        }
    }
    // Not written (the file held by another program): nothing new of Bloxkeep's is in it, so the note goes back as it was.
    if !write(&file, &updated) {
        if !ours.is_empty() {
            baseline.written = Some(written_before);
            save_baseline(&baseline);
        }
        return false;
    }
    *lock(&LAST_SEEN) = None;
    true
}

static LAST_SEEN: Mutex<Option<(String, SystemTime, u64)>> = Mutex::new(None);

fn file_stamp(file: &str) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(file).ok().filter(|m| m.is_file())?;
    Some((m.modified().ok()?, m.len()))
}

/// Puts the person's values back once a client has read Bloxkeep's, and again whenever one saves them (Roblox
/// saves as it joins a game). A value no longer equal to what Bloxkeep wrote is the person's and is kept.
/// `final_`: Roblox is closed, forget the copy (kept when the file couldn't be put back, for the next try
/// and the next start: the person's own are never forgotten).
pub fn restore(final_: bool) {
    let file = settings_file(!final_);
    // Nothing changed since the last look: skip reading anything (this runs every couple of seconds).
    if !final_
        && let Some(f) = &file
        && let Some((time, len)) = file_stamp(f)
        && lock(&LAST_SEEN).as_ref() == Some(&(f.clone(), time, len))
    {
        return;
    }
    let Some(Some(baseline)) = try_load_baseline() else { return };
    let mut back = true;
    let text = match &file {
        None => Some(String::new()),
        Some(f) => read(f),
    };
    match text {
        None => back = false, // can't be read just now
        Some(text) => {
            if let Some(file) = file.as_deref().filter(|_| !text.is_empty()) {
                let now = current(&text);
                let mut updated = text.clone();
                for (kind, name) in KEYS {
                    if let (Some(mine), Some(ours)) = (baseline.values().get(name), baseline.written().get(name))
                        && now.get(name) == Some(ours)
                    {
                        updated = element(kind, name).replacen(&updated, 1, |m: &Captures| format!("{}{mine}{}", &m[1], &m[3])).into_owned();
                    }
                }
                if updated != text {
                    back = write(file, &updated); // not written: tried again on the next tick, and at the final restore
                }
                // Only a file put back is marked seen: a write refused a moment (Roblox or an antivirus holding the file)
                // left unmarked is tried again next time, since Roblox may not touch the file again until the next join.
                if back
                    && let Some((time, len)) = file_stamp(file)
                {
                    *lock(&LAST_SEEN) = Some((file.to_string(), time, len));
                }
            }
        }
    }
    if !final_ || !back {
        return;
    }
    match std::fs::remove_file(baseline_path()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            crate::net::note_failed("Roblox settings copy", &baseline_path().to_string_lossy(), &format!("couldn't be removed: {e}"));
        }
        _ => {}
    }
}

pub fn has_baseline() -> bool {
    baseline_path().is_file()
}

fn current(text: &str) -> HashMap<String, String> {
    KEYS.iter().filter_map(|(kind, name)| element(kind, name).captures(text).map(|m| (name.to_string(), m[2].to_string()))).collect()
}

fn some_empty() -> Option<HashMap<String, String>> {
    Some(HashMap::new())
}

/// Missing: empty (the C# initialiser); null: damaged.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Baseline {
    #[serde(default = "some_empty")]
    values: Option<HashMap<String, String>>,
    #[serde(default = "some_empty")]
    written: Option<HashMap<String, String>>,
}

static EMPTY: std::sync::LazyLock<HashMap<String, String>> = std::sync::LazyLock::new(HashMap::new);

impl Baseline {
    fn values(&self) -> &HashMap<String, String> {
        self.values.as_ref().unwrap_or(&EMPTY)
    }
    fn written(&self) -> &HashMap<String, String> {
        self.written.as_ref().unwrap_or(&EMPTY)
    }
    fn values_mut(&mut self) -> &mut HashMap<String, String> {
        self.values.get_or_insert_with(HashMap::new)
    }
    fn written_mut(&mut self) -> &mut HashMap<String, String> {
        self.written.get_or_insert_with(HashMap::new)
    }
}

/// The copy on disk (Some(None): none). None when it is there but can't be read just now (held for a moment): the
/// caller then changes nothing. A damaged one is set aside as roblox-settings.json.damaged-<time>, never written
/// over, and taken as none.
fn try_load_baseline() -> Option<Option<Baseline>> {
    let path = baseline_path();
    if !path.is_file() {
        return Some(None);
    }
    let text = std::fs::read(&path).ok()?;
    let text = String::from_utf8_lossy(text.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&text)).into_owned();
    if let Ok(Some(b)) = serde_json::from_str::<Option<Baseline>>(&text)
        && b.values.is_some()
        && b.written.is_some()
    {
        return Some(Some(b));
    }
    let c = Stamp::now().local();
    let aside = format!("{}.damaged-{:04}{:02}{:02}-{:02}{:02}{:02}", path.to_string_lossy(), c.year, c.month, c.day, c.hour, c.minute, c.second);
    match std::fs::rename(&path, &aside) {
        Ok(()) => {
            log::note("roblox-settings.json couldn't be read; it was set aside beside it, and Light AFK's values in Roblox's file are left as they are.");
            Some(None)
        }
        Err(_) => None, // not set aside: never written over
    }
}

/// A copy was once found damaged (set aside beside it).
fn had_damaged_copy() -> bool {
    match std::fs::read_dir(vault::data_dir()) {
        Ok(entries) => entries.flatten().any(|e| e.file_name().to_string_lossy().starts_with("roblox-settings.json.damaged-") && e.path().is_file()),
        Err(_) => false,
    }
}

/// The values as a light-AFK preset leaves them: lowest graphics, no sound, its FPS limit or a lower one.
fn holds_light_afk(now: &HashMap<String, String>) -> bool {
    let number = |s: Option<&String>| s.and_then(|s| s.trim().parse::<f64>().ok());
    wanted(&light_afk()).iter().all(|(key, value)| match (number(now.get(key)), number(Some(value))) {
        (Some(have), Some(afk)) => {
            if key == "FramerateCap" {
                have > 0.0 && have <= afk
            } else {
                have == afk
            }
        }
        _ => false,
    })
}

/// Returns false when it couldn't be written.
fn save_baseline(baseline: &Baseline) -> bool {
    if std::fs::create_dir_all(vault::data_dir()).is_err() {
        return false;
    }
    let Ok(json) = serde_json::to_string(baseline) else { return false };
    // In one piece or not at all: a half-written copy of the person's own settings would never be put back.
    durable_file::write_text(baseline_path(), &json).is_ok()
}

/// None when it can't be read (gone, or held).
fn read(file: &str) -> Option<String> {
    let bytes = std::fs::read(file).ok()?;
    Some(String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes)).into_owned())
}

/// Written next to the file and swapped in, so Roblox never reads half a file.
/// Returns false when it couldn't be written (logged).
fn write(file: &str, text: &str) -> bool {
    let temp = format!("{file}.bloxkeep");
    match std::fs::write(&temp, text).and_then(|_| std::fs::rename(&temp, file)) {
        Ok(()) => true,
        Err(ex) => {
            log::write(&ex);
            let _ = std::fs::remove_file(&temp);
            false
        }
    }
}

/// The Roblox in-game settings Bloxkeep gives accounts it opens. 0 / -1 = leave as it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct RobloxConfig {
    /// off, all (every account), or marked (accounts with Account.use_roblox_config).
    pub mode: String,
    /// 1–10 like Roblox's graphics slider; 0 = leave as it is.
    pub graphics: i32,
    /// Frame rate limit (one of FPS_CHOICES); 0 = leave as it is.
    pub fps: i32,
    /// 0–100 %; -1 = leave as it is.
    pub volume: i32,
    /// Fields a newer Bloxkeep added, saved back as they were (see models.rs).
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for RobloxConfig {
    fn default() -> Self {
        RobloxConfig { mode: "off".into(), graphics: 0, fps: 0, volume: -1, extra: Extra::new() }
    }
}
