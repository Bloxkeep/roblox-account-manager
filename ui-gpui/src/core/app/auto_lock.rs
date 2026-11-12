//! AppController.AutoLock.cs: the lock screen after a while without use (Settings › Vault & security › Lock automatically),
//! like Windows' own: after the chosen time with no keyboard or mouse input (anti-AFK's nudges don't count), the page asks for
//! the master password again. The vault stays open behind it, so Roblox, auto-rejoin, scheduled presets and Discord carry on;
//! the page gets nothing of it meanwhile, commands that would show or change it are refused, and whatever shows an account
//! goes (sign-in and Quick Login windows, signed-in websites, a secret on the clipboard, the monitor tab's pictures).
//! The Lock button still closes Bloxkeep for real.
use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::App;
use crate::core::base::{Fail, Res, Stamp};
use crate::core::vault::Vault;
use crate::core::{backup, l, log, prefs, rt, ui};

/// The choices Settings offers, in minutes; 0 never.
pub const AUTO_LOCK_CHOICES: [i32; 7] = [0, 5, 10, 15, 30, 60, 120];

pub struct AutoLockState {
    /// The lock screen is up with the vault open behind it.
    pub screen_locked: Cell<bool>,
    /// When someone was last known to be there without Windows' input clock (an unlock, the setting chosen):
    /// the wait counts from the later of the two.
    pub active_at: Cell<Stamp>,
    pub settings_failure_told: Cell<bool>,
}

impl Default for AutoLockState {
    fn default() -> AutoLockState {
        AutoLockState { screen_locked: Cell::new(false), active_at: Cell::new(Stamp::utc_now()), settings_failure_told: Cell::new(false) }
    }
}

pub fn auto_lock_due(minutes: i32, idle: Duration) -> bool {
    minutes > 0 && idle >= Duration::from_secs(minutes as u64 * 60)
}

impl App {
    /// The vault, for what the page may see: None while locked, the screen too.
    pub fn open(&self) -> Option<Arc<Vault>> {
        if self.auto_lock.screen_locked.get() { None } else { self.vault_ref() }
    }

    /// The periodic check; idle comes from anti_afk::idle_time.
    pub fn check_auto_lock(&self, idle: Duration, now: Stamp) {
        self.report_settings_save_failure();
        let Some(vault) = self.vault_ref() else { return };
        if self.auto_lock.screen_locked.get() {
            return;
        }
        let since_active = Duration::from_secs_f64(now.secs_since(self.auto_lock.active_at.get()).max(0.0));
        let minutes = vault.data().settings.auto_lock_minutes;
        if auto_lock_due(minutes, if since_active < idle { since_active } else { idle }) {
            self.screen_lock();
        }
    }

    /// prefs.json couldn't be written (`prefs::save_failed`): said once, as the vault's own failure is, until a
    /// save works again. Checked with the lock timer, since prefs::save is called from everywhere and has no window to speak in.
    pub fn report_settings_save_failure(&self) {
        let failed = prefs::save_failed();
        // (a toast behind the lock screen is dropped: told once it's down)
        if failed == self.auto_lock.settings_failure_told.get() || self.auto_lock.screen_locked.get() {
            return;
        }
        self.auto_lock.settings_failure_told.set(failed);
        if failed {
            self.toast(&l::t("Couldn't save your settings. Does another program hold Bloxkeep's data folder?"), "error");
        }
    }

    pub fn screen_lock(&self) {
        self.auto_lock.screen_locked.set(true);
        self.quick_login_cancel();
        self.login_cancel();
        let _ = self.drop_vault_import(false);
        self.close_websites(None); // signed in as saved accounts
        ui::clear_secret_clipboard();
        // the live pictures of the Roblox windows go at once, whatever the page does next
        let set_area = self.monitor.set_monitor_area.borrow().clone();
        if let Some(set_area) = set_area {
            set_area(None);
        }
        *self.controller.page_view.borrow_mut() = None; // what the page showed doesn't come back without the password, nor after a restart
        let minutes = self.vault(|d| d.settings.auto_lock_minutes.to_string()).unwrap_or_default();
        log::note(&format!("Locked the screen after {minutes} min without use; Roblox and the background carry on."));
        self.push_state();
    }

    /// The lock screen's password, checked against the open vault's key; the page gets everything again.
    pub async fn unlock_screen(&self, open: Arc<Vault>, password: &str) -> Res<Value> {
        if !self.auto_lock.screen_locked.get() {
            return Ok(self.build_state());
        }
        if password.is_empty() {
            return Err(Fail::user(l::t("Enter your master password.")));
        }
        let (v, p) = (open.clone(), password.to_string());
        let right = rt::blocking(move || v.verify_password(&p)).await;
        // Locked for real (or unlocked by another try) meanwhile: that stands.
        if !self.is_vault(&open) || !self.auto_lock.screen_locked.get() {
            return Ok(self.build_state());
        }
        if !right {
            return Err(Fail::user(l::t("Wrong master password."))); // as the full unlock says it
        }
        self.auto_lock.screen_locked.set(false);
        self.auto_lock.active_at.set(Stamp::utc_now());
        self.backup.weak_password.set(backup::weak_password(password));
        *self.controller.last_metrics.borrow_mut() = None; // none went out while locked: the next check sends them again
        log::note("Unlocked the screen.");
        Ok(self.build_state())
    }
}
