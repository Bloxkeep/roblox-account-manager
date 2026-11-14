//! Roblox mods (modstore.rs) laid into Roblox's builds before a window opens, and taken out again.
//!
//! What each build was given is noted (mods\applied.json), with Roblox's own file kept aside first
//! (mods\.originals\<build>\…): a mod turned off, removed, or overridden by another gets Roblox's file back, byte for
//! byte, the next time a window opens, or at once with "Put Roblox's files back". A file is never written into: a new
//! one is written beside it and moved over it. That matters because Bloxkeep's kept copy of a build (roblox_installs::
//! kept_copy) shares Roblox's files by hard links, and writing into one would change both.
//!
//! Left alone: a file someone else changed since (Roblox, another tool): its note is dropped, Bloxkeep's copy of the
//! original kept. A build Roblox deleted: its note and copies go. A file held open by a running Roblox: tried again
//! next time, the note left as it was. Only Roblox's own builds get mods: a Bloxstrap-style launcher lays its own
//! mods (and checks its files), the Microsoft Store's build can't be written, and VNG's isn't touched.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::log;
use crate::modstore;

/// Per build folder: each file given, and whether Roblox had its own there (kept aside) or not (taken out on undo).
#[derive(Default, Serialize, Deserialize)]
struct Applied {
    builds: BTreeMap<String, BTreeMap<String, Given>>,
    /// A kept copy's own Roblox build, whose files (and kept-aside originals) it is matched to.
    #[serde(default)]
    sources: BTreeMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Given {
    /// Roblox had its own file there (a copy kept aside).
    had: bool,
    /// The size of what was put there, to tell it from someone else's file later.
    size: u64,
}

fn note_path() -> PathBuf {
    modstore::dir().join("applied.json")
}

fn originals(build: &str) -> PathBuf {
    let name = Path::new(build).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    // (Two folders of one name, Roblox's build and the kept copy, kept apart.)
    let tag = if build.to_ascii_lowercase().contains("bloxkeep") { "kept-" } else { "" };
    modstore::dir().join(".originals").join(format!("{tag}{name}"))
}

fn load() -> Applied {
    std::fs::read(note_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(a: &Applied) {
    let _ = std::fs::create_dir_all(modstore::dir());
    let Ok(text) = serde_json::to_string_pretty(a) else { return };
    let temp = modstore::dir().join("applied.json.new");
    if std::fs::write(&temp, text).and_then(|_| std::fs::rename(&temp, note_path())).is_err() {
        log::note("Roblox mods: the note of what was given couldn't be saved.");
    }
}

/// `from` put at `to` as a new file (written beside it, moved over it), never into the file there.
fn place(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(p) = to.parent() {
        std::fs::create_dir_all(p)?;
    }
    let temp = to.with_extension(format!("{}.bloxkeep", to.extension().and_then(|e| e.to_str()).unwrap_or("")));
    std::fs::copy(from, &temp)?;
    if let Err(e) = std::fs::rename(&temp, to) {
        let _ = std::fs::remove_file(&temp);
        return Err(e);
    }
    Ok(())
}

fn same_bytes(a: &Path, b: &Path) -> bool {
    match (std::fs::read(a), std::fs::read(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// The builds next to `client` (a Roblox build's RobloxPlayerBeta.exe): every version-… folder beside its own.
fn builds_beside(client: &str) -> Vec<String> {
    let Some(versions) = Path::new(client).parent().and_then(Path::parent) else { return Vec::new() };
    std::fs::read_dir(versions)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir() && e.file_name().to_string_lossy().to_ascii_lowercase().starts_with("version-") && e.path().join("RobloxPlayerBeta.exe").is_file())
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect()
}

/// Whether mods go to windows opened this way (Roblox's own builds only; see the top).
pub fn applies_to(launch_via: &str) -> bool {
    !matches!(launch_via, "bloxstrap" | "fishstrap" | "voidstrap" | "custom" | "store" | "vng") && crate::core::roblox_launcher::other_name(launch_via).is_none()
}

/// Before a window opens: every build beside `client` given the mods that are on, and what is no longer wanted put
/// back. True when anything changed.
pub fn apply(client: Option<&str>) -> bool {
    let Some(client) = client else { return false };
    if !crate::core::roblox_installs::is_roblox_own(client) {
        return false;
    }
    let builds = builds_beside(client);
    sync(&builds, &modstore::wanted())
}

/// The kept copy a window opens on (`kept`, a folder made from Roblox's build `source` by hard links) made to match:
/// the mods that are on, and elsewhere what `source` holds of Roblox's own (its file, or the one kept aside while
/// `source` still has a mod there). It keeps no originals of its own: Roblox's are `source`'s.
pub fn apply_in(kept: &str, source: &str) -> bool {
    match_kept(kept, source, &modstore::wanted())
}

fn match_kept(kept: &str, source: &str, want: &BTreeMap<String, PathBuf>) -> bool {
    let mut a = load();
    a.sources.insert(kept.to_string(), source.to_string());
    let src_given = a.builds.get(source).cloned().unwrap_or_default();
    let mut given = a.builds.remove(kept).unwrap_or_default();
    let (root, src) = (Path::new(kept), Path::new(source));
    let mut changed = false;
    let all: Vec<String> = want.keys().chain(given.keys()).cloned().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    for rel in all {
        let target = root.join(&rel);
        let desired: Option<PathBuf> = match (want.get(&rel), src_given.get(&rel)) {
            (Some(w), _) => Some(w.clone()),
            (None, Some(g)) if g.had => Some(originals(source).join(&rel)).filter(|p| p.is_file()),
            (None, Some(_)) => None,
            (None, None) => Some(src.join(&rel)).filter(|p| p.is_file()),
        };
        let done = match &desired {
            Some(from) if target.is_file() && same_bytes(from, &target) => Ok(()),
            Some(from) => place(from, &target),
            None => match std::fs::remove_file(&target) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
        };
        match done {
            Ok(()) => {
                if want.contains_key(&rel) {
                    let size = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                    given.insert(rel, Given { had: true, size });
                } else {
                    given.remove(&rel);
                }
                changed = true;
            }
            Err(e) => log::note(&format!("Roblox mods: {rel} couldn't be set in the kept copy just now ({e})." )),
        }
    }
    if !given.is_empty() {
        a.builds.insert(kept.to_string(), given);
    } else {
        a.sources.remove(kept);
    }
    save(&a);
    changed
}

/// Every build given back Roblox's own files ("Put Roblox's files back"): how many files, and how many couldn't be
/// (held by a running Roblox).
pub fn restore_all() -> (usize, usize) {
    let a = load();
    let before: usize = a.builds.values().map(|m| m.len()).sum();
    // Kept copies first: they are matched to their build's originals, which go once that build is put back.
    for (kept, source) in &a.sources {
        if Path::new(kept).is_dir() {
            match_kept(kept, source, &BTreeMap::new());
        }
    }
    let a = load();
    let builds: Vec<String> = a.builds.keys().filter(|b| !a.sources.contains_key(*b)).cloned().collect();
    sync(&builds, &BTreeMap::new());
    let after: usize = load().builds.values().map(|m| m.len()).sum();
    (before - after, after)
}

fn sync(builds: &[String], want: &BTreeMap<String, PathBuf>) -> bool {
    let mut a = load();
    let mut changed = false;
    // Builds Roblox has deleted: their notes and kept originals go.
    let gone: Vec<String> = a.builds.keys().filter(|b| !Path::new(b).is_dir()).cloned().collect();
    for b in gone {
        a.builds.remove(&b);
        a.sources.remove(&b);
        let _ = std::fs::remove_dir_all(originals(&b));
        changed = true;
    }
    for build in builds {
        let root = Path::new(build);
        // (A kept copy is matched by match_kept, to its own build.)
        if !root.is_dir() || a.sources.contains_key(build) {
            continue;
        }
        let keep = originals(build);
        let mut given = a.builds.remove(build).unwrap_or_default();
        // 1. What was given and isn't wanted now (or is wanted from another mod): Roblox's own back.
        let old: Vec<String> = given.keys().cloned().collect();
        for rel in old {
            let target = root.join(&rel);
            let g = given[&rel].clone();
            let wanted_now = want.get(&rel);
            if wanted_now.is_some_and(|w| target.is_file() && same_bytes(w, &target)) {
                continue; // still what is wanted
            }
            // Changed by someone else since (its size isn't ours): left as it is, forgotten.
            if target.is_file() && std::fs::metadata(&target).map(|m| m.len()).ok() != Some(g.size) && wanted_now.is_none() {
                given.remove(&rel);
                let _ = std::fs::remove_file(keep.join(&rel));
                changed = true;
                continue;
            }
            if wanted_now.is_some() {
                continue; // replaced below by the mod that wins now
            }
            let back = if g.had {
                let orig = keep.join(&rel);
                if orig.is_file() { place(&orig, &target).map(|_| { let _ = std::fs::remove_file(&orig); }) } else { Ok(()) }
            } else {
                match std::fs::remove_file(&target) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                    _ => Ok(()),
                }
            };
            match back {
                Ok(()) => {
                    given.remove(&rel);
                    changed = true;
                }
                Err(e) => log::note(&format!("Roblox mods: {rel} couldn't be put back in {} just now ({e}); tried again next time.", short(build))),
            }
        }
        // 2. What is wanted: Roblox's own kept aside first (once), then the mod's file moved in.
        for (rel, from) in want {
            let target = root.join(rel);
            if given.contains_key(rel) && target.is_file() && same_bytes(from, &target) {
                continue;
            }
            let had = match given.get(rel) {
                Some(g) => g.had,
                None => {
                    if target.is_file() {
                        let orig = keep.join(rel);
                        if let Some(p) = orig.parent() {
                            let _ = std::fs::create_dir_all(p);
                        }
                        if std::fs::copy(&target, &orig).is_err() {
                            log::note(&format!("Roblox mods: Roblox's {rel} couldn't be kept aside; left as it is."));
                            continue;
                        }
                        true
                    } else {
                        false
                    }
                }
            };
            match place(from, &target) {
                Ok(()) => {
                    let size = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                    given.insert(rel.clone(), Given { had, size });
                    changed = true;
                }
                Err(e) => {
                    // (Not given: its kept original isn't needed.)
                    if !given.contains_key(rel) && had {
                        let _ = std::fs::remove_file(keep.join(rel));
                    }
                    log::note(&format!("Roblox mods: {rel} couldn't be put in {} just now ({e}).", short(build)));
                }
            }
        }
        if !given.is_empty() {
            a.builds.insert(build.clone(), given);
        } else {
            let _ = std::fs::remove_dir_all(&keep);
        }
    }
    if changed {
        save(&a);
    }
    changed
}

fn short(build: &str) -> String {
    Path::new(build).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}
