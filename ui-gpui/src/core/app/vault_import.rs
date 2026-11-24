//! AppController.VaultImport.cs: "Import accounts from a vault file…": accounts from another vault file (another PC's vault.rmv,
//! a backup) are added and saved under this vault's password; that file is only read (`vault::read_other`). Its decrypted
//! contents stay in memory only while the preview is open, under a one-time token; the page gets names and states, never
//! cookies or passwords. Dropped on cancel, lock, a new page, the import itself, or after ten minutes.
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::App;
use super::backup::same_path;
use super::controller::{JsonKind, bool_of, error_text, has, str_of};
use super::presets::{AFK_PROFILE, MAX_PRESET_NAME, MAX_PRESETS, PRESET_SERVERS, RESTART_CHOICES};
use super::other_import::RamKind;
use super::settings::open_file_dialog;
use crate::core::base::{Fail, Guid, Res, Stamp, new_guid};
use crate::core::models::{Account, LaunchPreset, VaultData};
use crate::core::roblox_api::realm;
use crate::core::vault::{self, OtherStatus, Vault};
use crate::core::{l, log, prefs, rt, ui};

pub const VAULT_IMPORT_LIFETIME: Duration = Duration::from_secs(10 * 60);
/// Wrong passwords for one file before each further try waits `VAULT_IMPORT_WAIT`.
pub const VAULT_IMPORT_FREE_TRIES: i32 = 3;
pub const VAULT_IMPORT_WAIT: Duration = Duration::from_secs(10);

/// `owner`: the vault it was opened for: a lock (and unlock) in between voids it.
// PORT: the C# record held its expiry timer; here the expiry is a spawned wait that drops the session only if it is
// still this one (same token).
pub struct VaultImportSession {
    pub token: String,
    pub owner: Arc<Vault>,
    pub data: VaultData,
}

/// What kind of file was picked: another vault, or Roblox Account Manager's (other_import.rs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImportFrom {
    #[default]
    Vault,
    Ram,
}

#[derive(Default)]
pub struct VaultImportState {
    /// The file picked to import from; kept here, so the page never names a path to read.
    pub vault_import_path: RefCell<Option<String>>,
    pub vault_import_from: Cell<ImportFrom>,
    pub vault_import: RefCell<Option<VaultImportSession>>,
    pub vault_import_opening: Cell<bool>,
    /// Wrong passwords per file (full path, any case) and when the next try is allowed (None: DateTime.MinValue); in memory only.
    pub vault_import_tries: RefCell<HashMap<String, (i32, Option<Stamp>)>>,
}

/// `presets_skipped`: presets left out: none of their accounts is here, or the preset limit was reached.
#[derive(Clone, Debug)]
pub struct ImportOutcome {
    pub added: Vec<Account>,
    pub updated: Vec<Account>,
    pub presets_added: i32,
    pub presets_skipped: i32,
}

/// The tries map's key: the full path, any case (StringComparer.OrdinalIgnoreCase).
fn tries_key(path: &str) -> String {
    path.to_lowercase()
}

/// The user's own folder (Environment.SpecialFolder.UserProfile).
fn user_profile() -> String {
    std::env::var("USERPROFILE").unwrap_or_default()
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Refuses what can't be imported from, by the header alone (before any password is typed).
pub fn check_import_file(path: &str, vault: &Vault) -> Res<()> {
    let size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(ex) => {
            let ex = Fail::from(ex);
            log::note(&format!("Import from a vault file: the file couldn't be read ({}).", App::log_kind_of(&ex)));
            return Err(Fail::user(l::f("Couldn't open the data file: {0}", &[&error_text(&ex)])));
        }
    };
    if size > vault::MAX_OTHER_SIZE {
        return Err(Fail::user(l::t("This file is too large to be a Bloxkeep data file.")));
    }
    // The id comes from the salt: vault.rmv itself, its .bak and its backups all carry this vault's.
    let id = vault::id_of_path(Path::new(path));
    if same_path(path, Some(&vault.file_path().to_string_lossy())) || id.as_deref() == Some(vault.id().as_str()) {
        return Err(Fail::user(l::t("This is the vault you're using.")));
    }
    if id.is_none() {
        return Err(Fail::user(l::t("This file is not a Bloxkeep data file (.rmv).")));
    }
    Ok(())
}

/// The same Roblox account here: by user id within its realm (Roblox and Roblox VNG count ids apart, as in
/// `add_or_merge`), or by username (any case) when either side doesn't know the id. Its place in `here`.
pub fn import_match(here: &[Account], other: &Account) -> Option<usize> {
    let realm = realm::normalize(Some(&other.realm));
    let same_name = |a: &Account| !other.username.is_empty() && a.username.to_lowercase() == other.username.to_lowercase();
    let by_id = if other.user_id != 0 { here.iter().position(|a| a.user_id == other.user_id && realm::normalize(Some(&a.realm)) == realm) } else { None };
    by_id.or_else(|| here.iter().position(|a| (other.user_id == 0 || a.user_id == 0) && realm::normalize(Some(&a.realm)) == realm && same_name(a)))
}

/// Brings the ticked accounts (`keys`: places in the file) into `target`. New ones get a new id (the file's could already
/// name something here: a deleted account's play history, the launch order). One already here takes only the file's sign-in
/// (and password when it has none), if its own isn't known to work; never its name, group or note. Presets only when asked,
/// schedules off and without their own Discord channel. Settings, Discord, the launch order and play history stay this
/// vault's own.
pub fn merge_import(target: &mut VaultData, source: &VaultData, keys: &HashSet<i32>, with_presets: bool) -> ImportOutcome {
    let here = &mut target.accounts;
    // Places in `here` (C# kept the account objects).
    let mut added: Vec<usize> = Vec::new();
    let mut updated: Vec<usize> = Vec::new();
    // The file's account id → the same account's id here, for its presets.
    let mut local_ids: HashMap<Guid, Guid> = HashMap::new();
    for (i, other) in source.accounts.iter().enumerate() {
        // Matched against the list as it grows, so an account listed twice in the file is added once.
        let existing = import_match(here, other);
        if let Some(e) = existing {
            local_ids.entry(other.id).or_insert(here[e].id); // its presets can use it, ticked or not
        }
        if !keys.contains(&(i as i32)) {
            continue;
        }
        let Some(e) = existing else {
            let account = Account {
                user_id: other.user_id,
                username: other.username.clone(),
                display_name: other.display_name.clone(),
                alias: other.alias.clone(),
                group: other.group.clone(),
                note: other.note.clone(),
                password: other.password.clone(),
                authenticator: other.authenticator.clone(),
                cookie: other.cookie.clone(),
                realm: realm::normalize(Some(&other.realm)).to_string(),
                auto_rejoin: other.auto_rejoin,
                use_roblox_config: other.use_roblox_config,
                fps_cap: other.fps_cap,
                is_main: other.is_main,
                // When it was last played (on the other PC) is still true, and keeps "Recently used" meaningful.
                last_used: other.last_used,
                // What the other PC last knew may be out of date: checked right after the import.
                cookie_valid: None,
                // Fields from a newer Bloxkeep go along, as a save keeps them.
                extra: other.extra.clone(),
                ..Account::default()
            };
            local_ids.entry(other.id).or_insert(account.id);
            here.push(account);
            added.push(here.len() - 1);
            continue;
        };
        // An authenticator key the file has and the account here lacks comes along: it doesn't depend on the sign-in.
        let mut changed = false;
        let existing = &mut here[e];
        if existing.authenticator.is_none() && other.authenticator.is_some() {
            existing.authenticator = other.authenticator.clone();
            changed = true;
        }
        // A sign-in that works here is newer than the file's; only a dead or unchecked one is replaced.
        if existing.cookie_valid == Some(true) {
            if changed && !added.contains(&e) && !updated.contains(&e) {
                updated.push(e);
            }
            continue;
        }
        if !other.cookie.is_empty() && other.cookie != existing.cookie {
            existing.cookie = other.cookie.clone();
            existing.cookie_valid = None;
            changed = true;
        }
        if existing.password.is_empty() && !other.password.is_empty() {
            existing.password = other.password.clone();
            changed = true;
        }
        if changed && !added.contains(&e) && !updated.contains(&e) {
            updated.push(e);
        }
    }
    let added: Vec<Account> = added.iter().map(|i| here[*i].clone()).collect();
    let updated: Vec<Account> = updated.iter().map(|i| here[*i].clone()).collect();

    let (mut presets_added, mut presets_skipped) = (0, 0);
    if with_presets {
        let presets = &mut target.settings.presets;
        for p in &source.settings.presets {
            // Accounts that aren't here (not ticked) are left out; a preset left with none isn't added.
            let mut ids: Vec<Guid> = Vec::new();
            for id in p.account_ids.iter().filter_map(|id| local_ids.get(id)) {
                if !ids.contains(id) {
                    ids.push(*id);
                }
            }
            if ids.is_empty() || presets.len() >= MAX_PRESETS {
                presets_skipped += 1;
                continue;
            }
            // Everything it has (a followed player too), checked as saving a preset checks it.
            let Ok(mut copy) = p.json_clone() else {
                presets_skipped += 1;
                continue;
            };
            copy.id = new_guid();
            copy.name = imported_preset_name(presets, &p.name);
            copy.account_ids = ids.clone();
            copy.server = if PRESET_SERVERS.contains(&p.server.as_str()) { p.server.clone() } else { "any".into() };
            copy.delay_seconds = p.delay_seconds.map(|delay| delay.clamp(0, 120));
            copy.profile = if p.profile == AFK_PROFILE { AFK_PROFILE.into() } else { String::new() };
            copy.on_join = super::presets::on_join_value(&p.on_join).to_string();
            copy.restart_hours =
                if (!p.place_input.is_empty() || copy.server == "follow") && RESTART_CHOICES.contains(&p.restart_hours) { p.restart_hours } else { 0 };
            // "Rejoin into" one of its accounts: that account as it is here.
            copy.rejoin_with = if p.place_input.is_empty() || copy.server == "follow" {
                String::new()
            } else if p.rejoin_with == "any" {
                "any".into()
            } else {
                match super::controller::parse_guid(&p.rejoin_with).and_then(|with| local_ids.get(&with)) {
                    Some(mine) if ids.contains(mine) => mine.to_string(),
                    _ => String::new(),
                }
            };
            // Not its own Discord channel: a file from someone else would send this PC's alerts about these
            // accounts (names, games, status) to that person's channel. Set it again here if it's yours.
            copy.discord_webhook = String::new();
            copy.last_run_at = None;
            // Off, so the two PCs don't start the same accounts at the same minute.
            if let Some(s) = copy.schedule.as_mut() {
                s.enabled = false;
                s.last_run = String::new();
            }
            presets.push(copy);
            presets_added += 1;
        }
    }
    ImportOutcome { added, updated, presets_added, presets_skipped }
}

/// The preset's own name, or "… (imported)" (numbered when taken too) when a preset here already has it.
pub fn imported_preset_name(presets: &[LaunchPreset], name: &str) -> String {
    let mut name = App::cut_text(name.trim(), MAX_PRESET_NAME).trim_end().to_string();
    if name.is_empty() {
        name = l::t("Imported preset");
    }
    let names: HashSet<String> = presets.iter().map(|p| p.name.to_lowercase()).collect();
    if !names.contains(&name.to_lowercase()) {
        return name;
    }
    let mut n = 1;
    loop {
        let suffix = if n == 1 { l::t(" (imported)") } else { l::f(" (imported {0})", &[&n]) };
        // The name is shortened, not the suffix, so a 60-character name still reads as imported.
        let room = MAX_PRESET_NAME.saturating_sub(super::accounts::len16(&suffix));
        let candidate = App::cut_text(&name, room).trim_end().to_string() + &suffix;
        if !names.contains(&candidate.to_lowercase()) {
            return candidate;
        }
        n += 1;
    }
}

impl App {
    /// "Choose file…": returns the file's name, or null when cancelled (an earlier file and its preview stay). `locked`:
    /// it needs a password to open (a Roblox Account Manager file saved without one doesn't).
    pub async fn vault_import_pick(&self, from: ImportFrom) -> Res<Value> {
        let vault = self.require_vault_ref()?;
        let folder = if from == ImportFrom::Vault { prefs::lock().backup_folder.clone() } else { None };
        let initial = folder.filter(|f| Path::new(f).is_dir()).unwrap_or_else(user_profile);
        let owner = self.controller.owner.get();
        let (title, filter) = match from {
            ImportFrom::Vault => (l::t("Choose another Bloxkeep data file"), l::t("Bloxkeep data files") + " (*.rmv)|*.rmv"),
            ImportFrom::Ram => (l::t("Choose Roblox Account Manager's AccountData.json"), "AccountData.json|AccountData*.json;*.json".to_string()),
        };
        // On a worker thread, so the core keeps answering (the window's Quit, timers) while the dialog is open.
        let Some(chosen) = rt::blocking(move || ui::modal(|| open_file_dialog(owner, &title, &filter, &initial))).await else {
            return Ok(Value::Null);
        };
        let path = std::path::absolute(&chosen).map(|p: PathBuf| p.to_string_lossy().into_owned()).unwrap_or(chosen);
        if !self.is_vault(&vault) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        let locked = match from {
            ImportFrom::Vault => {
                check_import_file(&path, &vault)?;
                true
            }
            ImportFrom::Ram => App::ram_file_kind(&path)? == RamKind::Password,
        };
        self.drop_vault_import(false);
        *self.vault_import.vault_import_path.borrow_mut() = Some(path.clone());
        self.vault_import.vault_import_from.set(from);
        Ok(json!({ "name": file_name(&path), "wait": self.import_wait_seconds(&path), "locked": locked }))
    }

    pub fn import_wait_seconds(&self, path: &str) -> i32 {
        let tries = self.vault_import.vault_import_tries.borrow().get(&tries_key(path)).copied();
        let now = Stamp::utc_now();
        match tries {
            Some((_, Some(next))) if next > now => next.secs_since(now).ceil() as i32,
            _ => 0,
        }
    }

    /// Opens the picked file with its own master password and lists its accounts, each new or already here. A wrong
    /// password is an answer, not an error, so the page can count down the wait after three.
    pub async fn vault_import_open(self: &Rc<Self>, password: &str) -> Res<Value> {
        let vault = self.require_vault_ref()?;
        let picked = self.vault_import.vault_import_path.borrow().clone();
        let Some(path) = picked else {
            return Err(Fail::user(l::t("Choose the file first.")));
        };
        if self.vault_import.vault_import_from.get() == ImportFrom::Ram {
            if self.vault_import.vault_import_opening.get() {
                return Err(Fail::user(l::t("Still opening the file — please wait.")));
            }
            self.drop_vault_import(true);
            let _opening = crate::core::base::Busy::set(&self.vault_import.vault_import_opening);
            return self.ram_import_open(vault, path, password).await;
        }
        if password.is_empty() {
            return Err(Fail::user(l::t("Enter the master password of that file.")));
        }
        if self.vault_import.vault_import_opening.get() {
            return Err(Fail::user(l::t("Still opening the file — please wait.")));
        }
        let wait = self.import_wait_seconds(&path);
        if wait > 0 {
            return Ok(json!({ "status": "wrong", "wait": wait, "error": l::f("Too many wrong tries. Try again in {0} seconds.", &[&wait]) }));
        }
        check_import_file(&path, &vault)?; // it may have been replaced since it was picked
        self.drop_vault_import(true);
        // Held until the list is made (the pictures' short wait included): a second Open meanwhile is told to wait
        // rather than making a second session whose token voids the first.
        let _opening = crate::core::base::Busy::set(&self.vault_import.vault_import_opening);
        let (p, pw) = (PathBuf::from(&path), password.to_string());
        let (status, data) = rt::blocking(move || vault::read_other(&p, &pw)).await;
        // Locked meanwhile: what was decrypted isn't kept.
        if !self.is_vault(&vault) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        if !matches!(status, OtherStatus::Opened | OtherStatus::WrongPassword) {
            log::note(&format!("Import from a vault file: refused ({status:?})."));
        }
        match status {
            OtherStatus::WrongPassword => return Ok(self.import_wrong_try(&path, l::t("Wrong master password for this file."))),
            OtherStatus::TooLarge => return Err(Fail::user(l::t("This file is too large to be a Bloxkeep data file."))),
            OtherStatus::NotAVault => return Err(Fail::user(l::t("This file is not a Bloxkeep data file (.rmv)."))),
            OtherStatus::Newer => return Err(Fail::user(l::t("This data file was made by a newer Bloxkeep. Update Bloxkeep to open it."))),
            OtherStatus::Unreadable => return Err(Fail::user(l::t("Windows couldn't read or write the file (another program may be using it)."))),
            OtherStatus::Opened => {}
        }
        self.vault_import.vault_import_tries.borrow_mut().remove(&tries_key(&path));
        let Some(data) = data else { return Err(Fail::other("NullReferenceException: an opened file without its data")) };
        self.show_import(vault, data, file_name(&path), Some(path)).await
    }

    /// A wrong password counted for the file; after three, each further try waits. An answer, not an error.
    pub(super) fn import_wrong_try(&self, path: &str, error: String) -> Value {
        let key = tries_key(path);
        let wrong = self.vault_import.vault_import_tries.borrow().get(&key).map(|t| t.0).unwrap_or(0) + 1;
        let next = if wrong >= VAULT_IMPORT_FREE_TRIES { Some(Stamp::utc_now().plus_secs(VAULT_IMPORT_WAIT.as_secs_f64())) } else { None };
        self.vault_import.vault_import_tries.borrow_mut().insert(key, (wrong, next));
        json!({ "status": "wrong", "wait": self.import_wait_seconds(path), "error": error })
    }

    /// The opened accounts (a vault file's, Roblox Account Manager's, a pasted list's) kept under a new token for the
    /// import, and listed for the preview. `path`: the picked file's, None for a pasted list.
    pub(super) async fn show_import(self: &Rc<Self>, vault: Arc<Vault>, data: VaultData, name: String, path: Option<String>) -> Res<Value> {
        // Pictures for the list: a short wait at most (offline, initials show instead); the load goes on after it.
        let loaded = Rc::new(Cell::new(false));
        {
            let (app, loaded, ids) = (self.clone(), loaded.clone(), data.accounts.iter().map(|a| a.user_id).collect::<Vec<_>>());
            rt::spawn(async move {
                let _ = app.load_avatars(ids).await;
                loaded.set(true);
            });
        }
        let mut waited = Duration::ZERO;
        while !loaded.get() && waited < Duration::from_secs(3) {
            rt::sleep(Duration::from_millis(50)).await;
            waited += Duration::from_millis(50);
        }
        if !self.is_vault(&vault) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        // Another file picked while this one opened: its accounts aren't shown (or imported) under that one.
        if path.is_some() && *self.vault_import.vault_import_path.borrow() != path {
            log::note("Import from a vault file: another file was picked while it opened; dropped.");
            return Ok(Value::Null);
        }

        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(Fail::other)?;
        let token: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
        let reply = {
            let here = vault.data();
            let avatars = self.controller.avatar_urls.borrow();
            json!({
                "status": "ok",
                "token": token,
                "name": name,
                "presets": data.settings.presets.len(),
                // Only what the list shows; "key" is the account's place in the file, which the import sends back.
                "accounts": data.accounts.iter().enumerate().map(|(i, a)| json!({
                    "key": i,
                    "label": a.label(),
                    "username": a.username,
                    "realm": realm::normalize(Some(&a.realm)),
                    "group": a.group,
                    "hasPassword": !a.password.is_empty(),
                    "cookieValid": a.cookie_valid,
                    "exists": import_match(&here.accounts, a).is_some(),
                    "avatarUrl": avatars.get(&a.user_id),
                })).collect::<Vec<_>>(),
            })
        };
        *self.vault_import.vault_import.borrow_mut() = Some(VaultImportSession { token: token.clone(), owner: vault, data });
        let app = self.clone();
        rt::spawn(async move {
            rt::sleep(VAULT_IMPORT_LIFETIME).await;
            let still = app.vault_import.vault_import.borrow().as_ref().is_some_and(|s| s.token == token);
            if still {
                app.drop_vault_import(false);
            }
        });
        Ok(reply)
    }

    /// The import command: the merge itself is `merge_import`; then saved, shown and checked.
    pub fn vault_import_apply(self: &Rc<Self>, a: &Value) -> Res<Value> {
        let vault = self.require_vault_ref()?;
        let usable = {
            let session = self.vault_import.vault_import.borrow();
            match session.as_ref() {
                Some(s) => Arc::ptr_eq(&s.owner, &vault) && str_of(a, "token") == s.token,
                None => false,
            }
        };
        if !usable {
            let none = self.vault_import.vault_import.borrow().is_none();
            log::note(&format!(
                "Import from a vault file: not applied ({}).",
                if none { "the file was closed or timed out" } else { "it belongs to another vault or an old token" }
            ));
            return Err(Fail::user(l::t("The file was closed because it was open too long. Open it again.")));
        }
        let keys: HashSet<i32> = if has(a, "keys", JsonKind::Array) {
            a["keys"].as_array().into_iter().flatten().filter_map(|e| e.as_i64()).filter_map(|v| i32::try_from(v).ok()).collect()
        } else {
            HashSet::new()
        };
        let result = {
            let session = self.vault_import.vault_import.borrow();
            let source = &session.as_ref().expect("checked above").data;
            let mut target = vault.data();
            merge_import(&mut target, source, &keys, bool_of(a, "presets"))
        };
        self.drop_vault_import(false); // the token is good once
        self.persist();
        self.push_state();
        log::note(&format!(
            "Import from a vault file: {} picked, {} added, {} updated, {} presets added, {} skipped.",
            keys.len(),
            result.added.len(),
            result.updated.len(),
            result.presets_added,
            result.presets_skipped
        ));
        let touched: Vec<&Account> = result.added.iter().chain(&result.updated).collect();
        if !touched.is_empty() {
            self.spawn_load_avatars(touched.iter().map(|x| x.user_id).collect());
        }
        let to_check: Vec<Guid> = touched.iter().filter(|x| x.cookie_valid.is_none()).map(|x| x.id).collect();
        if !to_check.is_empty() {
            let app = self.clone();
            rt::spawn(async move { app.check_imported(to_check).await });
        }
        Ok(json!({
            "added": result.added.len(),
            "updated": result.updated.len(),
            "presets": result.presets_added,
            "presetsSkipped": result.presets_skipped,
            "ids": result.added.iter().map(|x| x.id).collect::<Vec<_>>(),
        }))
    }

    /// Forgets the opened file's contents and, unless `keep_path`, which file was picked. Rust's strings aren't wiped either;
    /// with the session gone its memory is freed at once.
    pub fn drop_vault_import(&self, keep_path: bool) -> Value {
        let session = self.vault_import.vault_import.borrow_mut().take();
        if let Some(mut session) = session {
            session.data.accounts.clear();
            session.data.settings.presets.clear();
        }
        if !keep_path {
            *self.vault_import.vault_import_path.borrow_mut() = None;
        }
        Value::Null
    }
}
