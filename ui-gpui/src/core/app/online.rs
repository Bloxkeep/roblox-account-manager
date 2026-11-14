//! AppController.Online.cs: Settings › Bloxkeep › Privacy › Add me to the online count (on until turned off): while Bloxkeep is
//! open, locked or not, it tells Bloxkeep's Discord bot so every minute (further apart when the bot says so) and says goodbye as
//! it closes (services/DiscordBot/worker/online.js). What goes: an id made from this PC (`online_id_for_this_pc`), Bloxkeep's
//! version, the app's language, how long this PC has had Bloxkeep in three rough groups (`install_age`), once, that this is a
//! new install's first open, and what this PC plays now: the games its accounts are in, how many run, the drops since the last
//! beat and its name on the alts board if it joined; never an account. The bot keeps numbers per day of the first ones, and
//! the rest only in memory while this PC counts as open. Turned off: a goodbye, the id forgotten, nothing more. Only an
//! installed Bloxkeep counts: a development copy or the checks never do, unless BLOXKEEP_ONLINE_URL points them at a server of
//! their own.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::App;
use super::rejoin::{SERVER_SHUTDOWN, lost_connection};
use crate::core::base::{Fail, Res, Stamp, new_guid};
use crate::core::models::stamp_min;
use crate::core::{l, log, prefs, rt, themes, vault};

/// Bloxkeep's Discord bot (services/DiscordBot/worker), which counts.
pub const ONLINE_URL: &str = "https://bloxkeep-bot.bloxkeep-app.workers.dev/online/ping";
/// Until the bot says otherwise (each answer says when to send the next, from a minute to six hours).
pub const ONLINE_BEAT: Duration = Duration::from_secs(60);
pub const ONLINE_BEAT_MIN: Duration = Duration::from_secs(60);
pub const ONLINE_BEAT_MAX: Duration = Duration::from_secs(6 * 60 * 60);
/// After a beat that didn't go through (no internet yet, just after Windows started); doubled each time it fails again.
pub const ONLINE_RETRY: Duration = Duration::from_secs(2 * 60);
pub const ONLINE_DROPS_MAX: usize = 20;

/// No cookies, no redirects followed, 20 s at most (C#'s OnlineHttp).
static ONLINE_HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| crate::net::builder().redirects(0).timeout(Duration::from_secs(20)).build());

pub const BOARD_WORDS1: [&str; 16] = ["Swift", "Brave", "Lucky", "Quiet", "Bold", "Calm", "Sly", "Wild", "Mighty", "Clever", "Rapid", "Silent", "Happy", "Fuzzy", "Shiny", "Cosmic"];
pub const BOARD_WORDS2: [&str; 16] = ["Fox", "Otter", "Panda", "Tiger", "Wolf", "Hawk", "Koala", "Lynx", "Falcon", "Badger", "Raven", "Shark", "Gecko", "Moose", "Bison", "Owl"];

static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
// .NET's `$` also matches before a final "\n": `\n?\z` here.
static BOARD_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\p{L}\p{N}][\p{L}\p{N} ._-]{1,19}\n?\z").unwrap());

/// An account dropped out since the last beat, for the bot's radar: its game (universe) and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnlineDrop {
    pub universe: i64,
    pub why: &'static str,
}

pub struct OnlineState {
    pub next_online: Cell<Stamp>,
    /// How far apart the bot last said beats should be; a failed beat is tried again sooner, never later than this.
    pub online_every: Cell<Duration>,
    pub online_retry: Cell<Duration>,
    pub online_sending: Cell<bool>,
    /// The last beat failed: written once, then once more when one goes through.
    pub online_failing: Cell<bool>,
    /// A beat went through since the app started (or the switch was turned on): there is a goodbye to say.
    pub online_said: Cell<bool>,
    pub online_drops: RefCell<Vec<OnlineDrop>>,
    /// What the bot last said Bloxkeep's users play (services/DiscordBot/worker/online.js communityOf).
    pub community: RefCell<Option<Value>>,
    pub community_at: Cell<Stamp>,
    pub community_asking: Cell<bool>,
    /// The last ask for the sums failed: written once, then once more when one goes through.
    pub community_failing: Cell<bool>,
}

impl Default for OnlineState {
    fn default() -> OnlineState {
        OnlineState {
            next_online: Cell::new(stamp_min()),
            online_every: Cell::new(ONLINE_BEAT),
            online_retry: Cell::new(ONLINE_RETRY),
            online_sending: Cell::new(false),
            online_failing: Cell::new(false),
            online_said: Cell::new(false),
            online_drops: RefCell::default(),
            community: RefCell::new(None),
            community_at: Cell::new(stamp_min()),
            community_asking: Cell::new(false),
            community_failing: Cell::new(false),
        }
    }
}

pub fn online_test_url() -> Option<String> {
    std::env::var("BLOXKEEP_ONLINE_URL").ok().filter(|u| !u.is_empty())
}

fn community_url() -> String {
    online_test_url().unwrap_or_else(|| ONLINE_URL.to_string()).replace("/online/ping", "/online/community")
}

/// What came back: the HTTP status and its text.
// PORT: HttpRequestException / TaskCanceledException are ureq's transport errors here; one of them is `Err`.
fn post_online(body: &Value) -> Res<(u16, String)> {
    let url = online_test_url().unwrap_or_else(|| ONLINE_URL.to_string());
    let answer = ONLINE_HTTP.post(&url).set("Content-Type", "application/json; charset=utf-8").send_string(&body.to_string());
    read_answer(answer)
}

fn read_answer(answer: Result<ureq::Response, ureq::Error>) -> Res<(u16, String)> {
    match answer {
        Ok(res) => {
            let status = res.status();
            Ok((status, res.into_string().map_err(Fail::other)?))
        }
        Err(ureq::Error::Status(status, res)) => Ok((status, res.into_string().unwrap_or_default())),
        Err(ureq::Error::Transport(t)) => Err(Fail::other(format!("HttpRequestException: {t}"))),
    }
}

/// "Closing": no longer counted at once, rather than once its last beat is too old. Off the core's thread, so it can be
/// waited for.
pub fn say_offline(id: String) -> std::sync::mpsc::Receiver<()> {
    let (done, wait) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new().name("online-goodbye".into()).spawn(move || {
        // Counted until its last beat is too old (a quarter of an hour at most).
        let _ = post_online(&json!({ "id": id, "gone": true }));
        let _ = done.send(());
    });
    wait
}

/// How long this PC has had Bloxkeep: "new" (under a day), "week" (under a week) or "old", from when its data
/// folder was made, which stays through updates and reinstalls that keep the accounts.
pub fn install_age() -> &'static str {
    let Ok(created) = std::fs::metadata(vault::data_dir()).and_then(|m| m.created()) else { return "old" };
    let days = std::time::SystemTime::now().duration_since(created).map(|d| d.as_secs_f64() / 86_400.0).unwrap_or(0.0);
    if days < 1.0 {
        "new"
    } else if days < 7.0 {
        "week"
    } else {
        "old"
    }
}

/// HKLM\SOFTWARE\Microsoft\Cryptography's MachineGuid, from the 64-bit view.
fn machine_guid() -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegGetValueW};
    let key = crate::core::win_kit::wide(r"SOFTWARE\Microsoft\Cryptography");
    let name = crate::core::win_kit::wide("MachineGuid");
    let mut buffer = vec![0u16; 256];
    let mut size = (buffer.len() * 2) as u32;
    // SAFETY: the strings are NUL-terminated; `buffer` holds `size` bytes and outlives the call.
    let result = unsafe {
        RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size)
    };
    if result != 0 {
        return None;
    }
    Some(crate::core::win_kit::from_wide(&buffer))
}

/// The id this PC is counted under: Windows' MachineGuid hashed with Bloxkeep's own label, so it leads back to
/// nothing (no account, vault, Roblox or Discord) yet stays the same after a reinstall, a data wipe or for a second copy:
/// one PC, one in the count. None where Windows' id can't be read (a random one is used then).
pub fn online_id_for_this_pc() -> Option<String> {
    let guid = machine_guid()?;
    if guid.trim().is_empty() {
        return None;
    }
    let hash = Sha256::digest(format!("bloxkeep-online:{}", guid.trim().to_lowercase()).as_bytes());
    Some(hash[..16].iter().map(|b| format!("{b:02x}")).collect())
}

/// RandomNumberGenerator.GetInt32(from, to): uniform in [from, to).
fn random_in(from: u32, to: u32) -> u32 {
    let span = to - from;
    // Rejection sampling keeps every value equally likely.
    let limit = u32::MAX - u32::MAX % span;
    loop {
        let mut bytes = [0u8; 4];
        if getrandom::fill(&mut bytes).is_err() {
            return from;
        }
        let v = u32::from_le_bytes(bytes);
        if v < limit {
            return from + v % span;
        }
    }
}

impl App {
    pub fn online_allowed(&self) -> bool {
        self.updates.updater.borrow().supported() || online_test_url().is_some()
    }

    pub fn tick_online(self: &Rc<Self>, now: Stamp) {
        if self.online.online_sending.get() || now < self.online.next_online.get() || !prefs::lock().count_online || !self.online_allowed() {
            return;
        }
        self.online.next_online.set(now.plus_secs(self.online.online_every.get().as_secs_f64()));
        let app = self.clone();
        rt::spawn(async move { app.beat().await });
    }

    pub async fn beat(self: &Rc<Self>) {
        // This PC's id, also in place of one an earlier Bloxkeep made at random.
        let pc = rt::blocking(online_id_for_this_pc).await;
        let (id, first_open_told) = {
            let mut s = prefs::lock();
            // Turned off during that read: the id it cleared isn't made again (the PC would stay counted, no goodbye sent).
            if !s.count_online {
                return;
            }
            let changed = s.online_id.is_none() || pc.as_ref().is_some_and(|pc| s.online_id.as_ref() != Some(pc));
            if changed {
                s.online_id = Some(pc.clone().unwrap_or_else(|| new_guid().simple().to_string()));
            }
            let id = s.online_id.clone().unwrap_or_default();
            let told = s.first_open_told;
            drop(s);
            if changed {
                prefs::save();
            }
            (id, told)
        };
        // Roughly how long this PC has had Bloxkeep, and whether this is a new install's first open (not an old copy just updated).
        let age = install_age();
        let first = !first_open_told && age == "new";
        // finally: cleared however this ends.
        let _sending = crate::core::base::Busy::set(&self.online.online_sending);
        let needs_name = {
            let s = prefs::lock();
            !s.board_name_chosen && s.board_name.is_none()
        };
        if needs_name {
            // On the alts board from the start under a made-up name, changed or emptied (off the board) in Statistics.
            {
                let mut s = prefs::lock();
                s.board_name = Some(App::random_board_name());
                s.board_name_chosen = true;
            }
            prefs::save();
            self.push_state();
        }
        // What this PC plays now, for the sums the bot tells back (community_dto): the games its accounts are in, how many
        // run, the drops since the last beat; its name on the alts board if it joined it; the theme picked, for Popular in
        // the theme picker (one of Bloxkeep's own: not a custom one, not "follow Windows"). No account.
        let drops: Vec<OnlineDrop> = std::mem::take(&mut *self.online.online_drops.borrow_mut());
        let (board, theme) = {
            let s = prefs::lock();
            (s.board_name.clone(), s.theme.clone())
        };
        let body = json!({
            "id": id,
            "app": self.updates.updater.borrow().current_version(),
            "language": l::language(),
            "age": age,
            "first": first,
            "games": self.playing_universes(),
            "alts": self.controller.running.borrow().len(),
            "drops": drops.iter().map(|d| json!({ "game": d.universe, "why": d.why })).collect::<Vec<_>>(),
            "board": board,
            "theme": if theme == themes::FOLLOW_WINDOWS || theme == themes::CUSTOM { None } else { Some(theme) },
        });
        let outcome: Res<()> = async {
            let (status, text) = rt::blocking(move || post_online(&body)).await?;
            if !(200..300).contains(&status) {
                {
                    let mut waiting = self.online.online_drops.borrow_mut();
                    let room = ONLINE_DROPS_MAX.saturating_sub(waiting.len());
                    let back: Vec<OnlineDrop> = drops.iter().take(room).copied().collect();
                    waiting.splice(0..0, back);
                }
                if !self.online.online_failing.get() {
                    log::note(&format!("Online count: the bot answered HTTP {status}; tried again sooner, then less and less often."));
                }
                self.online.online_failing.set(true);
                self.retry_online();
                return Ok(());
            }
            // Turned off while this was on its way: the goodbye went before it, so it goes again.
            if prefs::lock().online_id.as_deref() != Some(id.as_str()) {
                let _ = say_offline(id.clone());
                return Ok(());
            }
            self.online.online_said.set(true);
            if self.online.online_failing.get() {
                log::note("Online count: recovered.");
            }
            self.online.online_failing.set(false);
            self.online.online_retry.set(ONLINE_RETRY);
            let told = prefs::lock().first_open_told;
            if !told {
                prefs::lock().first_open_told = true;
                prefs::save();
            }
            let answer: Value = serde_json::from_str(&text)?;
            if let Some(seconds) = answer.get("next").and_then(Value::as_i64).filter(|s| i32::try_from(*s).is_ok()) {
                let every = (seconds as f64).clamp(ONLINE_BEAT_MIN.as_secs_f64(), ONLINE_BEAT_MAX.as_secs_f64());
                self.online.online_every.set(Duration::from_secs_f64(every));
                self.online.next_online.set(Stamp::utc_now().plus_secs(every));
            }
            if let Some(community) = answer.get("community").filter(|c| c.is_object()) {
                *self.online.community.borrow_mut() = Some(community.clone());
                self.online.community_at.set(Stamp::utc_now());
                self.push_state();
            }
            Ok(())
        }
        .await;
        if let Err(ex) = outcome {
            if !self.online.online_failing.get() {
                log::note(&format!("Online count: beat failed ({}); tried again sooner, then less and less often.", App::log_kind_of(&ex)));
            }
            self.online.online_failing.set(true);
            self.retry_online();
        }
    }

    /// Sooner than the next beat: two minutes, then four, eight… up to the usual gap, so a bot that can't answer
    /// (no internet, or over its day's budget) isn't asked more often than usual.
    pub fn retry_online(&self) {
        let (retry, every) = (self.online.online_retry.get(), self.online.online_every.get());
        let wait = if retry < every { retry } else { every };
        self.online.next_online.set(Stamp::utc_now().plus_secs(wait.as_secs_f64()));
        self.online.online_retry.set(wait + wait);
    }

    /// Bloxkeep closing: the goodbye, if the bot answers within a moment; not for an update in the background, back within seconds.
    pub fn say_offline_on_exit(&self) {
        let id = prefs::lock().online_id.clone();
        if self.online.online_said.get()
            && self.relaunch.relaunch_args.borrow().is_none()
            && self.online_allowed()
            && let Some(id) = id
        {
            let _ = say_offline(id).recv_timeout(Duration::from_secs(2));
        }
    }

    // ───────────────────────────── what Bloxkeep's users play ─────────────────────────────

    /// The games (Roblox universes) this PC's accounts are in now, the ones Bloxkeep knows the universe of.
    pub fn playing_universes(&self) -> Vec<i64> {
        let running = self.controller.running.borrow();
        let logs = self.controller.logs.borrow();
        let games = self.controller.games.borrow();
        let mut out: Vec<i64> = Vec::new();
        for (pid, launch) in running.values() {
            let place = logs.state_of(*pid).and_then(|s| s.place_id).or_else(|| launch.target.as_ref().map(|t| t.place_id)).unwrap_or(0);
            let universe = if place > 0 { games.get(&place).and_then(|g| g.as_ref()).map(|g| g.universe_id).unwrap_or(0) } else { 0 };
            if universe > 0 && !out.contains(&universe) {
                out.push(universe);
                if out.len() == 20 {
                    break;
                }
            }
        }
        out
    }

    /// A disconnect the radar counts (Activity's "disconnected"): kicked by the game, the connection lost, the server
    /// shut down, or couldn't join. Not the person's own (idle, another device, a private server, an update).
    pub fn note_drop(&self, place_id: i64, code: i32, at: Stamp) {
        let why = match code {
            279 => Some("join"),
            267 => Some("kick"),
            SERVER_SHUTDOWN => Some("shutdown"), // only 288, as AppController.Online.cs counts it
            _ if lost_connection(code) => Some("lost"),
            _ => None,
        };
        let universe = self.controller.games.borrow().get(&place_id).and_then(|g| g.as_ref()).map(|g| g.universe_id).unwrap_or(0);
        let Some(why) = why else { return };
        if universe <= 0 || Stamp::utc_now().secs_since(at) > 5.0 * 60.0 || !prefs::lock().count_online || self.online.online_drops.borrow().len() >= ONLINE_DROPS_MAX {
            return;
        }
        self.online.online_drops.borrow_mut().push(OnlineDrop { universe, why });
    }

    /// What the Games tab and Statistics show of it: null while nothing came yet. With the count on, from each beat's
    /// answer; off, from `community_now`.
    pub fn community_dto(&self) -> Value {
        match &*self.online.community.borrow() {
            Some(c) => json!({ "data": c, "at": self.online.community_at.get().unix_ms() }),
            None => Value::Null,
        }
    }

    /// The Games tab or Statistics is open while the count is off: the sums again, at most once a minute (the bot keeps
    /// them 30 s). With the count on, the beats bring them.
    pub fn community_now(self: &Rc<Self>) -> Value {
        if prefs::lock().count_online
            || !self.online_allowed()
            || self.online.community_asking.get()
            || Stamp::utc_now().secs_since(self.online.community_at.get()) < 55.0
        {
            return Value::Null;
        }
        self.online.community_asking.set(true);
        let app = self.clone();
        rt::spawn(async move { app.ask_community().await });
        Value::Null
    }

    /// The bot's sums for an app not in the count (GET /online/community): what is played, the radar, the board,
    /// nothing of this PC and nothing sent about it.
    pub async fn ask_community(self: &Rc<Self>) {
        // finally: cleared however this ends (community_now set it already).
        let _asking = crate::core::base::Busy::set(&self.online.community_asking);
        let outcome: Res<()> = async {
            let (status, text) = rt::blocking(|| read_answer(ONLINE_HTTP.get(&community_url()).call())).await?;
            if !(200..300).contains(&status) {
                if !self.online.community_failing.get() {
                    log::note(&format!("Community sums: the bot answered HTTP {status}; asked again in a minute."));
                }
                self.online.community_failing.set(true);
                return Ok(());
            }
            if self.online.community_failing.get() {
                log::note("Community sums: recovered.");
            }
            self.online.community_failing.set(false);
            let answer: Value = serde_json::from_str(&text)?;
            if !prefs::lock().count_online
                && let Some(community) = answer.get("community").filter(|c| c.is_object())
            {
                *self.online.community.borrow_mut() = Some(community.clone());
                self.online.community_at.set(Stamp::utc_now());
                self.push_state();
            }
            Ok(())
        }
        .await;
        if let Err(ex) = outcome {
            if !self.online.community_failing.get() {
                log::note(&format!("Community sums: not loaded ({}); asked again in a minute.", App::log_kind_of(&ex)));
            }
            self.online.community_failing.set(true);
        }
    }

    /// A made-up name for the alts board, like "Swift Otter 42": nothing of the PC or its accounts in it.
    pub fn random_board_name() -> String {
        format!(
            "{} {} {}",
            BOARD_WORDS1[random_in(0, BOARD_WORDS1.len() as u32) as usize],
            BOARD_WORDS2[random_in(0, BOARD_WORDS2.len() as u32) as usize],
            random_in(10, 100)
        )
    }

    /// A name for the alts board, tidied (spaces made one), or None when it isn't one (2 to 20 letters, digits,
    /// spaces, dots, dashes, underscores, from a letter or digit: the bot takes nothing else).
    pub fn board_name_of(name: Option<&str>) -> Option<String> {
        let normalized: String = name.unwrap_or("").nfc().collect();
        let tidy = SPACES.replace_all(&normalized, " ").trim().to_string();
        if BOARD_NAME.is_match(&tidy) { Some(tidy) } else { None }
    }

    /// {name}: on the alts board under this name; empty: off it. A name that isn't one is refused.
    pub fn set_board_name(&self, name: &str) -> Res<Value> {
        let tidy = if name.trim().is_empty() { None } else { App::board_name_of(Some(name)) };
        if !name.trim().is_empty() && tidy.is_none() {
            return Err(Fail::user(l::t("Use 2 to 20 letters or digits (spaces, dots, dashes and underscores too).")));
        }
        {
            let mut s = prefs::lock();
            s.board_name = tidy.clone();
            s.board_name_chosen = true;
        }
        prefs::save();
        self.online.next_online.set(stamp_min()); // on the board (or off it) at the next tick, not the next beat
        self.push_state();
        Ok(json!({ "name": tidy }))
    }

    pub fn set_count_online(&self, on: bool) -> Value {
        prefs::lock().count_online = on;
        if !on {
            *self.online.community.borrow_mut() = None; // the sums again without this PC's place (community_now)
            self.online.community_at.set(stamp_min());
            self.online.online_drops.borrow_mut().clear();
        }
        let id = prefs::lock().online_id.clone();
        if !on && let Some(id) = id {
            if self.online.online_said.get() && self.online_allowed() {
                let _ = say_offline(id);
            }
            prefs::lock().online_id = None;
            self.online.online_said.set(false);
        }
        prefs::save();
        self.online.next_online.set(stamp_min()); // on again: counted at once (this PC's id again)
        self.push_state();
        Value::Null
    }
}
