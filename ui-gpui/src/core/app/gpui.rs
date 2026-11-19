// AppController.Gpui.cs: what only the GPUI interface (1.1, Rust) may ask: it calls some of Roblox's services itself now
// (the core moves to Rust bit by bit, the owner's call on 2026-11-09), and the ones that answer only a signed-in account
// (the charts' games, an account's friends) need an account's sign-in.
//
// PORT: in the C# this came over the shell's own pipe to bloxkeep-ui, which kept it in memory only, and the web page's
// messages never reached it (ShellForm handed only the pipe's messages to HandleGpuiMessageAsync, which answered
// "robloxSignIn" itself and passed every other message to HandleMessageAsync). Here the interface is in the same
// process: "robloxSignIn" is an ordinary command, answered by `handle` before the lock screen rule, as the pipe's
// handler answered it before HandleMessageAsync's.
use serde_json::{Value, json};

use super::App;
use super::controller::{JsonKind, has, parse_guid};
use crate::core::{models::Account, roblox_api::realm, vault::Vault};

/// AppController.Gpui.cs declares no instance fields.
#[derive(Default)]
pub struct GpuiState {}

impl App {
    /// Keep the VNG sign-in in the core and expose only the selected panel's display data.
    pub async fn vng_panel(&self, id: crate::core::base::Guid, panel: String) -> crate::core::base::Res<Value> {
        use crate::core::{roblox_api::vng_panel, rt};
        let vault = self.held_vault();
        let account = self.vng_account(id)?;
        let value = rt::blocking(move || vng_panel::load(&panel, account.user_id, &account.cookie)).await?;
        self.same_vng_vault(vault, id)?;
        Ok(value)
    }

    /// vngSet { id, what, value }: one of a VNG account's privacy or trade settings (vng_panel::set), its sign-in
    /// kept in the core as vngPanel keeps it.
    pub async fn vng_set(&self, id: crate::core::base::Guid, what: String, value: String) -> crate::core::base::Res<Value> {
        use crate::core::{roblox_api::vng_panel, rt};
        let vault = self.held_vault();
        let account = self.vng_account(id)?;
        rt::blocking(move || vng_panel::set(&what, &value, &account.cookie)).await?;
        self.same_vng_vault(vault, id)?;
        Ok(Value::Null)
    }

    /// The VNG account `id`, with a sign-in to use.
    fn vng_account(&self, id: crate::core::base::Guid) -> crate::core::base::Res<Account> {
        use crate::core::{base::Fail, l};
        let account = self.account(id)?;
        if account.realm != realm::VNG || account.user_id <= 0 {
            return Err(Fail::user(l::t("Account not found.")));
        }
        if account.cookie.is_empty() || account.cookie_valid == Some(false) {
            return Err(Fail::user(l::t("Sign in again")));
        }
        Ok(account)
    }

    /// The vault and the account are still the ones asked of (a vault switched meanwhile gets no answer).
    fn same_vng_vault(&self, vault: Option<std::sync::Arc<Vault>>, id: crate::core::base::Guid) -> crate::core::base::Res<()> {
        use crate::core::{base::Fail, l};
        if vault.is_none() || !super::presets::same_vault(&vault, &self.held_vault()) || self.find_account(id).is_none() {
            return Err(Fail::user(l::t("Account not found.")));
        }
        Ok(())
    }

    /// robloxSignIn { id }: that account's sign-in (Friends looks through one account); without it, the app's pick: the
    /// main account when it has a good sign-in, else any roblox.com account's (as the app's own calls use). Its ids go
    /// with it, so the window knows whose it holds.
    pub fn roblox_sign_in(&self, a: &Value) -> Value {
        let wanted = if has(a, "id", JsonKind::String) { a["id"].as_str().and_then(parse_guid) } else { None };
        // Behind the idle lock screen no sign-in goes out, as no other answer does.
        let vault = if self.auto_lock.screen_locked.get() { None } else { self.controller.vault.borrow().clone() };
        let from = vault.and_then(|v| {
            let data = v.data();
            // OrderByDescending(a => a.IsMain).First(): the first main account, else the first one (min_by_key keeps the first).
            data.accounts
                .iter()
                .filter(|a| a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.realm == realm::GLOBAL && wanted.is_none_or(|w| a.id == w))
                .min_by_key(|a| !a.is_main)
                .cloned()
        });
        json!({
            "cookie": from.as_ref().map(|a| a.cookie.clone()),
            "userId": from.as_ref().map(|a| a.user_id).unwrap_or(0),
            "id": from.as_ref().map(|a| a.id),
        })
    }
}
