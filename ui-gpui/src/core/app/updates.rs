// AppController.Updates.cs: Bloxkeep's own updates and What's new.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{Value, json};

use super::App;
use super::controller::UPDATE_CHECK_INTERVAL;
use crate::core::app_updater::AppUpdater;
use crate::core::base::{Res, Stamp};
use crate::core::models::stamp_min;
use crate::core::roblox_installs::ClientVersion;
use crate::core::whats_new::{self, Release};
use crate::core::{prefs, rt};

/// The instance fields AppController.Updates.cs declares.
pub struct UpdatesState {
    /// "Restart to update" was clicked and the app is on its way out (see ApplyUpdateAsync).
    pub applying_update: Cell<bool>,
    // `readonly AppUpdater _updater = new()`; kept in an Arc so its checks can run on a worker.
    pub updater: RefCell<Arc<AppUpdater>>,
    /// Due at the first tick: an update found before the vault is opened installs at the lock screen
    /// (see UpdateOffersAtLock), so the sooner it is known the better.
    pub next_update_check: Cell<Stamp>,
    pub next_roblox_version_check: Cell<Stamp>,
    /// When Roblox's old logs are next cleared (roblox_log_watcher::clean_old_logs): at the first tick, then daily.
    pub next_log_clean: Cell<Stamp>,
    pub latest_roblox: RefCell<Option<ClientVersion>>,
    /// The version that ran before an update, until the page has shown what's new since then.
    pub updated_from: RefCell<Option<String>>,
}

impl Default for UpdatesState {
    fn default() -> UpdatesState {
        UpdatesState {
            applying_update: Cell::new(false),
            updater: RefCell::new(Arc::new(AppUpdater::new())),
            next_update_check: Cell::new(Stamp::utc_now()),
            next_roblox_version_check: Cell::new(stamp_min()),
            next_log_clean: Cell::new(stamp_min()),
            latest_roblox: RefCell::new(None),
            updated_from: RefCell::new(None),
        }
    }
}

fn notes_json(n: &Option<whats_new::Notes>) -> Value {
    match n {
        Some(n) => json!({ "en": n.en, "vi": n.vi }),
        None => Value::Null,
    }
}

/// Releases as the page gets them (WhatsNew's records, camelCase).
pub fn releases_json(list: &[Release]) -> Value {
    Value::Array(
        list.iter()
            .map(|r| {
                json!({
                    "version": r.version,
                    "date": r.date,
                    "en": r.en,
                    "vi": r.vi,
                    "highlights": r.highlights.as_ref().map(|h| h.iter().map(|h| json!({
                        "icon": h.icon, "go": h.go, "en": h.en, "vi": h.vi, "image": h.image,
                    })).collect::<Vec<_>>()),
                    "new": notes_json(&r.new),
                    "improved": notes_json(&r.improved),
                    "fixed": notes_json(&r.fixed),
                    "removed": notes_json(&r.removed),
                })
            })
            .collect(),
    )
}

impl App {
    /// `_updater.Changed += OnUpdaterChanged`: the updater reports from its own threads; each report hops to the core's.
    pub fn hook_updater(&self) {
        let updater = self.updates.updater.borrow().clone();
        updater.set_on_changed(Some(Arc::new(|| rt::post(|| App::with(|app| app.on_updater_changed())))));
    }

    pub fn on_updater_changed(&self) {
        // PORT: `!_owner.IsHandleCreated` (the window not made yet, or gone) and the InvalidOperationException of a
        // closing window: here the report simply runs on the core's thread, or not at all once disposed.
        if self.controller.disposed.get() {
            return;
        }
        // A download that failed: the lock screen asks again when the next check finds the update.
        if self.updates.updater.borrow().status() == "error" {
            self.relaunch.update_at_lock.set(None);
        }
        self.note_update_ready(); // Statistics › Activity
        self.push_state();
    }

    pub async fn check_updates(self: &Rc<Self>) -> Res<Value> {
        self.updates.next_update_check.set(Stamp::utc_now().plus_secs(UPDATE_CHECK_INTERVAL.as_secs_f64()));
        let updater = self.updates.updater.borrow().clone();
        let _ = rt::blocking({
            let updater = updater.clone();
            move || updater.check()
        })
        .await;
        Ok(updater.dto())
    }

    /// Remembers which version last ran. Older than this one: the page shows the release notes in between, once.
    /// A fresh install has nothing remembered and shows nothing.
    pub fn note_version(&self) {
        let updater = self.updates.updater.borrow().clone();
        let current = updater.current_version().to_string();
        let last_version = prefs::lock().last_version.clone();
        // The updater swapped the program files and rewrote the shortcuts; Explorer may keep showing a blank icon.
        if let Some(previous) = &last_version
            && *previous != current
            && updater.supported()
        {
            crate::core::ui::refresh_shell_icons();
        }
        if let Some(last) = &last_version
            && !whats_new::after_update(last, &current).is_empty()
        {
            *self.updates.updated_from.borrow_mut() = Some(last.clone());
            return;
        }
        // Only ever raised: an older build run afterwards (a development copy) must not make "what's new" show again.
        if last_version.as_deref().is_none_or(|seen| whats_new::older(seen, &current)) {
            prefs::lock().last_version = Some(current);
            prefs::save();
        }
    }

    pub fn seen_whats_new(&self) -> Value {
        *self.updates.updated_from.borrow_mut() = None;
        let current = self.updates.updater.borrow().current_version().to_string();
        prefs::lock().last_version = Some(current);
        prefs::save();
        Value::Null
    }

    pub fn set_auto_update(&self, enabled: bool) -> Value {
        prefs::lock().check_updates = enabled;
        prefs::save();
        if enabled {
            self.updates.next_update_check.set(Stamp::utc_now());
        }
        self.push_state();
        Value::Null
    }

    pub fn set_beta_updates(&self, enabled: bool) -> Value {
        prefs::lock().beta_updates = enabled;
        prefs::save();
        // Turned on: the beta out now comes with the next check, at once rather than within the hour. Turned off with a beta
        // downloaded: that one goes, and the regular versions are looked at again.
        let updater = self.updates.updater.borrow().clone();
        if enabled || updater.beta_turned_off() {
            self.updates.next_update_check.set(Stamp::utc_now());
        }
        self.push_state();
        Value::Null
    }
}
