// AppController.Friends.cs: friend each other: the chosen accounts become Roblox friends, every pair. Where a pair stands
// is asked first, so one already friends is left alone and a request already sent is only accepted. When Roblox holds a
// request for a check, it is solved in Bloxkeep's own window, as a join's bot check is (`answer_check`). A pause between
// requests, so Roblox doesn't rate-limit a dozen accounts. Not yet tried on real accounts (2026-11-05).
use std::{cell::Cell, collections::HashMap, rc::Rc, time::Duration};

use serde_json::{Value, json};

use super::App;
use super::controller::{id_of, long_of, str_of};
use super::presets::same_vault;
use crate::core::{
    base::{Cancel, Fail, Guid, Res},
    l, log,
    models::Account,
    roblox_api::{self, FriendAnswer, JoinChallenge, realm},
    rt,
};

pub const FRIEND_PAUSE: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct FriendsState {
    pub friending: Cell<bool>,
}

/// A friend request, an acceptance, a decline or an unfriend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FriendAct {
    Request,
    Accept,
    Decline,
    Unfriend,
}

/// One friend call, off the core's thread.
async fn friend_call(act: FriendAct, cookie: &str, user: i64, account_realm: &str, answered: Option<JoinChallenge>) -> Res<FriendAnswer> {
    let (cookie, account_realm) = (cookie.to_string(), account_realm.to_string());
    rt::blocking(move || {
        let ct = Cancel::new();
        let answered = answered.as_ref();
        match act {
            FriendAct::Request => roblox_api::request_friendship(&cookie, user, &account_realm, answered, &ct),
            FriendAct::Accept => roblox_api::accept_friend_request(&cookie, user, &account_realm, answered, &ct),
            FriendAct::Decline => roblox_api::decline_friend_request(&cookie, user, &account_realm, answered, &ct),
            FriendAct::Unfriend => roblox_api::unfriend(&cookie, user, &account_realm, answered, &ct),
        }
    })
    .await
}

fn codes_text(codes: &[i32]) -> String {
    codes.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")
}

/// How `make_friends`'s try ended: every pair done (its closing message is still to come), or stopped early (said already).
enum Ended {
    Done,
    Stopped,
}

impl App {
    pub async fn make_friends(self: &Rc<Self>, ids: &[Guid]) -> Res<Value> {
        let vault = self.held_vault().ok_or_else(|| Fail::user(l::t("Bloxkeep is locked.")))?;
        let accounts: Vec<Account> = self.vault(|d| {
            d.accounts
                .iter()
                .filter(|a| ids.contains(&a.id) && a.user_id > 0 && a.cookie_valid != Some(false) && realm::normalize(Some(&a.realm)) == realm::GLOBAL)
                .cloned()
                .collect()
        })?;
        if accounts.len() < 2 {
            return Err(Fail::user(l::t("Pick at least two signed-in roblox.com accounts to friend each other.")));
        }
        if self.friends.friending.get() {
            return Err(Fail::user(l::t("Already friending accounts. Wait for it to finish.")));
        }
        let friending = crate::core::base::Busy::set(&self.friends.friending);
        let (mut made, mut already, mut failed, mut done) = (0, 0, 0, 0);
        let pairs = accounts.len() * (accounts.len() - 1) / 2;
        let run: Res<Ended> = async {
            for i in 0..accounts.len() {
                let a = &accounts[i];
                let later: Vec<Account> = accounts[i + 1..].to_vec();
                if later.is_empty() {
                    break;
                }
                let stands: HashMap<i64, String> = {
                    let (cookie, user, others, account_realm) = (a.cookie.clone(), a.user_id, later.iter().map(|b| b.user_id).collect::<Vec<_>>(), a.realm.clone());
                    match rt::blocking(move || roblox_api::friend_statuses(&cookie, user, &others, &account_realm, &Cancel::new())).await {
                        Ok(s) => s,
                        Err(ex @ Fail::Roblox { .. }) if !ex.rate_limited() => {
                            // Unknown: asked anyway, and Roblox says if they are friends already.
                            log::note(&format!("Friend each other: statuses not read ({}).", ex.message()));
                            HashMap::new()
                        }
                        Err(ex) => return Err(ex),
                    }
                };
                for b in &later {
                    done += 1;
                    self.progress(Some(&l::f("Friending accounts… {0}/{1}", &[&done, &pairs])));
                    if !same_vault(&self.held_vault(), &Some(vault.clone())) {
                        log::note(&format!("Friend each other: stopped, the vault was locked or changed ({made} new so far)."));
                        return Ok(Ended::Stopped);
                    }
                    let stand = stands.get(&b.user_id).map(String::as_str).unwrap_or("NotFriends");
                    if stand == "Friends" {
                        already += 1;
                        continue;
                    }
                    let ok: Option<bool> = match stand {
                        "RequestReceived" => Some(self.friend_accept(a, b).await?), // b asked a already
                        "RequestSent" => Some(self.friend_accept(b, a).await?),     // a asked b already
                        _ => match self.friend_request(a, b).await? {
                            None => None,
                            Some(sent) => Some(sent && self.friend_accept(b, a).await?),
                        },
                    };
                    let Some(ok) = ok else {
                        log::note(&format!("Friend each other: stopped, a check was closed unsolved ({made} new of {pairs} pairs)."));
                        self.toast(&l::f("Stopped: the check was closed. {0} new friends so far.", &[&made]), "warn");
                        return Ok(Ended::Stopped);
                    };
                    if ok {
                        made += 1;
                    } else {
                        failed += 1;
                    }
                    rt::sleep(FRIEND_PAUSE).await;
                }
            }
            Ok(Ended::Done)
        }
        .await;
        // finally
        drop(friending);
        self.progress(None);
        match run {
            Err(ex) if ex.rate_limited() => {
                log::note(&format!("Friend each other: Roblox rate-limited it; stopped after {made} new of {pairs} pairs."));
                self.toast(&l::f("Roblox is rate-limiting requests; try again shortly. {0} new friends so far.", &[&made]), "warn");
                return Ok(Value::Null);
            }
            Err(ex) => return Err(ex),
            Ok(Ended::Stopped) => return Ok(Value::Null),
            Ok(Ended::Done) => {}
        }
        if failed > 0 {
            log::note(&format!("Friend each other: {made} new, {already} already friends, {failed} refused by Roblox of {pairs} pairs."));
        }
        self.toast(&l::f("Friend each other: {0} new, {1} already friends, {2} failed.", &[&made, &already, &failed]), if failed > 0 { "warn" } else { "ok" });
        Ok(Value::Null)
    }

    /// {id, userId, act}: one account's own move in Friends (the GPUI window's Friends): "request" asks that Roblox user to
    /// be friends, "accept" and "decline" answer their request, "unfriend" ends it. A check Roblox holds it for is shown to
    /// solve, then it is asked again. { ok, held (the check was closed unsolved), http, codes }; the window words it.
    pub async fn friend_act(self: &Rc<Self>, a: &Value) -> Res<Value> {
        self.vault(|_| ())?;
        let account = self.account(id_of(a))?;
        let user = long_of(a, "userId");
        let act_name = str_of(a, "act");
        let act = match act_name.as_str() {
            "request" => Some(FriendAct::Request),
            "accept" => Some(FriendAct::Accept),
            "decline" => Some(FriendAct::Decline),
            "unfriend" => Some(FriendAct::Unfriend),
            _ => None,
        };
        let Some(act) = act.filter(|_| user > 0 && !account.cookie.is_empty() && account.realm == realm::GLOBAL) else {
            return Ok(json!({ "ok": false, "held": false, "http": 0, "codes": [] }));
        };
        let mut answer = friend_call(act, &account.cookie, user, &account.realm, None).await?;
        if let Some(check) = answer.challenge.clone() {
            let Some(passed) = self.pass_check(&account, &check).await? else {
                return Ok(json!({ "ok": false, "held": true, "http": answer.http, "codes": [] }));
            };
            answer = friend_call(act, &account.cookie, user, &account.realm, Some(passed)).await?;
        }
        if !answer.ok() {
            log::note(&format!("Friends: {act_name} was refused (HTTP {}, codes {}).", answer.http, codes_text(&answer.codes)));
        }
        Ok(json!({ "ok": answer.ok(), "held": false, "http": answer.http, "codes": answer.codes }))
    }

    /// `from` asks `to`; Some(true) sent, Some(false) refused, None a check closed unsolved.
    pub async fn friend_request(self: &Rc<Self>, from: &Account, to: &Account) -> Res<Option<bool>> {
        let mut answer = friend_call(FriendAct::Request, &from.cookie, to.user_id, &from.realm, None).await?;
        if let Some(check) = answer.challenge.clone() {
            let Some(passed) = self.pass_check(from, &check).await? else { return Ok(None) };
            answer = friend_call(FriendAct::Request, &from.cookie, to.user_id, &from.realm, Some(passed)).await?;
        }
        if !answer.ok() {
            log::note(&format!("Friend each other: a request was refused (HTTP {}, codes {}).", answer.http, codes_text(&answer.codes)));
        }
        Ok(Some(answer.ok()))
    }

    /// `by` accepts the request `requester` sent it.
    pub async fn friend_accept(self: &Rc<Self>, by: &Account, requester: &Account) -> Res<bool> {
        let mut answer = friend_call(FriendAct::Accept, &by.cookie, requester.user_id, &by.realm, None).await?;
        if let Some(check) = answer.challenge.clone()
            && let Some(passed) = self.pass_check(by, &check).await?
        {
            answer = friend_call(FriendAct::Accept, &by.cookie, requester.user_id, &by.realm, Some(passed)).await?;
        }
        if !answer.ok() {
            log::note(&format!("Friend each other: an accept was refused (HTTP {}, codes {}).", answer.http, codes_text(&answer.codes)));
        }
        Ok(answer.ok())
    }

    /// A check solved and handed to Roblox, to carry on the request asked again; None when closed unsolved.
    pub async fn pass_check(self: &Rc<Self>, account: &Account, check: &JoinChallenge) -> Res<Option<JoinChallenge>> {
        let Some(answer) = self.answer_check(account, check).await? else { return Ok(None) };
        let solved = JoinChallenge { metadata_json: answer, ..check.clone() };
        let (account_realm, cookie, sent) = (account.realm.clone(), account.cookie.clone(), solved.clone());
        match rt::blocking(move || roblox_api::continue_challenge(&account_realm, &cookie, &sent, &Cancel::new())).await {
            Ok(next) => Ok(Some(next)),
            Err(ex @ Fail::Roblox { .. }) if !ex.server_error() && !ex.rate_limited() => {
                log::note(&format!("Friend each other: Roblox didn't take the check's answer ({}); the request is asked again with it.", App::log_kind_of(&ex)));
                Ok(Some(solved)) // as a join's: the request asked again carrying the answer
            }
            Err(ex) => Err(ex),
        }
    }
}
