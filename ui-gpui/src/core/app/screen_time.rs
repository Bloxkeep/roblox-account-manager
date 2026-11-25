// Hours played, as Roblox counts them: Roblox's Screen time (its Settings page, Privacy) tells an account the minutes it
// played on each of the last 7 days and nothing older, so Bloxkeep reads it for each signed-in account every few hours
// and keeps each day as it comes (in the vault: VaultData::screen_time). The days add up to the hours on the profile's
// badge (pages/medals.rs), counted from the first day kept.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::App;
use crate::core::base::{Cancel, Stamp};
use crate::core::roblox_api::{self, realm};
use crate::core::vault::Vault;
use crate::core::{log, rt};

/// How often each account's days are read again (today's minutes grow meanwhile; the days before stay as they are).
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Again this soon after Roblox said to slow down.
const RETRY: Duration = Duration::from_secs(10 * 60);
/// Between two accounts.
const SPACING: Duration = Duration::from_millis(1500);

thread_local! {
    /// The vault read for, and when it is read again.
    static DUE: RefCell<Option<(Arc<Vault>, Stamp)>> = const { RefCell::new(None) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

/// The local day `days_ago` days before `now`, as the vault keeps it ("2026-11-25").
fn day_key(now: Stamp, days_ago: i64) -> String {
    let d = now.plus_secs(-(days_ago as f64) * 86_400.).local();
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

impl App {
    pub fn refresh_screen_time_if_due(self: &Rc<Self>, now: Stamp) {
        let Some(vault) = self.vault_ref() else { return };
        if BUSY.get() || !self.network_up() {
            return;
        }
        let due = DUE.with_borrow(|d| match d {
            Some((v, at)) if Arc::ptr_eq(v, &vault) => now >= *at,
            _ => true,
        });
        if !due {
            return;
        }
        DUE.set(Some((vault.clone(), now.plus_secs(EVERY.as_secs_f64()))));
        // Roblox VNG's accounts have no Screen time on roblox.com's service.
        let accounts: Vec<(String, i64)> = vault
            .data()
            .accounts
            .iter()
            .filter(|a| App::has_robux(a) && realm::normalize(Some(&a.realm)) == realm::GLOBAL)
            .map(|a| (a.cookie.clone(), a.user_id))
            .collect();
        if accounts.is_empty() {
            return;
        }
        BUSY.set(true);
        let app = self.clone();
        rt::spawn(async move {
            let _busy = scopeguard(|| BUSY.set(false));
            let mut failed: Option<String> = None;
            for (cookie, user_id) in accounts {
                if !app.is_vault(&vault) || app.controller.disposed.get() {
                    return;
                }
                match rt::blocking(move || roblox_api::get_daily_screen_time(&cookie, user_id, &Cancel::new())).await {
                    Ok(Some(days)) if app.is_vault(&vault) => app.keep_screen_time(user_id, &days, Stamp::utc_now()),
                    Ok(_) => {}
                    Err(ex) => {
                        failed.get_or_insert_with(|| App::log_kind_of(&ex));
                        if ex.rate_limited() {
                            DUE.set(Some((vault.clone(), Stamp::utc_now().plus_secs(RETRY.as_secs_f64()))));
                            break;
                        }
                    }
                }
                rt::sleep(SPACING).await;
            }
            if let Some(kind) = failed {
                log::note(&format!("Screen time: not every account read ({kind}); the days kept stay."));
            }
        });
    }

    /// Keeps the days Roblox told of `user_id` (days ago, minutes): a day already kept keeps the larger count (today's
    /// grows through the day).
    fn keep_screen_time(&self, user_id: i64, days: &[(i64, i64)], now: Stamp) {
        let changed = self
            .vault_mut(|d| {
                let kept = d.screen_time.entry(user_id).or_default();
                let mut changed = false;
                for &(ago, minutes) in days {
                    let slot = kept.entry(day_key(now, ago)).or_insert(0);
                    if minutes > *slot {
                        *slot = minutes;
                        changed = true;
                    }
                }
                changed
            })
            .unwrap_or(false);
        if changed {
            self.persist_soon();
        }
    }

    /// The minutes kept for `user_id` and the first day kept ("" for none): {minutes, since}.
    pub fn screen_time_of(&self, user_id: i64) -> Value {
        let (minutes, since) = self
            .vault(|d| d.screen_time.get(&user_id).map(|days| (days.values().sum::<i64>(), days.keys().next().cloned().unwrap_or_default())))
            .ok()
            .flatten()
            .unwrap_or_default();
        json!({ "minutes": minutes, "since": since })
    }
}

/// Runs `f` when dropped (the round's busy mark is cleared however the round ends).
fn scopeguard(f: impl FnOnce()) -> impl Drop {
    struct Guard<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for Guard<F> {
        fn drop(&mut self) {
            if let Some(f) = self.0.take() {
                f();
            }
        }
    }
    Guard(Some(f))
}
