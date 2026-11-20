//! Hiding Roblox windows while their games run: the window and taskbar button go, the game, auto-rejoin and anti-AFK go
//! on (see roblox_processes::hide_windows). No accounts: every Roblox window of this PC. (AppController.HideWindows.cs)
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::{App, launching::running};
use crate::core::{base::Guid, input_turn, log, roblox_processes, rt};

/// The answer the turn's action leaves: Some(count) once it ran.
type Slot = Arc<Mutex<Option<i32>>>;

fn read(slot: &Slot) -> Option<i32> {
    *slot.lock().unwrap_or_else(|e| e.into_inner())
}

/// PushState from the action, on the core's thread (the action runs on a thread of its own when it waited for a turn).
fn push_state_later() {
    rt::post(|| App::with(|app| app.push_state()));
}

impl App {
    /// The chosen accounts' window processes now; None for every Roblox window of this PC (read when the action runs).
    // PORT: the C# read the accounts' windows inside the action, once the turn came; the App can't be reached from the
    // turn's thread, so a chosen account's window is the one it has as Hide or Show is clicked.
    fn window_pids(&self, ids: &[Guid]) -> Option<Vec<i32>> {
        if ids.is_empty() {
            return None;
        }
        let runs = running(self);
        Some(ids.iter().filter_map(|id| runs.iter().find(|(k, _)| k == id).map(|(_, r)| r.0)).collect())
    }

    // Both wait for a nudge, a picture or a macro's turn under way, as the hotkey does: a turn puts its windows back as it
    // found them, so a window shown meanwhile went unseen again while counted as shown (input_turn.rs).

    /// How many were hidden; null when it waits for a turn under way (done once it ends).
    pub fn hide_windows(&self, ids: &[Guid]) -> Value {
        let slot: Slot = Arc::new(Mutex::new(None));
        let chosen = self.window_pids(ids);
        let all = ids.is_empty();
        let out = slot.clone();
        input_turn::run_when_free("hide", move || {
            let pids = chosen.unwrap_or_else(roblox_processes::get_player_pids);
            let hidden = roblox_processes::hide_windows(&pids);
            *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(hidden);
            log::note(&format!(
                "Hide windows: {hidden} of {} Roblox window(s) hidden ({}; the rest had no window or were hidden already).",
                pids.len(),
                if all { "all" } else { "chosen accounts" }
            ));
            push_state_later();
        });
        let hidden = read(&slot);
        if hidden.is_none() {
            log::note("Hide windows: waits for a nudge, picture or macro turn to end.");
        }
        hidden.map_or(Value::Null, |h| json!(h))
    }

    /// How many were shown; null when it waits for a turn under way (done once it ends).
    pub fn show_windows(&self, ids: &[Guid]) -> Value {
        let slot: Slot = Arc::new(Mutex::new(None));
        let chosen = self.window_pids(ids);
        let count = ids.len();
        let out = slot.clone();
        input_turn::run_when_free("hide", move || {
            let shown = roblox_processes::show_windows(chosen.as_deref());
            *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(shown);
            log::note(&format!(
                "Show windows: {shown} hidden Roblox window(s) shown again ({}).",
                if count == 0 { "all".to_string() } else { format!("{count} chosen account(s)") }
            ));
            push_state_later();
        });
        let shown = read(&slot);
        if shown.is_none() {
            log::note("Show windows: waits for a nudge, picture or macro turn to end.");
        }
        shown.map_or(Value::Null, |s| json!(s))
    }
}
