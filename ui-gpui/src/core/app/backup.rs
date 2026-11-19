//! AppController.Backup.cs: vault backups (see `crate::core::backup`): a copy after a data change, at most every ten minutes,
//! and on lock and exit when something changed (waited for a few seconds at most, else made at the next start). Bookkeeping
//! alone (play time, recent games) is copied at most every twelve hours. Settings live in `prefs` so a restore works
//! before unlocking.
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::App;
use super::controller::{error_text, opt_bool, opt_str};
use super::persist::is_io_error;
use super::settings::open_file_dialog;
use crate::core::backup::{self, Folder};
use crate::core::base::{Cancel, Fail, Res, Stamp};
use crate::core::models::VaultData;
use crate::core::vault::{self, Vault};
use crate::core::{l, log, prefs, rt, ui, vaults};

pub const BACKUP_INTERVAL: Duration = Duration::from_secs(10 * 60);
pub const BOOKKEEPING_BACKUP_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);
/// The most the copy on lock, exit or sleep holds the window up; a slower one is made at the next start.
pub const BACKUP_CLOSING_WAIT: Duration = Duration::from_secs(5);

/// A copy's write on its own thread (C#'s `Task` of Backup.Write): waited for with or without a time limit.
pub struct BackupWrite {
    done: Mutex<Option<Res<()>>>,
    cv: Condvar,
}

impl BackupWrite {
    /// Task.Run(() => Backup.Write(folder, bytes, previousIds)).
    pub fn start(folder: PathBuf, bytes: Vec<u8>, previous_ids: Vec<String>) -> Arc<BackupWrite> {
        let task = Arc::new(BackupWrite { done: Mutex::new(None), cv: Condvar::new() });
        let t = task.clone();
        crate::threads::run(move || {
            let result = backup::write(&folder, &bytes, Some(&previous_ids)).map(|_| ());
            *t.done.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
            t.cv.notify_all();
        });
        task
    }

    pub fn is_completed(&self) -> bool {
        self.done.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    /// Its result, or None when it is still writing after `timeout`.
    pub fn wait_timeout(&self, timeout: Duration) -> Option<Res<()>> {
        let guard = self.done.lock().unwrap_or_else(|e| e.into_inner());
        let (guard, _) = self.cv.wait_timeout_while(guard, timeout, |d| d.is_none()).unwrap_or_else(|e| e.into_inner());
        guard.clone()
    }

    /// Waited for without a thread: the write is a pool job, and a pool job waiting for it could wait for ever when the
    /// pool is full.
    pub async fn wait_async(&self) -> Res<()> {
        loop {
            if let Some(r) = self.done.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                return r;
            }
            rt::sleep(Duration::from_millis(20)).await;
        }
    }
}

#[derive(Default)]
pub struct BackupState {
    // PORT: `_backupTimer` (a WinForms timer) is a spawned wait; this calls the waiting one off (Stop).
    pub backup_timer: RefCell<Option<Cancel>>,
    pub backup_dirty: Cell<bool>,
    /// Saves since unlocking, and how many of them the last backup holds: when they differ, bookkeeping changed.
    pub vault_saves: Cell<i32>,
    pub backed_up_saves: Cell<i32>,
    /// The vault on disk and the last backup, hashed without bookkeeping (`content_hash`).
    pub saved_hash: RefCell<Option<String>>,
    pub backup_hash: RefCell<Option<String>>,
    /// A copy in progress on a worker thread, so two never run at once (C#'s `_backupTask` while not completed).
    pub backup_busy: Cell<bool>,
    /// Its write alone (no bookkeeping, which needs the core's thread): what the copy on lock and exit waits for.
    pub backup_write: RefCell<Option<Arc<BackupWrite>>>,
    /// Judged when the password was typed; the password itself is never kept.
    pub weak_password: Cell<bool>,
    pub backup_error: RefCell<Option<String>>,
    /// Synced folders on this PC; looked for on page load and backup changes only, since it lists the drives.
    pub suggested_folders: RefCell<Option<Vec<Folder>>>,
    /// The vault file a restore replaced and where its copy went, until that vault next opens: an unlock that fails
    /// meanwhile says where the copy is, in case the backup itself was damaged (review, 2026-10-30).
    pub restored_over: RefCell<Option<(String, String)>>,
}

/// The App, for work that started on its thread and goes on there later (a timer, a copy not waited for).
fn this() -> Option<Rc<App>> {
    let mut app = None;
    App::with(|a| app = Some(a.clone()));
    app
}

fn backup_ready() -> bool {
    let s = prefs::lock();
    s.backup_enabled && s.backup_folder.is_some()
}

/// Path.GetFullPath, its ending separator trimmed.
fn full_path(p: &str) -> String {
    let full = std::path::absolute(p).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| p.to_string());
    let trimmed = full.trim_end_matches(['\\', '/']);
    // "C:\" keeps its separator (Path.TrimEndingDirectorySeparator leaves a root as it is).
    if trimmed.ends_with(':') { full } else { trimmed.to_string() }
}

pub fn same_path(a: &str, b: Option<&str>) -> bool {
    b.is_some_and(|b| full_path(a).to_lowercase() == full_path(b).to_lowercase())
}

/// What isn't hashed (`content_hash`), by class. Play time and activity change every few minutes, so they go with the
/// twelve-hourly bookkeeping copy.
const VAULT_DATA_BOOKKEEPING: &[&str] = &["OpenSessions", "EndedSessions", "WaitingRejoins", "PlaySessions", "Activity", "ScheduledRuns"];
const DISCORD_BOOKKEEPING: &[&str] = &["LastDaily", "LastWeekly", "BoardMessageId"];
const SETTINGS_BOOKKEEPING: &[&str] = &["LastPlaceInput", "LastJobId", "Layout", "Sort", "RecentGames", "RecentPlayers"];
const PRESET_BOOKKEEPING: &[&str] = &["LastRunAt"];
const SCHEDULE_BOOKKEEPING: &[&str] = &["LastRun"];
const ACCOUNT_BOOKKEEPING: &[&str] = &["LastUsed", "LastChecked"];

/// A property of the vault's JSON, whatever case its name is written in.
fn child<'a>(v: &'a mut Value, name: &str) -> Option<&'a mut Value> {
    let o = v.as_object_mut()?;
    let key = o.keys().find(|k| k.eq_ignore_ascii_case(name))?.clone();
    o.get_mut(&key)
}

fn strip(v: &mut Value, names: &[&str]) {
    if let Some(o) = v.as_object_mut() {
        o.retain(|k, _| !names.iter().any(|n| k.eq_ignore_ascii_case(n)));
    }
}

/// The vault's contents minus the bookkeeping: what makes a backup due within minutes.
// PORT: C# left the bookkeeping out while serialising, by class, and hashed the stream; here the vault's JSON is made as a
// value, the same properties taken out where those classes sit in the vault, and the text hashed.
pub fn content_hash(data: &VaultData) -> String {
    let mut v = match serde_json::to_value(data) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    strip(&mut v, VAULT_DATA_BOOKKEEPING);
    if let Some(discord) = child(&mut v, "Discord") {
        strip(discord, DISCORD_BOOKKEEPING);
    }
    if let Some(settings) = child(&mut v, "Settings") {
        if let Some(Value::Array(presets)) = child(settings, "Presets") {
            for preset in presets {
                strip(preset, PRESET_BOOKKEEPING);
                if let Some(schedule) = child(preset, "Schedule") {
                    strip(schedule, SCHEDULE_BOOKKEEPING);
                }
            }
        }
        strip(settings, SETTINGS_BOOKKEEPING);
    }
    if let Some(Value::Array(accounts)) = child(&mut v, "Accounts") {
        for account in accounts {
            strip(account, ACCOUNT_BOOKKEEPING);
        }
    }
    let hash = Sha256::digest(v.to_string().as_bytes());
    hash.iter().map(|b| format!("{b:02X}")).collect()
}

/// A restored backup with an id this PC never had (`prefs::State::known_vault_ids`) was made elsewhere: noted so its first
/// unlock runs `drop_foreign_links`. A backup of this PC's own vault keeps everything. The caller saves the prefs.
pub(crate) fn note_restored(file: &Path) {
    let id = vault::id_of_path(file);
    let mut s = prefs::lock();
    s.foreign_restore = id.filter(|id| !s.known_vault_ids.contains(id));
}

/// The user's own folder (Environment.SpecialFolder.UserProfile).
fn user_profile() -> String {
    std::env::var("USERPROFILE").unwrap_or_default()
}

pub mod folder_dialog {
    //! FolderBrowserDialog.ShowDialog(owner).
    use crate::core::win_kit;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize};
    use windows_sys::Win32::UI::Shell::{BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, SHBrowseForFolderW, SHGetPathFromIDListW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_USER};

    /// CoInitializeEx: the thread is already in the other kind of apartment.
    const RPC_E_CHANGED_MODE: i32 = 0x8001_0106u32 as i32;
    const BFFM_INITIALIZED: u32 = 1;
    const BFFM_SETSELECTIONW: u32 = WM_USER + 103;

    unsafe extern "system" fn on_event(hwnd: HWND, msg: u32, _lparam: LPARAM, data: LPARAM) -> i32 {
        if msg == BFFM_INITIALIZED && data != 0 {
            // SAFETY: `data` is the NUL-terminated folder `pick` keeps alive while the dialog is up.
            unsafe { SendMessageW(hwnd, BFFM_SETSELECTIONW, 1, data) };
        }
        0
    }

    /// The folder chosen, or None when cancelled.
    // PORT: .NET's FolderBrowserDialog shows Windows' Vista folder picker (IFileOpenDialog) with the description as its
    // title; this is the shell's folder browser (SHBrowseForFolder), the description above its tree, the "Make new
    // folder" button included. With BIF_NEWDIALOGSTYLE it needs a single-threaded COM apartment on the calling thread:
    // it runs on a worker (the core's apartment is multithreaded), so it starts one here and ends it before returning.
    pub fn pick(owner: isize, description: &str, initial_dir: &str) -> Option<String> {
        // SAFETY: plain call, paired with CoUninitialize below on this same thread when it succeeded.
        let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as _) };
        if hr == RPC_E_CHANGED_MODE {
            // This pool thread was left in a multithreaded apartment by an earlier job and can't start the single-threaded
            // one the dialog needs: a thread of its own (a rare click).
            let (description, initial_dir) = (description.to_string(), initial_dir.to_string());
            return crate::threads::own("folder-dialog", move || pick(owner, &description, &initial_dir)).ok().and_then(|h| h.join().ok()).flatten();
        }
        let chosen = browse(owner, description, initial_dir);
        if hr >= 0 {
            // SAFETY: CoInitializeEx succeeded on this thread.
            unsafe { CoUninitialize() };
        }
        chosen
    }

    fn browse(owner: isize, description: &str, initial_dir: &str) -> Option<String> {
        let title = win_kit::wide(description);
        let initial = win_kit::wide(initial_dir);
        let mut info: BROWSEINFOW = unsafe { std::mem::zeroed() };
        info.hwndOwner = owner as HWND;
        info.lpszTitle = title.as_ptr();
        info.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
        info.lpfn = Some(on_event);
        info.lParam = initial.as_ptr() as LPARAM;
        // SAFETY: `info` and the strings it points at outlive the modal call.
        let list = unsafe { SHBrowseForFolderW(&info) };
        if list.is_null() {
            return None;
        }
        let mut path = vec![0u16; 32_768];
        // SAFETY: `list` came from SHBrowseForFolderW; `path` has room for any path; the list is freed once.
        let ok = unsafe { SHGetPathFromIDListW(list, path.as_mut_ptr()) } != 0;
        unsafe { CoTaskMemFree(list as *const _) };
        if ok { Some(win_kit::from_wide(&path)) } else { None }
    }
}

impl App {
    pub fn backup_dto(&self) -> Value {
        let (enabled, folder, last_backup) = {
            let s = prefs::lock();
            (s.backup_enabled, s.backup_folder.clone(), s.last_backup())
        };
        let suggested: Vec<Value> = {
            let mut cached = self.backup.suggested_folders.borrow_mut();
            cached
                .get_or_insert_with(backup::suggestions)
                .iter()
                .filter(|s| !same_path(&s.path.to_string_lossy(), folder.as_deref()))
                .map(|s| json!({ "path": s.path.to_string_lossy(), "name": s.name }))
                .collect()
        };
        json!({
            "enabled": enabled,
            "folder": folder,
            "lastBackup": last_backup,
            "error": self.backup.backup_error.borrow().clone(),
            "suggested": suggested,
            "cloud": folder.as_deref().is_some_and(backup::looks_like_cloud),
            "weakPassword": self.vault_ref().is_some() && self.backup.weak_password.get(),
        })
    }

    /// The page opened Settings: look for synced folders again (a drive may have been connected since).
    pub fn refresh_backup_suggestions(&self) -> Value {
        *self.backup.suggested_folders.borrow_mut() = Some(backup::suggestions());
        self.push_state_if_changed();
        Value::Null
    }

    /// Hashed only when a backup needs it (none set up: no hash at all).
    pub fn saved_hash(&self) -> Option<String> {
        if self.backup.saved_hash.borrow().is_none() {
            let hash = self.vault_ref().map(|v| content_hash(&v.data()));
            *self.backup.saved_hash.borrow_mut() = hash;
        }
        self.backup.saved_hash.borrow().clone()
    }

    /// After each successful save: a backup is due when something other than bookkeeping changed, else later.
    pub fn on_vault_saved(&self) {
        self.backup.vault_saves.set(self.backup.vault_saves.get() + 1);
        *self.backup.saved_hash.borrow_mut() = None;
        // Without backups nothing reads the hash: a save doesn't pay for it. Turning them on copies at once anyway.
        if !backup_ready() {
            self.backup.backup_dirty.set(true);
            return;
        }
        let dirty = self.saved_hash() != *self.backup.backup_hash.borrow();
        self.backup.backup_dirty.set(dirty);
        // Noted outside the vault, so a change the app couldn't copy before it closed (killed, or the copy on exit took
        // too long) is copied at the next unlock.
        if dirty && backup_ready() {
            let mut s = prefs::lock();
            if !s.backup_pending() {
                s.set_backup_pending(true);
                drop(s);
                prefs::save();
            }
        }
        self.backup_when_due();
    }

    /// Backs up a moment later once the last copy is old enough: ten minutes for data, twelve hours for bookkeeping.
    pub fn backup_when_due(&self) {
        if !backup_ready() {
            return;
        }
        let every = if self.backup.backup_dirty.get() {
            BACKUP_INTERVAL
        } else if self.backup.vault_saves.get() != self.backup.backed_up_saves.get() {
            BOOKKEEPING_BACKUP_INTERVAL
        } else {
            return;
        };
        let last = prefs::lock().last_backup();
        // Never further off than the interval: a last-backup time in the future (the clock was set back) must not
        // hold the next copy off for days.
        let left = match last {
            Some(last) => last.plus_secs(every.as_secs_f64()).secs_since(Stamp::utc_now()).clamp(0.0, every.as_secs_f64()),
            None => 0.0,
        };
        self.schedule_backup(Duration::from_secs_f64(left));
    }

    /// A change the last run didn't copy (`backup_pending`), or no backup yet, is copied right after unlocking. Otherwise
    /// only bookkeeping can be newer (data is copied on lock and exit): the twelve-hour wait.
    pub fn on_vault_unlocked(&self) {
        *self.backup.saved_hash.borrow_mut() = None;
        let (last, pending) = {
            let s = prefs::lock();
            (s.last_backup(), s.backup_pending())
        };
        self.backup.backup_dirty.set(last.is_none() || pending);
        self.backup.backed_up_saves.set(0);
        let written = std::fs::metadata(self.open_vault_path()).and_then(|m| m.modified()).ok().map(|t| {
            let ms = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
            Stamp::from_unix_ms(ms)
        });
        let newer = matches!((last, written), (Some(last), Some(written)) if written > last);
        self.backup.vault_saves.set(if newer { 1 } else { 0 });
        if !self.backup.backup_dirty.get() && backup_ready() {
            let hash = self.saved_hash();
            *self.backup.backup_hash.borrow_mut() = hash;
        }
        self.backup_when_due();
    }

    /// Old backups still open only with the old password, so a fresh one goes out at once.
    pub fn on_password_changed(&self, password: &str) {
        self.backup.weak_password.set(backup::weak_password(password));
        *self.backup.backup_hash.borrow_mut() = None;
        self.backup.backup_dirty.set(true);
        if backup_ready() {
            spawn_try_backup(true);
        }
    }

    /// On lock and exit: whatever changed since the last copy goes out now, before the vault goes away.
    pub fn backup_if_dirty(&self) {
        if self.backup.backup_dirty.get() && backup_ready() && self.open_vault_path().is_file() {
            self.try_backup(false);
        }
    }

    /// A copy's write still going on its worker thread (`_backupWrite is { IsCompleted: false }`, read by Relaunch).
    pub fn backup_writing(&self) -> bool {
        self.backup.backup_write.borrow().as_ref().is_some_and(|w| !w.is_completed())
    }

    fn stop_backup_timer(&self) {
        if let Some(timer) = self.backup.backup_timer.borrow_mut().take() {
            timer.cancel();
        }
    }

    pub fn schedule_backup(&self, delay: Duration) {
        if self.controller.disposed.get() {
            return;
        }
        self.stop_backup_timer();
        let timer = Cancel::new();
        *self.backup.backup_timer.borrow_mut() = Some(timer.clone());
        let delay = delay.max(Duration::from_millis(1000));
        rt::spawn(async move {
            // Called off (a newer save rescheduled it): ends at once, so a burst of saves doesn't leave tasks parked
            // until their old time.
            if !rt::sleep_or(&timer, delay).await {
                return;
            }
            let Some(app) = this() else { return };
            app.stop_backup_timer();
            let due = app.backup.backup_dirty.get() || app.backup.vault_saves.get() != app.backup.backed_up_saves.get();
            if app.vault_ref().is_some() && due && backup_ready() {
                let _ = app.try_backup_async(true).await;
            }
        });
    }

    /// The open vault's own file (the active one may be another while a vault is being added).
    pub fn open_vault_path(&self) -> PathBuf {
        match self.vault_ref() {
            Some(v) => v.file_path().to_path_buf(),
            None => vault::vault_path(),
        }
    }

    /// The vault's bytes, read on the core's thread so no save can run meanwhile; None (and noted) when unreadable.
    pub fn read_vault_for_backup(&self) -> Option<Vec<u8>> {
        match std::fs::read(self.open_vault_path()) {
            Ok(bytes) => Some(bytes),
            Err(ex) => {
                self.backup_failed(&Fail::from(ex));
                None
            }
        }
    }

    /// `hash`: what the vault held when its bytes were read. `saves`: how many saves since unlocking those bytes held.
    pub fn backup_done(&self, hash: Option<String>, saves: i32) {
        prefs::lock().set_last_backup(Some(Stamp::utc_now()));
        *self.backup.backup_hash.borrow_mut() = hash.clone();
        self.backup.backed_up_saves.set(saves);
        let dirty = self.saved_hash() != hash;
        self.backup.backup_dirty.set(dirty);
        prefs::lock().set_backup_pending(dirty);
        prefs::save();
        if self.backup.backup_error.borrow().is_some() {
            log::note("Backup: recovered, a copy was written.");
        }
        *self.backup.backup_error.borrow_mut() = None;
    }

    /// The reason is shown under Settings › Backup and the copy tried again later.
    pub fn backup_failed(&self, ex: &Fail) {
        if self.backup.backup_error.borrow().is_none() {
            log::note(&format!("Backup: copy failed ({}); tried again in {} min.", App::log_kind_of(ex), BACKUP_INTERVAL.as_secs() / 60));
        }
        // A folder Windows won't let Bloxkeep write to (no permission, a drive gone, a file held open) is said as that,
        // not as a file error; the details go to the log either way.
        let text = if is_io_error(ex) {
            log::write(ex);
            l::t("Can't write to this folder.")
        } else {
            error_text(ex)
        };
        *self.backup.backup_error.borrow_mut() = Some(text);
        self.schedule_backup(BACKUP_INTERVAL);
    }

    /// The copy for lock, exit and sleep. The folder may be slow or synced, so it waits `BACKUP_CLOSING_WAIT` at most; a
    /// slower copy finishes on its own and `backup_pending` has the next unlock make it.
    pub fn try_backup(&self, push: bool) -> bool {
        self.stop_backup_timer();
        let clock = Instant::now();
        let left = || BACKUP_CLOSING_WAIT.saturating_sub(clock.elapsed());
        // A copy still writing on its worker thread would use the same temp file: it gets what is left of the wait. Only
        // its write is waited for; its bookkeeping needs this thread and runs later, if the app is still there.
        let writing = self.backup.backup_write.borrow().clone();
        if let Some(writing) = writing.filter(|w| !w.is_completed())
            && writing.wait_timeout(left()).is_none()
        {
            log::note("Backup on closing: the copy still being written took too long; none made now.");
            return false;
            // (a failed one: its own continuation reports the failure)
        }
        let mut ok = false;
        if let Some(bytes) = self.read_vault_for_backup() {
            let hash = self.saved_hash();
            let saves = self.backup.vault_saves.get();
            let folder = prefs::lock().backup_folder.clone().unwrap_or_default();
            let previous_ids: Vec<String> = self.vault_ref().map(|v| v.data().settings.previous_vault_ids.clone()).unwrap_or_default();
            let write = BackupWrite::start(PathBuf::from(folder), bytes, previous_ids);
            *self.backup.backup_write.borrow_mut() = Some(write.clone());
            match write.wait_timeout(left()) {
                Some(Ok(())) => {
                    self.backup_done(hash, saves);
                    ok = true;
                }
                Some(Err(ex)) => self.backup_failed(&ex),
                None => log::note("Backup on closing took too long; it is made at the next unlock."),
            }
        }
        if push {
            self.push_state();
        }
        ok
    }

    /// The usual copy, written and pruned on a worker thread (the folder is often slow or synced). A request
    /// during one waits for it, then copies what is on disk by then.
    pub async fn try_backup_async(self: &Rc<Self>, push: bool) -> bool {
        self.stop_backup_timer();
        // PORT: C# awaited the running copy's Task; here the flag is looked at again every 100 ms until it ends.
        while self.backup.backup_busy.get() {
            rt::sleep(Duration::from_millis(100)).await;
        }
        let busy = crate::core::base::Busy::set(&self.backup.backup_busy);
        let ok = self.backup_on_worker().await;
        drop(busy);
        if ok {
            self.backup_when_due(); // the vault changed again while the copy was being written (a failure retries later by itself)
        }
        if push {
            self.push_state();
        }
        ok
    }

    pub async fn backup_on_worker(self: &Rc<Self>) -> bool {
        // A copy the lock or exit stopped waiting for may still be writing the same temp file.
        let lingering = self.backup.backup_write.borrow().clone();
        if let Some(lingering) = lingering.filter(|w| !w.is_completed()) {
            // That copy failed: this one is made anyway.
            let _ = lingering.wait_async().await;
        }
        // A change still waiting to be saved (or saving in the background) goes into the file first, so copy and hash match.
        let saving = self.vault_ref().is_some_and(|v: Arc<Vault>| v.saving_in_background());
        if (self.persist.save_due.get().is_some() || saving) && self.vault_ref().is_some() {
            self.persist();
        }
        let Some(bytes) = self.read_vault_for_backup() else { return false };
        let hash = self.saved_hash();
        let saves = self.backup.vault_saves.get();
        let folder = prefs::lock().backup_folder.clone().unwrap_or_default();
        // a copy: the list belongs to the core's thread
        let previous_ids: Vec<String> = self.vault_ref().map(|v| v.data().settings.previous_vault_ids.clone()).unwrap_or_default();
        // Whose copy this is, kept now: a lock that gave up waiting for it may have let another vault open by the time it is
        // written, and its time went on that one's entry (review, 2026-10-30).
        let vault = self.vault_ref();
        let entry = vaults::current();
        let write = BackupWrite::start(PathBuf::from(folder), bytes, previous_ids);
        *self.backup.backup_write.borrow_mut() = Some(write.clone());
        // Any reason, not only a file Windows refused: a folder setting that isn't a usable path would otherwise stop
        // the copies without a word (the reason shows in Settings › Backup and goes to the log).
        match write.wait_async().await {
            Ok(()) => {
                let same = match (&vault, &*self.controller.vault.borrow()) {
                    (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                    (None, None) => true,
                    _ => false,
                };
                if same {
                    self.backup_done(hash, saves);
                } else if let Some(entry) = entry {
                    // Only the time: whether it holds the vault's last changes is for that vault's next unlock to find out.
                    {
                        let mut s = prefs::lock();
                        if let Some(e) = s.vault_list.iter_mut().find(|v| v.id == entry.id) {
                            e.last_backup = Some(Stamp::utc_now());
                        }
                    }
                    prefs::save();
                }
                true
            }
            Err(ex) => {
                self.backup_failed(&ex);
                false
            }
        }
    }

    pub async fn backup_now(self: &Rc<Self>) -> Res<Value> {
        self.vault(|_| ())?;
        if prefs::lock().backup_folder.is_none() {
            return Err(Fail::user(l::t("Choose a backup folder first.")));
        }
        if !self.try_backup_async(true).await {
            let error = self.backup.backup_error.borrow().clone().unwrap_or_default();
            return Err(Fail::user(l::f("Couldn't back up: {0}", &[&error])));
        }
        Ok(Value::Null)
    }

    /// The switch, or a suggested folder; a new folder gets a copy right away.
    pub fn set_backup(&self, a: &Value) -> Res<Value> {
        if let Some(enabled) = opt_bool(a, "enabled") {
            prefs::lock().backup_enabled = enabled;
        }
        if let Some(folder) = opt_str(a, "folder").filter(|f| !f.is_empty()) {
            self.use_backup_folder(&folder)?;
        }
        prefs::save();
        let last = prefs::lock().last_backup();
        if self.vault_ref().is_some() && backup_ready() && (self.backup.backup_dirty.get() || last.is_none()) {
            spawn_try_backup(true);
        }
        self.push_state();
        Ok(Value::Null)
    }

    pub fn use_backup_folder(&self, folder: &str) -> Res<()> {
        if !Path::new(folder).has_root() {
            return Err(Fail::user(l::t("Choose a folder on this PC or a synced drive.")));
        }
        // The live vault must not sit in the sync folder (the client locks files mid-save), so never the data folder.
        if backup::is_data_folder(Path::new(folder)) {
            return Err(Fail::user(l::t("Pick a folder outside Bloxkeep's own data folder.")));
        }
        let current = prefs::lock().backup_folder.clone();
        // The same folder picked again: still turns backups on (with them off, it stayed off before).
        if same_path(folder, current.as_deref()) {
            prefs::lock().backup_enabled = true;
            return Ok(());
        }
        {
            let mut s = prefs::lock();
            s.backup_folder = Some(full_path(folder));
            s.backup_enabled = true;
        }
        *self.backup.backup_error.borrow_mut() = None;
        *self.backup.backup_hash.borrow_mut() = None; // a new folder has no copy yet
        self.backup.backup_dirty.set(true);
        *self.backup.suggested_folders.borrow_mut() = None;
        Ok(())
    }

    pub async fn pick_backup_folder(&self) -> Res<Value> {
        let current = prefs::lock().backup_folder.clone();
        let initial = current.filter(|c| Path::new(c).is_dir()).unwrap_or_else(user_profile);
        let owner = self.controller.owner.get();
        let description = l::t("Choose a folder for backups");
        // On a worker thread, so the core keeps answering (the window's Quit, timers) while the dialog is open; `pick`
        // starts the single-threaded COM apartment the dialog needs and ends it, or uses a thread of its own when the
        // worker is already in a multithreaded one.
        let chosen = rt::blocking(move || ui::modal(|| folder_dialog::pick(owner, &description, &initial))).await;
        if let Some(chosen) = chosen {
            self.use_backup_folder(&chosen)?;
            prefs::save();
            if self.vault_ref().is_some() && backup_ready() {
                spawn_try_backup(true);
            }
        }
        self.push_state();
        Ok(Value::Null)
    }

    pub fn open_backup_folder() -> Res<Value> {
        let Some(folder) = prefs::lock().backup_folder.clone() else { return Ok(Value::Null) };
        // The folder may sit on a drive that isn't there right now (Google Drive not running).
        if let Err(ex) = std::fs::create_dir_all(&folder) {
            return Err(Fail::user(l::f("Couldn't open the backup folder: {0}", &[&error_text(&Fail::from(ex))])));
        }
        if std::process::Command::new("explorer.exe").arg(&folder).spawn().is_err() {
            return Err(Fail::user(l::f("Couldn't open the backup folder: {0}", &[&l::t("Windows couldn't open File Explorer.")])));
        }
        Ok(Value::Null)
    }

    /// Before a vault exists (first-run screen) the backup simply becomes the vault. Otherwise (Settings, or the
    /// lock screen after a forgotten password) the open one is locked and saved first, a copy of it kept beside the new
    /// one; the restored vault unlocks with its own master password.
    pub async fn restore_backup(&self) -> Res<Value> {
        if self.controller.launch_cts.borrow().is_some() {
            return Err(Fail::user(l::t("Wait until Bloxkeep finishes opening Roblox.")));
        }
        let folder = prefs::lock().backup_folder.clone();
        let initial = folder.filter(|f| Path::new(f).is_dir()).unwrap_or_else(user_profile);
        let owner = self.controller.owner.get();
        let title = l::t("Choose a Bloxkeep backup");
        let filter = l::t("Bloxkeep backups") + " (*.rmv)|*.rmv";
        // On a worker thread, as Choose folder's, so the core keeps answering (timers, the tray's Quit) while it is open.
        let Some(chosen) = rt::blocking(move || ui::modal(|| open_file_dialog(owner, &title, &filter, &initial))).await else {
            self.push_state();
            return Ok(Value::Null);
        };
        // Asked again: the core kept running while the dialog was open, so a Play may have started meanwhile.
        if self.controller.launch_cts.borrow().is_some() {
            return Err(Fail::user(l::t("Wait until Bloxkeep finishes opening Roblox.")));
        }
        let file = PathBuf::from(&chosen);
        let replace = self.vault_ref().is_some() || vault::exists();
        let run = || -> Res<()> {
            // Checked before locking, so a wrong file doesn't cost the session; whether it opens needs its password.
            if !vault::looks_like_vault(&file)? {
                log::note("Backup: restore refused, the chosen file isn't a Bloxkeep backup.");
                return Err(Fail::user(l::t("This file is not a Bloxkeep backup.")));
            }
            if self.vault_ref().is_some() {
                self.lock()?;
            }
            let before = backup::restore(&file, replace)?;
            *self.backup.restored_over.borrow_mut() = before.map(|b| (vault::active_file(), b.to_string_lossy().into_owned()));
            note_restored(&file);
            let foreign = prefs::lock().foreign_restore.is_some();
            log::note(&format!(
                "Backup: a backup was restored{}{}.",
                if foreign { " (made on another PC)" } else { "" },
                if replace { ", the vault it replaced copied beside it" } else { "" }
            ));
            Ok(())
        };
        if let Err(ex) = run() {
            if !is_io_error(&ex) {
                return Err(ex);
            }
            log::note(&format!("Backup: restore failed ({}).", App::log_kind_of(&ex)));
            return Err(Fail::user(l::f("Couldn't restore: {0}", &[&error_text(&ex)])));
        }
        self.backup.backup_dirty.set(false); // the vault now equals a backup
        prefs::lock().set_backup_pending(false);
        prefs::save();
        self.toast(&l::t("Backup restored. Unlock it with the master password it was saved with."), "ok");
        self.push_state();
        Ok(Value::Null)
    }

    /// At the first unlock of a vault made elsewhere: its program for opening Roblox and its Discord channels were
    /// chosen by whoever made it, so a shared "backup" could run their program at the next Play or send this PC's alerts
    /// about its accounts to their channel. Dropped before anything uses them; the person sets them again if they're theirs.
    pub fn drop_foreign_links(&self, vault: &Arc<Vault>) {
        let had = {
            let mut data = vault.data();
            let had = !data.settings.custom_launcher.is_empty()
                || !data.discord.webhook_url.is_empty()
                || data.settings.presets.iter().any(|p| !p.discord_webhook.is_empty());
            data.settings.custom_launcher = String::new();
            if data.settings.launch_via == "custom" {
                data.settings.launch_via = "auto".into();
            }
            data.discord.enabled = false;
            data.discord.webhook_url = String::new();
            for preset in &mut data.settings.presets {
                preset.discord_webhook = String::new();
            }
            had
        };
        self.persist_soon();
        if had {
            log::note("Backup: first unlock of a vault made on another PC; its launcher and Discord links were removed.");
        }
        if had {
            self.toast(
                &l::t("This backup was made on another PC: the program it opened Roblox with and its Discord links were removed. Set them again in Settings if they're yours."),
                "info",
            );
        }
    }

    /// The vault just unlocked (or given a new password) is this PC's from now on, with the ids it had before (its
    /// older backups carry them); not those of a vault from elsewhere, which wrote that list itself.
    pub fn remember_vault(&self, vault: &Arc<Vault>, foreign: bool) {
        let previous: Vec<String> = vault.data().settings.previous_vault_ids.clone();
        let changed = {
            let mut s = prefs::lock();
            let mut changed = s.known_vault_ids.insert(vault.id());
            if foreign {
                s.foreign_restore = None;
                changed = true;
            } else {
                for id in previous {
                    changed |= prefs::is_vault_id(Some(&id)) && s.known_vault_ids.insert(id);
                }
            }
            changed
        };
        if changed {
            prefs::save();
        }
    }
}

/// `_ = TryBackupAsync(push)`: a copy not waited for.
fn spawn_try_backup(push: bool) {
    rt::spawn(async move {
        if let Some(app) = this() {
            let _ = app.try_backup_async(push).await;
        }
    });
}

