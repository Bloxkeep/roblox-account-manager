//! AppController.Robux.cs: each account's Robux on its card: asked with its own cookie (Roblox VNG's service for a VNG account),
//! a few at a time, at unlock and then every half hour; an account not asked yet (just added, signed in again) at the next look.
//! Kept in memory only, never in the vault or a log. No answer (signed out, offline, refused) keeps the last number; until the
//! first, the card keeps the number's place ("…") so the row doesn't move.
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use super::App;
use crate::core::base::{Cancel, Guid, Stamp};
use crate::core::models::{Account, stamp_min};
use crate::core::roblox_api;
use crate::core::vault::Vault;
use crate::core::{log, rt};

pub const ROBUX_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// After a round Roblox cut short (too many requests) or that couldn't reach it: the next one comes sooner.
pub const ROBUX_RETRY: Duration = Duration::from_secs(2 * 60);
/// Accounts asked at once (each with its own cookie), each slot pausing a moment between two.
pub const ROBUX_AT_ONCE: usize = 4;
pub const ROBUX_SPACING: Duration = Duration::from_millis(250);

pub struct RobuxState {
    pub robux: RefCell<HashMap<Guid, i64>>,
    /// Accounts asked (and answered, or not) since this vault opened: the others are asked at the next look.
    pub robux_asked: RefCell<HashSet<Guid>>,
    /// The vault the numbers belong to: another one opened (a restored backup) starts over.
    pub robux_vault: RefCell<Option<Arc<Vault>>>,
    pub next_robux: Cell<Stamp>,
    pub robux_busy: Cell<bool>,
    /// The last round had errors: written to error.log once, then once more when a round goes through.
    pub robux_failing: Cell<bool>,
}

impl Default for RobuxState {
    fn default() -> RobuxState {
        RobuxState {
            robux: RefCell::default(),
            robux_asked: RefCell::default(),
            robux_vault: RefCell::default(),
            next_robux: Cell::new(stamp_min()),
            robux_busy: Cell::new(false),
            robux_failing: Cell::new(false),
        }
    }
}

/// One round's shared counts (the C# lambda's captured locals).
struct Round {
    cut_short: Cell<bool>,
    failures: Cell<i32>,
    first_kind: RefCell<Option<String>>,
    queue: RefCell<std::collections::VecDeque<Account>>,
}

impl App {
    pub fn has_robux(a: &Account) -> bool {
        a.user_id > 0 && !a.cookie.is_empty() && a.cookie_valid != Some(false) && a.banned != Some(true)
    }

    fn robux_vault_is(&self, vault: &Arc<Vault>) -> bool {
        matches!(&*self.robux.robux_vault.borrow(), Some(v) if Arc::ptr_eq(v, vault))
    }

    pub fn refresh_robux_if_due(self: &Rc<Self>, now: Stamp) {
        let Some(vault) = self.vault_ref() else { return };
        if !self.robux_vault_is(&vault) {
            *self.robux.robux_vault.borrow_mut() = Some(vault.clone());
            self.robux.robux.borrow_mut().clear();
            self.robux.robux_asked.borrow_mut().clear();
            self.robux.next_robux.set(now);
        }
        if self.robux.robux_busy.get() || !self.network_up() {
            return;
        }
        // Between rounds only the accounts not asked yet are copied (usually none): this runs on every 1.5 s tick,
        // and copying every account with its cookie each time was wasted work.
        let due = now >= self.robux.next_robux.get();
        let accounts: Vec<Account> = {
            let asked = self.robux.robux_asked.borrow();
            vault.data().accounts.iter().filter(|a| App::has_robux(a) && (due || !asked.contains(&a.id))).cloned().collect()
        };
        if due {
            self.robux.next_robux.set(now.plus_secs(ROBUX_INTERVAL.as_secs_f64()));
        } else if accounts.is_empty() {
            return;
        }
        // Busy from here, not from the task's first step: a second call in the same turn (unlock and the
        // process tick) would otherwise start a second round alongside. The task's own guard clears it.
        self.robux.robux_busy.set(true);
        let app = self.clone();
        rt::spawn(async move { app.refresh_robux(vault, accounts).await });
    }

    pub async fn refresh_robux(self: &Rc<Self>, vault: Arc<Vault>, accounts: Vec<Account>) {
        // finally: cleared however this ends.
        let _busy = crate::core::base::Busy::set(&self.robux.robux_busy);
        let count = accounts.len();
        let round = Round {
            cut_short: Cell::new(false),
            failures: Cell::new(0),
            first_kind: RefCell::new(None),
            queue: RefCell::new(accounts.iter().cloned().collect()),
        };
        // SemaphoreSlim(RobuxAtOnce): four slots, each taking the next account in turn.
        let (r, v) = (&round, &vault);
        let slot = || async move {
            loop {
                let next = r.queue.borrow_mut().pop_front();
                let Some(account) = next else { break };
                if r.cut_short.get() || !self.is_vault(v) || self.controller.disposed.get() {
                    continue;
                }
                let (cookie, realm, user_id) = (account.cookie.clone(), account.realm.clone(), account.user_id);
                match rt::blocking(move || roblox_api::get_robux(&cookie, &realm, user_id, &Cancel::new())).await {
                    Ok(Some(robux)) => {
                        if self.is_vault(v) {
                            self.robux.robux.borrow_mut().insert(account.id, robux);
                        }
                    }
                    Ok(None) => {}
                    Err(ex) if matches!(ex, crate::core::base::Fail::Roblox { .. }) && ex.rate_limited() => {
                        r.cut_short.set(true); // the rest wait for the next round, a little later
                        r.failures.set(r.failures.get() + 1);
                        r.first_kind.borrow_mut().get_or_insert_with(|| App::log_kind_of(&ex));
                    }
                    Err(ex) if roblox_api::is_expected_error(&ex) => {
                        if !self.network_up() {
                            r.cut_short.set(true);
                        }
                        r.failures.set(r.failures.get() + 1);
                        r.first_kind.borrow_mut().get_or_insert_with(|| App::log_kind_of(&ex));
                    }
                    Err(ex) => {
                        // Background work must never take the app down; the next round tries again.
                        log::write(&ex);
                    }
                }
                if self.is_vault(v) {
                    self.robux.robux_asked.borrow_mut().insert(account.id);
                    self.push_state_if_changed(); // its number, or its place given back
                }
                rt::sleep(ROBUX_SPACING).await;
            }
        };
        futures::future::join_all((0..ROBUX_AT_ONCE.min(count)).map(|_| slot())).await;
        let (failures, cut_short) = (round.failures.get(), round.cut_short.get());
        let first_kind = round.first_kind.borrow().clone().unwrap_or_default();
        if failures > 0 && !self.robux.robux_failing.get() && self.is_vault(&vault) {
            log::note(&format!(
                "Robux: {failures} of {count} accounts not read ({first_kind}){}.",
                if cut_short { "; the round was cut short, next try in 2 minutes" } else { "; their last number is kept" }
            ));
        } else if failures == 0 && self.robux.robux_failing.get() && self.is_vault(&vault) {
            log::note(&format!("Robux: recovered, {count} accounts read."));
        }
        if self.is_vault(&vault) {
            self.robux.robux_failing.set(failures > 0);
        }
        if cut_short {
            // Asked again soon, those not asked included (they show nothing meanwhile rather than waiting).
            {
                let mut asked = self.robux.robux_asked.borrow_mut();
                for account in &accounts {
                    asked.insert(account.id);
                }
            }
            let soon = Stamp::utc_now().plus_secs(ROBUX_RETRY.as_secs_f64());
            if soon < self.robux.next_robux.get() {
                self.robux.next_robux.set(soon);
            }
            self.push_state_if_changed();
        }
    }

    pub fn robux_of(&self, a: &Account) -> Option<i64> {
        self.robux.robux.borrow().get(&a.id).copied()
    }

    pub fn robux_waiting(&self, a: &Account) -> bool {
        let same = match (&*self.robux.robux_vault.borrow(), &*self.controller.vault.borrow()) {
            (Some(r), Some(v)) => Arc::ptr_eq(r, v),
            (None, None) => true,
            _ => false,
        };
        same && !self.robux.robux.borrow().contains_key(&a.id)
            && !self.robux.robux_asked.borrow().contains(&a.id)
            && App::has_robux(a)
            && self.power.network_up.get()
    }
}
