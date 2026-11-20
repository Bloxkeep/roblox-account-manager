// AppController.Hotkeys.cs: hotkeys from any app (the window registers them with Windows): Ctrl+Alt+H hides every Roblox
// window, or shows them again when any is hidden; Ctrl+Alt+A arranges them on the main screen; Ctrl+Alt+M minimizes them
// all, or opens them again when all are; Ctrl+Alt+N brings the next account's window to the front; Ctrl+Alt+B opens
// Bloxkeep (the window handles that one). Settings › Bloxkeep › This app turns them off.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::App;
use super::macros::{HOTKEY_END_RECORDING, HOTKEY_MACRO_FIRST, HOTKEY_STOP_MACROS};
use crate::core::{input_turn, log, roblox_processes, rt};

pub const HOTKEY_HIDE: i32 = 1;
pub const HOTKEY_ARRANGE: i32 = 2;
pub const HOTKEY_MINIMIZE: i32 = 3;
pub const HOTKEY_NEXT: i32 = 4;
pub const HOTKEY_BLOXKEEP: i32 = 5;

/// The instance fields AppController.Hotkeys.cs declares.
#[derive(Default)]
pub struct HotkeysState {
    /// The window registers or drops the hotkeys again (the switch changed).
    pub hotkeys_changed: RefCell<Option<Rc<dyn Fn()>>>,
    /// The hotkeys another app already holds ("Ctrl+Alt+H"…), as the window found when registering them.
    pub hotkeys_taken: RefCell<Vec<String>>,
    /// The hotkeys left unregistered because an installed keyboard types a character with them (AltGr is Ctrl+Alt) and
    /// its keyboard hook was refused.
    pub hotkeys_typing: RefCell<Vec<String>>,
}

pub fn hotkey_name(id: i32) -> String {
    match id {
        HOTKEY_HIDE => "Ctrl+Alt+H".into(),
        HOTKEY_ARRANGE => "Ctrl+Alt+A".into(),
        HOTKEY_MINIMIZE => "Ctrl+Alt+M".into(),
        HOTKEY_NEXT => "Ctrl+Alt+N".into(),
        HOTKEY_STOP_MACROS => "Ctrl+Alt+0".into(),
        HOTKEY_END_RECORDING => "F8".into(),
        _ if (HOTKEY_MACRO_FIRST..HOTKEY_MACRO_FIRST + 9).contains(&id) => format!("Ctrl+Alt+{}", id - HOTKEY_MACRO_FIRST + 1),
        _ => format!("#{id}"),
    }
}

/// The Roblox windows on screen (not the hidden ones), the accounts' in the list's order first, then any other.
/// `order`: each client's place in the account list (from `window_order`, read on the core's thread).
pub fn shown_roblox_windows_by(pids: Option<&[i32]>, order: &HashMap<i32, usize>) -> Vec<(i32, isize)> {
    let pids: Vec<i32> = match pids {
        Some(p) => p.to_vec(),
        None => roblox_processes::get_player_pids(),
    };
    let windows = roblox_processes::visible_windows(&pids);
    let mut sorted = pids.clone();
    sorted.sort_by_key(|pid| (order.get(pid).copied().unwrap_or(i32::MAX as usize), *pid));
    sorted
        .into_iter()
        .map(|pid| (pid, windows.get(&pid).map(|w| roblox_processes::main_window_of(w)).unwrap_or(0)))
        .filter(|(_, w)| *w != 0 && !roblox_processes::is_hidden(*w))
        .collect()
}

/// Ctrl+Alt+M: every Roblox window minimized (resting: the RAM limit then applies), or, when all of them already
/// are, opened again without taking the keyboard from the app in front.
pub fn minimize_or_restore_all_by(pids: Option<&[i32]>, order: &HashMap<i32, usize>) {
    let shown = shown_roblox_windows_by(pids, order);
    if shown.iter().any(|w| !roblox_processes::is_minimised(w.1)) {
        for w in &shown {
            roblox_processes::minimise(w.1);
        }
    } else {
        for w in &shown {
            roblox_processes::restore_quietly(w.1);
        }
    }
}

/// Ctrl+Alt+N: the window after the one in front (the first when none is), opened if minimized, in front.
pub fn focus_next_window_by(pids: Option<&[i32]>, order: &HashMap<i32, usize>) {
    let shown = shown_roblox_windows_by(pids, order);
    if shown.is_empty() {
        return;
    }
    let foreground = roblox_processes::foreground_pid();
    let front = shown.iter().position(|w| w.0 == foreground).map(|i| i as i64).unwrap_or(-1);
    roblox_processes::focus(shown[((front + 1) as usize) % shown.len()].0);
}

impl App {
    pub fn on_hotkey(self: &Rc<Self>, id: i32) {
        if self.controller.disposed.get() {
            return;
        }
        // Each one noted: "hotkeys stop once a Roblox window came to the front" (owner, 2026-11-07) needs to know
        // whether the key reached Bloxkeep at all, and what it did.
        let waiting = input_turn::holder().map(|who| format!(", waiting for {who}'s turn")).unwrap_or_default();
        log::note(&format!("Hotkey {} received{}.", hotkey_name(id), waiting));
        // Hide, minimise and next wait for a nudge, a picture or a macro's turn under way (input_turn.rs).
        // PORT: the turn's action may run on a thread of its own here (input_turn::run_when_free), so what it needs from
        // the App is read first and the state push hops back to the core's thread.
        if id == HOTKEY_HIDE {
            input_turn::run_when_free("hotkey", || {
                let pids = roblox_processes::get_player_pids();
                if pids.iter().any(|pid| roblox_processes::has_hidden(*pid)) {
                    roblox_processes::show_windows(None);
                } else {
                    roblox_processes::hide_windows(&pids);
                }
                rt::post(|| App::with(|app| app.push_state()));
            });
        } else if id == HOTKEY_ARRANGE && self.controller.vault.borrow().is_some() {
            let app = self.clone();
            rt::spawn(async move { app.arrange_by_hotkey().await });
        } else if id == HOTKEY_MINIMIZE {
            let order = self.window_order();
            input_turn::run_when_free("hotkey", move || minimize_or_restore_all_by(None, &order));
        } else if id == HOTKEY_NEXT {
            let order = self.window_order();
            input_turn::run_when_free("hotkey", move || focus_next_window_by(None, &order));
        } else if id == HOTKEY_STOP_MACROS || id == HOTKEY_END_RECORDING {
            // Ctrl+Alt+0 stops everything; F8 only ends a recording.
            if id == HOTKEY_STOP_MACROS
                && let Some(player) = self.macros.macro_player.borrow().clone()
            {
                player.stop(None, "byUser", None, false);
            }
            let app = self.clone();
            rt::spawn(async move {
                let _ = app.stop_recording().await;
            });
            self.push_state();
        } else if (HOTKEY_MACRO_FIRST..HOTKEY_MACRO_FIRST + 9).contains(&id) && self.controller.vault.borrow().is_some() {
            self.run_macro_hotkey(id - HOTKEY_MACRO_FIRST + 1);
        }
    }

    pub async fn arrange_by_hotkey(self: &Rc<Self>) {
        // Its outcome is written by arrange_windows.
        if let Err(e) = self.arrange_windows(-1, false, true).await {
            log::write(&e);
        }
    }

    /// Each running client's place in the vault's account list (ShownRobloxWindows' Order): none without a vault.
    pub fn window_order(&self) -> HashMap<i32, usize> {
        let Some(vault) = self.controller.vault.borrow().clone() else {
            return HashMap::new();
        };
        let ids: Vec<_> = vault.data().accounts.iter().map(|a| a.id).collect();
        let running = self.controller.running.borrow();
        let mut order = HashMap::new();
        for (i, id) in ids.iter().enumerate() {
            if let Some((pid, _)) = running.get(id) {
                order.entry(*pid).or_insert(i);
            }
        }
        order
    }

}
