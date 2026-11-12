// AppController.Mini.cs: what the mini window (MiniWindow) shows and does: the clients open now, from what the process
// check already tracks (no snapshot of its own), and the page's close actions.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use serde_json::{Value, json};

use super::App;
use super::controller::{MB, PendingLaunch};
use crate::core::base::Guid;
use crate::core::roblox_processes::{self, PlayerProcess};
use crate::core::{l, log, prefs, themes};

/// `state`: "game" (in a game, or getting in), "menu" (Roblox's home screen) or "disconnected".
#[derive(Clone, Debug)]
pub struct MiniClient {
    pub id: Guid,
    pub label: String,
    pub game: String,
    pub pid: i32,
    pub ram_mb: i64,
    pub state: String,
}

/// The instance fields AppController.Mini.cs declares.
#[derive(Default)]
pub struct MiniState {
    /// RAM per client as of the last process check (every 1.5–5 s).
    pub client_ram: RefCell<HashMap<i32, i64>>,
    /// The window shows or hides the mini window (None toggles) and says whether it is open afterwards.
    pub set_mini_open: RefCell<Option<Rc<dyn Fn(Option<bool>) -> bool>>>,
    /// Each client's own memory (commit) as of the last process check: what Settings' "Restart a heavy window" reads.
    pub client_commit: RefCell<HashMap<i32, i64>>,
}

impl App {
    pub fn note_client_memory(&self, players: &[PlayerProcess]) {
        *self.mini.client_ram.borrow_mut() = players.iter().map(|p| (p.pid, p.ram())).collect();
        *self.mini.client_commit.borrow_mut() = players.iter().map(|p| (p.pid, if p.private > 0 { p.private } else { p.ram() })).collect();
    }

    /// Every Roblox client on this PC as of the last process check, Bloxkeep's or not.
    pub fn roblox_count(&self) -> i32 {
        self.mini.client_ram.borrow().len() as i32
    }

    /// The clients Bloxkeep opened that are still running, in the launch order (as the page and monitor list them); empty
    /// while locked (the screen too).
    pub fn running_clients(&self) -> Vec<MiniClient> {
        let mut list = Vec::new();
        let Some(vault) = self.open() else {
            return list;
        };
        let running: Vec<(Guid, (i32, Rc<PendingLaunch>))> = self.controller.running.borrow().iter().map(|(k, v)| (*k, v.clone())).collect();
        for (id, (pid, launch)) in running {
            let Some(account) = self.find_account(id) else {
                continue;
            };
            let state = match self.connection(&account, pid, &launch) {
                "menu" => "menu",
                "disconnected" => "disconnected",
                _ => "game",
            };
            let game = match state {
                "menu" => l::t("No game"),
                "disconnected" => l::t("Disconnected"),
                _ => self.session_game_name(pid, &launch),
            };
            let ram = self.mini.client_ram.borrow().get(&pid).copied().unwrap_or(0) / MB;
            list.push(MiniClient { id, label: self.name_text(&account), game, pid, ram_mb: ram, state: state.into() });
        }
        let order = App::effective_launch_order(&vault.data());
        let rank: HashMap<Guid, usize> = order.into_iter().enumerate().map(|(i, id)| (id, i)).collect();
        list.sort_by_key(|c| rank.get(&c.id).copied().unwrap_or(i32::MAX as usize));
        list
    }

    /// The game's name the way the page shows it (see `session_game_dto`), as text.
    pub fn session_game_name(&self, pid: i32, launch: &PendingLaunch) -> String {
        let logged = self.controller.logs.borrow().state_of(pid).and_then(|s| s.place_id);
        let opened = launch.target.as_ref().map(|t| t.place_id);
        let name_of = |id: i64| self.controller.games.borrow().get(&id).cloned().flatten().map(|g| g.name).filter(|n| !n.is_empty());
        let logged_named = logged.is_some_and(|l| name_of(l).is_some());
        let place_id = if logged_named || opened.is_none() { logged.or(opened) } else { opened };
        let Some(id) = place_id else {
            return l::t("No game");
        };
        name_of(id).unwrap_or_else(|| l::f("Place {0}", &[&id]))
    }

    /// The mini window's page: the clients (empty while locked), every Roblox on the PC, whether "Close all" asks
    /// first, the theme and the language.
    pub fn mini_message(&self) -> Value {
        let unlocked = self.unlocked();
        let clients: Vec<Value> = if unlocked { self.running_clients() } else { Vec::new() }
            .into_iter()
            .map(|c| json!({ "id": c.id, "name": c.label, "game": c.game, "pid": c.pid, "ramMb": c.ram_mb, "state": c.state }))
            .collect();
        let ask_close_all = !prefs::lock().skip_confirm.contains("closeAll");
        json!({
            "unlocked": unlocked,
            "clients": clients,
            "robloxCount": if unlocked { self.roblox_count() } else { 0 },
            "askCloseAll": ask_close_all,
            "theme": themes::current().id,
            "customTheme": App::custom_theme_dto(),
            "language": l::language(),
        })
    }

    /// The mini window's Show: the account's window brought forward in its turn.
    pub fn show_client(&self, id: Guid) {
        let run = self.controller.running.borrow().get(&id).map(|r| r.0);
        if self.unlocked()
            && let Some(pid) = run
        {
            roblox_processes::focus_in_turn(pid);
        } else {
            let short = &id.to_string()[..8];
            log::note(&format!("Mini window: Show did nothing for account {short} ({}).", if self.unlocked() { "its Roblox isn't open" } else { "locked" }));
        }
    }

    pub fn close_client(&self, id: Guid) {
        if self.unlocked() {
            let _ = self.close_accounts(&[id], false);
        } else {
            log::note("Mini window: Close did nothing (locked).");
        }
    }

    /// Closes every Roblox on this PC, like the page's "Close all"; the number of clients closed. Not while locked,
    /// the lock screen too (a click on the mini window's button just before it came up).
    pub fn close_all_clients(&self) -> i32 {
        if !self.unlocked() {
            log::note("Mini window: Close all did nothing (locked).");
            return 0;
        }
        // CloseAll's answer is the number closed.
        let closed = self.close_all(false);
        serde_json::to_value(closed).ok().and_then(|v| v.as_i64()).unwrap_or(0) as i32
    }

    /// The page's mini-window button; the window pushes the new state once it has shown or hidden it.
    pub fn toggle_mini(&self) -> Value {
        let set = self.mini.set_mini_open.borrow().clone();
        match set {
            Some(set) => {
                set(None);
            }
            None => log::note("Mini window: the page's button did nothing (no window to show it)."),
        }
        Value::Null
    }
}
