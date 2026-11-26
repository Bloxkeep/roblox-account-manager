// AppController.Website.cs: www.roblox.com in Bloxkeep's own browser window (forms/website_form.rs), one tab per
// account, each its own private sign-in: Open website signs a tab in as a saved account; Open roblox.com (or the +)
// opens a blank one whose sign-in Save to Bloxkeep adds. A session cookie goes only into that tab's in-memory profile,
// never the person's own browser.
use std::{
    cell::RefCell,
    collections::HashSet,
    rc::{Rc, Weak},
};

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;

use super::App;
use crate::core::{
    base::{Fail, Guid, Res, Stamp},
    forms::{
        website_form::{WebsiteAccount, WebsiteForm},
        website_tab::WebsiteTab,
    },
    l, log,
    models::Account,
    roblox_api::{self, RobloxUser, realm},
    rt,
};

/// Roblox's Violations & Appeals page: a ban is appealed there, signed in as the banned account (DevForum,
/// "Update on Account Bans, Alt Detection, and Appeals", 2026-11).
pub const APPEAL_URL: &str = "https://www.roblox.com/report-appeals";
pub const SETTINGS_URL: &str = "https://www.roblox.com/my/account";

pub(crate) fn game_servers_url(place: i64) -> Res<String> {
    if place <= 0 {
        return Err(Fail::user(l::t("Invalid Place ID.")));
    }
    Ok(format!("https://www.roblox.com/games/{place}#!/game-instances"))
}

#[derive(Default)]
pub struct WebsiteState {
    pub web_window: RefCell<Option<WebsiteForm>>,
    /// Every website tab open, saved accounts' and blank ones.
    pub web_tabs: RefCell<Vec<WebsiteTab>>,
    /// The tab open for a saved account, by account id.
    pub websites: RefCell<IndexMap<Guid, WebsiteTab>>,
}

impl App {
    /// The selected account uses Roblox's own private-server checkout in its isolated website tab.
    pub fn open_game_servers(self: &Rc<Self>, id: Guid, place: i64) -> Res<Value> {
        let url = game_servers_url(place)?;
        self.open_website(id, Some(&url))
    }

    /// The website window, unless it was closed (C#: `_webWindow is { IsDisposed: false }`).
    fn open_web_window(&self) -> Option<WebsiteForm> {
        self.website.web_window.borrow().clone().filter(|w| !w.is_disposed())
    }

    /// `url`: a page on roblox.com to open instead of its home (Appeal); an open tab goes there.
    pub fn open_website(self: &Rc<Self>, id: Guid, url: Option<&str>) -> Res<Value> {
        let account = self.account(id)?;
        if realm::normalize(Some(&account.realm)) == realm::VNG {
            return Err(Fail::user(l::t("The website can't be opened for Roblox VNG accounts yet.")));
        }
        let open = self.website.websites.borrow().get(&id).cloned();
        if let Some(open) = open.as_ref().filter(|t| !t.is_closed())
            && let Some(window) = self.open_web_window()
        {
            if let Some(url) = url {
                open.go(url);
            }
            window.select_tab(&open);
            App::show_web_window(&window);
            return Ok(Value::Null);
        }
        // Its old tab outlived the window: closed, so the account doesn't end up with two.
        if let Some(old) = open.filter(|t| !t.is_closed()) {
            old.close_now();
            self.website.web_tabs.borrow_mut().retain(|t| *t != old);
        }
        let tab = self.add_website_tab(Some(id), Some(self.name_text(&account)), account.cookie.clone(), self.tab_avatar(&account), url.map(str::to_string));
        self.website.websites.borrow_mut().insert(id, tab);
        Ok(Value::Null)
    }

    /// A blank tab at roblox.com's sign-in page: nothing is saved unless the person presses Save to Bloxkeep.
    pub fn open_blank_website(self: &Rc<Self>) -> Res<Value> {
        self.vault(|_| ())?;
        self.add_website_tab(None, None, String::new(), None, None);
        Ok(Value::Null)
    }

    pub fn add_website_tab(self: &Rc<Self>, id: Option<Guid>, name: Option<String>, cookie: String, avatar: Option<String>, url: Option<String>) -> WebsiteTab {
        let me: Weak<App> = Rc::downgrade(self);
        let on_session = {
            let me = me.clone();
            move |id: Guid, cookie: String| -> LocalBoxFuture<'static, bool> {
                let me = me.clone();
                Box::pin(async move {
                    match me.upgrade() {
                        Some(app) => app.website_session_changed(id, cookie).await,
                        None => true,
                    }
                })
            }
        };
        // The tab hands itself to Save (C#: the closure reads `tab`, set once it is made): it is the account's tab from
        // then on.
        let made: Rc<RefCell<Option<WebsiteTab>>> = Rc::new(RefCell::new(None));
        let on_save = {
            let me = me.clone();
            let made = made.clone();
            move |session: String, user: RobloxUser| -> Res<(Guid, String, String)> {
                let tab = made.borrow().clone();
                match (me.upgrade(), tab) {
                    (Some(app), Some(tab)) => app.save_website_account(&tab, &session, &user),
                    _ => Err(Fail::user(l::t("Bloxkeep is locked."))),
                }
            }
        };
        let tab = WebsiteTab::new(id, name.as_deref(), &cookie, on_session, on_save);
        *made.borrow_mut() = Some(tab.clone());
        tab.set_avatar(avatar);
        tab.set_start_url(url);
        {
            let me = me.clone();
            let closed = tab.clone();
            tab.on_closed(move || {
                let Some(app) = me.upgrade() else { return };
                app.website.web_tabs.borrow_mut().retain(|t| *t != closed);
                app.website.websites.borrow_mut().retain(|_, t| *t != closed);
            });
        }
        self.website.web_tabs.borrow_mut().push(tab.clone());
        let window = self.web_window();
        window.add_tab(&tab);
        App::show_web_window(&window);
        tab
    }

    pub fn web_window(self: &Rc<Self>) -> WebsiteForm {
        if let Some(open) = self.open_web_window() {
            return open;
        }
        let me: Weak<App> = Rc::downgrade(self);
        let window = WebsiteForm::new();
        // The + and Ctrl+T: a blank tab, while Bloxkeep is unlocked (locking closes the window anyway).
        {
            let me = me.clone();
            window.set_new_blank_tab(move || {
                let unlocked = me.upgrade().filter(|app| app.controller.vault.borrow().is_some());
                if let Some(app) = unlocked {
                    app.add_website_tab(None, None, String::new(), None, None);
                }
            });
        }
        {
            let me = me.clone();
            window.set_accounts(move || me.upgrade().map(|app| app.website_accounts()).unwrap_or_default());
        }
        {
            let me = me.clone();
            window.set_switch_account(move |tab: WebsiteTab, id: Guid| {
                if let Some(app) = me.upgrade() {
                    app.switch_website_account(&tab, id);
                }
            });
        }
        window.place_near(self.controller.owner.get());
        {
            let me = me.clone();
            let closed = window.clone();
            window.on_form_closed(move || {
                let Some(app) = me.upgrade() else { return };
                let mine = app.website.web_window.borrow().as_ref().is_some_and(|w| *w == closed);
                if mine {
                    *app.website.web_window.borrow_mut() = None;
                }
            });
        }
        *self.website.web_window.borrow_mut() = Some(window.clone());
        window
    }

    /// The account picker's list: the saved accounts in the list's order, as the app names them; not Roblox VNG's (the
    /// website can't open for them).
    pub fn website_accounts(&self) -> Vec<WebsiteAccount> {
        let accounts: Vec<Account> = self
            .vault(|d| d.accounts.iter().filter(|a| realm::normalize(Some(&a.realm)) != realm::VNG).cloned().collect())
            .unwrap_or_default();
        accounts.iter().map(|a| WebsiteAccount { id: a.id, name: self.name_text(a), avatar: self.tab_avatar(a) }).collect()
    }

    /// The account picker: this tab signs in as that account, on the page it shows; when the account has a tab open
    /// already, that one is shown instead (one tab per account).
    pub fn switch_website_account(self: &Rc<Self>, tab: &WebsiteTab, id: Guid) {
        if self.controller.vault.borrow().is_none() || tab.is_closed() || tab.account_id() == Some(id) {
            return;
        }
        let Some(account) = self.find_account(id) else { return };
        if realm::normalize(Some(&account.realm)) == realm::VNG {
            return;
        }
        let open = self.website.websites.borrow().get(&id).cloned();
        if let Some(open) = open.filter(|o| !o.is_closed() && o != tab)
            && let Some(window) = self.open_web_window()
        {
            window.select_tab(&open);
            open.focus_page();
            return;
        }
        {
            let mut websites = self.website.websites.borrow_mut();
            websites.retain(|_, t| t != tab);
            websites.insert(id, tab.clone());
        }
        let (name, cookie, avatar) = (self.name_text(&account), account.cookie.clone(), self.tab_avatar(&account));
        // C#: `_ = tab.SwitchToAsync(...)`: started on the tab's window thread, not waited for.
        tab.switch_to(id, &name, &cookie, avatar);
        tab.focus_page();
    }

    pub fn show_web_window(window: &WebsiteForm) {
        if !window.visible() {
            window.show();
        }
        if window.is_minimized() {
            window.restore();
        }
        window.activate();
    }

    /// Save to Bloxkeep on a blank tab: the account signed in there joins the list (or its saved sign-in is renewed).
    pub fn save_website_account(self: &Rc<Self>, tab: &WebsiteTab, cookie: &str, user: &RobloxUser) -> Res<(Guid, String, String)> {
        self.vault(|_| ())?;
        let result = self.add_or_merge(Account {
            cookie: cookie.to_string(),
            user_id: user.id,
            username: user.name.clone(),
            display_name: user.display_name.clone(),
            cookie_valid: Some(true),
            last_checked: Some(Stamp::now()),
            ..Account::default()
        })?;
        // The account's tab from now on, unless it has one open already.
        self.website.websites.borrow_mut().entry(result.id).or_insert_with(|| tab.clone());
        let message = l::f(if result.merged { "{0} was already here — its sign-in was updated." } else { "{0} saved." }, &[&result.label]);
        Ok((result.id, result.label.clone(), message))
    }

    /// The site holds another session cookie than the tab was given (Roblox renewed it, or the person signed in again
    /// there). Saved only when Roblox says it's the same account. False when Roblox couldn't be asked (no internet): the
    /// tab asks again later.
    pub async fn website_session_changed(self: &Rc<Self>, id: Guid, cookie: String) -> bool {
        let vault = self.held_vault();
        if vault.is_none() {
            return true;
        }
        let Some(account) = self.find_account(id) else { return true };
        if account.cookie == cookie {
            return true;
        }
        let user: Option<RobloxUser> = {
            let (cookie, realm) = (cookie.clone(), account.realm.clone());
            match rt::blocking(move || roblox_api::get_authenticated_user(&cookie, &realm, &crate::core::base::Cancel::new())).await {
                Ok(u) => u,
                Err(ex) if roblox_api::is_expected_error(&ex) => return false,
                Err(ex) => {
                    // PORT: an unexpected failure ended the C# task (the tab saw the exception); here it is logged and
                    // treated as handled.
                    log::write(&ex);
                    return true;
                }
            }
        };
        // Read again: C#'s `account` is the vault's own object, changed meanwhile by anyone.
        let Some(now) = self.find_account(id) else { return true };
        if !super::presets::same_vault(&self.held_vault(), &vault) || now.cookie == cookie {
            return true;
        }
        if user.as_ref().is_none_or(|u| u.id != now.user_id) {
            log::note("Website tab: the site holds a session of another account, or one that doesn't work; the saved one is kept.");
            return true;
        }
        self.update_account(id, |a| {
            a.cookie = cookie.clone();
            a.cookie_valid = Some(true);
            a.last_checked = Some(Stamp::now());
            App::signed_in_again(a);
        });
        self.persist();
        self.push_state();
        log::note("Website tab: Roblox gave the site a new session for the account; saved.");
        true
    }

    /// Closes all website tabs with their window (lock, quit; a blank tab's sign-in goes too), or these accounts' tabs.
    pub fn close_websites(&self, ids: Option<&[Guid]>) {
        let Some(ids) = ids else {
            let window = self.website.web_window.borrow().clone();
            if let Some(window) = window {
                window.close_now();
            }
            let tabs: Vec<WebsiteTab> = self.website.web_tabs.borrow().clone();
            for tab in tabs {
                tab.close_now();
            }
            return;
        };
        let which: HashSet<Guid> = ids.iter().copied().collect();
        let tabs: Vec<WebsiteTab> = self.website.websites.borrow().iter().filter(|(k, _)| which.contains(k)).map(|(_, t)| t.clone()).collect();
        for tab in tabs {
            tab.close_now();
            if let Some(window) = self.open_web_window() {
                // C#: `_ = window.CloseTabAsync(tab)`.
                rt::spawn(async move {
                    window.close_tab(&tab).await;
                });
            }
        }
    }

    /// The tab titles follow a new alias, Hide account names and the language.
    pub fn rename_websites(&self) {
        let tabs: Vec<WebsiteTab> = self.website.web_tabs.borrow().iter().filter(|t| !t.is_closed()).cloned().collect();
        for tab in tabs {
            let account = tab.account_id().and_then(|id| self.find_account(id));
            tab.set_name(account.as_ref().map(|a| self.name_text(a)).as_deref());
            tab.set_avatar(account.as_ref().and_then(|a| self.tab_avatar(a)));
        }
        if let Some(window) = self.open_web_window() {
            window.refresh_top();
        }
    }

    /// The account's avatar on its tab and in the tab's account list. Shown while names are hidden too, as the main
    /// window keeps the pictures then: without them two accounts shortened to "K…" can't be told apart in the list.
    pub fn tab_avatar(&self, account: &Account) -> Option<String> {
        self.controller.avatar_urls.borrow().get(&account.user_id).cloned()
    }
}
