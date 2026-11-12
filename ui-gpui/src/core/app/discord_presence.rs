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
}

impl Default for DiscordPresenceState {
    fn default() -> DiscordPresenceState {
        DiscordPresenceState { discord_presence: Arc::new(DiscordPresence::new()), next_discord_presence: Cell::new(stamp_min()) }
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

    pub fn presence_activity(&self) -> Activity {
        let windows = self.roblox_count();
        let runs: Vec<(i32, Rc<PendingLaunch>)> = self.controller.running.borrow().values().cloned().collect();
        let mut games: Vec<String> = Vec::new();
        for run in &runs {
            if let Some(name) = self.playing_place((run.0, &run.1)).and_then(|id| self.known_game(id)).map(|g| g.name) {
                if !name.is_empty() && !games.contains(&name) {
                    games.push(name);
                }
            }
        }
        let details = if windows == 0 {
            l::t("Managing Roblox accounts")
        } else {
            l::p(windows as i64, "1 Roblox window open", "{0} Roblox windows open", &[&windows])
        };
        let state = match games.len() {
            1 => Some(games[0].clone()),
            0 => None,
            n => Some(l::f("{0} games", &[&n])),
        };
        let since = self.relaunch.started.get().unix_secs();
        Activity { details, state, start_unix: since, button_label: l::t("Get Bloxkeep"), button_url: WEBSITE_URL.to_string() }
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
