// RobloxFlags.cs: Roblox's leanest drawing for light-AFK windows, through Fast Flags.
//! With Settings › Staying in game › Fast Flags on, windows get Roblox's leanest drawing: Fast Flags written to the
//! client's ClientSettings\ClientAppSettings.json just before it opens, and the file put back as it was once it has
//! started (Roblox reads it in its first second: "LoadClientSettingsFromLocal" in its log). Only flags on Roblox's
//! allowlist (devforum 3966569, 2025-09-29), each still in the client of 2026-10-29; Roblox ignores any other, and
//! takes no action against setting them. On a real PC they turned the sky grey and the light flat, and saved a little
//! RAM (about 70 MB of 3.3 GB, Steal An Egg): every bit helps a PC full of idling accounts. A file of the person's own
//! keeps its other flags. A Bloxstrap-style launcher writes its own file as it opens Roblox: theirs wins.
use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value, json};

use crate::core::base::Stamp;
use crate::core::roblox_api::{eq_ci, starts_with_ci};
use crate::core::{durable_file, log, vault};

/// The flags, as Roblox takes them in the file.
fn light_afk() -> [(&'static str, Value); 10] {
    [
        ("DFIntDebugFRMQualityLevelOverride", json!(1)), // the lowest graphics level, whatever the game or Roblox's auto
        ("DFFlagDebugPauseVoxelizer", json!(true)),      // no voxel lighting kept up to date
        ("DFIntCSGLevelOfDetailSwitchingDistance", json!(0)), // unions at their least detail…
        ("DFIntCSGLevelOfDetailSwitchingDistanceL12", json!(0)),
        ("DFIntCSGLevelOfDetailSwitchingDistanceL23", json!(0)),
        ("DFIntCSGLevelOfDetailSwitchingDistanceL34", json!(0)),
        ("FIntDebugForceMSAASamples", json!(0)), // no anti-aliasing
        ("FIntFRMMaxGrassDistance", json!(0)),   // no grass
        ("FIntFRMMinGrassDistance", json!(0)),
        ("FFlagDebugSkyGray", json!(true)), // a grey sky, its pictures not loaded
    ]
}

/// The files written, each with what it held before (None: there was none), kept on disk so a Bloxkeep that
/// closed before putting them back does it at its next start.
fn state_path() -> PathBuf {
    vault::data_dir().join("roblox-flags.json")
}

/// A Dictionary<string, V> in insertion order. A new one compares keys ignoring case (the C#'s
/// StringComparer.OrdinalIgnoreCase); one read from disk compares them exactly, as System.Text.Json's own
/// dictionary replaces the initialiser's.
#[derive(Clone, Debug)]
struct Pairs<V> {
    items: Vec<(String, V)>,
    ignore_case: bool,
}

impl<V> Pairs<V> {
    fn new() -> Self {
        Pairs { items: Vec::new(), ignore_case: true }
    }
    fn same(&self, a: &str, b: &str) -> bool {
        if self.ignore_case { eq_ci(a, b) } else { a == b }
    }
    fn get(&self, key: &str) -> Option<&V> {
        self.items.iter().find(|(k, _)| self.same(k, key)).map(|(_, v)| v)
    }
    fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
    fn insert(&mut self, key: &str, value: V) {
        let ignore_case = self.ignore_case;
        match self.items.iter_mut().find(|(k, _)| if ignore_case { eq_ci(k, key) } else { k == key }) {
            Some(slot) => slot.1 = value,
            None => self.items.push((key.to_string(), value)),
        }
    }
    fn remove(&mut self, key: &str) {
        let ignore_case = self.ignore_case;
        self.items.retain(|(k, _)| !(if ignore_case { eq_ci(k, key) } else { k == key }));
    }
    fn len(&self) -> usize {
        self.items.len()
    }
}

impl<V: Serialize> Serialize for Pairs<V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.items.len()))?;
        for (k, v) in &self.items {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for Pairs<V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct PairsVisitor<V>(std::marker::PhantomData<V>);
        impl<'de, V: Deserialize<'de>> Visitor<'de> for PairsVisitor<V> {
            type Value = Pairs<V>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut pairs = Pairs { items: Vec::new(), ignore_case: false };
                while let Some((k, v)) = access.next_entry::<String, V>()? {
                    pairs.insert(&k, v);
                }
                Ok(pairs)
            }
        }
        d.deserialize_map(PairsVisitor(std::marker::PhantomData))
    }
}

/// Missing: empty (the C# initialiser); null: damaged.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct State {
    #[serde(default = "some_pairs")]
    before: Option<Pairs<Option<String>>>,
    #[serde(default = "some_pairs")]
    written: Option<Pairs<String>>,
}

fn some_pairs<V>() -> Option<Pairs<V>> {
    Some(Pairs::new())
}

impl State {
    fn new() -> Self {
        State { before: Some(Pairs::new()), written: Some(Pairs::new()) }
    }
    fn before(&mut self) -> &mut Pairs<Option<String>> {
        self.before.get_or_insert_with(Pairs::new)
    }
    fn written(&mut self) -> &mut Pairs<String> {
        self.written.get_or_insert_with(Pairs::new)
    }
}

pub fn pending() -> bool {
    state_path().is_file()
}

/// Before opening a window: the flags in the file of every build next to `client` (Roblox
/// starts the account's channel's build, which may be another folder than the newest). False when nothing was written.
pub fn apply(client: Option<&str>) -> bool {
    let Some(versions) = client.and_then(|c| Path::new(c).parent()).and_then(Path::parent).map(|v| v.to_string_lossy().into_owned()) else {
        return false;
    };
    // The note can't be read just now: nothing written, so the person's own in it is never replaced.
    let Some(loaded) = try_load() else { return false };
    let mut state = loaded.unwrap_or_else(State::new);
    let mut planned: Vec<(String, String, bool, Option<String>)> = Vec::new();
    for build in builds(&versions) {
        let file = Path::new(&build).join("ClientSettings").join("ClientAppSettings.json").to_string_lossy().into_owned();
        let had = read(&file);
        if had.is_none() && Path::new(&file).is_file() {
            continue; // there but can't be read just now: never written over without knowing what it held
        }
        // Still ours from an earlier window: what it held before that is what goes back.
        let written = state.written().get(&file).cloned();
        let ours = written.is_some() && written == had && state.before().contains_key(&file);
        // The flags already in it with no note of what it held before (the note lost or damaged): that isn't the
        // person's own, so it is never noted as theirs (it would be "put back" with the flags for good).
        if !ours && had.as_deref().is_some_and(holds_light_afk) {
            continue;
        }
        let source = if ours { state.before().get(&file).cloned().flatten() } else { had.clone() };
        let mut flags = match source.filter(|s| !s.is_empty()) {
            Some(before) => match serde_json::from_str::<Value>(&before) {
                Ok(Value::Object(o)) => o,
                Ok(_) => Map::new(),
                Err(_) => {
                    // A file of the person's that isn't JSON: left alone (Roblox wouldn't read it anyway), and said, as
                    // Bloxkeep's own flags aren't put in that copy of Roblox.
                    let folder = Path::new(&file).parent().and_then(|p| p.parent()).and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    log::note(&format!("Fast Flags: {folder}'s ClientAppSettings.json isn't JSON (someone else's); left alone, Bloxkeep's flags not put in there."));
                    continue;
                }
            },
            None => Map::new(),
        };
        for (name, value) in light_afk() {
            flags.insert(name.to_string(), value);
        }
        // PORT: serde_json escapes less than System.Text.Json (which writes non-ASCII and <>&'+ as \uXXXX) and writes a
        // parsed number in its own form; Roblox reads both the same.
        let text = serde_json::to_string_pretty(&Value::Object(flags)).unwrap_or_default();
        if !ours {
            state.before().insert(&file, had.clone());
        }
        state.written().insert(&file, text.clone());
        planned.push((file, text, ours, written));
    }
    // The note first: a file is never changed without a note on disk of what it held (one written without it could
    // never be put back).
    if planned.is_empty() || !save(&state) {
        return false;
    }
    let (mut any, mut undone) = (false, false);
    for (file, text, ours, written_before) in planned {
        if write(&file, &text) {
            any = true;
            continue;
        }
        undone = true;
        if ours {
            state.written().insert(&file, written_before.unwrap_or_default()); // still holds what was written for the earlier window
        } else {
            state.before().remove(&file);
            state.written().remove(&file);
        }
    }
    if undone {
        keep(&mut state);
    }
    any
}

/// Every file written put back as it was: a window not of a light-AFK preset is about to open, or those that were
/// have started. One changed since (Roblox updated, the person edited it) is left as it is now. One that can't be put
/// back just now (held, read-only, a build without the rights) stays in the note, tried again next time and at the
/// next start: the person's own is never forgotten.
pub fn restore() {
    let Some(Some(mut state)) = try_load() else { return };
    let entries = state.before().items.clone();
    for (file, before) in entries {
        let now = read(&file);
        if now.is_none() && Path::new(&file).is_file() {
            continue; // can't be read just now: kept for the next try
        }
        let mut back = true;
        if let Some(written) = state.written().get(&file).cloned()
            && now.as_deref() == Some(written.as_str())
        {
            match &before {
                None => {
                    delete(&file);
                    back = !Path::new(&file).exists();
                    // The folder too, when Bloxkeep made it and it's empty again.
                    if back
                        && let Some(dir) = Path::new(&file).parent()
                        && dir.is_dir()
                        && std::fs::read_dir(dir).is_ok_and(|mut d| d.next().is_none())
                    {
                        let _ = std::fs::remove_dir(dir);
                    }
                }
                Some(before) => back = write(&file, before),
            }
        }
        if back {
            state.before().remove(&file);
            state.written().remove(&file);
        }
    }
    keep(&mut state);
}

/// The note as it now stands: gone once nothing is left to put back.
fn keep(state: &mut State) {
    if state.before().len() == 0 {
        delete(&state_path().to_string_lossy());
    } else {
        save(state);
    }
}

/// Every Light AFK flag in it, at its value.
fn holds_light_afk(text: &str) -> bool {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(flags)) => light_afk().iter().all(|(name, value)| flags.get(*name).is_some_and(|v| v.to_string() == value.to_string())),
        _ => false,
    }
}

fn builds(versions: &str) -> Vec<String> {
    match std::fs::read_dir(versions) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.path().is_dir() && starts_with_ci(&e.file_name().to_string_lossy(), "version-") && e.path().join("RobloxPlayerBeta.exe").is_file())
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// The note on disk (Some(None): none). None when it is there but can't be read just now (held for a moment): the
/// caller then changes nothing. A damaged one is set aside as roblox-flags.json.damaged-<time>, never written over,
/// and taken as none.
fn try_load() -> Option<Option<State>> {
    let path = state_path();
    if !path.is_file() {
        return Some(None);
    }
    let bytes = std::fs::read(&path).ok()?;
    let text = String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes)).into_owned();
    if let Ok(Some(state)) = serde_json::from_str::<Option<State>>(&text)
        && state.before.is_some()
        && state.written.is_some()
    {
        return Some(Some(state));
    }
    let c = Stamp::now().local();
    let aside = format!("{}.damaged-{:04}{:02}{:02}-{:02}{:02}{:02}", path.to_string_lossy(), c.year, c.month, c.day, c.hour, c.minute, c.second);
    match std::fs::rename(&path, &aside) {
        Ok(()) => {
            log::note("roblox-flags.json couldn't be read; it was set aside beside it, and files holding Light AFK's flags are left as they are.");
            Some(None)
        }
        Err(_) => None, // not set aside: never written over
    }
}

/// Returns false when it couldn't be written.
fn save(state: &State) -> bool {
    if std::fs::create_dir_all(vault::data_dir()).is_err() {
        return false;
    }
    let Ok(json) = serde_json::to_string(state) else { return false };
    // In one piece or not at all: a half-written copy of the person's own file would never be put back.
    durable_file::write_text(state_path(), &json).is_ok()
}

fn read(file: &str) -> Option<String> {
    if !Path::new(file).is_file() {
        return None;
    }
    let bytes = std::fs::read(file).ok()?;
    Some(String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes)).into_owned())
}

/// Written next to the file and swapped in, so Roblox never reads half a file. False when it couldn't be
/// (a build in Program Files, say, without the rights).
fn write(file: &str, text: &str) -> bool {
    let temp = format!("{file}.bloxkeep");
    let done = Path::new(file)
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|_| std::fs::write(&temp, text))
        .and_then(|_| std::fs::rename(&temp, file));
    if done.is_err() {
        delete(&temp);
        return false;
    }
    true
}

fn delete(file: &str) {
    // File.Delete refuses a read-only file (UnauthorizedAccessException); Rust's remove_file deletes it anyway, which
    // would drop a file of the person's held read-only from the note while it is still Bloxkeep's.
    if std::fs::metadata(file).is_ok_and(|m| m.permissions().readonly()) {
        return;
    }
    match std::fs::remove_file(file) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            // Left there, Roblox keeps reading these Fast Flags.
            crate::net::note_failed("Fast Flags file", file, &format!("couldn't be removed: {e}"));
        }
        _ => {}
    }
}
