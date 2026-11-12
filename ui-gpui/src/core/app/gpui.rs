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
use crate::core::roblox_api::realm;

/// AppController.Gpui.cs declares no instance fields.
#[derive(Default)]
pub struct GpuiState {}

impl App {
    /// robloxSignIn { id }: that account's sign-in (Friends looks through one account); without it, the app's pick: the
    /// main account when it has a good sign-in, else any roblox.com account's (as the app's own calls use). Its ids go
    /// with it, so the window knows whose it holds.
    pub fn roblox_sign_in(&self, a: &Value) -> Value {
        let wanted = if has(a, "id", JsonKind::String) { a["id"].as_str().and_then(parse_guid) } else { None };
        let vault = self.controller.vault.borrow().clone();
        let from = vault.and_then(|v| {
            let data = v.data();
            let mut matching: Vec<_> = data
                .accounts
                .iter()
                .filter(|a| a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.realm == realm::GLOBAL && wanted.is_none_or(|w| a.id == w))
                .cloned()
                .collect();
            // OrderByDescending(a => a.IsMain): stable, the main accounts first.
            matching.sort_by_key(|a| !a.is_main);
            matching.into_iter().next()
        });
        json!({
            "cookie": from.as_ref().map(|a| a.cookie.clone()),
            "userId": from.as_ref().map(|a| a.user_id).unwrap_or(0),
            "id": from.as_ref().map(|a| a.id),
        })
    }
}
