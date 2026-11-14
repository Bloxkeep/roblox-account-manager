//! AppController.Accounts.cs: adding, editing, checking and removing accounts, and what is done to them from the page.
use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use futures::future::LocalBoxFuture;
use serde::Serialize;
use serde_json::{Value, json};

use super::App;
use super::controller::{error_text, id_of, opt_str, str_of};
use crate::core::base::{Cancel, Fail, Guid, Res, Stamp};
use crate::core::forms::browser_login_form::BrowserLoginForm;
use crate::core::forms::captcha_form::CaptchaForm;
use crate::core::models::Account;
use crate::core::roblox_api::{
    self, CaptchaPrompt, HumanCaptchaPrompt, LoginResult, QuickLoginCode, QuickLoginStatus, RobloxApiException, TwoStepPrompt, realm,
};
use crate::core::vault::Vault;
use crate::core::{l, log, roblox_game_settings, roblox_processes, rt, totp, ui};

/// A sign-in (browser) or captcha window open now (C#'s `List<Form>` of them).
#[derive(Clone)]
pub enum SignInWindow {
    Browser(Rc<BrowserLoginForm>),
    Captcha(Rc<CaptchaForm>),
}

impl SignInWindow {
    fn same(&self, other: &SignInWindow) -> bool {
        match (self, other) {
            (SignInWindow::Browser(a), SignInWindow::Browser(b)) => Rc::ptr_eq(a, b),
            (SignInWindow::Captcha(a), SignInWindow::Captcha(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    async fn show_dialog(&self, owner: isize) {
        match self {
            SignInWindow::Browser(f) => {
                let _ = f.show_dialog(owner).await;
            }
            SignInWindow::Captcha(f) => {
                let _ = f.show_dialog(owner).await;
            }
        }
    }

    fn close(&self) {
        match self {
            SignInWindow::Browser(f) => f.close(),
            SignInWindow::Captcha(f) => f.close(),
        }
    }
}

#[derive(Default)]
pub struct AccountsState {
    /// The sign-in (browser) and captcha windows open now. They are modal, so nothing but a close here can take them
    /// down when the screen locks or the vault closes: they'd stay over the lock screen, and the browser one could still save an
    /// account into the locked vault.
    pub sign_in_windows: RefCell<Vec<SignInWindow>>,
    /// The code on screen in the Quick Login window; one at a time.
    pub quick_login: RefCell<Option<Rc<QuickLoginCode>>>,
    pub quick_login_finishing: Cell<bool>,
    pub quick_login_starts: Cell<i32>,
    pub quick_login_finish_tries: Cell<i32>,
    /// The session a confirmed code was traded for, while its account lookup is being retried.
    pub quick_login_cookie: RefCell<Option<(Rc<QuickLoginCode>, String)>>,
    // A two-step prompt or captcha waiting for the user; held only while the sign-in is in progress, never persisted.
    pub two_step: RefCell<Option<Rc<RefCell<TwoStepPrompt>>>>,
    pub captcha: RefCell<Option<Rc<CaptchaPrompt>>>,
    pub human_captcha: RefCell<Option<Rc<HumanCaptchaPrompt>>>,
    /// The puzzle of this sign-in attempt was already shown and closed: it can't be opened again (it comes up blank).
    pub captcha_used: Cell<bool>,
    /// Puzzles asked for again in this attempt after their window was closed; each one sends the password again.
    pub fresh_puzzles: Cell<i32>,
    /// A session whose account couldn't be looked up yet (network hiccup): "Try again" only looks it up again,
    /// rather than resending the password or a spent code.
    pub login_cookie: RefCell<Option<(String, String)>>,
    /// Roblox said "too many tries" to a 2-step code: Verify and Send wait until then (None: DateTime's default).
    pub two_step_wait_until: Cell<Option<Stamp>>,
    /// Bumped by a cancel or a lock: a sign-in step that started before it must not keep its result.
    pub login_generation: Cell<i32>,
    /// Cancelled (and replaced) with each bump of `login_generation`, for the steps that go on by themselves.
    pub login_cts: RefCell<Cancel>,
    /// The main accounts' Roblox windows (their processes); remembered while the vault is locked, as the windows go on.
    pub main_pids: RefCell<HashSet<i32>>,
    /// The last avatar load failed: written once, then once more when one goes through.
    pub avatars_failing: Cell<bool>,
}

pub const MAX_FRESH_PUZZLES: i32 = 3;
pub const TWO_STEP_WAIT: Duration = Duration::from_secs(60);
pub const CHECK_PARALLELISM: usize = 6;
pub const SETTLED_DELAY_MS: u64 = 1500;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    pub id: Guid,
    pub label: String,
    pub merged: bool,
}

/// A step of the loading screen and how far along it is (0–1); the page shows it until it is None.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootProgress {
    pub stage: String,
    pub percent: f64,
}

impl BootProgress {
    pub fn new(stage: &str, percent: f64) -> BootProgress {
        BootProgress { stage: stage.into(), percent }
    }
}

/// `RobloxApi.CancelQuickLoginAsync(login)`, not waited for.
fn cancel_quick_login_later(login: QuickLoginCode) {
    let _ = std::thread::Builder::new().name("quick-login-cancel".into()).spawn(move || {
        let _ = roblox_api::cancel_quick_login(&login);
    });
}

/// Uri.TryCreate(url, Absolute) and its host ending ".rbxcdn.com".
fn rbxcdn(url: &str) -> bool {
    // An absolute address: "scheme://[user@]host[:port][/…]".
    let Some((scheme, rest)) = url.split_once("://") else { return false };
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // Browsers and C#'s Uri read '\' as '/': "https://evil.com\@a.rbxcdn.com/" is evil.com.
    if authority.contains('\\') {
        return false;
    }
    let host = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    let host = host.split(':').next().unwrap_or("").to_ascii_lowercase();
    host.ends_with(".rbxcdn.com")
}

/// C#'s `string.Length` (UTF-16 units).
pub(crate) fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The seconds from now to `until`, never below zero, cut as C#'s `(int)` cuts.
fn seconds_left(until: Stamp) -> i32 {
    until.secs_since(Stamp::utc_now()).max(0.0) as i32
}

struct CheckRun {
    queue: RefCell<VecDeque<Guid>>,
    ok: Cell<i32>,
    failed: Cell<i32>,
    done: Cell<i32>,
    moderated: Cell<i32>,
    refused: RefCell<Vec<Guid>>,
    rejected: RefCell<Vec<(Guid, String)>>,
    accepted_in: RefCell<HashSet<String>>,
    resume_at: Cell<Stamp>,
    newly_banned: RefCell<Vec<Guid>>,
    failed_kind: RefCell<Option<String>>,
    ban_unknown: Cell<i32>,
}

impl CheckRun {
    /// Waits while every check is paused (Roblox asked to slow down).
    async fn wait_resume(&self) {
        let now = Stamp::utc_now();
        let resume = self.resume_at.get();
        if resume > now {
            rt::sleep(Duration::from_secs_f64(resume.secs_since(now))).await;
        }
    }

    /// As long as Roblox says (its reset: 30–50 s seen), not a guess of a few seconds that ran out first.
    fn pause_for(&self, attempt: i32, said: Option<Duration>) {
        let wait = Stamp::utc_now().plus_secs(said.map(|d| d.as_secs_f64()).unwrap_or(4.0 * attempt as f64));
        if wait > self.resume_at.get() {
            self.resume_at.set(wait);
        }
    }

    fn note_kind(&self, ex: &Fail) {
        let mut kind = self.failed_kind.borrow_mut();
        if kind.is_none() {
            *kind = Some(App::log_kind_of(ex));
        }
    }
}

impl App {
    /// Opens the sign-in window; every account signed in there is saved right away (several in a row).
    /// `create`: Roblox's sign-up form instead: each account made there is saved the same way.
    pub async fn add_via_browser(self: &Rc<Self>, create: bool) -> Res<Value> {
        self.vault(|_| ())?;
        let added: Rc<RefCell<Vec<AddResult>>> = Rc::default();
        let on_signed_in: Rc<dyn Fn(String) -> LocalBoxFuture<'static, Res<String>>> = {
            let app = self.clone();
            let added = added.clone();
            Rc::new(move |cookie: String| -> LocalBoxFuture<'static, Res<String>> {
                let app = app.clone();
                let added = added.clone();
                Box::pin(async move {
                    let c = cookie.clone();
                    let user = rt::blocking(move || roblox_api::get_authenticated_user(&c, realm::GLOBAL, &Cancel::new()))
                        .await?
                        .ok_or_else(|| Fail::user(l::t("Roblox didn't accept that sign-in. Please try again.")))?;
                    let result = app.add_or_merge(Account {
                        cookie,
                        user_id: user.id,
                        username: user.name,
                        display_name: user.display_name,
                        cookie_valid: Some(true),
                        last_checked: Some(Stamp::now()),
                        ..Account::default()
                    })?;
                    let text = l::f(if result.merged { "{0} was already here — its sign-in was updated." } else { "{0} saved." }, &[&result.label]);
                    added.borrow_mut().push(result);
                    Ok(text)
                })
            })
        };
        let form = Rc::new(BrowserLoginForm::new(move |cookie| on_signed_in(cookie), create));
        self.show_sign_in_window(SignInWindow::Browser(form)).await;
        let added = added.borrow().clone();
        Ok(json!({ "added": added }))
    }

    /// A sign-in or captcha window, shown over the app until it is closed (by the person, or by `close_sign_in_windows`).
    pub async fn show_sign_in_window(&self, form: SignInWindow) {
        self.accounts.sign_in_windows.borrow_mut().push(form.clone());
        form.show_dialog(self.controller.owner.get()).await;
        self.accounts.sign_in_windows.borrow_mut().retain(|f| !f.same(&form));
    }

    pub fn close_sign_in_windows(&self) {
        let windows = self.accounts.sign_in_windows.borrow().clone();
        for form in windows {
            // Closing already: ShowDialog's return takes it off the list (C# caught InvalidOperation/ObjectDisposed here;
            // the forms' close asks harmlessly then).
            form.close();
        }
    }

    pub async fn quick_login_start(self: &Rc<Self>, realm: &str) -> Res<Value> {
        self.vault(|_| ())?;
        self.quick_login_cancel();
        let start = self.accounts.quick_login_starts.get() + 1;
        self.accounts.quick_login_starts.set(start);
        let r = realm.to_string();
        let login = rt::blocking(move || roblox_api::create_quick_login(&r, &Cancel::new())).await?;
        // Another code was asked for meanwhile (tab switched, "new code" pressed twice): that one is on screen.
        if start != self.accounts.quick_login_starts.get() || self.vault_ref().is_none() {
            log::note("Quick Login: a code came after another was asked for (or the vault locked); it was cancelled.");
            cancel_quick_login_later(login);
            return Ok(Value::Null);
        }
        let login = Rc::new(login);
        *self.accounts.quick_login.borrow_mut() = Some(login.clone());
        let l2 = (*login).clone();
        let qr = rt::blocking(move || roblox_api::get_quick_login_qr(&l2, &Cancel::new())).await?;
        // Again after the picture: a newer code asked for meanwhile is the one polled, so this one isn't shown.
        if start != self.accounts.quick_login_starts.get() {
            return Ok(Value::Null);
        }
        let seconds = seconds_left(login.expires_utc);
        Ok(json!({ "code": login.code, "qr": qr, "seconds": seconds }))
    }

    fn quick_login_is(&self, login: &Rc<QuickLoginCode>) -> bool {
        matches!(&*self.accounts.quick_login.borrow(), Some(l) if Rc::ptr_eq(l, login))
    }

    fn quick_cookie_is(&self, login: &Rc<QuickLoginCode>) -> bool {
        matches!(&*self.accounts.quick_login_cookie.borrow(), Some((l, _)) if Rc::ptr_eq(l, login))
    }

    /// Polled while the window is open; once confirmed on the device the code is traded for a session and saved.
    pub async fn quick_login_poll(self: &Rc<Self>) -> Res<Value> {
        let finishing = self.accounts.quick_login_finishing.get();
        let current = self.accounts.quick_login.borrow().clone();
        let Some(login) = current.filter(|_| !finishing) else {
            return Ok(json!({ "status": if finishing { "adding" } else { "none" } }));
        };
        // A session already in hand only needs its account looked up again: the spent code's status no longer matters.
        let status = if self.quick_cookie_is(&login) {
            QuickLoginStatus { status: "Validated".into(), account_name: None, picture_url: None }
        } else {
            let l2 = (*login).clone();
            rt::blocking(move || roblox_api::get_quick_login_status(&l2, &Cancel::new())).await?
        };
        if !self.quick_login_is(&login) {
            return Ok(json!({ "status": "none" })); // the window asked for a new code meanwhile
        }
        let picture = status.picture_url.clone().filter(|p| rbxcdn(p));
        match status.status.as_str() {
            "Validated" => {
                // Cleared by the finally below, or by a panic or a dropped poll: "adding" mustn't stay for good.
                let finishing = crate::core::base::Busy::set(&self.accounts.quick_login_finishing);
                let mut retry = false;
                let outcome: Res<Value> = async {
                    // The code is spent once traded: a retry after the account lookup failed reuses the session it gave.
                    let held = self.accounts.quick_login_cookie.borrow().clone();
                    let cookie = match held {
                        Some((l, c)) if Rc::ptr_eq(&l, &login) => c,
                        _ => {
                            let l2 = (*login).clone();
                            let c = rt::blocking(move || roblox_api::finish_quick_login(&l2, &Cancel::new())).await?;
                            *self.accounts.quick_login_cookie.borrow_mut() = Some((login.clone(), c.clone()));
                            c
                        }
                    };
                    let (c2, r2) = (cookie.clone(), login.realm.clone());
                    let user = rt::blocking(move || roblox_api::get_authenticated_user(&c2, &r2, &Cancel::new()))
                        .await?
                        .ok_or_else(|| Fail::user(l::t("Roblox didn't accept that sign-in. Please try again.")))?;
                    let added = self.add_or_merge(Account {
                        cookie,
                        realm: login.realm.clone(),
                        user_id: user.id,
                        username: user.name,
                        display_name: user.display_name,
                        cookie_valid: Some(true),
                        last_checked: Some(Stamp::now()),
                        ..Account::default()
                    })?;
                    Ok(json!({ "status": "added", "added": added }))
                }
                .await;
                let result = match outcome {
                    Ok(v) => Ok(v),
                    // A network hiccup right after the person approved: the next poll retries a couple of times rather than making
                    // them start over. A refusal (403, 429) is final: the code may be spent. With the session in hand, only the lookup is repeated.
                    Err(ex)
                        if (App::is_network_error(&ex) || self.quick_cookie_is(&login) && roblox_api::is_expected_error(&ex)) && {
                            let tries = self.accounts.quick_login_finish_tries.get() + 1;
                            self.accounts.quick_login_finish_tries.set(tries);
                            tries < 3
                        } =>
                    {
                        retry = true;
                        log::note(&format!(
                            "Quick Login: finishing failed ({}), try {} of 3; tried again at the next poll.",
                            App::log_kind_of(&ex),
                            self.accounts.quick_login_finish_tries.get()
                        ));
                        Ok(json!({ "status": "adding" }))
                    }
                    Err(ex) => Err(ex),
                };
                // finally
                drop(finishing);
                if !retry {
                    self.accounts.quick_login_finish_tries.set(0);
                    *self.accounts.quick_login_cookie.borrow_mut() = None;
                    if self.quick_login_is(&login) {
                        *self.accounts.quick_login.borrow_mut() = None;
                    }
                }
                result
            }
            "UserLinked" => Ok(json!({ "status": "linked", "name": status.account_name, "picture": picture })),
            "Cancelled" | "Expired" => {
                log::note(&format!("Quick Login: the code was {} on Roblox's side.", status.status.to_lowercase()));
                if self.quick_login_is(&login) {
                    *self.accounts.quick_login.borrow_mut() = None;
                }
                Ok(json!({ "status": status.status.to_lowercase() }))
            }
            _ => Ok(json!({ "status": "waiting", "seconds": seconds_left(login.expires_utc) })),
        }
    }

    pub fn quick_login_cancel(&self) -> Value {
        let finishing = self.accounts.quick_login_finishing.get();
        let current = self.accounts.quick_login.borrow().clone();
        if let Some(login) = current
            && !finishing
        {
            cancel_quick_login_later((*login).clone());
        }
        if !finishing {
            *self.accounts.quick_login.borrow_mut() = None;
            self.accounts.quick_login_finish_tries.set(0);
            *self.accounts.quick_login_cookie.borrow_mut() = None;
        }
        // a code still being asked for is cancelled when it arrives
        self.accounts.quick_login_starts.set(self.accounts.quick_login_starts.get() + 1);
        Value::Null
    }

    /// The password sign-in window closed: drop the half-done sign-in, which holds the password. A late reply to
    /// an earlier sign-in is dropped too (see `login_generation`). Its puzzle window and the browser sign-in window
    /// close with it: Lock and the lock screen come through here, and neither may leave them up.
    pub fn login_cancel(&self) -> Value {
        self.accounts.login_generation.set(self.accounts.login_generation.get() + 1);
        self.close_sign_in_windows();
        // A check being answered on its own (proof-of-work) stops where it is: its answer would send the password again.
        let old = std::mem::take(&mut *self.accounts.login_cts.borrow_mut());
        old.cancel();
        self.drop_login_prompts();
        Value::Null
    }

    /// Forgets what the sign-in in progress holds: its prompts (with the password) and a session not saved yet.
    pub fn drop_login_prompts(&self) {
        *self.accounts.two_step.borrow_mut() = None;
        *self.accounts.captcha.borrow_mut() = None;
        *self.accounts.human_captcha.borrow_mut() = None;
        self.accounts.captcha_used.set(false);
        self.accounts.fresh_puzzles.set(0);
        *self.accounts.login_cookie.borrow_mut() = None;
        self.accounts.two_step_wait_until.set(None);
    }

    pub fn has_login_prompt(&self) -> bool {
        self.accounts.two_step.borrow().is_some() || self.accounts.captcha.borrow().is_some() || self.accounts.human_captcha.borrow().is_some()
    }

    /// One step of a password sign-in. Roblox ending the attempt (wrong password, locked account, too many tries) isn't
    /// an error of this step: the prompt holding the password is dropped so nothing can resend it, and the page goes back to
    /// the username and password with the reason ({ status: "login", error }).
    /// `new_attempt`: a new password sign-in: whatever attempt was going on is dropped, and a late reply to it too.
    pub async fn login_step<F, Fut>(self: &Rc<Self>, step: F, new_attempt: bool) -> Res<Value>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Res<Value>>,
    {
        if new_attempt {
            self.login_cancel();
        }
        let generation = self.accounts.login_generation.get();
        let had_prompt = self.has_login_prompt();
        match step().await {
            Ok(v) => Ok(v),
            // Called off (cancel, lock, the page gone) while a check was being answered: nothing is kept.
            Err(ex)
                if roblox_api::is_fail_kind(&ex, roblox_api::TASK_CANCELED_EXCEPTION) && generation != self.accounts.login_generation.get() =>
            {
                Ok(Value::Null)
            }
            Err(ex) if ex.attempt_over() => {
                if generation != self.accounts.login_generation.get() {
                    return Ok(Value::Null);
                }
                log::note(&format!("Password sign-in: Roblox ended the attempt ({}); back to the start.", App::log_kind_of(&ex)));
                self.drop_login_prompts();
                Ok(json!({ "status": "login", "error": ex.message() }))
            }
            // Too many codes tried: the code screen stays (the prompt is still good), with Verify held back for a minute.
            Err(ex) if ex.rate_limited() && self.accounts.two_step.borrow().is_some() => {
                if generation != self.accounts.login_generation.get() {
                    return Ok(Value::Null);
                }
                self.accounts.two_step_wait_until.set(Some(Stamp::utc_now().plus_secs(TWO_STEP_WAIT.as_secs_f64())));
                log::note("Password sign-in: too many 2-step codes tried; Verify waits a minute.");
                Ok(json!({ "status": "wait", "error": ex.message(), "seconds": TWO_STEP_WAIT.as_secs() as i32 }))
            }
            // The step spent its prompt (puzzle or code answered), then failed: back to username and password with the reason.
            // A session already handed out is kept and only looked up again.
            Err(ex) if matches!(ex, Fail::User(_)) || roblox_api::is_expected_error(&ex) => {
                if generation != self.accounts.login_generation.get() {
                    return Ok(Value::Null);
                }
                let has_cookie = self.accounts.login_cookie.borrow().is_some();
                let has_prompt = self.has_login_prompt();
                log::note(&format!(
                    "Password sign-in: a step failed ({}){}.",
                    App::log_kind_of(&ex),
                    if has_cookie {
                        "; the session is kept, only its lookup is retried"
                    } else if had_prompt && !has_prompt {
                        "; back to the start"
                    } else {
                        ""
                    }
                ));
                if has_cookie {
                    return Ok(json!({ "status": "finish", "error": error_text(&ex) }));
                }
                if had_prompt && !has_prompt {
                    return Ok(json!({ "status": "login", "error": error_text(&ex) }));
                }
                Err(ex)
            }
            Err(ex) => Err(ex),
        }
    }

    pub async fn login_password(self: &Rc<Self>, username: &str, password: &str, realm: &str) -> Res<Value> {
        self.vault(|_| ())?;
        if username.trim().is_empty() || password.is_empty() {
            return Err(Fail::user(l::t("Enter the username and password.")));
        }
        let generation = self.accounts.login_generation.get();
        let (u, p, r) = (username.trim().to_string(), password.to_string(), realm.to_string());
        let result = rt::blocking(move || roblox_api::login_with_password(&u, &p, &r, &Cancel::new())).await?;
        self.handle_login(result, realm, generation).await
    }

    /// Ok(Null) when the sign-in was cancelled (or the vault locked) while Roblox was answering: nothing is kept.
    pub async fn handle_login(self: &Rc<Self>, mut result: LoginResult, realm: &str, generation: i32) -> Res<Value> {
        // Proof-of-work needs no input: solve it and carry on. Answering repeats the password sign-in, so at most once per click,
        // and not once the sign-in was cancelled or Bloxkeep locked (the token stops it mid-solve).
        if let Some(pow) = result.proof_of_work.clone() {
            if generation != self.accounts.login_generation.get() {
                return Ok(Value::Null);
            }
            let ct = self.accounts.login_cts.borrow().clone();
            result = rt::blocking(move || roblox_api::finish_proof_of_work_login(&pow, &ct)).await?;
        }
        // Which 2-step methods the account has decides the tabs, or that only the phone or browser can finish it.
        if let Some(asked) = result.two_step.clone() {
            result = rt::blocking(move || roblox_api::with_two_step_methods(&asked, &Cancel::new())).await?;
        }
        if generation != self.accounts.login_generation.get() {
            return Ok(Value::Null);
        }
        if result.proof_of_work.is_some() {
            log::note("Password sign-in: Roblox didn't accept the solved proof-of-work check.");
            return Err(Fail::user(l::t("Roblox didn't accept the sign-in check. Please try again.")));
        }
        // A check only Roblox's site or app can show ends this attempt, with the reason, like a refusal does.
        if let Some(blocked) = result.blocked.clone() {
            log::note("Password sign-in: Roblox asked for a check only its own site or app can show.");
            return Err(RobloxApiException { attempt_over: true, ..RobloxApiException::new(blocked) }.into());
        }
        if let Some(captcha) = result.captcha.clone() {
            log::note("Password sign-in: Roblox asked for a puzzle (Arkose).");
            *self.accounts.captcha.borrow_mut() = Some(Rc::new(captcha));
            self.accounts.captcha_used.set(false);
            return Ok(json!({ "status": "captcha" }));
        }
        if let Some(human) = result.human_captcha.clone() {
            log::note("Password sign-in: Roblox asked for a puzzle (human check).");
            *self.accounts.human_captcha.borrow_mut() = Some(Rc::new(human));
            self.accounts.captcha_used.set(false);
            return Ok(json!({ "status": "captcha" }));
        }
        if let Some(prompt) = result.two_step.clone() {
            log::note(&format!("Password sign-in: Roblox asked for 2-step verification ({}).", prompt.methods.join(", ")));
            // autofill: the account's authenticator key is saved here, so the page fills its code in (Verify stays the person's).
            let autofill = self.authenticator_account(&prompt).is_some();
            let media = prompt.methods.first().cloned();
            let methods = prompt.methods.clone();
            *self.accounts.two_step.borrow_mut() = Some(Rc::new(RefCell::new(prompt)));
            // The first method is the one the code was sent by, or Roblox's preferred one.
            return Ok(json!({ "status": "twostep", "media": media, "methods": methods, "autofill": autofill }));
        }
        let cookie = result.cookie.clone().ok_or_else(|| Fail::other("NullReferenceException: the sign-in gave no cookie"))?;
        self.save_login(cookie, realm, generation).await
    }

    /// Shows the captcha for the user to solve; on success the sign-in carries on (it may still ask for 2-step).
    pub async fn login_captcha(self: &Rc<Self>) -> Res<Value> {
        self.vault(|_| ())?;
        let generation = self.accounts.login_generation.get();
        // A puzzle belongs to one sign-in attempt; once its window was closed, ask Roblox for a fresh one.
        let captcha = self.accounts.captcha.borrow().clone();
        let human = self.accounts.human_captcha.borrow().clone();
        let username = captcha.as_ref().map(|c| c.username.clone()).or_else(|| human.as_ref().map(|h| h.username.clone()));
        if self.accounts.captcha_used.get()
            && let Some(username) = username
        {
            // Each fresh puzzle sends the password again: a few per attempt, then the attempt ends.
            if self.accounts.fresh_puzzles.get() >= MAX_FRESH_PUZZLES {
                log::note(&format!("Password sign-in: {MAX_FRESH_PUZZLES} fresh puzzles asked already; the attempt ends."));
            }
            if self.accounts.fresh_puzzles.get() >= MAX_FRESH_PUZZLES {
                return Err(RobloxApiException {
                    attempt_over: true,
                    ..RobloxApiException::new(l::t("Roblox keeps asking for a new puzzle. Sign in with your phone instead, or try again later."))
                }
                .into());
            }
            self.accounts.fresh_puzzles.set(self.accounts.fresh_puzzles.get() + 1);
            let password = match (&captcha, &human) {
                (Some(c), _) => c.password.clone(),
                (None, Some(h)) => h.password.clone(),
                (None, None) => String::new(),
            };
            let realm = match (&captcha, &human) {
                (Some(c), _) => c.realm.clone(),
                (None, Some(h)) => h.realm.clone(),
                (None, None) => String::new(),
            };
            *self.accounts.captcha.borrow_mut() = None;
            *self.accounts.human_captcha.borrow_mut() = None;
            self.accounts.captcha_used.set(false);
            let (u, p, r) = (username, password, realm.clone());
            let result = rt::blocking(move || roblox_api::login_with_password(&u, &p, &r, &Cancel::new())).await?;
            let again = self.handle_login(result, &realm, generation).await?;
            // Roblox may let it through without a puzzle this time, or ask for 2-step instead.
            if generation != self.accounts.login_generation.get()
                || (self.accounts.captcha.borrow().is_none() && self.accounts.human_captcha.borrow().is_none())
            {
                return Ok(again);
            }
        }
        let human = self.accounts.human_captcha.borrow().clone();
        if let Some(human) = human {
            let redemption = self.show_captcha(CaptchaForm::human(&human.realm, &human.captcha_challenge_id)).await;
            // Cancelled, locked or the page gone while the puzzle was open: nothing more goes to Roblox.
            let same = matches!(&*self.accounts.human_captcha.borrow(), Some(h) if Rc::ptr_eq(h, &human));
            if generation != self.accounts.login_generation.get() || self.vault_ref().is_none() || !same {
                return Ok(Value::Null);
            }
            let Some(redemption) = redemption else {
                log::note("Password sign-in: the puzzle window was closed unsolved.");
                self.accounts.captcha_used.set(true);
                return Ok(json!({ "status": "captcha", "closed": true }));
            };
            *self.accounts.human_captcha.borrow_mut() = None;
            let h2 = (*human).clone();
            let result = rt::blocking(move || roblox_api::finish_human_captcha_login(&h2, &redemption, &Cancel::new())).await?;
            return self.handle_login(result, &human.realm, generation).await;
        }
        let captcha = self.accounts.captcha.borrow().clone();
        let Some(prompt) = captcha else {
            return Err(Fail::user(l::t("Start the sign-in again.")));
        };
        let r = prompt.realm.clone();
        let key = rt::blocking(move || roblox_api::get_login_captcha_key(&r, &Cancel::new())).await?;
        if generation != self.accounts.login_generation.get() {
            return Ok(Value::Null);
        }
        let token = self.show_captcha(CaptchaForm::arkose(&prompt.realm, &key, &prompt.blob)).await;
        let same = matches!(&*self.accounts.captcha.borrow(), Some(c) if Rc::ptr_eq(c, &prompt));
        if generation != self.accounts.login_generation.get() || self.vault_ref().is_none() || !same {
            return Ok(Value::Null);
        }
        let Some(token) = token else {
            log::note("Password sign-in: the puzzle window was closed unsolved.");
            self.accounts.captcha_used.set(true);
            return Ok(json!({ "status": "captcha", "closed": true }));
        };
        *self.accounts.captcha.borrow_mut() = None;
        let p2 = (*prompt).clone();
        let result = rt::blocking(move || roblox_api::finish_captcha_login(&p2, &token, &Cancel::new())).await?;
        self.handle_login(result, &prompt.realm, generation).await
    }

    /// What the solved puzzle handed back, or None when the window was closed first.
    pub async fn show_captcha(&self, form: CaptchaForm) -> Option<String> {
        let form = Rc::new(form);
        self.show_sign_in_window(SignInWindow::Captcha(form.clone())).await;
        form.token()
    }

    pub async fn login_two_step(self: &Rc<Self>, method: &str, code: &str) -> Res<Value> {
        self.vault(|_| ())?;
        let current = self.accounts.two_step.borrow().clone();
        let Some(prompt) = current else {
            return Err(Fail::user(l::t("Start the sign-in again.")));
        };
        let snapshot = prompt.borrow().clone();
        if code.trim().is_empty() && snapshot.verification_token.is_none() {
            return Err(Fail::user(l::t("Enter the verification code.")));
        }
        self.wait_for_two_step()?;
        let generation = self.accounts.login_generation.get();
        // A wrong code throws and keeps the prompt, so it can be typed again; anything else moves the sign-in on.
        // Once the code was accepted, a retry after a network hiccup skips it (it is spent) and only finishes.
        let chosen = App::two_step_method(&snapshot, method);
        let code = code.trim().to_string();
        let mut p = snapshot;
        let (p, answer) = rt::blocking(move || {
            let r = roblox_api::verify_two_step(&mut p, &chosen, &code, &Cancel::new());
            (p, r)
        })
        .await;
        // The prompt is one object in C#: the verification token the call kept is the held prompt's.
        prompt.borrow_mut().verification_token = p.verification_token.clone();
        let result = match answer {
            Ok(r) => r,
            Err(ex) => {
                if p.verification_token.is_some() && !App::is_network_error(&ex) {
                    // Accepted, then refused on the way: nothing is left to answer on this screen.
                    let same = matches!(&*self.accounts.two_step.borrow(), Some(t) if Rc::ptr_eq(t, &prompt));
                    if generation == self.accounts.login_generation.get() && same {
                        *self.accounts.two_step.borrow_mut() = None;
                    }
                }
                return Err(ex);
            }
        };
        if generation != self.accounts.login_generation.get() {
            return Ok(Value::Null);
        }
        *self.accounts.two_step.borrow_mut() = None;
        self.handle_login(result, &p.realm, generation).await
    }

    pub async fn login_two_step_send(self: &Rc<Self>, method: &str) -> Res<Value> {
        self.vault(|_| ())?;
        let current = self.accounts.two_step.borrow().clone();
        let Some(prompt) = current else {
            return Err(Fail::user(l::t("Start the sign-in again.")));
        };
        let snapshot = prompt.borrow().clone();
        let chosen = App::two_step_method(&snapshot, method);
        if chosen == "authenticator" {
            return Err(Fail::user(l::t("Enter the code from the authenticator app.")));
        }
        self.wait_for_two_step()?;
        rt::blocking(move || roblox_api::send_two_step_code(&snapshot, &chosen, &Cancel::new())).await?;
        Ok(json!({ "status": "sent" }))
    }

    /// The method the page picked, if it is one this account was offered; otherwise the first offered.
    pub fn two_step_method(prompt: &TwoStepPrompt, method: &str) -> String {
        if prompt.methods.iter().any(|m| m == method) { method.to_string() } else { prompt.methods.first().cloned().unwrap_or_default() }
    }

    pub fn wait_for_two_step(&self) -> Res<()> {
        if let Some(until) = self.accounts.two_step_wait_until.get() {
            let left = until.secs_since(Stamp::utc_now());
            if left > 0.0 {
                return Err(Fail::user(l::f("Too many tries. Try again in {0} seconds.", &[&(left.ceil() as i32)])));
            }
        }
        Ok(())
    }

    /// Offline, a timeout or Roblox down: the request may never have been answered, so it's worth repeating.
    pub fn is_network_error(ex: &Fail) -> bool {
        roblox_api::is_fail_kind(ex, roblox_api::HTTP_REQUEST_EXCEPTION) || roblox_api::is_fail_kind(ex, roblox_api::TASK_CANCELED_EXCEPTION) || ex.server_error()
    }

    /// "Try again" after the account couldn't be looked up: the session Roblox handed out is used, no new sign-in.
    pub async fn login_finish(self: &Rc<Self>) -> Res<Value> {
        self.vault(|_| ())?;
        let held = self.accounts.login_cookie.borrow().clone();
        let Some((cookie, realm)) = held else {
            return Err(Fail::user(l::t("Start the sign-in again.")));
        };
        self.save_login(cookie, &realm, self.accounts.login_generation.get()).await
    }

    pub async fn save_login(self: &Rc<Self>, cookie: String, realm: &str, generation: i32) -> Res<Value> {
        *self.accounts.login_cookie.borrow_mut() = Some((cookie.clone(), realm.to_string()));
        let (c2, r2) = (cookie.clone(), realm.to_string());
        let user = rt::blocking(move || roblox_api::get_authenticated_user(&c2, &r2, &Cancel::new())).await?;
        // Locked or cancelled meanwhile: the account isn't saved.
        if generation != self.accounts.login_generation.get() {
            return Ok(Value::Null);
        }
        let Some(user) = user else {
            log::note("Sign-in: Roblox didn't accept the session it just handed out; nothing saved.");
            *self.accounts.login_cookie.borrow_mut() = None; // Roblox's answer, not a hiccup: the session is no good
            return Err(Fail::user(l::t("Roblox didn't accept that sign-in. Please try again.")));
        };
        let added = self.add_or_merge(Account {
            cookie,
            realm: realm.to_string(),
            user_id: user.id,
            username: user.name,
            display_name: user.display_name,
            cookie_valid: Some(true),
            last_checked: Some(Stamp::now()),
            ..Account::default()
        })?;
        *self.accounts.login_cookie.borrow_mut() = None;
        Ok(json!({ "status": "added", "added": added }))
    }

    pub async fn add_via_cookie(self: &Rc<Self>, a: &Value) -> Res<Value> {
        self.vault(|_| ())?;
        let cookie = roblox_api::normalize_cookie(&str_of(a, "cookie"));
        if cookie.is_empty() {
            return Err(Fail::user(l::t("Paste the .ROBLOSECURITY cookie.")));
        }
        if !roblox_api::cookie_text_valid(&cookie) {
            return Err(Fail::user(roblox_api::cookie_text_error()));
        }
        let c2 = cookie.clone();
        let user = rt::blocking(move || roblox_api::get_authenticated_user(&c2, realm::GLOBAL, &Cancel::new())).await?;
        if user.is_none() {
            log::note("Add by cookie: Roblox didn't accept the pasted cookie.");
        }
        let Some(user) = user else {
            return Err(Fail::user(l::t("The cookie is invalid or has expired.")));
        };
        let result = self.add_or_merge(Account {
            cookie,
            user_id: user.id,
            username: user.name,
            display_name: user.display_name,
            alias: str_of(a, "alias").trim().to_string(),
            group: str_of(a, "group").trim().to_string(),
            password: str_of(a, "password"),
            note: str_of(a, "note").trim().to_string(),
            cookie_valid: Some(true),
            last_checked: Some(Stamp::now()),
            ..Account::default()
        })?;
        Ok(serde_json::to_value(result)?)
    }

    /// Signed in again: auto-rejoin comes back on if a sign-in Roblox ended was what switched it off.
    pub fn signed_in_again(account: &mut Account) {
        if !account.rejoin_off_by_sign_out {
            return;
        }
        account.rejoin_off_by_sign_out = false;
        account.auto_rejoin = true;
    }

    /// Roblox accepted a call made with the account's sign-in: not signed out, and auto-rejoin back on if a sign-out switched it off.
    pub fn sign_in_accepted(account: &mut Account) {
        account.cookie_valid = Some(true);
        App::signed_in_again(account);
    }

    /// An account already in the list just gets its cookie refreshed. Roblox and Roblox VNG count user ids apart,
    /// so the same id in the other realm is another account.
    pub fn add_or_merge(self: &Rc<Self>, incoming: Account) -> Res<AddResult> {
        self.vault(|_| ())?;
        // A sign-in finishing behind the lock screen (its window was just closed, or a reply came late) saves nothing.
        if self.auto_lock.screen_locked.get() {
            log::note("Accounts: a sign-in finished behind the lock screen; nothing saved.");
            return Err(Fail::user(l::t("Bloxkeep is locked.")));
        }
        let realm = realm::normalize(Some(&incoming.realm));
        let (account, merged) = self.vault_mut(|d| {
            let existing = d.accounts.iter_mut().find(|a| a.user_id != 0 && a.user_id == incoming.user_id && realm::normalize(Some(&a.realm)) == realm);
            match existing {
                Some(existing) => {
                    existing.cookie = incoming.cookie.clone();
                    existing.realm = incoming.realm.clone();
                    existing.username = incoming.username.clone();
                    existing.display_name = incoming.display_name.clone();
                    existing.cookie_valid = incoming.cookie_valid;
                    existing.last_checked = incoming.last_checked;
                    if incoming.cookie_valid == Some(true) {
                        App::signed_in_again(existing);
                    }
                    if !incoming.password.is_empty() {
                        existing.password = incoming.password.clone();
                    }
                    if !incoming.alias.is_empty() {
                        existing.alias = incoming.alias.clone();
                    }
                    if !incoming.group.is_empty() {
                        existing.group = incoming.group.clone();
                    }
                    if !incoming.note.is_empty() {
                        existing.note = incoming.note.clone();
                    }
                    (existing.clone(), true)
                }
                None => {
                    d.accounts.push(incoming.clone());
                    (incoming.clone(), false)
                }
            }
        })?;
        log::note(&format!(
            "Accounts: account {} {} ({realm}).",
            &account.id.to_string()[..8],
            if merged { "signed in again (already in the list)" } else { "added" }
        ));
        self.persist();
        self.push_state();
        self.spawn_load_avatars(vec![account.user_id]);
        self.refresh_presence_soon(Duration::from_secs(2));
        Ok(AddResult { id: account.id, label: self.name_text(&account), merged })
    }

    /// Imported cookies may be old, so verify them right away and report the result.
    pub async fn check_imported(self: &Rc<Self>, ids: Vec<Guid>) {
        let run: Res<()> = async {
            while self.controller.checking.get() {
                rt::sleep(Duration::from_millis(1000)).await;
            }
            let Some(vault) = self.vault_ref() else {
                log::note("Import: imported accounts not checked, Bloxkeep was locked.");
                return Ok(());
            };
            self.check_accounts(&ids).await?;
            let (valid, expired) = {
                let d = vault.data();
                let checked: Vec<&Account> = d.accounts.iter().filter(|a| ids.contains(&a.id)).collect();
                (checked.iter().filter(|a| a.cookie_valid == Some(true)).count(), checked.iter().filter(|a| a.cookie_valid == Some(false)).count())
            };
            self.toast(&l::f("Imported accounts checked: {0} valid, {1} expired.", &[&valid, &expired]), "ok");
            Ok(())
        }
        .await;
        // Vault locked or another check started meanwhile; the user can check again from the menu.
        if let Err(Fail::User(_)) = run {
            log::note("Import: imported accounts not checked (locked, or another check started).");
        }
    }

    pub async fn update_account_command(self: &Rc<Self>, a: &Value) -> Res<Value> {
        let vault = self.require_vault_ref()?;
        let account = self.account(id_of(a))?;
        let id = account.id;
        // Checked before anything changes: a key with a typo is refused with what's wrong, and nothing is saved.
        // "" removes the key; no field at all keeps it.
        let typed_key = opt_str(a, "authenticator").map(|k| k.trim().to_string());
        let authenticator = match &typed_key {
            Some(k) if !k.is_empty() => Some(App::parse_authenticator(k)?),
            _ => None,
        };
        let mut user = None;
        let cookie = opt_str(a, "cookie").map(|raw| roblox_api::normalize_cookie(&raw));
        if let Some(c) = cookie.as_ref().filter(|c| !c.is_empty() && **c != account.cookie) {
            if !roblox_api::cookie_text_valid(c) {
                return Err(Fail::user(roblox_api::cookie_text_error()));
            }
            let (c2, r2) = (c.clone(), account.realm.clone());
            let found = rt::blocking(move || roblox_api::get_authenticated_user(&c2, &r2, &Cancel::new())).await?;
            let short = account.id.to_string()[..8].to_string();
            if found.is_none() {
                log::note(&format!("Edit account: Roblox didn't accept the new cookie for account {short}; nothing saved."));
            }
            let Some(found) = found else {
                return Err(Fail::user(l::t("The new cookie is invalid or has expired.")));
            };
            // Another user's cookie would turn this account into them, with its alias, password and authenticator (review,
            // 2026-10-30): refused. One saved without a user yet takes whoever it is.
            if account.user_id > 0 && found.id != account.user_id {
                log::note(&format!("Edit account: the new cookie of account {short} signs in another user; refused."));
            }
            if account.user_id > 0 && found.id != account.user_id {
                return Err(Fail::user(l::t("This cookie signs in another account. Add it as a new account instead.")));
            }
            // Locked (and maybe reopened) meanwhile: this account object belongs to the old vault.
            if !self.is_vault(&vault) {
                return Err(Fail::user(l::t("Bloxkeep was locked.")));
            }
            user = Some(found);
        }

        let alias = opt_str(a, "alias");
        let group = str_of(a, "group").trim().to_string();
        let note = str_of(a, "note").trim().to_string();
        let password = opt_str(a, "password");
        let new_user = user.clone();
        // C# changes the account object it holds; one deleted meanwhile is simply not saved (no error, as there).
        let _ = self.update_account(id, |account| {
            // No alias sent while names are hidden: the form can't show it then, so it stays as it was.
            if let Some(alias) = alias {
                account.alias = alias.trim().to_string();
            }
            account.group = group;
            account.note = note;
            if let Some(password) = password {
                account.password = password;
            }
            if typed_key.is_some() {
                account.authenticator = authenticator;
            }
            if let Some(user) = new_user {
                account.cookie = cookie.clone().unwrap_or_default();
                account.user_id = user.id;
                account.username = user.name;
                account.display_name = user.display_name;
                account.cookie_valid = Some(true);
                account.last_checked = Some(Stamp::now());
                App::signed_in_again(account);
            }
        });
        if let Some(user) = user {
            self.spawn_load_avatars(vec![user.id]);
        }
        self.persist();
        self.push_state();
        Ok(json!(true))
    }

    pub fn set_use_roblox_config(&self, ids: &[Guid], enabled: bool) -> Res<Value> {
        let changed = self.vault_mut(|d| {
            let mut changed = 0;
            for account in d.accounts.iter_mut().filter(|a| ids.contains(&a.id)) {
                account.use_roblox_config = enabled;
                changed += 1;
            }
            changed
        })?;
        self.persist();
        self.push_state();
        Ok(json!(changed))
    }

    /// Each account's own limit in Roblox, from its next opening; 0 takes it off.
    pub fn set_fps_cap(&self, ids: &[Guid], fps: i32) -> Res<Value> {
        if fps != 0 && !roblox_game_settings::FPS_CHOICES.contains(&fps) {
            log::note(&format!("Accounts: FPS cap {fps} isn't one of the choices; nothing changed."));
            return Ok(json!(0));
        }
        let changed = self.vault_mut(|d| {
            let mut changed = 0;
            for account in d.accounts.iter_mut().filter(|a| ids.contains(&a.id)) {
                account.fps_cap = fps;
                changed += 1;
            }
            changed
        })?;
        self.persist();
        self.push_state();
        Ok(json!(changed))
    }

    /// Marks the accounts as ones the person plays on (or not); their open windows are let go of at once.
    pub fn set_main(self: &Rc<Self>, ids: &[Guid], on: bool) -> Res<Value> {
        let changed = self.vault_mut(|d| {
            let mut changed = 0;
            for account in d.accounts.iter_mut().filter(|a| ids.contains(&a.id)) {
                account.is_main = on;
                changed += 1;
            }
            changed
        })?;
        self.persist();
        // Its window back to normal RAM and cores now, not at the next scan.
        for pid in self.main_pids() {
            if self.memory.low_memory_priority.borrow_mut().remove(&pid) {
                let _ = roblox_processes::set_memory_priority(pid, false);
            }
            if self.memory.ram_limited.borrow_mut().remove(&pid).is_some() {
                let app = self.clone();
                rt::spawn(async move {
                    let _ = app.limit_ram_in_turn(pid, None).await;
                });
            }
        }
        self.step("SplitCores(_corePids)", || {
            let pids = self.cores.core_pids.borrow().clone();
            self.split_cores(&pids);
            Ok(())
        });
        self.push_state();
        Ok(json!(changed))
    }

    /// The main accounts' Roblox windows (their processes); remembered while the vault is locked, as the windows go on.
    pub fn main_pids(&self) -> HashSet<i32> {
        if let Some(vault) = self.vault_ref() {
            let pids: HashSet<i32> = {
                let running = self.controller.running.borrow();
                let d = vault.data();
                d.accounts.iter().filter(|a| a.is_main).filter_map(|a| running.get(&a.id).map(|r| r.0)).collect()
            };
            *self.accounts.main_pids.borrow_mut() = pids;
        }
        self.accounts.main_pids.borrow().clone()
    }

    pub fn set_auto_rejoin(&self, ids: &[Guid], enabled: bool) -> Res<Value> {
        let accounts: Vec<Account> = self.vault(|d| d.accounts.iter().filter(|a| ids.contains(&a.id)).cloned().collect())?;
        let mut changed = 0;
        for mut account in accounts {
            account.auto_rejoin = enabled;
            account.rejoin_off_by_sign_out = false; // the person's own choice from now on
            let _ = self.update_account(account.id, |a| {
                a.auto_rejoin = enabled;
                a.rejoin_off_by_sign_out = false;
            });
            if enabled {
                self.controller.rejoins_in_a_row.borrow_mut().remove(&account.id);
            } else {
                // a client whose preset turns auto-rejoin on, and a planned restart, still go ahead
                let entries: Vec<(Guid, bool, Option<Guid>)> =
                    self.controller.rejoin_queue.borrow().iter().map(|r| (r.account_id, r.restart, r.preset_id)).collect();
                let keep: Vec<bool> = entries.iter().map(|(who, restart, preset)| !(*who == account.id && !*restart && !self.rejoin_on(&account, *preset))).collect();
                let mut i = 0;
                self.controller.rejoin_queue.borrow_mut().retain(|_| {
                    let k = keep.get(i).copied().unwrap_or(true);
                    i += 1;
                    k
                });
            }
            changed += 1;
        }
        self.persist();
        self.push_state();
        Ok(json!(changed))
    }

    pub fn reveal(&self, id: Guid, field: &str) -> Res<Value> {
        let account = self.account(id)?;
        match field {
            "password" => Ok(json!(account.password)),
            _ => Err(Fail::user(l::t("Invalid field."))),
        }
    }

    pub fn delete_accounts(&self, ids: &[Guid]) -> Res<Value> {
        let set: HashSet<Guid> = ids.iter().copied().collect();
        let removed = self.vault_mut(|d| {
            let before = d.accounts.len();
            d.accounts.retain(|a| !set.contains(&a.id));
            before - d.accounts.len()
        })?;
        self.close_websites(Some(ids));
        self.vault_mut(|d| {
            // Gone from presets too; a preset left empty gets its schedule off rather than reporting "no accounts left" daily.
            for preset in &mut d.settings.presets {
                let before = preset.account_ids.len();
                preset.account_ids.retain(|i| !set.contains(i));
                if preset.account_ids.len() < before
                    && preset.account_ids.is_empty()
                    && let Some(schedule) = preset.schedule.as_mut()
                    && schedule.enabled
                {
                    schedule.enabled = false;
                }
            }
            // …and from macros; one aimed at accounts picked by hand and left with none is turned off (its triggers would fire into
            // nothing, and saving it again would fail with "Pick at least one account").
            for m in &mut d.settings.macros {
                let before = m.account_ids.len();
                m.account_ids.retain(|i| !set.contains(i));
                if m.account_ids.len() < before && m.targets == "accounts" && m.account_ids.is_empty() {
                    m.enabled = false;
                }
            }
            d.settings.launch_order.retain(|i| !set.contains(i));
            d.discord.summary_accounts.retain(|i| !set.contains(i));
        })?;
        for id in &set {
            self.controller.running.borrow_mut().shift_remove(id);
            self.schedule.stop_closing.borrow_mut().remove(id);
            self.controller.rejoins_in_a_row.borrow_mut().remove(id);
            self.presets.restarts.borrow_mut().remove(id);
            self.controller.offline_strikes.borrow_mut().remove(id);
            self.controller.user_closed.borrow_mut().remove(id);
            self.end_play(*id, None, Stamp::utc_now()); // its stay ends now, not at the next lock
            self.forget_scheduled_open(*id);
        }
        self.controller.pending.borrow_mut().retain(|p| !set.contains(&p.account_id));
        self.launching.late.borrow_mut().retain(|l| !set.contains(&l.account_id));
        self.launching.handed_over.borrow_mut().retain(|(l, _)| !set.contains(&l.account_id));
        self.controller.close_when_claimed.borrow_mut().retain(|l| !set.contains(&l.0.account_id));
        self.controller.rejoin_queue.borrow_mut().retain(|r| !set.contains(&r.account_id));
        log::note(&format!("Accounts: {removed} of {} account(s) deleted.", set.len()));
        self.persist();
        self.push_state();
        Ok(json!(removed))
    }

    /// Asks Roblox who each saved sign-in belongs to (the read-only call the website makes on every page). A rejected
    /// sign-in means Roblox ended that session (password change, "log out everywhere", security). Six at a time; when Roblox
    /// asks to slow down, all checks pause.
    pub async fn check_accounts(self: &Rc<Self>, ids: &[Guid]) -> Res<Value> {
        let vault = self.require_vault_ref()?;
        if self.controller.checking.get() {
            return Err(Fail::user(l::t("Already checking — please wait.")));
        }
        let accounts: Vec<Guid> = {
            let d = vault.data();
            if ids.is_empty() { d.accounts.iter().map(|a| a.id).collect() } else { d.accounts.iter().filter(|a| ids.contains(&a.id)).map(|a| a.id).collect() }
        };
        let total = accounts.len();

        // Cleared at the finally below, or if this is dropped or panics first (check_imported waits on it).
        let checking = crate::core::base::Busy::set(&self.controller.checking);
        self.push_state();
        // Roblox and Roblox VNG sign in apart: one accepted on Roblox says nothing of VNG's service (review, 2026-10-30).
        let st = CheckRun {
            queue: RefCell::new(accounts.iter().copied().collect()),
            ok: Cell::new(0),
            failed: Cell::new(0),
            done: Cell::new(0),
            moderated: Cell::new(0),
            refused: RefCell::new(Vec::new()),
            rejected: RefCell::new(Vec::new()),
            accepted_in: RefCell::new(HashSet::new()),
            resume_at: Cell::new(Stamp::utc_now()),
            newly_banned: RefCell::new(Vec::new()),
            failed_kind: RefCell::new(None),
            ban_unknown: Cell::new(0),
        };
        let mut bad = 0;
        let find = |id: Guid| -> Option<Account> { vault.data().accounts.iter().find(|a| a.id == id).cloned() };

        let st_ref = &st;
        let vault_ref = &vault;
        let find_ref = &find;
        // Every continuation runs on the core's thread, so the queue and counters need no locking.
        let work = || async move {
            let (st, vault, find) = (st_ref, vault_ref, find_ref);
            while self.is_vault(vault) {
                let next = st.queue.borrow_mut().pop_front();
                let Some(id) = next else { break };
                let mut attempt = 1;
                loop {
                    st.wait_resume().await;
                    // C# goes on with the account object even when it was deleted meanwhile; here it ends with it.
                    let Some(account) = find(id) else { break };
                    let cookie = account.cookie.clone();
                    let (c2, r2) = (cookie.clone(), account.realm.clone());
                    match rt::blocking(move || roblox_api::get_authenticated_user(&c2, &r2, &Cancel::new())).await {
                        Ok(user) => {
                            let mut d = vault.data();
                            let Some(a) = d.accounts.iter_mut().find(|a| a.id == id) else { break };
                            if a.cookie != cookie {
                                break; // signed in again meanwhile: the old cookie's answer says nothing about the new one
                            }
                            // A rejection is marked below, once Roblox's sign-in service is known to be up.
                            match user {
                                None => st.rejected.borrow_mut().push((id, cookie)),
                                Some(user) => {
                                    st.accepted_in.borrow_mut().insert(a.realm.clone());
                                    App::sign_in_accepted(a);
                                    a.user_id = user.id;
                                    a.username = user.name;
                                    a.display_name = user.display_name;
                                    st.ok.set(st.ok.get() + 1);
                                }
                            }
                            a.last_checked = Some(Stamp::now());
                        }
                        Err(ex) if ex.rate_limited() && attempt < 4 => {
                            st.pause_for(attempt, ex.retry_after());
                            attempt += 1;
                            continue;
                        }
                        Err(ex) if matches!(ex, Fail::Roblox { .. }) && !ex.server_error() && !ex.rate_limited() => {
                            // Roblox answered and said no (403: a warning waiting to be read, a lock…): not the network.
                            st.refused.borrow_mut().push(id);
                            if ex.moderated() {
                                st.moderated.set(st.moderated.get() + 1);
                            }
                        }
                        Err(ex) if roblox_api::is_expected_error(&ex) => {
                            st.failed.set(st.failed.get() + 1);
                            st.note_kind(&ex);
                        }
                        Err(ex) => return Err(ex),
                    }
                    break;
                }
                // A ban, from the public profile (no cookie); no clear answer leaves it as it was. Rate limits pause every check.
                let mut attempt = 1;
                while self.is_vault(vault) {
                    st.wait_resume().await;
                    let answer = match find(id).filter(|_| self.is_vault(vault)) {
                        Some(account) => self.check_ban(&account).await,
                        None => Ok(None),
                    };
                    match answer {
                        Ok(Some(true)) => st.newly_banned.borrow_mut().push(id),
                        Ok(_) => {}
                        Err(ex) if ex.rate_limited() => {
                            st.pause_for(attempt, None);
                            if attempt < 3 {
                                attempt += 1;
                                continue;
                            }
                        }
                        Err(ex) if roblox_api::is_expected_error(&ex) => {
                            st.ban_unknown.set(st.ban_unknown.get() + 1); // no clear answer: left as it was
                            st.note_kind(&ex);
                        }
                        Err(ex) => return Err(ex),
                    }
                    break;
                }
                st.done.set(st.done.get() + 1);
                self.progress(Some(&l::f("Checking sign-ins… {0}/{1}", &[&st.done.get(), &total])));
            }
            Ok::<(), Fail>(())
        };

        let run: Res<()> = async {
            self.progress(Some(&l::f("Checking sign-ins… {0}/{1}", &[&0, &total])));
            let results = futures::future::join_all((0..CHECK_PARALLELISM.min(total)).map(|_| work())).await;
            for r in results {
                r?;
            }
            // One accepted of the same Roblox means its sign-in service is up, so a rejection is a sign-out. With none, an outage
            // answers 401 to everyone: each is marked only if Roblox accepts another account now (ConfirmSignedOutAsync), else not
            // checked. Signed in again meanwhile (another cookie now): the old one's rejection says nothing, so it's left alone.
            let rejected = st.rejected.borrow().clone();
            for (id, cookie) in rejected {
                if !self.is_vault(&vault) {
                    break;
                }
                let Some(account) = find(id) else { continue };
                if account.cookie != cookie {
                    continue;
                }
                let up_here = st.accepted_in.borrow().contains(&account.realm);
                if up_here && let Some(a) = vault.data().accounts.iter_mut().find(|a| a.id == id) {
                    a.cookie_valid = Some(false);
                }
                if up_here || self.confirm_signed_out(id, &Cancel::new(), false).await {
                    bad += 1;
                } else {
                    st.failed.set(st.failed.get() + 1);
                }
            }
            Ok(())
        }
        .await;
        // finally
        drop(checking);
        self.progress(None);
        self.persist();
        self.push_state();
        run?;

        let (ok, failed, refused, moderated, ban_unknown) = (st.ok.get(), st.failed.get(), st.refused.borrow().clone(), st.moderated.get(), st.ban_unknown.get());
        let newly_banned = st.newly_banned.borrow().clone();
        let failed_kind = st.failed_kind.borrow().clone();
        let locked = !self.is_vault(&vault);
        if bad > 0 || failed > 0 || !refused.is_empty() || !newly_banned.is_empty() || ban_unknown > 0 || locked {
            log::note(&format!(
                "Sign-in check: {total} asked, {ok} fine, {bad} signed out, {} refused ({moderated} with a notice), {failed} unanswered, {} newly banned, {ban_unknown} ban checks unanswered{}{}.",
                refused.len(),
                newly_banned.len(),
                failed_kind.map(|k| format!(" ({k})")).unwrap_or_default(),
                if locked { "; stopped, the vault was locked" } else { "" }
            ));
        }
        let (user_ids, banned_now, banned_count): (Vec<i64>, Vec<Account>, usize) = {
            let d = vault.data();
            let checked: Vec<&Account> = d.accounts.iter().filter(|a| accounts.contains(&a.id)).collect();
            (
                checked.iter().map(|a| a.user_id).collect(),
                newly_banned.iter().filter_map(|id| d.accounts.iter().find(|a| a.id == *id).cloned()).collect(),
                checked.iter().filter(|a| a.banned == Some(true)).count(),
            )
        };
        self.spawn_load_avatars(user_ids);
        if !newly_banned.is_empty() && self.is_vault(&vault) {
            self.notify_banned(banned_now);
        }
        Ok(json!({
            "ok": ok, "bad": bad, "failed": failed, "refused": refused.len(), "moderated": moderated, "refusedIds": refused,
            "banned": banned_count,
        }))
    }

    pub fn copy(&self, id: Guid, field: &str) -> Res<Value> {
        let account = self.account(id)?;
        let now = Stamp::utc_now();
        let (value, secret, missing) = match field {
            "username" => (account.username.clone(), false, "This account has no saved username."),
            "password" => (account.password.clone(), true, "This account has no saved password."),
            // The authenticator code of the moment (never the key); off the clipboard again like a password.
            "totp" => (account.authenticator.as_ref().map(|k| totp::code(k, now)).unwrap_or_default(), true, "This account has no authenticator key."),
            // The client's process id, for other tools that pick a window by it.
            "pid" => (
                self.controller.running.borrow().get(&id).map(|r| r.0.to_string()).unwrap_or_default(),
                false,
                "This account has no Roblox window open.",
            ),
            _ => return Err(Fail::user(l::t("Invalid field."))),
        };
        if value.is_empty() {
            return Err(Fail::user(l::t(missing)));
        }
        if !ui::copy_to_clipboard(&value, secret) {
            return Err(Fail::user(l::t("Couldn't access the clipboard; try again.")));
        }
        let seconds = match (field, account.authenticator.as_ref()) {
            ("totp", Some(key)) => json!(totp::seconds_left(key, now)),
            _ => Value::Null,
        };
        Ok(json!({ "secret": secret, "seconds": seconds }))
    }

    /// `_ = LoadAvatarsAsync(ids)`.
    pub fn spawn_load_avatars(self: &Rc<Self>, user_ids: Vec<i64>) {
        let app = self.clone();
        rt::spawn(async move {
            let _ = app.load_avatars(user_ids).await;
        });
    }

    pub async fn load_avatars(self: &Rc<Self>, user_ids: Vec<i64>) -> Res<()> {
        let missing: Vec<i64> = {
            let urls = self.controller.avatar_urls.borrow();
            let mut seen = HashSet::new();
            user_ids.into_iter().filter(|id| *id > 0 && !urls.contains_key(id) && seen.insert(*id)).collect()
        };
        let run: Res<()> = async {
            if missing.is_empty() {
                return Ok(());
            }
            let m = missing.clone();
            let asked_in = self.vault_ref();
            let got = rt::blocking(move || roblox_api::get_headshot_urls(m, &Cancel::new())).await?;
            // Locked (the pictures cleared) or another vault opened meanwhile: these belong to the one that was open.
            if !asked_in.as_ref().is_some_and(|v| self.is_vault(v)) {
                return Ok(());
            }
            {
                let mut urls = self.controller.avatar_urls.borrow_mut();
                for (id, url) in got {
                    urls.insert(id, url);
                }
            }
            if self.accounts.avatars_failing.get() {
                log::note("Avatars: recovered.");
            }
            self.accounts.avatars_failing.set(false);
            self.push_state();
            Ok(())
        }
        .await;
        let result = match run {
            Err(ex) if roblox_api::is_expected_error(&ex) => {
                if !self.accounts.avatars_failing.get() {
                    log::note(&format!("Avatars: {} not loaded ({}); initials show until the next try.", missing.len(), App::log_kind_of(&ex)));
                }
                self.accounts.avatars_failing.set(true);
                Ok(())
            }
            other => other,
        };
        // finally
        if !self.controller.boot_avatars_done.get() {
            self.controller.boot_avatars_done.set(true);
            self.advance_boot(true);
        }
        result
    }

    /// Moves the loading screen to the first step still running: status (30 → 80 %), avatars (80 → 100 %), done.
    pub fn advance_boot(self: &Rc<Self>, push: bool) {
        if self.controller.boot.borrow().is_none() {
            return;
        }
        let next = if !self.controller.boot_status_done.get() {
            Some(BootProgress::new("status", 0.3))
        } else if !self.controller.boot_avatars_done.get() {
            Some(BootProgress::new("avatars", 0.8))
        } else {
            None
        };
        let done = next.is_none();
        *self.controller.boot.borrow_mut() = next;
        if push {
            self.push_state();
        }
        if done {
            let app = self.clone();
            rt::spawn(async move { app.settled().await });
        }
    }

    /// A moment after loading is done (or a new page came to an open vault): the page hears "settled" and prepares
    /// what its unopened tabs show (Statistics, preset icons, their game pictures) instead of on first open (0.3-1.4 s from
    /// here). A sleeping page hears it on waking.
    pub async fn settled(self: &Rc<Self>) {
        let vault = self.vault_ref();
        rt::sleep(Duration::from_millis(SETTLED_DELAY_MS)).await;
        if let Some(vault) = vault
            && self.is_vault(&vault)
            && self.controller.boot.borrow().is_none()
            && self.controller.page_attached.get()
        {
            self.emit("settled", Value::Null);
        }
    }

    /// `_vault` as it is now (C# kept the reference to compare it later).
    pub fn vault_ref(&self) -> Option<Arc<Vault>> {
        self.controller.vault.borrow().clone()
    }

    /// `_vault == vault`.
    pub fn is_vault(&self, vault: &Arc<Vault>) -> bool {
        matches!(&*self.controller.vault.borrow(), Some(v) if Arc::ptr_eq(v, vault))
    }

    /// RequireVault(), keeping the vault it returned.
    pub fn require_vault_ref(&self) -> Res<Arc<Vault>> {
        self.vault(|_| ())?;
        self.vault_ref().ok_or_else(|| Fail::user(l::t("Bloxkeep is locked.")))
    }
}
