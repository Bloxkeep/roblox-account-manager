//! Whether a new Roblox window can be a launch's, by the account its log says is signed in. (AppController.ClaimCheck.cs)
use super::App;
use crate::core::models::Account;

#[derive(Default)]
pub struct ClaimCheckState {}

impl App {
    /// False when the window's log names another signed-in user than `account` (its join report, see
    /// RobloxLogWatcher::user_id_of): a late launch mustn't take a window started by hand as someone else (audit R11).
    /// True while the log names nobody yet (not in a game, signed out since, no log): the launch's own checks decide.
    /// A Global and a Roblox VNG account may share the number; the log can't tell those apart, so the number is all that
    /// counts.
    pub fn claim_user_matches(&self, pid: i32, account: &Account) -> bool {
        match self.controller.logs.borrow().user_id_of(pid) {
            None => true,
            Some(user) => user == account.user_id,
        }
    }
}
