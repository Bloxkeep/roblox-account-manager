// AppController.DiscordPresence.cs.
//! Settings › Discord › Show on my Discord profile (`prefs` discord_presence, off by default): the profile
//! shows "Playing Bloxkeep" with how many Roblox windows are open and which game, never an account's name, and a link to
//! the website. Updated every few seconds through the Discord app on this PC (`discord_presence::DiscordPresence`);
//! turning it off or quitting takes it away.
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::core::app::App;
use crate::core::app::controller::PendingLaunch;
use crate::core::base::{Res, Stamp};
use crate::core::discord_presence::{Activity, DiscordPresence};
use crate::core::models::stamp_min;
use crate::core::{l, prefs, rt};

/// Bloxkeep's application on Discord's developer portal (created 2026-10-20, with the logo as its icon), whose name and icon the profile shows.
pub const DISCORD_APPLICATION_ID: &str = "1561788269933498408";
pub const WEBSITE_URL: &str = "https://bloxkeep.github.io/roblox-account-manager/";
pub const DISCORD_PRESENCE_INTERVAL: Duration = Duration::from_secs(15);

pub struct DiscordPresenceState {
    /// Shared with the work threads that talk to the Discord app.
    pub discord_presence: Arc<DiscordPresence>,
    pub next_discord_presence: Cell<Stamp>,
    /// Which game the profile shows next when there are several.
    pub presence_turn: Cell<usize>,
}

impl Default for DiscordPresenceState {
    fn default() -> DiscordPresenceState {
        DiscordPresenceState { discord_presence: Arc::new(DiscordPresence::new()), next_discord_presence: Cell::new(stamp_min()), presence_turn: Cell::new(0) }
    }
}

impl App {
    pub fn tick_discord_presence(&self, now: Stamp) {
        if now < self.discord_presence.next_discord_presence.get() {
            return;
        }
        self.discord_presence.next_discord_presence.set(now.plus_secs(DISCORD_PRESENCE_INTERVAL.as_secs_f64()));
        let on = prefs::lock().discord_presence && !DISCORD_APPLICATION_ID.is_empty();
        let activity = if on { Some(self.presence_activity()) } else { None };
        let presence = self.discord_presence.discord_presence.clone();
        rt::spawn(async move {
            rt::blocking(move || presence.set(activity.as_ref(), DISCORD_APPLICATION_ID)).await;
        });
    }

    /// In a game: its name and icon (Bloxkeep's logo at the corner), how many windows, and a button to its page. With
    /// several games each takes its turn, one per update (DISCORD_PRESENCE_INTERVAL), so the profile moves through them.
    /// Otherwise how many windows are open, or that Bloxkeep is managing accounts.
    pub fn presence_activity(&self) -> Activity {
        let windows = self.roblox_count();
        let runs: Vec<(i32, Rc<PendingLaunch>)> = self.controller.running.borrow().values().cloned().collect();
        let mut games: Vec<crate::core::roblox_api::GameInfo> = Vec::new();
        for run in &runs {
            if let Some(game) = self.playing_place((run.0, &run.1)).and_then(|id| self.known_game(id)) {
                if !game.name.is_empty() && !games.iter().any(|g| g.name == game.name) {
                    games.push(game);
                }
            }
        }
        let open = l::p(windows as i64, "1 Roblox window open", "{0} Roblox windows open", &[&windows]);
        let since = self.relaunch.started.get().unix_secs();
        let base = |details: String, state: Option<String>| Activity {
            details,
            state,
            start_unix: since,
            button_label: l::t("Get Bloxkeep"),
            button_url: WEBSITE_URL.to_string(),
            ..Default::default()
        };
        if games.is_empty() {
            return base(if windows == 0 { l::t("Managing Roblox accounts") } else { open }, None);
        }
        let turn = self.discord_presence.presence_turn.get();
        self.discord_presence.presence_turn.set(turn.wrapping_add(1));
        let shown = &games[turn % games.len()];
        let state = if games.len() > 1 { l::f("{0} · {1} games", &[&open, &games.len()]) } else { open };
        Activity {
            picture: shown.icon_url.clone().filter(|icon| icon.starts_with("https://")),
            picture_text: Some(shown.name.clone()),
            first_label: Some(l::t("View game")),
            first_url: Some(format!("https://www.roblox.com/games/{}", shown.place_id)),
            ..base(shown.name.clone(), Some(state))
        }
    }

    /// The game a window is playing: none once it left for Roblox's home screen, lost its connection or was signed
    /// out (the log keeps the last place then; seen 2026-10-20). Without a log yet, the one it was opened with.
    pub fn playing_place(&self, run: (i32, &PendingLaunch)) -> Option<i64> {
        match self.controller.logs.borrow().state_of(run.0) {
            Some(log) => {
                if log.status == "ingame" || log.status == "joining" {
                    log.place_id
                } else {
                    None
                }
            }
            None => run.1.target.as_ref().map(|t| t.place_id),
        }
    }

    pub fn set_discord_presence(&self, on: bool) -> Res<Value> {
        prefs::lock().discord_presence = on;
        prefs::save();
        self.discord_presence.next_discord_presence.set(stamp_min()); // at once, not at the next round
        self.tick_discord_presence(Stamp::utc_now());
        self.push_state();
        Ok(Value::Null)
    }
}
