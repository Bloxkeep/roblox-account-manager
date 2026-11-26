//! Roblox's bot check on a join (the window held on Roblox's Security page, blank in Vietnam): "Solve the bot check" asks
//! the join itself, shows the check in Bloxkeep's own window (puzzle, press-and-hold; proof of work is computed, as
//! roblox.com does), hands the answer to Roblox and asks the join again carrying it; if let in, the held window reopens.
//! error.log notes each step by kind and field names only, so an unhandled kind shows what it was.
//! (AppController.BotCheck.cs)
use std::rc::Rc;

use serde_json::Value;

use super::{
    App,
    launching::{require_vault, run_of, same_vault},
};
use crate::core::{
    base::{Cancel, Fail, Guid, Res},
    close_plan::{Cause, Then},
    forms::captcha_form::CaptchaForm,
    l, log,
    models::Account,
    roblox_api::{self, JoinChallenge},
    rt,
};

/// The check's whole metadata plus captchaToken: sent back with only unifiedCaptchaId, captchaToken and actionType, Roblox
/// answered 403 "an internal error occurred" (2026-10-27).
pub(crate) fn captcha_answer(challenge: &JoinChallenge, token: &str) -> Res<String> {
    challenge.meta_with("captchaToken", token)
}

impl App {
    pub async fn solve_bot_check(self: &Rc<Self>, id: Guid) -> Res<Value> {
        let vault = require_vault(self)?;
        let account = self.account(id)?;
        let run = run_of(self, id);
        let held = run.as_ref().filter(|r| r.1.target.is_some() && self.controller.logs.borrow().join_challenged_at(r.0).is_some());
        let Some(run) = held.cloned() else {
            return Err(Fail::user(l::f("{0} isn't held by a bot check right now.", &[&self.name(&account)])));
        };
        let target = run.1.target.clone().expect("checked above");
        let place = self.controller.logs.borrow().state_of(run.0).and_then(|s| s.place_id).unwrap_or(target.place_id);

        // As a preset's restart does: whatever auto-rejoin says, where it can be opened again (close_plan.rs A4).
        // True when the window is being closed to open again; the toast says "opening again" only then.
        let reopen = |app: &App| -> bool {
            let Some(now) = run_of(app, id) else {
                return false;
            };
            if now.0 != run.0 {
                return false;
            }
            let restart = app.restart_target(now.0, &now.1);
            let asked = app.ask_close_with(Some(id), now.0, Cause::BotCheck, Then::Reopen(restart), false, || {
                app.rejoin.join_refused.borrow_mut().retain(|l| !Rc::ptr_eq(l, &now.1));
            });
            app.push_state();
            asked.closing()
        };

        let (cookie, account_realm, job) = (account.cookie.clone(), account.realm.clone(), target.job_id.clone());
        let first = rt::blocking(move || roblox_api::join_check(&cookie, place, job.as_deref(), &account_realm, &Cancel::new(), None)).await?;
        let Some(challenge) = first.challenge.clone() else {
            log::note(&format!(
                "Bot check asked about: no check now (http {}, status {}).",
                first.http,
                first.status.map(|s| s.to_string()).unwrap_or_default()
            ));
            if reopen(self) {
                self.toast(&l::f("Roblox no longer asks {0} for a check. Opening its window again…", &[&self.name(&account)]), "ok");
            }
            return Ok(Value::Null);
        };

        let answer = self.answer_check(&account, &challenge).await?;
        let Some(answer) = answer else {
            return Ok(Value::Null);
        };
        if !same_vault(self, &vault) {
            return Ok(Value::Null);
        }
        // The account as it is once the check was solved (that can take minutes; the C# held the live one): a sign-in
        // renewed meanwhile is the one the answer goes with.
        let Some(account) = self.find_account(id) else { return Ok(Value::Null) };

        // challenge/continue refused a join's check (2026-10-27) with 403 "an internal error occurred", whatever the
        // answer's shape: then the join is asked again carrying the answer, as that request's retry.
        let solved = JoinChallenge { metadata_json: answer, ..challenge.clone() };
        let (account_realm, cookie, sent) = (account.realm.clone(), account.cookie.clone(), solved.clone());
        let passed = match rt::blocking(move || roblox_api::continue_challenge(&account_realm, &cookie, &sent, &Cancel::new())).await {
            Ok(passed) => passed,
            Err(ex @ Fail::Roblox { .. }) if !ex.server_error() && !ex.rate_limited() => {
                let _ = ex;
                log::note("Bot check: continue refused; asking the join again carrying the answer.");
                solved
            }
            Err(ex) => return Err(ex),
        };
        let (cookie, account_realm, job) = (account.cookie.clone(), account.realm.clone(), target.job_id.clone());
        let again = rt::blocking(move || roblox_api::join_check(&cookie, place, job.as_deref(), &account_realm, &Cancel::new(), Some(&passed))).await?;
        log::note(&format!(
            "Bot check answered: the join asked again got http {}, status {}{}.",
            again.http,
            again.status.map(|s| s.to_string()).unwrap_or_default(),
            if again.challenge.is_none() { "" } else { ", held again" }
        ));
        if again.challenge.is_some() || again.http == 403 {
            return Err(Fail::user(l::f("The check was solved, but Roblox still won't let {0} join.", &[&self.name(&account)])));
        }
        if reopen(self) {
            self.toast(&l::f("Check passed. Opening {0}'s window again…", &[&self.name(&account)]), "ok");
        }
        Ok(Value::Null)
    }

    /// A check's answer as its metadata (proof of work computed, a puzzle in Bloxkeep's own window); None when the window
    /// was closed unsolved. A join's and a friend request's checks alike.
    pub async fn answer_check(self: &Rc<Self>, account: &Account, challenge: &JoinChallenge) -> Res<Option<String>> {
        let unknown = |kind: &str| {
            Fail::user(l::f(
                "Roblox asked {0} for a kind of check Bloxkeep can't show yet ({1}). It was noted, to be added.",
                &[&self.name(account), &(if kind.is_empty() { "?" } else { kind })],
            ))
        };
        match challenge.r#type.as_str() {
            "proofofwork" if challenge.meta("sessionId").is_some_and(|s| !s.is_empty()) => {
                let session = challenge.meta("sessionId").unwrap_or_default();
                let account_realm = account.realm.clone();
                let token = rt::blocking(move || roblox_api::proof_of_work_token(&account_realm, &session, &Cancel::new())).await?;
                Ok(Some(challenge.meta_with("redemptionToken", &token)?))
            }
            "captchav2" if challenge.meta("challengeId").is_some_and(|s| !s.is_empty()) => {
                let human = challenge.meta("challengeId").unwrap_or_default();
                match self.show_captcha(CaptchaForm::human(&account.realm, &human)).await {
                    Some(redemption) => Ok(Some(challenge.meta_with("redemptionToken", &redemption)?)),
                    None => Ok(None),
                }
            }
            "captcha"
                if challenge.meta("dataExchangeBlob").is_some_and(|s| !s.is_empty())
                    && challenge.meta("unifiedCaptchaId").is_some_and(|s| !s.is_empty()) =>
            {
                let blob = challenge.meta("dataExchangeBlob").unwrap_or_default();
                let action_type = challenge.meta("actionType").unwrap_or_else(|| "Generic".to_string());
                self.arkose_answer(account, challenge, &blob, &action_type).await
            }
            other => Err(unknown(other)),
        }
    }

    /// The puzzle kind of check, in Bloxkeep's captcha window; None when it was closed unsolved.
    async fn arkose_answer(self: &Rc<Self>, account: &Account, challenge: &JoinChallenge, blob: &str, action_type: &str) -> Res<Option<String>> {
        let (account_realm, action) = (account.realm.clone(), action_type.to_string());
        let key = rt::blocking(move || roblox_api::get_captcha_key(&account_realm, &action, &Cancel::new())).await?;
        let Some(key) = key else {
            return Err(Fail::user(l::f(
                "Roblox asked {0} for a kind of check Bloxkeep can't show yet ({1}). It was noted, to be added.",
                &[&self.name(account), &action_type],
            )));
        };
        match self.show_captcha(CaptchaForm::arkose(&account.realm, &key, blob)).await {
            Some(token) => Ok(Some(captcha_answer(challenge, &token)?)),
            None => Ok(None),
        }
    }
}
