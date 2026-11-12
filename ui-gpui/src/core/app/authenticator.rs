//! AppController.Authenticator.cs: authenticator (2-step) codes per account (`totp`). The key sits in the vault, encrypted like
//! the password, and never goes to the page or the log: the page gets only the current code (edit form, "Copy 2FA code", the
//! sign-in's code box). Anyone with the master password can make codes; the edit form says so.
use serde_json::{Value, json};

use super::App;
use crate::core::base::{Fail, Guid, Res, Stamp};
use crate::core::models::Account;
use crate::core::roblox_api::{TwoStepPrompt, realm};
use crate::core::totp::{self, TotpKey, TotpProblem};
use crate::core::l;

#[derive(Default)]
pub struct AuthenticatorState {}

impl App {
    /// A key typed in the edit form, as kept; a typo is refused with what's wrong rather than kept to make wrong codes.
    pub fn parse_authenticator(input: &str) -> Res<TotpKey> {
        let (key, problem, detail) = totp::parse(Some(input));
        key.ok_or_else(|| Fail::user(App::authenticator_problem(problem, &detail)))
    }

    pub fn authenticator_problem(problem: TotpProblem, detail: &str) -> String {
        match problem {
            TotpProblem::BadCharacter => {
                l::f("The key can't contain \"{0}\": it only has the letters A–Z and the digits 2–7. Check it for a typo.", &[&detail])
            }
            TotpProblem::TooShort => l::t("The key is too short. Check that you copied all of it."),
            TotpProblem::CutOff => l::t("The key looks cut off or has an extra character. Check that you copied all of it."),
            TotpProblem::NotTimeBased => l::t("This link makes counter-based codes. Bloxkeep only makes time-based codes, the kind Roblox uses."),
            TotpProblem::NoSecret => l::t("The link has no key in it."),
            TotpProblem::Unsupported => l::f("Bloxkeep can't make codes with this setting from the link: {0}.", &[&detail]),
            _ => l::t("That isn't an authenticator key. Paste the key (letters A–Z and digits 2–7) or the otpauth:// link."),
        }
    }

    /// The code of the moment and the seconds it has left: all the page ever gets from a key.
    pub fn code_dto(key: &TotpKey) -> Value {
        let now = Stamp::utc_now();
        json!({ "ok": true, "code": totp::code(key, now), "seconds": totp::seconds_left(key, now), "period": key.period })
    }

    /// The edit form's live code for the key saved with an account.
    pub fn authenticator_code(&self, id: Guid) -> Res<Value> {
        let key = self.account(id)?.authenticator.ok_or_else(|| Fail::user(l::t("This account has no authenticator key.")))?;
        Ok(App::code_dto(&key))
    }

    /// A key being typed: whether it reads and its current code, to compare with the app before saving. Nothing is kept.
    pub fn check_authenticator(input: &str) -> Value {
        match totp::parse(Some(input)) {
            (Some(key), _, _) => App::code_dto(&key),
            (None, problem, detail) => json!({ "ok": false, "error": App::authenticator_problem(problem, &detail) }),
        }
    }

    /// The account a sign-in's 2-step code is for, when its key is saved here: same user id (named in the challenge), else
    /// same username, on the same Roblox.
    pub fn authenticator_account(&self, prompt: &TwoStepPrompt) -> Option<Account> {
        if !prompt.methods.iter().any(|m| m == "authenticator") {
            return None;
        }
        let realm = realm::normalize(Some(&prompt.realm));
        self.vault(|d| {
            let with_key: Vec<&Account> = d.accounts.iter().filter(|a| a.authenticator.is_some() && realm::normalize(Some(&a.realm)) == realm).collect();
            with_key
                .iter()
                .find(|a| a.user_id > 0 && a.user_id == prompt.user_id)
                .or_else(|| with_key.iter().find(|a| !prompt.username.is_empty() && a.username.to_lowercase() == prompt.username.to_lowercase()))
                .map(|a| (*a).clone())
        })
        .ok()
        .flatten()
    }

    /// The code for the sign-in's 2-step box, filled in for the person, who still presses Verify: Bloxkeep never sends a
    /// code by itself, nor tries another after Roblox refused one.
    pub fn login_two_step_code(&self) -> Res<Value> {
        self.vault(|_| ())?;
        let prompt = self.accounts.two_step.borrow().clone();
        let key = prompt.and_then(|p| {
            let p = p.borrow().clone();
            self.authenticator_account(&p).and_then(|a| a.authenticator)
        });
        let Some(key) = key else {
            return Err(Fail::user(l::t("Start the sign-in again.")));
        };
        Ok(App::code_dto(&key))
    }
}
