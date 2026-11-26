//! Roblox mods (modstore.rs) for the windows Bloxkeep opens, and taken out again.
//!
//! Since 2026-11-17 (after RoALT's way) Roblox's own folder is never written: a window with mods opens from Bloxkeep's
//! own copy of Roblox's build (`mod_copy`, %LOCALAPPDATA%\Bloxkeep\Roblox mods\<build>), which shares Roblox's files by
//! hard links and has the mods as new files over them. Mods an older Bloxkeep laid into Roblox's builds are taken back
//! out at the next window (Roblox's files back, byte for byte). The notes below still serve those, and the copies (as
//! kept copies, `match_kept`).
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
use crate::core::roblox_installs as installs;
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
    // Lost, it no longer knows which of Roblox's files are a mod's: said, as mods then can't be taken out cleanly.
    crate::net::saved("Roblox mods note", &note_path()).unwrap_or_default()
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

/// Before a window opens: the client to start it from when mods are on, Bloxkeep's own copy of Roblox's build with the
/// mods over it; Roblox's own builds get back any mod an older Bloxkeep put in them. None: no mods on, not Roblox's own
/// build, a build behind Roblox's live one (it updates itself first; mods from the next window), or the copy couldn't
/// be made (said in error.log; the window opens without mods).
pub fn mod_copy(client: Option<&str>) -> Option<String> {
    let client = client?;
    if !installs::is_roblox_own(client) {
        return None;
    }
    sync(&builds_beside(client), &BTreeMap::new());
    let want = modstore::wanted();
    if want.is_empty() {
        return None;
    }
    let source = Path::new(client).parent()?.to_string_lossy().into_owned();
    let folder = Path::new(&source).file_name()?.to_string_lossy().into_owned();
    if let Some(live) = installs::live_build().filter(|l| !l.eq_ignore_ascii_case(&folder)) {
        log::note(&format!("Roblox mods: not on this window, its build {} is behind Roblox's live {live} (Roblox updates it first).", short(&source)));
        return None;
    }
    // (Two windows opening at once would make the same copy together.)
    let _held = COPIES.lock().unwrap_or_else(|e| e.into_inner());
    match installs::copy_build(client, &installs::mods_root()) {
        Ok(exe) => {
            let dir = Path::new(&exe).parent()?.to_string_lossy().into_owned();
            match_kept(&dir, &source, &want);
            Some(exe)
        }
        Err(e) => {
            log::note(&format!("Roblox mods: Bloxkeep's copy of build {} couldn't be made ({e}); this window opens without mods.", short(&source)));
            None
        }
    }
}

/// Held while a copy is made or removed.
static COPIES: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// With no Roblox open: the copies of builds other than Roblox's live one go (their notes with them, `sync`). Runs at
/// every scan: skipped while a copy is being made, a copy that can't be removed said once.
pub fn forget_old_copies() {
    static SAID: std::sync::Mutex<std::collections::BTreeSet<String>> = std::sync::Mutex::new(std::collections::BTreeSet::new());
    let Some(live) = installs::live_build() else { return };
    let Ok(dirs) = std::fs::read_dir(installs::mods_root()) else { return };
    let _held = match COPIES.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return,
    };
    for e in dirs.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if e.path().is_dir() && !name.eq_ignore_ascii_case(&live) {
            match std::fs::remove_dir_all(e.path()) {
                Ok(()) => log::note(&format!("Roblox mods: the copy of an old build ({name}) is removed.")),
                Err(err) => {
                    if SAID.lock().unwrap_or_else(|e| e.into_inner()).insert(name.clone()) {
                        log::note(&format!("Roblox mods: the copy of an old build ({name}) couldn't be removed ({err})."));
                    }
                }
            }
        }
    }
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

#[cfg(test)]
mod copy_tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use crate::core::roblox_installs as installs;

    /// A made-up build copied as a window with mods opens on it: the mod is in the copy, Roblox's own file untouched,
    /// no updater in the copy, its Fast Flags fresh.
    #[test]
    fn mods_go_into_the_copy_never_into_roblox() {
        let base = std::env::temp_dir().join(format!("bk-modcopy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let build = base.join("Versions").join("version-0123456789abcdef");
        std::fs::create_dir_all(build.join("content").join("textures")).unwrap();
        std::fs::create_dir_all(build.join("ClientSettings")).unwrap();
        std::fs::write(build.join("RobloxPlayerBeta.exe"), b"exe").unwrap();
        std::fs::write(build.join("RobloxPlayerInstaller.exe"), b"updater").unwrap();
        std::fs::write(build.join("content").join("textures").join("cursor.png"), b"roblox's own").unwrap();
        std::fs::write(build.join("ClientSettings").join("ClientAppSettings.json"), b"{}").unwrap();
        let modded = base.join("mod-cursor.png");
        std::fs::write(&modded, b"the mod").unwrap();

        let root = base.join("Roblox mods");
        let exe = installs::copy_build(&build.join("RobloxPlayerBeta.exe").to_string_lossy(), &root.to_string_lossy()).unwrap();
        let copy = PathBuf::from(&exe).parent().unwrap().to_path_buf();
        let want: BTreeMap<String, PathBuf> = [("content\\textures\\cursor.png".to_string(), modded.clone())].into_iter().collect();
        super::match_kept(&copy.to_string_lossy(), &build.to_string_lossy(), &want);

        assert_eq!(std::fs::read(copy.join("content").join("textures").join("cursor.png")).unwrap(), b"the mod");
        assert_eq!(std::fs::read(build.join("content").join("textures").join("cursor.png")).unwrap(), b"roblox's own");
        assert!(!copy.join("RobloxPlayerInstaller.exe").exists());
        assert!(copy.join("ClientSettings").join("ClientAppSettings.json").is_file());

        // The mod turned off: Roblox's file back in the copy, still untouched in Roblox's build.
        super::match_kept(&copy.to_string_lossy(), &build.to_string_lossy(), &BTreeMap::new());
        assert_eq!(std::fs::read(copy.join("content").join("textures").join("cursor.png")).unwrap(), b"roblox's own");
        assert_eq!(std::fs::read(build.join("content").join("textures").join("cursor.png")).unwrap(), b"roblox's own");
        let _ = std::fs::remove_dir_all(&base);
    }
}
