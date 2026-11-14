//! Backup.cs: copies of the encrypted vault in a folder of the person's choice, typically one Google Drive or OneDrive
//! syncs, so accounts survive a dead PC. Bloxkeep never talks to those services: a backup is a byte copy of vault.rmv,
//! and the live vault never moves into the synced folder, where the sync client could lock it mid-save.
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use regex::Regex;

use crate::core::base::{Fail, Res, Stamp};
use crate::core::l;
use crate::core::log;
use crate::core::vault;

const PREFIX: &str = "Bloxkeep-backup-";
const KEEP_LATEST: usize = 10;
const KEEP_DAYS: i64 = 30;
// Bloxkeep-backup-<vault id>-<time>.rmv; the first releases wrote no id. (.NET's `$` also matches before a final "\n".)
static NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^Bloxkeep-backup-(?:([0-9a-f]{8})-)?(\d{8}-\d{6})\.rmv\n?\z").unwrap());
const SUGGESTION_CACHE_LIFETIME: Duration = Duration::from_secs(60);

struct Cache {
    suggestions: Option<Vec<(PathBuf, String)>>,
    suggested_at: Option<Stamp>,
    /// An old backup couldn't be deleted: written to error.log once per run.
    prune_fail_noted: bool,
}

static CACHE: Mutex<Cache> = Mutex::new(Cache { suggestions: None, suggested_at: None, prune_fail_noted: false });

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    pub path: PathBuf,
    pub name: String,
}

/// A backup in a folder: where, when (by its name, this PC's clock), and whose (None for files named the old way).
#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub at: Stamp,
    pub vault_id: Option<String>,
}

fn yyyymmdd_hhmmss(at: Stamp) -> String {
    let c = at.local();
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", c.year, c.month, c.day, c.hour, c.minute, c.second)
}

pub fn file_name(vault_id: Option<&str>, at: Stamp) -> String {
    format!("{PREFIX}{}{}.rmv", vault_id.map(|id| format!("{id}-")).unwrap_or_default(), yyyymmdd_hhmmss(at))
}

/// Writes the vault's bytes under a temp name, then renames it so the sync client never sees a half file. Safe
/// on a worker thread (the caller read the bytes, so no save runs meanwhile).
/// `previous_ids`: ids the vault had before its password changed (`AppSettings::previous_vault_ids`).
pub fn write(folder: &Path, vault_bytes: &[u8], previous_ids: Option<&[String]>) -> Res<PathBuf> {
    let id = vault::id_of(vault_bytes);
    fs::create_dir_all(folder)?;
    let final_path = folder.join(file_name(id.as_deref(), Stamp::now()));
    crate::core::durable_file::write(&final_path, vault_bytes)?; // never an empty file under a backup's name, counted among the kept ones
    // A set compared OrdinalIgnoreCase: held lower-cased.
    let mut ids: HashSet<String> = previous_ids.unwrap_or(&[]).iter().map(|i| i.to_lowercase()).collect();
    if let Some(id) = &id {
        ids.insert(id.to_lowercase());
    }
    prune(folder, &ids, Some(&final_path))?;
    Ok(final_path)
}

/// A copy named later than this past the clock was made before the clock went back (see `prune`).
const FUTURE_SLACK_SECS: f64 = 5.0 * 60.0;

/// Vault ids read from old-format files' headers, by path (lower-cased), good while the file's size and time are the same.
static HEADER_IDS: LazyLock<Mutex<HashMap<String, (u64, Option<SystemTime>, Option<String>)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// A copy cut short (the app ended mid-write) leaves its temp file behind; one this old isn't being written.
const STALE_TEMP: Duration = Duration::from_secs(5 * 60);

fn path_key(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

/// The calendar day of a moment on this PC's clock.
fn day(at: Stamp) -> (i32, u32, u32) {
    let c = at.local();
    (c.year, c.month, c.day)
}

/// Keeps the newest 10, plus the newest of each day in the last 30 days. Only this vault's backups count (current and
/// earlier-password ids; an old-format file by its header): PCs with different vaults may share one synced folder.
/// Names carry the PC's clock, which can go back: a copy named later than now ranks after newer ones, and
/// `just_written` is never removed. `vault_ids` are lower case (compared OrdinalIgnoreCase).
pub fn prune(folder: &Path, vault_ids: &HashSet<String>, just_written: Option<&Path>) -> Res<()> {
    delete_stale_temps(folder)?;
    let all = list(folder)?;
    let backups: Vec<Entry> = all
        .iter()
        .filter(|b| match &b.vault_id {
            Some(named) => vault_ids.contains(&named.to_lowercase()),
            None => !vault_ids.is_empty() && header_id(&b.path).is_some_and(|read| vault_ids.contains(&read.to_lowercase())),
        })
        .cloned()
        .collect();
    {
        // Headers of files no longer there are forgotten.
        let present: HashSet<String> = all.iter().map(|b| path_key(&b.path)).collect();
        guard(&HEADER_IDS).retain(|path, _| present.contains(path));
    }
    let future = Stamp::now().plus_secs(FUTURE_SLACK_SECS);
    let mut ranked = backups.clone();
    ranked.sort_by_key(|b| b.at > future); // stable: newest first, those named in the future last
    let mut keep: HashSet<String> = ranked.iter().take(KEEP_LATEST).map(|b| path_key(&b.path)).collect();
    if let Some(just) = just_written {
        keep.insert(path_key(just));
    }
    let mut days = HashSet::new();
    let today = Stamp::now().local();
    // DateTime.Now.Date.AddDays(-30): noon of today, 30 days back, read as a day (noon keeps a clock change out of it).
    let cutoff = day(Stamp::from_local(today.year, today.month, today.day, 12, 0, 0).plus_secs(-(KEEP_DAYS as f64) * 86400.0));
    for b in &backups {
        let d = day(b.at);
        if d >= cutoff && days.insert(d) {
            keep.insert(path_key(&b.path));
        }
    }
    for b in backups.iter().filter(|b| !keep.contains(&path_key(&b.path))) {
        if let Err(e) = fs::remove_file(&b.path) {
            if e.kind() == std::io::ErrorKind::NotFound {
                continue;
            }
            // The sync client may be uploading it; next time.
            let mut cache = guard(&CACHE);
            if !cache.prune_fail_noted {
                log::note(&format!("Backup: an old backup couldn't be deleted ({:?}); tried again at the next copy.", e.kind()));
            }
            cache.prune_fail_noted = true;
        }
    }
    Ok(())
}

/// An old-format file's header id, read once per size and time: reading a cloud-only copy downloads it.
fn header_id(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let (size, written_at) = (meta.len(), meta.modified().ok());
    if let Some(known) = guard(&HEADER_IDS).get(&path_key(path)) {
        if known.0 == size && known.1 == written_at {
            return known.2.clone();
        }
    }
    let id = vault::id_of_path(path);
    guard(&HEADER_IDS).insert(path_key(path), (size, written_at, id.clone()));
    id
}

/// The files in `folder` whose names start with `prefix` and end with `suffix` (Directory.EnumerateFiles(folder,
/// "<prefix>*<suffix>"), any case).
fn enumerate(folder: &Path, prefix: &str, suffix: &str) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for e in fs::read_dir(folder)? {
        let e = e?;
        if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_lowercase();
        if name.starts_with(&prefix.to_lowercase()) && name.ends_with(&suffix.to_lowercase()) && name.len() >= prefix.len() + suffix.len() {
            out.push(e.path());
        }
    }
    Ok(out)
}

fn delete_stale_temps(folder: &Path) -> Res<()> {
    for tmp in enumerate(folder, PREFIX, ".rmv.tmp")? {
        // Held by the sync client; next time.
        let Ok(written) = fs::metadata(&tmp).and_then(|m| m.modified()) else { continue };
        if SystemTime::now().duration_since(written).unwrap_or_default() > STALE_TEMP {
            let _ = fs::remove_file(&tmp);
        }
    }
    Ok(())
}

/// Backups in the folder, of every vault, newest first; `vault_id` is None for files named the old way.
pub fn list(folder: &Path) -> Res<Vec<Entry>> {
    let mut list = Vec::new();
    if !folder.is_dir() {
        return Ok(list);
    }
    for path in enumerate(folder, PREFIX, ".rmv")? {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let Some(m) = NAME.captures(&name) else { continue };
        // DateTime.TryParseExact(…, "yyyyMMdd-HHmmss"): a moment on this PC's clock.
        let t = &m[2];
        let num = |a: usize, b: usize| t.get(a..b).and_then(|s| s.parse::<u32>().ok());
        let (Some(y), Some(mo), Some(d), Some(h), Some(mi), Some(s)) = (num(0, 4), num(4, 6), num(6, 8), num(9, 11), num(11, 13), num(13, 15)) else { continue };
        if !valid_date(y as i32, mo, d) || h > 23 || mi > 59 || s > 59 || y == 0 {
            continue;
        }
        let at = Stamp::from_local(y as i32, mo, d, h, mi, s);
        list.push(Entry { path, at, vault_id: m.get(1).map(|g| g.as_str().to_lowercase()) });
    }
    // OrderByDescending: stable, newest first.
    list.sort_by(|a, b| b.at.cmp(&a.at));
    Ok(list)
}

fn valid_date(y: i32, m: u32, d: u32) -> bool {
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

/// Puts a backup in place as the vault. With a vault already there, the current one is first copied beside it as
/// old-vaults\vault-before-restore-….rmv, so a restore of the wrong file loses nothing.
/// Returns the copy kept of the vault it replaced, or None when there was none.
pub fn restore(file: &Path, replace_existing: bool) -> Res<Option<PathBuf>> {
    if !vault::looks_like_vault(file)? {
        return Err(Fail::user(l::t("This file is not a Bloxkeep backup.")));
    }
    let exists = vault::vault_path().is_file();
    if exists && !replace_existing {
        return Err(Fail::user(l::t("Bloxkeep is already set up.")));
    }
    let bytes = fs::read(file)?;
    let mut before = None;
    if exists {
        fs::create_dir_all(vault::old_vaults_dir())?;
        let path = vault::old_vaults_dir().join(format!("vault-before-restore-{}.rmv", yyyymmdd_hhmmss(Stamp::now())));
        // File.Copy(…, overwrite: false).
        if path.exists() {
            return Err(vault::exists_error(windows_sys::Win32::Foundation::ERROR_FILE_EXISTS, &path).into());
        }
        fs::copy(vault::vault_path(), &path)?;
        before = Some(path);
    }
    vault::replace_file(&bytes)?;
    Ok(before)
}

/// Path.GetFullPath without the ending separator.
fn full(p: &Path) -> String {
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    abs.to_string_lossy().trim_end_matches(['\\', '/']).to_string()
}

/// The live vault must stay out of the backup folder, and the folder out of the live data.
pub fn is_data_folder(folder: &Path) -> bool {
    let a = full(folder).to_lowercase();
    let b = full(vault::data_dir()).to_lowercase();
    a == b || a.starts_with(&format!("{b}\\"))
}

/// A path inside a folder a cloud client syncs, judged by name only.
pub fn looks_like_cloud(folder: &str) -> bool {
    let f = folder.to_lowercase();
    ["onedrive", "my drive", "google drive", "dropbox", "icloud"].iter().any(|n| f.contains(n))
}

/// Short enough, or made of digits and lowercase letters only, to be guessed from a leaked backup.
pub fn weak_password(password: &str) -> bool {
    password.encode_utf16().count() < 8 || password.chars().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
}

/// Synced folders found on this PC (OneDrive, Google Drive's "My Drive"), each with a Bloxkeep subfolder.
pub fn suggestions() -> Vec<Folder> {
    let mut cache = guard(&CACHE);
    let stale = match cache.suggested_at {
        Some(at) => Stamp::utc_now().secs_since(at) > SUGGESTION_CACHE_LIFETIME.as_secs_f64(),
        None => true,
    };
    if cache.suggestions.is_none() || stale {
        cache.suggestions = Some(find_synced_folders());
        cache.suggested_at = Some(Stamp::utc_now());
    }
    cache.suggestions.as_ref().map(|s| s.iter().map(|(path, name)| Folder { path: path.clone(), name: name.clone() }).collect()).unwrap_or_default()
}

fn find_synced_folders() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut add = |root: Option<PathBuf>, name: &str| {
        let Some(root) = root.filter(|r| !r.as_os_str().is_empty()) else { return };
        if root.is_dir() && seen.insert(full(&root).to_lowercase()) {
            found.push((root.join("Bloxkeep"), name.to_string()));
        }
    };
    for variable in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        add(std::env::var_os(variable).map(PathBuf::from), "OneDrive");
    }
    let profile = PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default());
    // Google Drive for desktop: a virtual drive (G:\My Drive) when streaming, a folder in the profile when mirroring.
    add(Some(profile.join("My Drive")), "Google Drive");
    add(Some(profile.join("Google Drive")), "Google Drive");
    for root in fixed_ready_drives() {
        add(Some(root.join("My Drive")), "Google Drive");
    }
    found
}

/// DriveInfo.GetDrives() that are fixed and ready, as their root folders ("C:\").
fn fixed_ready_drives() -> Vec<PathBuf> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    const DRIVE_FIXED: u32 = 3;
    // SAFETY: no arguments; returns a bit mask of drive letters.
    let mask = unsafe { GetLogicalDrives() };
    let mut out = Vec::new();
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let root = format!("{}:\\", (b'A' + i as u8) as char);
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a NUL-terminated UTF-16 root path that lives across the call.
        let kind = unsafe { GetDriveTypeW(wide.as_ptr()) };
        // IsReady: the drive's root can be read.
        if kind == DRIVE_FIXED && fs::read_dir(&root).is_ok() {
            out.push(PathBuf::from(root));
        }
    }
    out
}
