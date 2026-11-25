//! Commands from the Watch remotely page (remote-monitor/, opened on a phone through its private link): Rejoin, Another
//! server and Close for a window, Open game (a preset, as Discord's /pc preset), and Clean RAM. They come through the
//! app's own loopback link with the session's key (live_monitor.rs), as the page's list of windows does, and do what
//! the same buttons do in Bloxkeep. Nothing is asked of the person at the PC: a preset that would ask first isn't run,
//! and Clean RAM goes only through auto clean's task (Windows' question would wait at the PC).
//!
//! Each answer is a code the page puts in its own words, with the app's own words in `text` for what it doesn't know.
use std::rc::Rc;

use serde_json::{Value, json};

use super::App;
use crate::core::{
    base::{Fail, Guid},
    log,
    roblox_processes::PlayerProcess,
};

impl App {
    /// The account whose window the page calls `window` ("pid-created", as its list gives it).
    fn page_account(&self, window: &str, players: &[PlayerProcess]) -> Option<Guid> {
        let pid = players.iter().find(|p| p.created > 0 && format!("{}-{}", p.pid, p.created) == window)?.pid;
        self.controller.running.borrow().iter().find(|(_, (p, _))| *p == pid).map(|(id, _)| *id)
    }

    fn page_failed(fail: &Fail) -> Value {
        json!({ "code": "failed", "text": super::controller::error_text(fail) })
    }

    /// What the page's command `order` ({do, window?, preset?}) did. `players` are the PC's Roblox clients now.
    pub async fn remote_page_do(self: &Rc<Self>, order: Value, players: Vec<PlayerProcess>) -> Value {
        if !self.unlocked() || self.controller.disposed.get() {
            return json!({ "code": "locked" });
        }
        let what = order.get("do").and_then(Value::as_str).unwrap_or("");
        let window = order.get("window").and_then(Value::as_str).unwrap_or("");
        let account = || self.page_account(window, &players);
        let named = |id: Guid| self.find_account(id).map(|a| self.name_text(&a)).unwrap_or_default();
        log::note(&format!("Remote page: {what}."));
        match what {
            "rejoin" => match account() {
                Some(id) if self.rejoin_now(id) => json!({ "code": "rejoining", "name": named(id) }),
                Some(_) => json!({ "code": "cantRejoin" }),
                None => json!({ "code": "noAccount" }),
            },
            "switch" => match account() {
                Some(id) => match self.switch_server(&[id]).await {
                    Ok(_) => json!({ "code": "switching", "name": named(id) }),
                    Err(fail) => Self::page_failed(&fail),
                },
                None => json!({ "code": "noAccount" }),
            },
            "close" => match account() {
                Some(id) => {
                    let name = named(id);
                    self.close_accounts(&[id], false);
                    json!({ "code": "closed", "name": name })
                }
                None => json!({ "code": "noAccount" }),
            },
            "preset" => {
                let wanted = order.get("preset").and_then(Value::as_str).and_then(|s| s.parse::<Guid>().ok());
                let preset = self.vault(|d| d.settings.presets.clone()).unwrap_or_default().into_iter().find(|p| Some(p.id) == wanted);
                let Some(preset) = preset else { return json!({ "code": "noPreset" }) };
                match self.start_preset(preset.id, false).await {
                    Ok(r) if r.needs_confirm => json!({ "code": "presetAsks", "name": preset.name, "text": r.message.unwrap_or_default() }),
                    Ok(r) => {
                        let mut ids: Vec<Guid> = Vec::new();
                        for id in &preset.account_ids {
                            if !ids.contains(id) && self.find_account(*id).is_some() {
                                ids.push(*id);
                            }
                        }
                        json!({
                            "code": if r.cancelled { "presetStopped" } else { "presetDone" },
                            "name": preset.name,
                            "opened": r.launched,
                            "total": ids.len(),
                            "errors": r.errors.unwrap_or_default().into_iter().take(5).collect::<Vec<_>>(),
                        })
                    }
                    Err(fail) => Self::page_failed(&fail),
                }
            }
            "ram" => {
                use crate::clean_ram::Cleaned;
                let areas = self.vault(|d| d.settings.clean_areas).unwrap_or(crate::clean_ram::DEFAULT_AREAS);
                let got = crate::threads::call(move || crate::clean_ram::auto_clean(areas)).await.ok().and_then(|r| r.ok());
                match got {
                    Some(Cleaned::Freed(bytes)) => {
                        let mb = bytes / (1024 * 1024);
                        log::note(&format!("Remote page: Clean RAM freed {mb} MB."));
                        json!({ "code": "ramCleaned", "mb": mb })
                    }
                    Some(Cleaned::Gone) => json!({ "code": "ramNeedsAuto" }),
                    Some(Cleaned::Failed(why)) => {
                        log::note(&format!("Remote page: Clean RAM failed ({why})."));
                        json!({ "code": "ramFailed" })
                    }
                    Some(Cleaned::Refused) | None => json!({ "code": "ramFailed" }),
                }
            }
            _ => json!({ "code": "bad" }),
        }
    }

    /// What the page's list of windows adds for its commands: the presets (to open a game with), which windows are in
    /// a game (Rejoin, Another server), and whether Clean RAM can run without asking (auto clean's task is there).
    pub fn remote_page_extras(&self, ids: &std::collections::HashMap<i32, String>) -> Value {
        let presets: Vec<Value> = self
            .vault(|d| d.settings.presets.iter().map(|p| json!({ "id": p.id.to_string(), "name": p.name })).collect())
            .unwrap_or_default();
        let ram = self.vault(|d| d.settings.auto_clean_at > 0 || d.settings.auto_clean_every > 0).unwrap_or(false);
        let mut accounts = serde_json::Map::new();
        for (id, (pid, _)) in self.controller.running.borrow().iter() {
            if let Some(window) = ids.get(pid) {
                accounts.insert(window.clone(), Value::String(id.to_string()));
            }
        }
        json!({ "presets": presets, "ram": ram, "commands": accounts.keys().cloned().collect::<Vec<_>>() })
    }
}
