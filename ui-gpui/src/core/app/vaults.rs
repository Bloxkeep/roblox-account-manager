//! AppController.Vaults.cs: several vaults (`crate::core::vaults`): the lock screen's Steam-like picker ("Who's using Bloxkeep?"),
//! shown at start when asked for (`prefs::State::ask_vault`) or from Switch vault. Opening the picker or the new-vault form over
//! an open vault leaves it open, so Back returns without the password; only picking or setting up another vault locks it
//! (its Roblox windows keep running; auto-rejoin, schedules and Discord wait as when locked).
use std::cell::{Cell, RefCell};
use std::path::Path;

use serde_json::{Value, json};

use super::App;
use super::controller::error_text;
use super::custom_theme::{MAX_THEME_PICTURE, PICTURE_TYPES};
use super::persist::is_io_error;
use super::settings::{CSIDL_MYPICTURES, open_file_dialog, special_folder};
use crate::core::base::{Fail, Res, Stamp};
use crate::core::prefs::{self, VaultEntry};
use crate::core::vault::{self};
use crate::core::{l, log, themes, ui, vaults};

pub struct VaultsState {
    /// The picker is up instead of the password (start, or Switch vault).
    pub vault_pick: Cell<bool>,
    /// A vault being added: the form sets it up (name, master password) before it joins the list; the vault offered
    /// (or open) stays the active one until then.
    pub new_vault: RefCell<Option<VaultEntry>>,
    /// Adding started from the picker: Cancel goes back to it, else to the screen it came from.
    pub add_from_picker: Cell<bool>,
    /// The vault offered (or open) before a vault being added took its place, for Cancel.
    pub vault_before_add: RefCell<Option<VaultEntry>>,
}

impl Default for VaultsState {
    fn default() -> VaultsState {
        let pick = {
            let s = prefs::lock();
            s.ask_vault && s.vault_list.len() > 1
        };
        VaultsState { vault_pick: Cell::new(pick), new_vault: RefCell::new(None), add_from_picker: Cell::new(false), vault_before_add: RefCell::new(None) }
    }
}

/// The list's entry with this id, or the C#'s "no longer on this PC".
fn entry_of(id: &str) -> Res<VaultEntry> {
    prefs::lock().vault_list.iter().find(|v| v.id == id).cloned().ok_or_else(|| Fail::user(l::t("That vault is no longer on this PC.")))
}

/// Changes the list's entry with this id in place (C# changed the entry object itself).
fn change_entry(id: &str, f: impl FnOnce(&mut VaultEntry)) {
    let mut s = prefs::lock();
    if let Some(e) = s.vault_list.iter_mut().find(|v| v.id == id) {
        f(e);
    }
}

fn same(a: Option<&VaultEntry>, b: Option<&VaultEntry>) -> bool {
    a.map(|e| &e.id) == b.map(|e| &e.id)
}

/// DateTime.UtcNow.Ticks.
fn utc_ticks() -> i64 {
    Stamp::utc_now().unix_ms() * 10_000 + 621_355_968_000_000_000
}

pub fn delete_vault_picture(entry: &VaultEntry) {
    let Some(picture) = &entry.picture else { return };
    if !themes::is_picture_name(picture) {
        return;
    }
    let result = std::fs::remove_file(vault::pictures_dir().join(picture))
        .or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) })
        // one vault::tidy couldn't move yet
        .and_then(|_| std::fs::remove_file(vault::data_dir().join(picture)).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }));
    if let Err(ex) = result {
        log::write(&ex); // left behind: its name is no longer used
    }
}

impl App {
    pub fn vaults_dto(&self) -> Value {
        let (list, ask) = {
            let s = prefs::lock();
            let list: Vec<Value> = s
                .vault_list
                .iter()
                .map(|v| json!({ "id": v.id, "name": v.name, "picture": v.picture.as_ref().map(|p| format!("user/{p}")) }))
                .collect();
            (list, s.ask_vault)
        };
        let new_vault = self.vaults.new_vault.borrow().clone();
        let current = vaults::current();
        let about = new_vault.clone().or_else(|| current.clone());
        json!({
            "list": list,
            // The vault the lock screen (or the form over the open vault) is about: the one being added, or the one offered.
            "current": about.as_ref().map(|v| v.id.clone()),
            "name": about.as_ref().map(|v| v.name.clone()),
            "picture": if new_vault.is_none() { current.as_ref().and_then(|c| c.picture.as_ref()).map(|p| format!("user/{p}")) } else { None },
            // The vault open right now (none while locked): the sidebar's Switch vault, Settings › Vault & security.
            "open": if self.vault_ref().is_none() { None } else { current.as_ref().map(|v| v.id.clone()) },
            "pick": self.vaults.vault_pick.get(),
            "adding": new_vault.is_some(),
            "ask": ask,
        })
    }

    pub fn show_vault_picker(&self) -> Value {
        *self.vaults.new_vault.borrow_mut() = None;
        self.vaults.vault_pick.set(true);
        self.push_state();
        Value::Null
    }

    /// The picker's Back: to the open vault as it was, or to the password of the vault the lock screen offers.
    pub fn close_vault_picker(&self) -> Value {
        self.vaults.vault_pick.set(false);
        self.push_state();
        Value::Null
    }

    /// A vault chosen in the picker: the open one, back to it as it was; another, the open one locks and the lock
    /// screen asks for the chosen one's master password.
    pub fn select_vault(&self, id: &str) -> Res<Value> {
        let entry = entry_of(id)?;
        *self.vaults.new_vault.borrow_mut() = None;
        if self.vault_ref().is_some() && !same(Some(&entry), vaults::current().as_ref()) {
            self.lock()?;
        }
        vaults::use_entry(Some(&entry));
        let changed = {
            let mut s = prefs::lock();
            let changed = s.last_vault.as_deref() != Some(entry.id.as_str());
            if changed {
                s.last_vault = Some(entry.id.clone());
            }
            changed
        };
        if changed {
            prefs::save();
        }
        self.vaults.vault_pick.set(false);
        self.push_state();
        Ok(Value::Null)
    }

    /// "Add a vault" (the picker's +, or the lock screen's Create a new vault): a form for its name and own master
    /// password; an open vault stays open until the new one is set up.
    pub fn add_vault(&self) -> Value {
        self.vaults.add_from_picker.set(self.vaults.vault_pick.get());
        *self.vaults.new_vault.borrow_mut() = Some(vaults::new());
        self.vaults.vault_pick.set(false);
        self.push_state();
        Value::Null
    }

    pub fn cancel_add_vault(&self) -> Value {
        // Its set-up had started (and failed, or is still running): the lock screen offers the vault from before again, not a
        // file that may never be written (review, 2026-10-30). A set-up that finishes after all opens its own vault.
        let adding = self.vaults.new_vault.borrow().clone();
        if let Some(adding) = adding
            && vault::active_file().to_lowercase() == adding.file.to_lowercase()
        {
            let before = self.vaults.vault_before_add.borrow().clone();
            vaults::use_entry(before.as_ref());
        }
        *self.vaults.new_vault.borrow_mut() = None;
        let any = !prefs::lock().vault_list.is_empty();
        self.vaults.vault_pick.set(self.vaults.add_from_picker.get() && any);
        self.push_state();
        Value::Null
    }

    /// The vault being added is about to be set up: the open one locks (saved first) and the new one becomes the active vault.
    pub fn switch_to_new_vault(&self) -> Res<()> {
        let new_vault = self.vaults.new_vault.borrow().clone();
        let Some(new_vault) = new_vault else { return Ok(()) };
        if self.vault_ref().is_some() {
            self.lock()?;
        }
        if vault::active_file().to_lowercase() != new_vault.file.to_lowercase() {
            *self.vaults.vault_before_add.borrow_mut() = vaults::current();
        }
        vaults::use_entry(Some(&new_vault));
        Ok(())
    }

    /// `entry`: the vault just set up: the one being added, or the first one on this PC (vault.rmv, "Vault 1").
    /// Returns false when prefs.json couldn't be written: the vault is on the list for this session only, and the notice
    /// (`report_settings_save_failure`) says so. Its file is fine and it opens; the list is written by the next save that works.
    pub fn join_vault_list(&self, entry: &VaultEntry, name: Option<&str>) -> bool {
        let mut entry = entry.clone();
        if let Some(typed) = vaults::clean_name(name) {
            entry.name = typed;
        }
        {
            let mut s = prefs::lock();
            match s.vault_list.iter_mut().find(|v| v.id == entry.id) {
                Some(listed) => listed.name = entry.name.clone(),
                None => s.vault_list.push(entry.clone()),
            }
            s.last_vault = Some(entry.id.clone());
        }
        *self.vaults.new_vault.borrow_mut() = None;
        prefs::save()
    }

    pub fn rename_vault(&self, id: &str, name: &str) -> Res<Value> {
        entry_of(id)?;
        let name = vaults::clean_name(Some(name)).ok_or_else(|| Fail::user(l::t("Give the vault a name.")))?;
        change_entry(id, |e| e.name = name);
        prefs::save();
        self.push_state();
        Ok(Value::Null)
    }

    /// A picture from the PC for a vault's tile, copied into the data folder (shown before unlocking, so anyone at the PC sees it).
    pub fn pick_vault_picture(&self, id: &str) -> Res<Value> {
        let entry = entry_of(id)?;
        let owner = self.controller.owner.get();
        let title = l::t("Choose a picture for this vault");
        let filter = l::t("Pictures") + " (*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp)|*.png;*.jpg;*.jpeg;*.webp;*.gif;*.bmp";
        let initial = special_folder(CSIDL_MYPICTURES);
        let Some(chosen) = ui::modal(|| open_file_dialog(owner, &title, &filter, &initial)) else {
            return Ok(Value::Null);
        };
        let chosen = Path::new(&chosen);
        let kind = chosen.extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase())).unwrap_or_default();
        if !PICTURE_TYPES.contains(&kind.as_str()) {
            return Err(Fail::user(l::t("Pick a picture: a PNG, JPG, WebP, GIF or BMP file.")));
        }
        if std::fs::metadata(chosen).map_err(Fail::other)?.len() > MAX_THEME_PICTURE {
            return Err(Fail::user(l::t("That picture is over 20 MB. Pick a smaller one.")));
        }
        std::fs::create_dir_all(vault::pictures_dir()).map_err(Fail::other)?;
        let name = format!("vault-picture-{}-{}{kind}", entry.id, utc_ticks());
        let target = vault::pictures_dir().join(&name);
        if target.exists() {
            // File.Copy without overwrite.
            return Err(Fail::other(format!("The file '{}' already exists.", target.display())));
        }
        std::fs::copy(chosen, &target).map_err(Fail::other)?;
        delete_vault_picture(&entry);
        change_entry(id, |e| e.picture = Some(name));
        prefs::save();
        self.push_state();
        Ok(Value::Null)
    }

    pub fn clear_vault_picture(&self, id: &str) -> Res<Value> {
        let entry = entry_of(id)?;
        delete_vault_picture(&entry);
        change_entry(id, |e| e.picture = None);
        prefs::save();
        self.push_state();
        Ok(Value::Null)
    }

    /// Takes a vault off this PC's list (its password lost, say): its file is set aside, not deleted
    /// (`vault::set_aside`), and can come back with Restore from a backup. Not the vault open right now.
    pub fn remove_vault(&self, id: &str) -> Res<Value> {
        let entry = entry_of(id)?;
        if self.vault_ref().is_some() && same(Some(&entry), vaults::current().as_ref()) {
            return Err(Fail::user(l::t("Switch to another vault first.")));
        }
        let set_aside = || -> Res<()> {
            let path = vault::data_dir().join(&entry.file);
            let with = |suffix: &str| {
                let mut p = path.clone().into_os_string();
                p.push(suffix);
                std::path::PathBuf::from(p)
            };
            if path.is_file() || with(".bak").is_file() || with(".tmp").is_file() {
                if !path.is_file() {
                    let from = if with(".tmp").is_file() { with(".tmp") } else { with(".bak") };
                    std::fs::rename(from, &path).map_err(Fail::other)?;
                }
                let aside = vault::set_aside(&entry.file)?;
                log::note(&format!(
                    "A vault was taken off the list; its file was set aside as {}.",
                    aside.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                ));
            }
            Ok(())
        };
        if let Err(ex) = set_aside() {
            if !is_io_error(&ex) {
                return Err(ex);
            }
            log::note(&format!("Remove vault: its file couldn't be set aside ({}); kept on the list.", App::log_kind_of(&ex)));
            return Err(Fail::user(l::f("Couldn't remove the vault: {0}", &[&error_text(&ex)])));
        }
        delete_vault_picture(&entry);
        let offered = same(Some(&entry), vaults::current().as_ref());
        {
            let mut s = prefs::lock();
            s.forget_remote_key(entry.remote_key.as_deref());
            s.vault_list.retain(|v| v.id != entry.id);
            if offered || s.last_vault.as_deref() == Some(entry.id.as_str()) {
                let next = s.vault_list.first().cloned();
                vaults::use_entry(next.as_ref());
                s.last_vault = next.map(|n| n.id);
            }
            let count = s.vault_list.len();
            self.vaults.vault_pick.set(count > 1 || (self.vaults.vault_pick.get() && count > 0));
        }
        prefs::save();
        self.push_state();
        Ok(Value::Null)
    }

    pub fn set_ask_vault(&self, on: bool) -> Value {
        prefs::lock().ask_vault = on;
        prefs::save();
        self.push_state();
        Value::Null
    }
}
