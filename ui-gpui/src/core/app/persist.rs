//! AppController.Persist.cs: saving the vault: soon after a change, in the background, again after a failed save, and before closing.
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use super::App;
use super::controller::{SAVE_DELAY, error_text};
use crate::core::base::{Fail, Stamp};
use crate::core::{l, log, rt};

#[derive(Default)]
pub struct PersistState {
    pub save_due: Cell<Option<Stamp>>,
    pub save_failed: Cell<bool>,
    /// Saves in a row that failed: the next try waits longer (see Persist).
    pub save_failures: Cell<i32>,
}

pub const MAX_SAVE_RETRY: Duration = Duration::from_secs(5 * 60);

/// IOException or UnauthorizedAccessException: a write Windows refused (an antivirus, a sync client, a read-only folder).
// PORT: Fail carries only its text; an io::Error's text ends "(os error N)", which is what tells them apart here.
pub(crate) fn is_io_error(fail: &Fail) -> bool {
    matches!(fail, Fail::Other(m) if m.contains("(os error "))
}

impl App {
    /// Saves a few seconds later, so a burst of changes writes the encrypted vault once; on lock and exit too.
    pub fn persist_soon(&self) {
        if self.persist.save_due.get().is_none() {
            self.persist.save_due.set(Some(Stamp::utc_now().plus_secs(SAVE_DELAY.as_secs_f64())));
        }
    }

    pub fn persist_if_due(self: &Rc<Self>, now: Stamp) {
        if let Some(due) = self.persist.save_due.get()
            && now >= due
        {
            // The C#'s async method cleared it before its first await, so within this tick; rt::spawn starts the save later.
            self.persist.save_due.set(None);
            let app = self.clone();
            rt::spawn(async move { app.persist_in_background().await });
        }
    }

    /// The usual save: encrypted, written and flushed on a worker (`Vault::save_in_background`), which took the
    /// window 20-70 ms with a long play history. A save that must be on disk before going on (lock, exit, a deleted
    /// account) is `persist`.
    pub async fn persist_in_background(self: &Rc<Self>) {
        self.persist.save_due.set(None);
        let Some(vault) = self.vault_ref() else { return };
        self.note_open_sessions(); // as Persist does: after a crash the next unlock still finds the clients open now
        let result = match vault.save_in_background() {
            Ok(task) => rt::blocking(move || task.wait()).await,
            Err(ex) => Err(ex),
        };
        match result {
            Ok(()) => {
                if !self.is_vault(&vault) {
                    return; // locked meanwhile: closing finished the write
                }
                self.persist.save_failed.set(false);
                self.persist.save_failures.set(0);
                self.on_vault_saved();
            }
            Err(ex) if self.is_vault(&vault) => {
                if !is_io_error(&ex) {
                    log::write(&ex);
                }
                self.save_failed(&ex);
            }
            // ObjectDisposedException: the vault was closed meanwhile.
            Err(ex) => {
                if !is_io_error(&ex) {
                    log::write(&ex);
                }
            }
        }
    }

    /// false when the vault couldn't be written (it is tried again shortly).
    pub fn persist(&self) -> bool {
        self.persist.save_due.set(None);
        let Some(vault) = self.vault_ref() else { return true };
        // Every save holds the clients open now, so after a crash the next unlock still finds them (auto-rejoin, scheduled stop).
        self.note_open_sessions();
        match vault.save() {
            Ok(()) => {
                self.persist.save_failed.set(false);
                self.persist.save_failures.set(0);
                self.on_vault_saved();
                true
            }
            // PORT: the C# catches only IOException and UnauthorizedAccessException here and lets anything else crash the
            // command; Vault::save's other failures (a value that can't be written) are counted as a failed save too.
            Err(ex) => {
                if !is_io_error(&ex) {
                    log::write(&ex);
                }
                self.save_failed(&ex);
                false
            }
        }
    }

    /// Retried shortly, then less and less often up to every 5 minutes (an antivirus usually lets go within seconds; a
    /// sync client or read-only folder may not). Said once.
    pub fn save_failed(&self, ex: &Fail) {
        let shift = self.persist.save_failures.get().min(7) as u32;
        let wait = (SAVE_DELAY.as_secs_f64() * f64::from(1u32 << shift)).min(MAX_SAVE_RETRY.as_secs_f64());
        self.persist.save_due.set(Some(Stamp::utc_now().plus_secs(wait)));
        self.persist.save_failures.set(self.persist.save_failures.get() + 1);
        if !self.persist.save_failed.get() {
            self.toast(&l::f("Couldn't save data: {0}", &[&error_text(ex)]), "error");
        }
        self.persist.save_failed.set(true);
    }

    /// Before the vault goes away (lock, exit, an update's restart) or the PC sleeps: writes what waits and which clients
    /// are open. Tried a few times, since an antivirus usually lets go within a second; false when it still couldn't be written,
    /// so the caller can refuse rather than lose changes.
    pub fn save_before_closing(&self) -> bool {
        // A save still being written in the background is written again now, so a failure is known here.
        let open = self.note_open_sessions();
        let play = self.note_play_sessions();
        let runs = self.note_scheduled_runs();
        let saving = self.vault_ref().is_some_and(|v: Arc<_>| v.saving_in_background());
        let due = open | play | runs | self.persist.save_due.get().is_some() | saving;
        let mut saved = !due;
        let mut attempt = 1;
        while !saved && attempt <= 3 {
            if attempt > 1 {
                std::thread::sleep(Duration::from_millis(500));
            }
            saved = self.persist();
            attempt += 1;
        }
        self.backup_if_dirty();
        saved
    }
}
