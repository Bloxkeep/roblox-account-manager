//! AppController.Unlock.cs: creating, unlocking and locking the vault, and changing its password.
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::App;
use super::accounts::BootProgress;
use super::controller::{PENDING_TIMEOUT, PendingLaunch, REJOIN_DELAY, RejoinRequest, WAITING_REJOIN_KEPT, error_text};
use super::launching::LATE_TIMEOUT;
use super::persist::is_io_error;
use super::state::STALE_PRESENCE;
use crate::core::base::{Fail, Res, Stamp};
use crate::core::models::{EndedSession, OpenSession, WaitingRejoin};
use crate::core::vault::{self, Vault};
use crate::core::{backup, l, log, multi_instance, prefs, roblox_launcher, roblox_processes, rt, ui, vaults};

/// Leeway between a launch's time and its client's start, for picking saved clients up again.
pub const ADOPT_SLACK: Duration = Duration::from_secs(2 * 60);

/// DateTime.FromFileTimeUtc.
fn from_file_time(created: i64) -> Stamp {
    Stamp::from_unix_ms((created - 116_444_736_000_000_000) / 10_000)
}

/// Environment.TickCount64.
fn tick_count64() -> i64 {
    // SAFETY: no arguments; reads the milliseconds since Windows started.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() as i64 }
}

/// Path.GetFileName.
fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

impl App {
    pub async fn create_vault(self: &Rc<Self>, password: &str, name: &str) -> Res<Value> {
        if super::accounts::len16(password) < 6 {
            return Err(Fail::user(l::t("The master password needs at least 6 characters.")));
        }
        // Six spaces pass the length check but are no password at all.
        if password.trim().is_empty() {
            return Err(Fail::user(l::t("The master password can't be only spaces.")));
        }
        // Which vault this is, taken before anything waits and before its file exists (the first vault's is vault.rmv only while
        // there is none): Back pressed during the create cleared the one being added, and the list then got another entry with
        // another file than the one written, which was left an orphan (review, 2026-10-30).
        let new_vault = self.vaults.new_vault.borrow().clone();
        let entry = new_vault.or_else(vaults::current).unwrap_or_else(vaults::new);
        // A vault being added: the one open locks only now, once the new one's password is fine.
        self.switch_to_new_vault()?;
        if vault::exists() {
            return Err(Fail::user(l::t("Bloxkeep is already set up.")));
        }
        let path = vault::vault_path();
        let p = password.to_string();
        let created = rt::blocking(move || Vault::create(&p, Some(path))).await?;
        // Another create or unlock finished first: that vault stays open, and the file is made its own again (this may have overwritten it).
        if self.vault_ref().is_some() {
            log::note("Create vault: another create or unlock finished first; that vault stays open.");
            created.dispose();
            self.persist();
            return Ok(self.build_state());
        }
        self.backup.weak_password.set(backup::weak_password(password));
        vaults::use_entry(Some(&entry)); // the one written, whatever Back did meanwhile
        self.join_vault_list(&entry, Some(name)); // before it opens: its backup state is its own from the start
        self.on_unlocked(Arc::new(created));
        Ok(self.build_state())
    }

    pub async fn unlock(self: &Rc<Self>, password: &str) -> Res<Value> {
        if let Some(open) = self.vault_ref() {
            return self.unlock_screen(open, password).await;
        }
        if password.is_empty() {
            return Err(Fail::user(l::t("Enter your master password.")));
        }
        let unlocking = vaults::current();
        let path = vault::vault_path(); // now, not when the worker starts: another vault may be picked meanwhile
        let (p, at) = (password.to_string(), path.clone());
        let vault = match rt::blocking(move || Vault::open(&p, Some(at))).await {
            Ok(v) => v,
            Err(ex) => {
                // The vault's own refusals are sentences of the app; a file Windows wouldn't read, or broken JSON, aren't.
                // PORT: Vault::open gives the vault's refusals (wrong password, damaged, broken JSON as "newer") as Fail::User and a
                // file it couldn't read as Fail::Other; JsonException's own log line and sentence are the refusal's here.
                let text = match &ex {
                    Fail::User(m) => m.clone(),
                    _ => {
                        log::note(&format!("Unlock: the data file couldn't be opened ({}).", App::log_kind_of(&ex)));
                        l::f("Couldn't open the data file: {0}", &[&error_text(&ex)])
                    }
                };
                // Just restored over another vault: the backup may be what's broken, and the vault it replaced is still there.
                let over = self.backup.restored_over.borrow().clone();
                let text = match over {
                    Some((file, copy)) if file.to_lowercase() == file_name(&path).to_lowercase() && Path::new(&copy).is_file() => {
                        format!("{text}\n{}", l::f("If this backup won't open, the vault it replaced is kept as {0}: restore that file to get it back.", &[&copy]))
                    }
                    _ => text,
                };
                return Err(Fail::user(text));
            }
        };
        let restored_here = self.backup.restored_over.borrow().as_ref().is_some_and(|(file, _)| file.to_lowercase() == file_name(&path).to_lowercase());
        if restored_here {
            *self.backup.restored_over.borrow_mut() = None; // it opened
        }
        // Another unlock finished first (two commands at once), or another vault was picked while this one's key was being
        // worked out: this copy is let go (opened, it would be shown as the other vault's).
        let picked_other = vaults::current().map(|v| v.id) != unlocking.map(|v| v.id);
        if self.vault_ref().is_some() || picked_other {
            log::note(&format!(
                "Unlock: let go, {}.",
                if self.vault_ref().is_some() { "another unlock finished first" } else { "another vault was picked meanwhile" }
            ));
            vault.dispose();
            return Ok(self.build_state());
        }
        self.backup.weak_password.set(backup::weak_password(password));
        if let Some(opened) = vaults::current() {
            let mut s = prefs::lock();
            if s.last_vault.as_deref() != Some(opened.id.as_str()) {
                s.last_vault = Some(opened.id.clone());
                drop(s);
                prefs::save();
            }
        }
        self.on_unlocked(Arc::new(vault));
        Ok(self.build_state())
    }

    pub fn on_unlocked(self: &Rc<Self>, vault: Arc<Vault>) {
        *self.controller.vault.borrow_mut() = Some(vault.clone());
        self.relaunch.update_at_lock.set(None); // in the app now: an update waits for a quiet moment
        // A backup made somewhere else: its launcher and Discord links go before anything below can use them.
        let foreign = prefs::lock().foreign_restore.as_deref() == Some(vault.id().as_str());
        if foreign {
            self.drop_foreign_links(&vault);
        }
        self.remember_vault(&vault, foreign);
        self.auto_lock.active_at.set(Stamp::utc_now()); // the wait for the lock screen starts now
        let multi = {
            let mut d = vault.data();
            let s = &mut d.settings;
            if !roblox_launcher::is_valid_option(&s.launch_via) {
                s.launch_via = if s.direct_launch { "direct".into() } else { "auto".into() };
            }
            s.multi_instance
        };
        *self.installs.installs.borrow_mut() = None; // re-read for this vault's launch option
        if multi && !multi_instance::armed() {
            multi_instance::enable();
        }
        self.controller.last_armed.set(multi_instance::armed());
        let count = vault.data().accounts.len();
        log::note(&format!(
            "Unlock: vault opened, {count} accounts{}.",
            if multi && !self.controller.last_armed.get() { "; Multi-Roblox is on but not armed" } else { "" }
        ));
        App::keep_multi_at_start(multi);
        // Read before AdoptOpenSessions: its save writes these lists from memory, still empty then.
        let (ended, waiting): (Vec<EndedSession>, Vec<WaitingRejoin>) = {
            let d = vault.data();
            (d.ended_sessions.clone(), d.waiting_rejoins.clone())
        };
        {
            let mut ended_here = self.state.ended_here.borrow_mut();
            for e in ended {
                if Stamp::utc_now().secs_since(e.at) < STALE_PRESENCE.as_secs_f64() {
                    ended_here.entry(e.account_id).or_insert((e.job_id, e.place_id, e.at));
                }
            }
        }
        self.adopt_open_sessions(&vault);
        self.restore_waiting_rejoins(&vault, &waiting);
        if !waiting.is_empty() {
            self.persist_soon(); // AdoptOpenSessions may have saved the vault without them
        }

        self.resume_play_sessions();
        self.resume_schedules(&vault);
        let (any, user_ids): (bool, Vec<i64>) = {
            let d = vault.data();
            (d.accounts.iter().any(|a| a.user_id > 0), d.accounts.iter().map(|a| a.user_id).collect())
        };
        self.controller.boot_status_done.set(!any);
        self.controller.boot_avatars_done.set(!any);
        *self.controller.boot.borrow_mut() = Some(BootProgress::new("status", 0.3));
        self.advance_boot(false);
        // Now rather than at the next check. A refresh still running from before the lock drops its answer; the next check redoes it.
        if self.controller.presence_busy.get() {
            self.controller.next_presence.set(Stamp::utc_now());
        } else {
            let app = self.clone();
            rt::spawn(async move {
                let _ = app.refresh_presence().await;
            });
        }
        self.spawn_load_avatars(user_ids);
        {
            let app = self.clone();
            rt::spawn(async move {
                let _ = app.load_recent_games().await;
            });
        }
        self.refresh_robux_if_due(Stamp::utc_now()); // alongside those, not after them
        self.refresh_screen_time_if_due(Stamp::utc_now()); // hours played, kept day by day (screen_time.rs)
        self.on_vault_unlocked();
        self.add_locked_activity(); // what happened while locked (a window closed) is in the log at once, also if Bloxkeep quits next
    }

    /// Remembers the open clients, waiting rejoins and those that ended lately (`ended_here`), so after
    /// closing or restarting Bloxkeep (an update) or a lock, the next unlock picks them up again, auto-rejoin included.
    /// Returns true when the lists changed, i.e. the vault needs saving.
    pub fn note_open_sessions(&self) -> bool {
        let Some(vault) = self.vault_ref() else { return false };
        let now = Stamp::utc_now();
        let ended_here = self.state.ended_here.borrow().clone();
        let waiting: Vec<WaitingRejoin> = self
            .controller
            .rejoin_queue
            .borrow()
            .iter()
            .map(|r| WaitingRejoin { account_id: r.account_id, target: Some(r.target.clone()), due: r.due, preset_id: r.preset_id, restart: r.restart })
            .collect();
        let running: Vec<OpenSession> = self
            .controller
            .running
            .borrow()
            .iter()
            .map(|(id, (pid, launch))| OpenSession {
                account_id: *id,
                pid: *pid,
                started_at: launch.started_at.get(),
                target: launch.target.clone(),
                preset_id: launch.preset_id,
                fps_limit: launch.fps_limit,
                home_screen: launch.home_screen,
            })
            .collect();
        let mut d = vault.data();
        let mut ended: Vec<EndedSession> = ended_here
            .into_iter()
            .filter(|(id, (_, _, at))| now.secs_since(*at) < STALE_PRESENCE.as_secs_f64() && d.accounts.iter().any(|a| a.id == *id))
            .map(|(id, (job_id, place_id, at))| EndedSession { account_id: id, job_id, place_id, at })
            .collect();
        ended.sort_by_key(|e| e.at);
        let mut ended_changed = !d.ended_sessions.iter().map(|e| (e.account_id, e.at)).eq(ended.iter().map(|e| (e.account_id, e.at)));
        if ended_changed {
            d.ended_sessions = ended;
        }
        if !d.waiting_rejoins.iter().map(|w| (w.account_id, w.due)).eq(waiting.iter().map(|w| (w.account_id, w.due))) {
            d.waiting_rejoins = waiting;
            ended_changed = true;
        }
        // Only this vault's: another vault's windows stay tracked after a switch (they can't be adopted here), but their
        // account ids, process ids and servers aren't written into this vault (audit R18).
        let sessions: Vec<OpenSession> = running.into_iter().filter(|s| d.accounts.iter().any(|a| a.id == s.account_id)).collect();
        let noted = &d.open_sessions;
        if noted.len() == sessions.len() && noted.iter().zip(&sessions).all(|(a, b)| a.account_id == b.account_id && a.pid == b.pid) {
            return ended_changed;
        }
        d.open_sessions = sessions;
        true
    }

    /// Rejoins waiting when Bloxkeep restarted or locked: back on at their time (overdue ones shortly), unless the account
    /// is gone or open again. Not ones from long ago: Bloxkeep opened the next day must not start Roblox by itself.
    pub fn restore_waiting_rejoins(&self, vault: &Arc<Vault>, waiting: &[WaitingRejoin]) {
        let before = self.controller.rejoin_queue.borrow().len();
        for w in waiting {
            let Some(target) = &w.target else { continue };
            let now = Stamp::utc_now();
            let known = vault.data().accounts.iter().any(|a| a.id == w.account_id);
            let open = self.controller.running.borrow().contains_key(&w.account_id);
            let queued = self.controller.rejoin_queue.borrow().iter().any(|r| r.account_id == w.account_id);
            if now.secs_since(w.due) < WAITING_REJOIN_KEPT.as_secs_f64() && known && !open && !queued {
                let due = if w.due > now { w.due } else { now.plus_secs(REJOIN_DELAY.as_secs_f64()) };
                let mut request = RejoinRequest::new(w.account_id, target.clone(), due);
                request.preset_id = w.preset_id;
                request.restart = w.restart;
                self.controller.rejoin_queue.borrow_mut().push(request);
            }
        }
        if !waiting.is_empty() {
            let put_back = self.controller.rejoin_queue.borrow().len() - before;
            log::note(&format!("Unlock: {put_back} of {} waiting rejoin(s) put back (the rest too old, gone or open).", waiting.len()));
        }
    }

    pub fn adopt_open_sessions(&self, vault: &Arc<Vault>) {
        let saved: Vec<OpenSession> = vault.data().open_sessions.clone();
        if saved.is_empty() {
            return;
        }
        let started: HashMap<i32, i64> = roblox_processes::get_players().into_iter().map(|p| (p.pid, p.created)).collect();
        // After a restart of the PC the same process ids belong to other programs (also after a shutdown with Windows'
        // Fast Startup, which doesn't restart the uptime): the client must have started around its launch.
        let booted = Stamp::utc_now().plus_secs(-(tick_count64() as f64) / 1000.0);
        let mut adopted = 0;
        let same_client = |session: &OpenSession| -> bool {
            let Some(&created) = started.get(&session.pid) else { return false };
            if session.started_at < booted {
                return false;
            }
            if created == 0 {
                return true;
            }
            let at = from_file_time(created);
            let latest = (PENDING_TIMEOUT + LATE_TIMEOUT + ADOPT_SLACK).as_secs_f64();
            at >= session.started_at.plus_secs(-ADOPT_SLACK.as_secs_f64()) && at <= session.started_at.plus_secs(latest)
        };
        for session in &saved {
            let known = vault.data().accounts.iter().any(|a| a.id == session.account_id);
            let taken = {
                let running = self.controller.running.borrow();
                running.contains_key(&session.account_id) || running.values().any(|r| r.0 == session.pid)
            };
            if same_client(session) && known && !taken {
                let mut launch = PendingLaunch::new(session.account_id, Default::default(), session.started_at, session.target.clone(), false, session.preset_id);
                launch.fps_limit = session.fps_limit;
                launch.home_screen = session.home_screen;
                self.controller.running.borrow_mut().insert(session.account_id, (session.pid, Rc::new(launch)));
                // A light-AFK window was minimised once already, before Bloxkeep closed: one the person brought back up stays up.
                self.presets.afk_minimised.borrow_mut().insert(session.pid);
                adopted += 1;
            }
        }
        log::note(&format!("Unlock: {adopted} of {} Roblox window(s) open before were picked up again.", saved.len()));
        vault.data().open_sessions.clear();
        self.persist();
    }

    pub fn lock(&self) -> Res<Value> {
        // Edits that can't be written yet (the file held by an antivirus) would be lost with the vault.
        if !self.save_before_closing() {
            log::note("Lock: refused, the changes couldn't be saved; Bloxkeep stays unlocked.");
            return Err(Fail::user(l::t("Couldn't save your changes, so Bloxkeep stays unlocked. Try again in a moment.")));
        }
        crate::core::remote_watch::stop();
        if let Some(cts) = self.controller.launch_cts.borrow().as_ref() {
            cts.cancel();
        }
        if let Some(cts) = self.controller.rejoin_cts.borrow().as_ref() {
            cts.cancel();
        }
        self.quick_login_cancel();
        self.login_cancel();
        let _ = self.drop_vault_import(false); // another vault's accounts, opened for an import, go with this one
        self.close_websites(None); // signed in as saved accounts
        ui::clear_secret_clipboard();
        // A second lock (already locked) would start the missed-schedule bookkeeping over and lose what was noted.
        if self.vault_ref().is_some() {
            self.remember_schedules_for_lock();
        }
        let _ = self.final_discord_board(false);
        self.stop_macros_for_lock(); // what plays and records stops with the vault
        self.close_panels_for_lock();
        // Nothing looks after the windows while locked (a window opened from the taskbar stayed limited, at low priority).
        self.lift_cpu_limits();
        self.lift_ram_limits();
        self.memory.auto_ram.borrow_mut().clear();
        self.give_memory_priority_back();
        let old = self.controller.vault.borrow_mut().take();
        if let Some(old) = old {
            old.dispose();
        }
        // The idle lock screen closes for real only now: a lock refused above leaves it up.
        self.auto_lock.screen_locked.set(false);
        // Nothing of the vault stays reachable while locked: dispose only wipes the key, so the decrypted accounts,
        // cookies and passwords go only once no field holds the vault (or what was made from it) any more.
        self.discord.vault.set(None);
        // The Robux round keeps the vault it counted for (and the balances): both go with the lock.
        *self.robux.robux_vault.borrow_mut() = None;
        self.robux.robux.borrow_mut().clear();
        self.robux.robux_asked.borrow_mut().clear();
        self.games.friend_ids.borrow_mut().clear();
        self.games.friend_servers.borrow_mut().clear();
        self.games.private_servers.borrow_mut().clear(); // their access codes are the vault's accounts'
        *self.games.owned_servers.borrow_mut() = None; // the vault's accounts' own
        *self.game_picker.favorite_games.borrow_mut() = None;
        *self.game_picker.popular_games.borrow_mut() = None;
        self.join_player.players.borrow_mut().clear();
        self.controller.avatar_urls.borrow_mut().clear();
        self.state.ended_here.borrow_mut().clear();
        self.state.ended_recent.borrow_mut().clear();
        self.controller.presence.borrow_mut().clear();
        self.controller.presence_reliable.borrow_mut().clear();
        self.controller.presence_loaded.set(false);
        *self.controller.boot.borrow_mut() = None;
        self.controller.rejoin_queue.borrow_mut().clear();
        self.close_desk.stop_moving_all(); // their reopening was in that queue
        self.close_desk.held_closed.borrow_mut().clear(); // no launch of the vault's is under way any more
        // Close, update, reopen stops waiting: the windows it asked to close still close (their records stay), and with no
        // vault their reopen finds no account, as a locked rejoin does. Roblox's own updater, once started, runs on.
        if self.roblox_updates.update_reopen_since.take().is_some() {
            self.roblox_updates.update_reopen_updating.set(false);
            log::note("Close, update, reopen: called off, Bloxkeep was locked; its windows aren't reopened.");
        }
        self.controller.offline_strikes.borrow_mut().clear();
        self.reset_discord_queue(); // nothing waits to go to Discord while locked
        *self.controller.last_metrics.borrow_mut() = None;
        *self.controller.last_state.borrow_mut() = None;
        *self.controller.page_view.borrow_mut() = None;
        // The monitor tab would show every client as a nameless "Roblox" over the lock screen, and the mini window's counts go stale.
        let set_area = self.monitor.set_monitor_area.borrow().clone();
        if let Some(set_area) = set_area {
            set_area(None);
        }
        self.mini.client_ram.borrow_mut().clear();
        self.push_state();
        Ok(Value::Null)
    }

    pub async fn change_password(self: &Rc<Self>, current: &str, next: &str) -> Res<Value> {
        let vault = self.require_vault_ref()?;
        if super::accounts::len16(next) < 6 {
            return Err(Fail::user(l::t("The new password needs at least 6 characters.")));
        }
        if next.trim().is_empty() {
            return Err(Fail::user(l::t("The master password can't be only spaces.")));
        }
        let (v, c) = (vault.clone(), current.to_string());
        if !rt::blocking(move || v.verify_password(&c)).await {
            return Err(Fail::user(l::t("The current master password is incorrect.")));
        }
        if next == current {
            return Err(Fail::user(l::t("The new password is the same as the current one.")));
        }
        let n = next.to_string();
        let prepared = rt::blocking(move || Vault::prepare_key(&n)).await;
        if !self.is_vault(&vault) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        // Old backups carry the old id (from the salt, which changes with the password): remembered so they stay this vault's to prune.
        let id = vault.id();
        {
            let mut d = vault.data();
            let previous = &mut d.settings.previous_vault_ids;
            if !previous.contains(&id) {
                previous.push(id);
            }
        }
        // Written here on the core's thread, where the accounts are changed, so nothing changes mid-save.
        if let Err(ex) = vault.change_password(prepared) {
            if is_io_error(&ex) {
                log::note(&format!("Change password: the vault couldn't be written ({}).", App::log_kind_of(&ex)));
                return Err(Fail::user(l::f("Couldn't save data: {0}", &[&error_text(&ex)])));
            }
            return Err(ex);
        }
        self.on_password_changed(next);
        self.remember_vault(&vault, false); // its new id: backups from now on carry it
        Ok(json!(true))
    }
}
