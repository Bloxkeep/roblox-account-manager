//! Vaults.cs: several vaults on one PC, picked like Steam's accounts: each its own file, master password and backups
//! (named by vault id). The list is in prefs.json (`prefs::State::vault_list`); the lock screen opens
//! `vault::active_file()`. Roblox windows opened from one keep running while another is in use.
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::core::prefs::{self, VaultEntry};
use crate::core::{log, vault};

/// The first vault's file, as it was before there could be several.
pub const FIRST_FILE: &str = "vault.rmv";

// .NET's `$` also matches before a final "\n": `\n?\z` here.
static ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{8}\n?\z").unwrap());
static FILE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^vault-[0-9a-f]{8}\.rmv\n?\z").unwrap());
/// A vault's file in the data folder, or a save of it that `vault::exists` would put back: the file's name and its id.
static FILE_ON_DISK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(vault-([0-9a-f]{8})\.rmv)(?:\.tmp|\.bak)?\z").unwrap());

/// The vault in use, or the one the lock screen offers; None before one is set up (or while one is being added). A copy:
/// change it through `prefs::lock()` and `State::current_index`.
pub fn current() -> Option<VaultEntry> {
    let s = prefs::lock();
    s.current_index().map(|i| s.vault_list[i].clone())
}

pub fn is_id(id: &str) -> bool {
    ID.is_match(id)
}

pub fn is_file(file: &str) -> bool {
    file == FIRST_FILE || FILE.is_match(file)
}

/// A name as typed, trimmed and at most 40 characters; None when there is nothing left.
pub fn clean_name(name: Option<&str>) -> Option<String> {
    let n = name?.trim();
    if n.is_empty() {
        return None;
    }
    Some(crate::core::base::cut_utf16(n, 40).trim_end().to_string())
}

/// After prefs.json is read: a pre-multi-vault vault.rmv joins the list with its backup state; vaults whose
/// file is gone leave it, and vault-<id>.rmv files missing from it (a damaged prefs.json) join it again. A data
/// folder that can't be read now (a drive not ready, BLOXKEEP_DATA on a share) changes nothing.
pub fn load() {
    vault::set_active_file(FIRST_FILE);
    let has_first = prefs::lock().vault_list.iter().any(|v| v.file == FIRST_FILE);
    if !has_first && vault::exists() {
        let mut s = prefs::lock();
        let (last_backup, pending) = (s.last_backup(), s.backup_pending()); // before it joins: what was kept for it
        let entry = VaultEntry { id: new_id(), file: FIRST_FILE.into(), name: name_for(&s.vault_list, 0), last_backup, backup_pending: pending, ..VaultEntry::default() };
        s.vault_list.insert(0, entry);
    }
    let on_disk = files_on_disk();
    let mut s = prefs::lock();
    if let Some(files) = on_disk {
        for (file, id) in files {
            if s.vault_list.iter().any(|v| v.file.eq_ignore_ascii_case(&file)) {
                continue;
            }
            // Its own id when free (what `new` gives a vault-<id>.rmv), else a new one.
            let id = if s.vault_list.iter().any(|v| v.id == id) { new_id() } else { id };
            let name = name_for(&s.vault_list, s.vault_list.len());
            log::note("Vaults: a vault file missing from prefs.json was put back on the list.");
            s.vault_list.push(VaultEntry { id, file, name, ..VaultEntry::default() });
        }
        let missing: Vec<Option<String>> = s.vault_list.iter().filter(|v| !exists_on_disk(&v.file)).map(|v| v.remote_key.clone()).collect();
        for key in missing {
            s.forget_remote_key(key.as_deref());
        }
        s.vault_list.retain(|v| exists_on_disk(&v.file));
    }
    let last = s.last_vault.clone();
    let chosen = s.vault_list.iter().find(|v| Some(&v.id) == last.as_ref()).or_else(|| s.vault_list.first()).cloned();
    use_entry(chosen.as_ref());
    let current = s.current_index();
    s.move_remote_to(current);
}

/// The lock screen offers `entry` (the first vault's file when none is set up yet). (C#'s `Use`.)
pub fn use_entry(entry: Option<&VaultEntry>) {
    vault::set_active_file(entry.map(|e| e.file.as_str()).unwrap_or(FIRST_FILE));
}

/// A vault about to be set up (not in the list until it is): a new id, its file, "Vault 2" and so on.
pub fn new() -> VaultEntry {
    let s = prefs::lock();
    let mut id;
    loop {
        id = new_id();
        if !(s.vault_list.iter().any(|v| v.id == id) || vault::data_dir().join(format!("vault-{id}.rmv")).is_file()) {
            break;
        }
    }
    // The first vault keeps vault.rmv (what builds before several vaults open), the others their own file.
    let first = !s.vault_list.iter().any(|v| v.file == FIRST_FILE) && !exists_on_disk(FIRST_FILE);
    let file = if first { FIRST_FILE.to_string() } else { format!("vault-{id}.rmv") };
    VaultEntry { name: name_for(&s.vault_list, s.vault_list.len()), id, file, ..VaultEntry::default() }
}

fn new_id() -> String {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).expect("the system's random source");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// "Vault 1", "Vault 2"…: the first such name not in use.
fn name_for(list: &[VaultEntry], count: usize) -> String {
    let mut n = count + 1;
    loop {
        let name = format!("Vault {n}");
        if !list.iter().any(|v| v.name.to_lowercase() == name.to_lowercase()) {
            return name;
        }
        n += 1;
    }
}

/// The vault-<id>.rmv files in the data folder (one per vault, its .tmp/.bak counted as it), sorted; None when the
/// folder can't be listed, so nothing is taken off the list for a folder that is only away for now.
fn files_on_disk() -> Option<Vec<(String, String)>> {
    let mut files: Vec<(String, String)> = std::fs::read_dir(vault::data_dir())
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let m = FILE_ON_DISK.captures(&name)?;
            // Only a real vault (a 0-byte .tmp left by a crash during setup isn't one); one held just now still counts.
            if matches!(vault::looks_like_vault(&e.path()), Ok(false)) {
                return None;
            }
            Some((m[1].to_string(), m[2].to_string()))
        })
        .collect();
    files.sort();
    files.dedup();
    Some(files)
}

/// The vault file, or a save of it that `vault::exists` would put back.
fn exists_on_disk(file: &str) -> bool {
    let path = vault::data_dir().join(file);
    let with = |suffix: &str| {
        let mut p = path.clone().into_os_string();
        p.push(suffix);
        Path::new(&p).is_file()
    };
    path.is_file() || with(".tmp") || with(".bak")
}
