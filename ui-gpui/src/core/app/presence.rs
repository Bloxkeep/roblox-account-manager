//! Roblox's online status of each account, and the checks whether an account was signed out.
//! (AppController.Presence.cs)
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

use futures::StreamExt;
use serde_json::{Value, json};

use super::{
    App,
    controller::{BACKGROUND_PRESENCE_INTERVAL, PRESENCE_INTERVAL},
    launching::{min_stamp, pending, running, same_vault, secs, short, vault_now},
};
use crate::core::{
    base::{Cancel, Guid, Res, Stamp},
    log,
    models::Account,
    roblox_api::{self, Presence, realm},
    rt,
};

pub struct PresenceState {
    /// The last status check failed: written once, then once more when one goes through.
    pub presence_failing: Cell<bool>,
    /// When a sign-in outage was last written to error.log (at most every half hour).
    pub sign_in_outage_noted: Cell<Stamp>,
    /// Accounts Roblox answered "User is moderated" for (a notice to accept on roblox.com), and when: every call with
    /// such a sign-in is refused, so for MODERATED_FOR the status check and a share link are asked with another account.
    /// One held notice on the first account left every account's status unread and every share link unopened.
    pub moderated: RefCell<HashMap<Guid, Stamp>>,
}

/// How long an account Roblox called moderated is passed over as the one asking for the others; then it is tried again
/// (the notice may have been accepted).
const MODERATED_FOR: Duration = Duration::from_secs(10 * 60);

impl Default for PresenceState {
    fn default() -> PresenceState {
        PresenceState { presence_failing: Cell::new(false), sign_in_outage_noted: Cell::new(min_stamp()), moderated: RefCell::new(HashMap::new()) }
    }
}

impl App {
    /// Roblox answered "User is moderated" for this account's sign-in just now.
    pub fn note_moderated(&self, account_id: Guid) {
        self.presence.moderated.borrow_mut().insert(account_id, Stamp::utc_now());
    }

    /// Roblox called this account moderated within MODERATED_FOR: not the one to ask for the others.
    pub fn moderated_lately(&self, account_id: Guid) -> bool {
        self.presence.moderated.borrow().get(&account_id).is_some_and(|at| Stamp::utc_now().secs_since(*at) < secs(MODERATED_FOR))
    }

    pub fn refresh_presence_soon(&self, delay: Duration) {
        let at = Stamp::utc_now().plus_secs(secs(delay));
        if at < self.controller.next_presence.get() {
            self.controller.next_presence.set(at);
        }
    }

    /// Who is online and playing what: one request for everyone; accounts hidden by privacy are asked with their own cookie.
    pub async fn refresh_presence(self: &Rc<Self>) -> Res<Value> {
        let Some(vault) = vault_now(self) else {
            return Ok(Value::Null);
        };
        if self.controller.presence_busy.get() {
            return Ok(json!({ "skipped": true })); // one is under way; its answer is on its way to the page
        }
        // In the background (nobody is looking) only the clients open here whose log isn't followed are asked — auto-rejoin
        // relies on their status — and everyone else waits for the full round every few minutes.
        let light = self.controller.background.get()
            && self.controller.presence_loaded.get()
            && Stamp::utc_now().secs_since(self.controller.last_full_presence.get()) < secs(BACKGROUND_PRESENCE_INTERVAL);
        let runs: HashMap<Guid, i32> = running(self).into_iter().map(|(id, r)| (id, r.0)).collect();
        let accounts: Vec<Account> = self
            .vault(|d| d.accounts.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.user_id > 0 && (!light || runs.get(&a.id).is_some_and(|pid| !self.log_followed(*pid))))
            .collect();
        self.controller.next_presence.set(Stamp::utc_now().plus_secs(secs(PRESENCE_INTERVAL)));
        if accounts.is_empty() {
            return Ok(Value::Null);
        }

        let busy = crate::core::base::Busy::set(&self.controller.presence_busy);
        // The first account whose sign-in Roblox takes; one holding a notice (moderated_lately) only when there is no other.
        let can_ask = |a: &&Account| a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.realm == realm::GLOBAL;
        let viewer: Option<Account> = accounts.iter().filter(can_ask).find(|a| !self.moderated_lately(a.id)).or_else(|| accounts.iter().find(can_ask)).cloned();
        let viewer_held = viewer.as_ref().is_some_and(|v| self.moderated_lately(v.id));
        let run: Res<bool> = async {
            // Roblox's accounts only: Roblox VNG counts user ids apart, so a VNG account's number is someone else on Roblox
            // (whose game and server its card once showed). A VNG account's status comes with its own sign-in below.
            let global_ids: Vec<i64> = accounts.iter().filter(|a| a.realm == realm::GLOBAL).map(|a| a.user_id).collect();
            let viewer_cookie = viewer.as_ref().map(|v| v.cookie.clone());
            let mut result: HashMap<i64, Presence> =
                rt::blocking(move || roblox_api::get_presence(viewer_cookie.as_deref(), &global_ids, realm::GLOBAL, &Cancel::new())).await?;
            // First answer since unlocking: shown now rather than "status unknown" until the follow-ups below are back.
            if !self.controller.presence_loaded.get() && same_vault(self, &vault) {
                {
                    let mut presence = self.controller.presence.borrow_mut();
                    for (user_id, p) in &result {
                        presence.insert(*user_id, p.clone());
                    }
                }
                self.controller.presence_loaded.set(true);
                self.controller.boot_status_done.set(true);
                self.advance_boot(true);
                self.push_state_if_changed();
            }

            // Privacy can hide an account from the others. Accounts with a client here are asked with their own cookie
            // (auto-rejoin relies on it) unless its log is followed; others only when their game is hidden.
            let viewer_id = viewer.as_ref().map(|v| v.id);
            let ask_own: Vec<Account> = accounts
                .iter()
                .filter(|a| {
                    Some(a.id) != viewer_id
                        && a.cookie_valid != Some(false)
                        && !a.cookie.is_empty()
                        // One whose log says in a game is asked too when the others' answer says not (detect_disconnected_sessions).
                        && !runs.get(&a.id).is_some_and(|pid| {
                            self.log_followed(*pid) && self.controller.logs.borrow().state_of(*pid).is_none_or(|s| s.status != "ingame")
                        })
                        && (a.realm == realm::VNG
                            || result.get(&a.user_id).is_some_and(|p| {
                                if runs.contains_key(&a.id) { p.r#type != 2 || p.place_id.is_none() } else { p.r#type == 2 && p.place_id.is_none() }
                            }))
                })
                .cloned()
                .collect();
            let ask_own: Vec<Account> = ask_own
                .iter()
                .filter(|a| runs.contains_key(&a.id))
                .chain(ask_own.iter().filter(|a| !runs.contains_key(&a.id)).take(10))
                .cloned()
                .collect();
            let mut reliable: HashSet<i64> = result.iter().filter(|(_, p)| p.r#type == 2).map(|(k, _)| *k).collect();
            if let Some(v) = &viewer {
                reliable.insert(v.user_id);
            }
            // Accounts whose own request was rejected: at most one is marked per round, after a second call agrees.
            let mut rejected: Vec<Account> = Vec::new();
            // SemaphoreSlim(4) + Task.WhenAll: four at a time, answers taken as they come.
            let answers: Vec<(Account, Res<HashMap<i64, Presence>>)> = futures::stream::iter(ask_own.into_iter().map(|account| async move {
                let (cookie, user_id, account_realm) = (account.cookie.clone(), account.user_id, account.realm.clone());
                let own = rt::blocking(move || roblox_api::get_presence(Some(&cookie), &[user_id], &account_realm, &Cancel::new())).await;
                (account, own)
            }))
            .buffer_unordered(4)
            .collect()
            .await;
            for (account, own) in answers {
                match own {
                    Ok(own) => {
                        if let Some(p) = own.get(&account.user_id) {
                            result.insert(account.user_id, p.clone());
                            reliable.insert(account.user_id);
                        }
                    }
                    // Keep what the other account saw; auto-rejoin won't act on it.
                    Err(ex) if roblox_api::is_expected_error(&ex) => {
                        if ex.moderated() {
                            self.note_moderated(account.id);
                        } else if ex.cookie_invalid() {
                            rejected.push(account);
                        }
                    }
                    // Not caught in C#: WhenAll throws it, out of the whole round.
                    Err(ex) => return Err(ex),
                }
            }
            if let Some(first) = rejected.first()
                && same_vault(self, &vault)
            {
                self.confirm_signed_out(first.id, &Cancel::new(), false).await; // the rest are asked again next round
            }

            let mut games: Vec<(i64, i64)> = result
                .values()
                .filter(|p| p.root_place_id.or(p.place_id).unwrap_or(0) > 0)
                .map(|p| (p.root_place_id.or(p.place_id).unwrap_or(0), p.universe_id.unwrap_or(0)))
                .collect();
            let launches = running(self).into_iter().map(|(_, r)| r.1).chain(pending(self));
            games.extend(launches.filter_map(|l| l.target.as_ref().filter(|t| t.place_id > 0).map(|t| (t.place_id, 0))));
            // Many games move players to another place of the same game after joining; the log has that one.
            games.extend(
                running(self)
                    .into_iter()
                    .map(|(_, r)| self.controller.logs.borrow().state_of(r.0).and_then(|s| s.place_id).unwrap_or(0))
                    .filter(|id| *id > 0)
                    .map(|id| (id, 0)),
            );
            self.load_games(games, false).await?;

            if !same_vault(self, &vault) {
                return Ok(false);
            }
            // A light round only updates the accounts it asked about.
            if !light {
                self.controller.presence.borrow_mut().clear();
                self.controller.presence_reliable.borrow_mut().clear();
                self.controller.last_full_presence.set(Stamp::utc_now());
            }
            {
                let mut presence = self.controller.presence.borrow_mut();
                for (user_id, p) in result {
                    presence.insert(user_id, p);
                }
            }
            self.controller.presence_reliable.borrow_mut().extend(reliable.iter().copied());
            self.controller.presence_loaded.set(true);
            self.detect_disconnected_sessions(&reliable);
            self.push_state_if_changed();
            if self.presence.presence_failing.get() {
                log::note("Status check: recovered.");
            }
            self.presence.presence_failing.set(false);
            Ok(true)
        }
        .await;
        let outcome: Res<()> = match run {
            Ok(_) => Ok(()),
            // The account asking was signed out meanwhile: it is marked so (the next check asks with another one) — once
            // a second call agrees, so an outage of Roblox's sign-in service doesn't sign everyone out here one by one.
            Err(ex) if ex.cookie_invalid() && viewer.is_some() && same_vault(self, &vault) => {
                if let Some(v) = &viewer {
                    self.confirm_signed_out(v.id, &Cancel::new(), false).await;
                }
                Ok(())
            }
            // The account asking holds a notice from Roblox: passed over, and the check goes again at once with the next
            // account. (Already passed over and asked for want of another: a failed check like any other, below.)
            Err(ex) if ex.moderated() && viewer.is_some() && !viewer_held && same_vault(self, &vault) => {
                if let Some(v) = &viewer {
                    self.note_moderated(v.id);
                    log::note(&format!(
                        "Status check: Roblox holds a notice on account {} (moderated), so its sign-in is refused; the others are asked with another account.",
                        short(v.id)
                    ));
                }
                self.controller.next_presence.set(Stamp::utc_now().plus_secs(2.0));
                Ok(())
            }
            Err(ex) if roblox_api::is_expected_error(&ex) => {
                self.controller.next_presence.set(Stamp::utc_now().plus_secs(secs(PRESENCE_INTERVAL) * 2.0));
                if !self.presence.presence_failing.get() {
                    log::note(&format!(
                        "Status check: failed ({}); accounts keep their last status, next try in a minute.",
                        App::log_kind_of(&ex)
                    ));
                }
                self.presence.presence_failing.set(true);
                Ok(())
            }
            Err(ex) => Err(ex),
        };
        // finally
        drop(busy);
        // A failed first try leaves the accounts as "status unknown" instead of loading forever.
        if !self.controller.presence_loaded.get() {
            self.controller.boot_status_done.set(true);
            self.advance_boot(true);
            self.push_state_if_changed();
        }
        outcome?;
        Ok(Value::Null)
    }

    /// One rejected request isn't proof of a sign-out (an outage of Roblox's sign-in service answers 401 for everyone):
    /// marked only when the "who am I" call rejects the cookie too and Roblox accepts another account's sign-in meanwhile.
    /// No clear answer leaves it as it was; one accepted after all is signed in again.
    /// `trusted`: the daily check already ruled out an outage (or trusts its second run): no other account is asked.
    // PORT: the C# re-reads the live Account object after each await; here the vault's copy is read again by id. A failure
    // that isn't one of Roblox's expected ones (it threw out of the C#) is logged and answers false; a call-off answers
    // false too, and the caller checks its token (launch_accounts) as the exception made it.
    pub async fn confirm_signed_out(self: &Rc<Self>, account_id: Guid, ct: &Cancel, trusted: bool) -> bool {
        let Some(account) = self.find_account(account_id) else {
            return false;
        };
        let cookie = account.cookie.clone();
        let (c, r, t) = (cookie.clone(), account.realm.clone(), ct.clone());
        match rt::blocking(move || roblox_api::get_authenticated_user(&c, &r, &t)).await {
            Ok(Some(_)) => {
                let now = self.find_account(account_id);
                if now.as_ref().is_some_and(|a| a.cookie == cookie && a.cookie_valid == Some(false)) {
                    log::note(&format!("Sign-in check: account {} is accepted by Roblox again.", short(account_id)));
                    self.update_account(account_id, App::sign_in_accepted);
                    self.persist_soon();
                }
                return false;
            }
            Ok(None) => {}
            Err(ex) if !ct.is_cancelled() && roblox_api::is_expected_error(&ex) => {
                log::note(&format!(
                    "Sign-in check: a rejected sign-in couldn't be confirmed ({}); account {} left as it was.",
                    App::log_kind_of(&ex),
                    short(account_id)
                ));
                return false;
            }
            Err(ex) => {
                if !ct.is_cancelled() {
                    log::write(&ex);
                }
                return false;
            }
        }
        // Signed in again meanwhile: the old cookie's answer says nothing about the new one.
        if self.find_account(account_id).is_none_or(|a| a.cookie != cookie) {
            return false;
        }
        // Without this, an outage marked one more account per request that met it, and a rejoin turned auto-rejoin off.
        if !trusted && self.sign_in_service_down(&account, ct).await {
            if Stamp::utc_now().secs_since(self.presence.sign_in_outage_noted.get()) > 30.0 * 60.0 {
                self.presence.sign_in_outage_noted.set(Stamp::utc_now());
                log::note("Sign-in check: Roblox rejected every account asked; taken as an outage, nobody is marked signed out.");
            }
            return false;
        }
        if ct.is_cancelled() {
            return false; // PORT: the C# threw out of sign_in_service_down here
        }
        if self.find_account(account_id).is_none_or(|a| a.cookie != cookie) {
            return false;
        }
        self.update_account(account_id, |a| a.cookie_valid = Some(false));
        self.persist_soon();
        log::note(&format!(
            "Sign-in check: account {} marked signed out (Roblox rejected its sign-in twice{}).",
            short(account_id),
            if trusted { "" } else { ", another account was accepted" }
        ));
        true
    }

    /// Whether Roblox's sign-in service rejects everyone now, by asking up to two other signed-in accounts of the same
    /// Roblox (VNG's is another service), last checked first. False when one is accepted or there is none to ask (no
    /// telling, so a rejection counts); true when all asked were rejected or didn't answer.
    async fn sign_in_service_down(&self, rejected: &Account, ct: &Cancel) -> bool {
        let mut others: Vec<Account> = self
            .vault(|d| {
                d.accounts
                    .iter()
                    .filter(|a| a.id != rejected.id && a.realm == rejected.realm && a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.cookie != rejected.cookie)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        // OrderByDescending(CookieValid == true).ThenByDescending(LastChecked ?? MinValue): a stable sort.
        others.sort_by(|a, b| {
            (b.cookie_valid == Some(true))
                .cmp(&(a.cookie_valid == Some(true)))
                .then_with(|| b.last_checked.unwrap_or_else(min_stamp).cmp(&a.last_checked.unwrap_or_else(min_stamp)))
        });
        others.truncate(2);
        if others.is_empty() {
            return false;
        }
        for other in others {
            let (c, r, t) = (other.cookie.clone(), other.realm.clone(), ct.clone());
            match rt::blocking(move || roblox_api::get_authenticated_user(&c, &r, &t)).await {
                Ok(Some(_)) => return false,
                Ok(None) => {}
                Err(ex) if !ct.is_cancelled() && roblox_api::is_expected_error(&ex) => {}
                // PORT: thrown out of the C# (a call-off, or an unexpected failure): taken as no answer, the failure written.
                Err(ex) => {
                    if !ct.is_cancelled() {
                        log::write(&ex);
                    }
                }
            }
        }
        true
    }
}

