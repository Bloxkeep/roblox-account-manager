// AppController.Latest.cs: a new Bloxkeep found within minutes: GitHub's API allows an hour's update checks per address
// (an internet café shares one), so every few minutes the bot is asked instead (services/DiscordBot/worker /latest,
// read from GitHub every 2 min, cached 1 min per Cloudflare location; nothing is sent). A newer version brings the hourly
// check forward once, so a copy on the lock screen installs it before anyone unlocks. Only installed copies with
// automatic updates on ask.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::Duration;

use serde_json::Value;

use super::App;
use super::installs::net_version;
use crate::core::base::Stamp;
use crate::core::models::stamp_min;
use crate::core::{prefs, rt};

pub const LATEST_URL: &str = "https://bloxkeep-bot.bloxkeep-app.workers.dev/latest";
pub const LATEST_LOOK: Duration = Duration::from_secs(5 * 60);

/// The instance fields AppController.Latest.cs declares.
pub struct LatestState {
    pub next_latest_look: Cell<Stamp>,
    pub latest_looking: Cell<bool>,
    pub latest_seen: RefCell<Option<String>>,
}

impl Default for LatestState {
    fn default() -> LatestState {
        LatestState { next_latest_look: Cell::new(stamp_min()), latest_looking: Cell::new(false), latest_seen: RefCell::new(None) }
    }
}

pub fn latest_test_url() -> Option<String> {
    std::env::var("BLOXKEEP_LATEST_URL").ok().filter(|u| !u.is_empty())
}

/// OnlineHttp (AppController.Online.cs): no cookies, no redirects followed, 20 s at most.
static ONLINE_HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| crate::net::builder().redirects(0).timeout(Duration::from_secs(20)).build());

/// GET a URL: its text when it answered 2xx; None for any other answer; Err when it couldn't be reached or read.
fn get_text(url: &str) -> Result<Option<String>, String> {
    match ONLINE_HTTP.get(url).call() {
        Ok(res) if (200..300).contains(&res.status()) => res.into_string().map(Some).map_err(|e| e.to_string()),
        Ok(_) | Err(ureq::Error::Status(..)) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// "1.0.13" against "1.0.12", or against "1.0.13-beta.1" (a beta comes before its release); anything unreadable isn't newer.
pub fn is_newer_version(latest: &str, current: Option<&str>) -> bool {
    fn read(text: Option<&str>) -> (Option<[i64; 4]>, bool) {
        let s = text.unwrap_or("").trim().trim_start_matches(['v', 'V']);
        match s.find('-') {
            Some(dash) => (net_version(Some(&s[..dash])), true),
            None => (net_version(Some(s)), false),
        }
    }
    let (have, have_pre) = read(current);
    let (got, got_pre) = read(Some(latest));
    let (Some(have), Some(got)) = (have, got) else {
        return false;
    };
    if got_pre {
        return false;
    }
    got > have || (got == have && have_pre)
}

impl App {
    pub fn tick_latest(self: &Rc<Self>, now: Stamp) {
        let supported = self.updates.updater.borrow().supported();
        if self.latest.latest_looking.get() || now < self.latest.next_latest_look.get() || !prefs::lock().check_updates || !(supported || latest_test_url().is_some()) {
            return;
        }
        self.latest.next_latest_look.set(now.plus_secs(LATEST_LOOK.as_secs_f64()));
        let app = self.clone();
        rt::spawn(async move { app.look_at_latest().await });
    }

    pub async fn look_at_latest(self: &Rc<Self>) {
        let _looking = crate::core::base::Busy::set(&self.latest.latest_looking);
        let url = latest_test_url().unwrap_or_else(|| LATEST_URL.to_string());
        // Not reached, or not JSON: asked again in a few minutes; the hourly check goes on meanwhile.
        if let Ok(Some(text)) = rt::blocking(move || get_text(&url)).await
            && let Ok(doc) = serde_json::from_str::<Value>(&text)
            && let Some(Value::String(latest)) = doc.as_object().and_then(|o| o.get("version"))
        {
            let current = self.updates.updater.borrow().current_version().to_string();
            if self.latest.latest_seen.borrow().as_deref() != Some(latest.as_str()) && is_newer_version(latest, Some(&current)) {
                *self.latest.latest_seen.borrow_mut() = Some(latest.clone());
                if self.updates.next_update_check.get() > Stamp::utc_now() {
                    self.updates.next_update_check.set(Stamp::utc_now());
                }
            }
        }
    }
}
