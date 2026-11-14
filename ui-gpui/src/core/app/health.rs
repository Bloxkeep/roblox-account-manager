//! AppController.Health.cs: the daily sign-in check. Roblox ends saved sessions silently (password change, "log out everywhere",
//! security sweep) and the person finds out only when Play fails. So once a day while unlocked, every sign-in is verified quietly
//! with the account menu's read-only call, one at a time with a pause, and one notification names those that stopped working.
//! The same run reads each public profile for a ban.
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::App;
use super::names::Shown;
use crate::core::base::{Cancel, Fail, Guid, Res, Stamp};
use crate::core::models::{Account, stamp_min};
use crate::core::roblox_api::{self, realm};
use crate::core::vault::Vault;
use crate::core::{l, log, prefs, rt};

/// A day less a few hours: a check made at 9 am is due again at 9 am tomorrow, not at noon.
pub const SIGN_IN_CHECK_INTERVAL: Duration = Duration::from_secs(20 * 60 * 60);
/// Right after unlocking, status and avatars are being loaded; the quiet check waits its turn.
pub const SIGN_IN_CHECK_START_DELAY: Duration = Duration::from_secs(45);
pub const SIGN_IN_CHECK_PAUSE: Duration = Duration::from_secs(2);
/// After a run that couldn't finish (offline, rate-limited for good), not again before this.
pub const SIGN_IN_CHECK_RETRY: Duration = Duration::from_secs(60 * 60);
pub const SIGN_IN_CHECK_NAMED: usize = 3;
/// A longer name is cut in the notification: a balloon holds about 250 characters in all.
pub const SIGN_IN_CHECK_NAME_LENGTH: usize = 30;

pub struct HealthState {
    pub sign_in_check_running: Cell<bool>,
    pub sign_in_check_retry_at: Cell<Stamp>,
    /// Accounts the last run couldn't reach: a run that fails on the same ones again still counts as done.
    pub sign_in_check_failed: RefCell<HashSet<Guid>>,
    /// The last run looked like an outage of Roblox's sign-in service; the next one trusts its rejections.
    pub sign_in_check_outage: Cell<bool>,
}

impl Default for HealthState {
    fn default() -> HealthState {
        HealthState {
            sign_in_check_running: Cell::new(false),
            sign_in_check_retry_at: Cell::new(stamp_min()),
            sign_in_check_failed: RefCell::default(),
            sign_in_check_outage: Cell::new(false),
        }
    }
}

/// A name cut to `SIGN_IN_CHECK_NAME_LENGTH` (UTF-16 units, as C#'s `n[..29] + "…"`).
fn cut_name(n: String) -> String {
    let units: Vec<u16> = n.encode_utf16().collect();
    if units.len() > SIGN_IN_CHECK_NAME_LENGTH { String::from_utf16_lossy(&units[..SIGN_IN_CHECK_NAME_LENGTH - 1]) + "…" } else { n }
}

fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

impl App {
    pub fn set_daily_sign_in_check(&self, enabled: bool) -> Value {
        prefs::lock().daily_sign_in_check = enabled;
        prefs::save();
        self.push_state();
        Value::Null
    }

    /// From the process timer: starts the day's check once it is due and nothing else is checking.
    pub fn run_sign_in_check_if_due(self: &Rc<Self>, now: Stamp) {
        let (daily, last) = {
            let p = prefs::lock();
            (p.daily_sign_in_check, p.last_sign_in_check)
        };
        let since_last = last.map(|l| now.secs_since(l)).unwrap_or(f64::MAX);
        if !daily
            || self.vault_ref().is_none()
            || self.health.sign_in_check_running.get()
            || self.controller.checking.get()
            || self.controller.boot.borrow().is_some()
            || !self.network_up()
            || now < self.health.sign_in_check_retry_at.get()
            || since_last < secs(SIGN_IN_CHECK_INTERVAL)
        {
            return;
        }
        let app = self.clone();
        rt::spawn(async move { app.daily_sign_in_check().await });
    }

    pub async fn daily_sign_in_check(self: &Rc<Self>) {
        let _running = crate::core::base::Busy::set(&self.health.sign_in_check_running);
        // Background work must never take the app down; the next due time tries again.
        if let Err(ex) = self.quiet_sign_in_check().await {
            log::write(&ex);
            self.health.sign_in_check_retry_at.set(Stamp::utc_now().plus_secs(secs(SIGN_IN_CHECK_RETRY)));
        }
    }

    /// The vault is still open, the person hasn't started their own check or turned this off, and there's a connection.
    pub fn sign_in_check_can_go_on(&self, vault: &Arc<Vault>) -> bool {
        self.is_vault(vault) && !self.controller.disposed.get() && !self.controller.checking.get() && self.network_up() && prefs::lock().daily_sign_in_check
    }

    pub async fn quiet_sign_in_check(self: &Rc<Self>) -> Res<()> {
        let Some(vault) = self.vault_ref() else { return Err(Fail::other("NullReferenceException: no vault for the sign-in check")) };
        rt::sleep(SIGN_IN_CHECK_START_DELAY).await;
        let find = |id: Guid| -> Option<Account> { vault.data().accounts.iter().find(|a| a.id == id).cloned() };
        // Skips accounts known signed out (already told) and ones checked today by hand, so a run cut short by a lock resumes.
        let due: Vec<Guid> = vault
            .data()
            .accounts
            .iter()
            .filter(|a| {
                a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.last_checked.is_none_or(|at| Stamp::now().secs_since(at) >= secs(SIGN_IN_CHECK_INTERVAL))
            })
            .map(|a| a.id)
            .collect();
        let mut ok = 0;
        let (mut changed, mut stopped, mut rate_limited) = (false, false, false);
        let mut rejected: Vec<Guid> = Vec::new();
        let mut failed: Vec<Guid> = Vec::new();
        for &id in &due {
            if !self.sign_in_check_can_go_on(&vault) {
                stopped = true;
                break;
            }
            if find(id).is_none() {
                continue; // deleted meanwhile: neither asked about nor named
            }
            let mut attempt = 1;
            loop {
                // Read again each try, as the C#'s account.Cookie: after a rate limit's pause the account may have been
                // signed in again, and the old cookie's answer would only be thrown away.
                let Some(account) = find(id) else { break };
                let cookie = account.cookie.clone();
                let (c, r) = (cookie.clone(), account.realm.clone());
                match rt::blocking(move || roblox_api::get_authenticated_user(&c, &r, &Cancel::new())).await {
                    Ok(user) => {
                        if !self.sign_in_check_can_go_on(&vault) {
                            stopped = true;
                            break;
                        }
                        let mut d = vault.data();
                        let Some(a) = d.accounts.iter_mut().find(|a| a.id == id) else { break };
                        // Signed in again meanwhile: the old cookie's answer says nothing about the new one.
                        if a.cookie != cookie {
                            break;
                        }
                        // A rejection is confirmed below, with a second call, before the account is marked.
                        let Some(user) = user else {
                            rejected.push(id);
                            break;
                        };
                        ok += 1;
                        App::sign_in_accepted(a);
                        a.user_id = user.id;
                        a.username = user.name;
                        a.display_name = user.display_name;
                        a.last_checked = Some(Stamp::now());
                        changed = true;
                    }
                    Err(ex) if matches!(ex, Fail::Roblox { .. }) && ex.rate_limited() => {
                        // Rate-limited: a long pause, then the same account; still refused, the whole run waits an hour.
                        if attempt >= 3 {
                            rate_limited = true;
                            break;
                        }
                        rt::sleep(Duration::from_secs(20 * attempt as u64)).await;
                        if self.sign_in_check_can_go_on(&vault) {
                            attempt += 1;
                            continue;
                        }
                        stopped = true;
                        break;
                    }
                    Err(ex) if roblox_api::is_expected_error(&ex) => failed.push(id),
                    Err(ex) => return Err(ex),
                }
                break;
            }
            // Three rejections before any success: Roblox's sign-in service is most likely down (401 to everyone).
            if stopped || rate_limited || (!self.health.sign_in_check_outage.get() && ok == 0 && rejected.len() >= 3) {
                break;
            }
            rt::sleep(SIGN_IN_CHECK_PAUSE).await;
        }

        // Many rejections and few successes is an outage, not a mass sign-out: nothing is marked and the run repeats in an hour,
        // then trusting what it hears so a real mass sign-out is still reported. One or two rejections with none accepted say
        // nothing either way (the confirming call would meet the same outage), so they wait for that run too.
        let outage = !self.health.sign_in_check_outage.get() && !rejected.is_empty() && (ok == 0 || rejected.len() >= 3 && rejected.len() > ok);
        let mut signed_out: Vec<Guid> = Vec::new();
        if !stopped && !outage {
            for &id in &rejected {
                rt::sleep(SIGN_IN_CHECK_PAUSE).await;
                if !self.sign_in_check_can_go_on(&vault) {
                    stopped = true;
                    break;
                }
                if find(id).is_none() {
                    continue;
                }
                if !self.confirm_signed_out(id, &Cancel::new(), true).await {
                    continue; // accepted after all, or no clear answer: asked again next time
                }
                signed_out.push(id);
                if let Some(a) = vault.data().accounts.iter_mut().find(|a| a.id == id) {
                    a.last_checked = Some(Stamp::now());
                }
                changed = true;
            }
        }

        if !stopped && !outage && !rate_limited {
            changed |= self.recheck_signed_out(&vault, &signed_out).await?;
        }

        // Bans, from each account's public profile (no cookie): the ones checked above, and the signed-out and banned
        // ones too, so a ban that ends is noticed as well. Roblox asking to slow down ends this part until the next run.
        let mut banned: Vec<Guid> = Vec::new();
        if !stopped && !rate_limited {
            let to_check: Vec<Account> = vault
                .data()
                .accounts
                .iter()
                .filter(|a| a.user_id > 0 && (due.contains(&a.id) || a.cookie_valid == Some(false) || a.banned == Some(true)))
                .cloned()
                .collect();
            // roblox.com's user list leaves banned users out: one request per 100 finds the unbanned. Only those it leaves out and
            // ones banned before are asked one by one with the pause (Roblox VNG's are on another host: one by one).
            let mut listed: HashSet<i64> = HashSet::new();
            let unbanned: Vec<i64> = to_check.iter().filter(|a| a.realm == realm::GLOBAL && a.banned != Some(true)).map(|a| a.user_id).collect();
            if unbanned.len() > 1 {
                match rt::blocking(move || roblox_api::get_users(unbanned, &Cancel::new(), false)).await {
                    Ok(users) => listed = users.keys().copied().collect(),
                    // No list: each is asked about on its own, as before.
                    Err(ex) if roblox_api::is_expected_error(&ex) => {}
                    Err(ex) => return Err(ex),
                }
            }
            for account in &to_check {
                if account.realm == realm::GLOBAL && account.banned != Some(true) && listed.contains(&account.user_id) {
                    if let Some(a) = vault.data().accounts.iter_mut().find(|a| a.id == account.id) {
                        a.banned = Some(false); // as check_ban has it for one that isn't banned
                        a.banned_since = None;
                    }
                    continue;
                }
                if !self.sign_in_check_can_go_on(&vault) {
                    stopped = true;
                    break;
                }
                let current = find(account.id).unwrap_or_else(|| account.clone());
                match self.check_ban(&current).await {
                    Ok(change) => {
                        if change == Some(true) && find(account.id).is_some() {
                            banned.push(account.id);
                        }
                        changed |= change.is_some();
                    }
                    Err(ex) if matches!(ex, Fail::Roblox { .. }) && ex.rate_limited() => break,
                    // No clear answer: asked again next time.
                    Err(ex) if roblox_api::is_expected_error(&ex) => {}
                    Err(ex) => return Err(ex),
                }
                rt::sleep(SIGN_IN_CHECK_PAUSE).await;
            }
        }

        if changed && self.is_vault(&vault) {
            self.persist();
            self.push_state_if_changed();
        }
        if !signed_out.is_empty() && self.is_vault(&vault) {
            // (Accounts deleted during the check's pauses drop out: no "0 accounts…" when none is left.)
            let accounts: Vec<Account> = signed_out.iter().filter_map(|id| find(*id)).collect();
            if !accounts.is_empty() {
                let text = self.signed_out_text(&accounts);
                self.notify(&text, "warn", "signin", Guid::nil(), true, None);
            }
        }
        if !banned.is_empty() && self.is_vault(&vault) {
            let accounts: Vec<Account> = banned.iter().filter_map(|id| find(*id)).collect();
            if !accounts.is_empty() {
                self.notify_banned(accounts);
            }
        }
        if stopped {
            return Ok(());
        }
        if outage || rate_limited {
            self.health.sign_in_check_outage.set(outage);
            self.health.sign_in_check_retry_at.set(Stamp::utc_now().plus_secs(secs(SIGN_IN_CHECK_RETRY)));
            return Ok(());
        }
        self.health.sign_in_check_outage.set(false);
        // Unreachable accounts retry in an hour, unless the same as last time (a VNG service unreachable from here: hourly forever).
        let same_failures = !failed.is_empty() && failed.iter().all(|id| self.health.sign_in_check_failed.borrow().contains(id));
        {
            let mut last = self.health.sign_in_check_failed.borrow_mut();
            last.clear();
            last.extend(failed.iter().copied());
        }
        if !failed.is_empty() && !same_failures {
            self.health.sign_in_check_retry_at.set(Stamp::utc_now().plus_secs(secs(SIGN_IN_CHECK_RETRY)));
            return Ok(());
        }
        prefs::lock().last_sign_in_check = Some(Stamp::utc_now());
        prefs::save();
        Ok(())
    }

    /// Accounts marked signed out, asked again quietly. One Roblox accepts now (marked in an outage, or its session came
    /// back) is signed in again, auto-rejoin back on. Those this run marked (`marked_now`) are skipped.
    pub async fn recheck_signed_out(self: &Rc<Self>, vault: &Arc<Vault>, marked_now: &[Guid]) -> Res<bool> {
        let mut changed = false;
        let list: Vec<(Guid, String, String)> = vault
            .data()
            .accounts
            .iter()
            .filter(|a| a.cookie_valid == Some(false) && !a.cookie.is_empty() && !marked_now.contains(&a.id))
            .map(|a| (a.id, a.cookie.clone(), a.realm.clone()))
            .collect();
        for (id, cookie, account_realm) in list {
            if !self.sign_in_check_can_go_on(vault) {
                break;
            }
            let c = cookie.clone();
            match rt::blocking(move || roblox_api::get_authenticated_user(&c, &account_realm, &Cancel::new())).await {
                Ok(Some(user)) => {
                    let can_go_on = self.sign_in_check_can_go_on(vault);
                    let mut d = vault.data();
                    if let Some(a) = d.accounts.iter_mut().find(|a| a.id == id)
                        && a.cookie == cookie
                        && can_go_on
                    {
                        App::sign_in_accepted(a);
                        a.user_id = user.id;
                        a.username = user.name;
                        a.display_name = user.display_name;
                        a.last_checked = Some(Stamp::now());
                        changed = true;
                    }
                }
                Ok(None) => {}
                Err(ex) if matches!(ex, Fail::Roblox { .. }) && ex.rate_limited() => break,
                // No clear answer: asked again next time.
                Err(ex) if roblox_api::is_expected_error(&ex) => {}
                Err(ex) => return Err(ex),
            }
            rt::sleep(SIGN_IN_CHECK_PAUSE).await;
        }
        Ok(changed)
    }

    /// The names go in as they are written out (`name`), so Discord's copy follows its own switch and language.
    pub fn signed_out_text(&self, accounts: &[Account]) -> String {
        let named: Vec<Account> = accounts.iter().take(SIGN_IN_CHECK_NAMED).cloned().collect();
        let more = accounts.len() - named.len();
        let list = Shown::new(move |name| {
            let text = named.iter().map(|a| cut_name(name(a))).collect::<Vec<_>>().join(", ");
            if more == 0 {
                return text;
            }
            // Not l::f: that would replace the sentence the Discord copy is made again from.
            let mut language = l::language();
            App::with(|app| {
                if let Some((_, lang)) = app.names.name_style.borrow().as_ref() {
                    language = lang.clone();
                }
            });
            text + &l::format(&l::t_in(" and {0} more", &language), &[more.to_string()])
        });
        l::p(accounts.len() as i64, "{0} account needs signing in again: {1}", "{0} accounts need signing in again: {1}", &[&accounts.len(), &list])
    }

    /// Reads the account's public profile (no cookie) for a ban and notes it: marked, or cleared when it ended.
    /// Ok(Some(true)) when it was found banned just now, Ok(Some(false)) when a ban was cleared, Ok(None) when nothing changed.
    // C# changed the account object it was given; here the vault's account with its id (a copy of it is passed in).
    pub async fn check_ban(&self, account: &Account) -> Res<Option<bool>> {
        if account.user_id <= 0 {
            return Ok(None);
        }
        let user_id = account.user_id;
        let r = account.realm.clone();
        let banned = rt::blocking(move || roblox_api::get_banned(user_id, &r, &Cancel::new())).await?;
        let id = account.id;
        let mut snapshot = account.clone();
        let change = |a: &mut Account| -> Option<bool> {
            // No clear answer, or the account was signed in as someone else meanwhile: left as it was.
            let banned = banned?;
            if a.user_id != user_id {
                return None;
            }
            if banned {
                if a.banned == Some(true) {
                    return None;
                }
                a.banned = Some(true);
                a.banned_since = Some(Stamp::now());
                return Some(true);
            }
            let was_banned = a.banned == Some(true);
            a.banned = Some(false);
            a.banned_since = None;
            if was_banned { Some(false) } else { None }
        };
        let in_vault = self.vault_mut(|d| d.accounts.iter_mut().find(|a| a.id == id).map(|a| change(a))).ok().flatten();
        Ok(match in_vault {
            Some(result) => result,
            // Gone from the vault meanwhile: the answer is still given, as the C# gave it for the object it held.
            None => change(&mut snapshot),
        })
    }

    /// One notification for the accounts a check found banned (Settings' sign-in kind; Discord's too).
    pub fn notify_banned(&self, banned: Vec<Account>) {
        if banned.len() == 1 {
            let n = self.name(&banned[0]);
            self.notify(&l::f("{0} is banned on Roblox.", &[&n]), "warn", "banned", banned[0].id, true, None);
        } else {
            let n = self.names(&banned);
            let text = l::p(banned.len() as i64, "{0} account is banned on Roblox: {1}", "{0} accounts are banned on Roblox: {1}", &[&banned.len(), &n]);
            self.notify(&text, "warn", "banned", Guid::nil(), true, None);
        }
    }
}
