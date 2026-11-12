//! Auto-rejoin: a window that ended, why, whether it goes back, and going back (join checks, game bans, waits).
//! (AppController.Rejoin.cs)
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use futures::StreamExt;

use super::{
    App,
    controller::{GET_IN_TIMEOUT, JOINING_TIMEOUT, MAX_NETWORK_RETRIES, MAX_OPEN_RETRIES, MAX_TICKET_RETRIES, PendingLaunch, REJOIN_DELAY, RejoinRequest, DISCONNECT_GRACE},
    launching::{
        Run, add_launch, has_launch, is_running, is_start_error, launch_fail_kind, linked, log_of, remove_launch, retry_delay, run_of, running, secs,
        short, vault_now,
    },
};
use crate::core::{
    base::{Cancel, Fail, Guid, Res, Stamp},
    game_target::GameTarget,
    l, log,
    models::{Account, GameBan},
    multi_instance,
    roblox_api::{self, JoinAnswer, Playability, is_fail_kind},
    roblox_log_watcher::ClientState,
    roblox_processes, rt,
    system_memory::{self, MemoryStatus},
};

/// A join check's answer as it comes: None while asked, Some(None) no answer (asked again later), Some(Some(..)) Roblox's.
/// (C#'s Task<JoinAnswer?>, read for IsCompleted and Result.)
pub type JoinSlot = Rc<RefCell<Option<Option<JoinAnswer>>>>;

/// A join check kept with its window: the answer and when it was asked.
#[derive(Clone)]
pub struct JoinCheck {
    pub answer: JoinSlot,
    pub at: Stamp,
}

/// Asks Roblox's playability for (account, universe): the checks answer for Roblox.
pub type AskPlayability = Arc<dyn Fn(&Account, i64, &Cancel) -> Res<Option<Playability>> + Send + Sync>;
/// Asks Roblox's join answer for (account, target): the checks answer for Roblox.
pub type AskJoin = Arc<dyn Fn(&Account, &GameTarget, &Cancel) -> Res<JoinAnswer> + Send + Sync>;

#[derive(Default)]
pub struct RejoinState {
    pub game_ban_noted: RefCell<HashSet<i32>>,
    pub no_rejoin_told: RefCell<HashSet<i32>>,
    /// Windows whose drop was logged as not rejoined (once each).
    pub rejoin_held_noted: RefCell<HashSet<i32>>,
    pub join_checks: RefCell<HashMap<i32, JoinCheck>>,
    /// The answer that gave a client its place in a full game's line, shown until the new one.
    pub in_line: RefCell<HashMap<i32, JoinAnswer>>,
    /// Launches refused (JoinAnswer::refused) or held for a bot check: not rejoined once they end. By reference, not
    /// value: a launch's start time moves with a set-back clock.
    pub join_refused: RefCell<Vec<Rc<PendingLaunch>>>,
    /// A field, so the checks can answer for Roblox. None: roblox_api::get_playability.
    pub ask_playability: RefCell<Option<AskPlayability>>,
    /// A field, so the checks can answer for Roblox. None: roblox_api::join_check.
    pub ask_join: RefCell<Option<AskJoin>>,
    /// Why due rejoins last waited, as logged (None: none waiting).
    pub rejoin_held_why: RefCell<Option<String>>,
    /// A field, so the checks can answer for Windows. None: system_memory::read.
    pub read_memory: RefCell<Option<Rc<dyn Fn() -> MemoryStatus>>>,
    pub memory_held_told: RefCell<HashSet<Guid>>,
    /// Accounts whose window was hidden when their client ended: the window their rejoin opens is hidden too.
    pub hide_when_in: RefCell<HashSet<Guid>>,
    pub ran_out_of_memory: RefCell<HashSet<Guid>>,
    /// The rejoin `rejoin` is doing now: out of the queue, its window not asked for yet.
    pub rejoin_in_flight: RefCell<Option<Rc<RejoinRequest>>>,
    /// Clients already being closed for a kick in their log (so it happens once).
    pub closing_for_rejoin: RefCell<HashSet<i32>>,
    /// Rejoins that took `rejoin_cts` so far (see rejoin).
    pub rejoin_generation: std::cell::Cell<u64>,
}

/// Rejoins in a row after which "keeps getting disconnected" is said, once. Every rejoin waits REJOIN_DELAY however
/// many came before (the owner's choice, 2026-10-21): a game's ban stops them instead.
pub const KEEPS_DROPPING: i32 = 2;
/// A client in its game this long is fine again: its next drop counts from one.
pub const STAY_RESETS_REJOINS: Duration = Duration::from_secs(10 * 60);
/// No answer in time, no question.
pub const PLAYABILITY_LIMIT: Duration = Duration::from_secs(6);
pub const JOIN_CHECK_RETRY: Duration = Duration::from_secs(30);
pub const QUEUE_RECHECK: Duration = Duration::from_secs(60);
pub const GAME_BAN_RECHECK_LIMIT: Duration = Duration::from_secs(6);
pub const MEMORY_HOLD_RECHECK: Duration = Duration::from_secs(30);
/// How long anti-AFK waits after a window was held on Roblox's bot check (its nudges would take the front).
pub const BOT_CHECK_HOLD: Duration = Duration::from_secs(5 * 60);
/// Roblox's code for "the same account joined from another device": never fight that with a rejoin.
pub const SAME_ACCOUNT_ELSEWHERE: i32 = 273;
/// Roblox's ServerShutdown: the game's server closed (an update, the developer's shutdown).
pub const SERVER_SHUTDOWN: i32 = 288;
/// Roblox's code for "kicked for being idle 20 minutes".
pub const IDLE_KICK: i32 = 278;
/// Codes whose Roblox error box has no Reconnect button (reconnectDisabledList in Connection.lua, roblox_codes.rs): a
/// rejoin would only be refused again. Bloxkeep skips them too, except a game's kick (267), which games often ask to
/// rejoin after.
pub fn no_reconnect(code: i32) -> bool {
    code != 267 && crate::roblox_codes::NO_RECONNECT.contains(&code)
}

/// Roblox's DisconnectRejoin (276): same message as SAME_ACCOUNT_ELSEWHERE, so taken the same way.
pub fn signed_in_elsewhere(code: Option<i32>) -> bool {
    matches!(code, Some(SAME_ACCOUNT_ELSEWHERE) | Some(276))
}

pub fn roblox_wont_reconnect(code: Option<i32>) -> bool {
    code.is_some_and(no_reconnect)
}

/// Codes Roblox's own error screen reconnects by itself (autoReconnectAllowedList in Connection.lua). Only these are said
/// as a lost connection; a disconnect without a code is said as one with an unknown reason.
pub fn lost_connection(code: i32) -> bool {
    crate::roblox_codes::AUTO_RECONNECT.contains(&code)
}

/// Why Roblox's join answer refuses: the status in the client's own words (its error is 512 + the status, so 12 is 524
/// PlacelaunchUnauthorized, 30 is 542 PlacelaunchAgeVerificationRequired: roblox_codes.rs), then Roblox's message when it
/// sent one ("Unable to join Game 311"); a status outside 1 to 33 (the ones that line up by name) is said by its number.
pub fn join_refusal_words(status: Option<i32>, message: Option<&str>) -> String {
    let known = status.and_then(|s| crate::roblox_codes::why(512 + s)).filter(|c| (513..=545).contains(&c.code));
    let message = message.map(str::trim).filter(|m| !m.is_empty());
    // Roblox's own words win: status and code don't always line up (status 29 came with a trusted-friend message,
    // while 512 + 29 is the private-server-owner code), so the table's words only stand in when Roblox said nothing.
    match (known, message) {
        (_, Some(m)) => m.to_string(),
        (Some(c), None) => l::t(c.words),
        (None, None) => l::f("Roblox answered status {0}", &[&status.map(|s| s.to_string()).unwrap_or_default()]),
    }
}

pub fn rejoin_place(launch: &PendingLaunch, last_log: Option<&ClientState>) -> i64 {
    last_log.and_then(|l| l.chosen_place).or(launch.target.as_ref().map(|t| t.place_id)).unwrap_or(0)
}

pub fn playability_why(answer: &Playability) -> String {
    match answer.status {
        Some(2) => l::t("Roblox doesn't take this account's sign-in"),
        Some(3 | 9) => l::t("the game is under review by Roblox"),
        Some(4 | 13) => l::t("the game isn't set up to be played"),
        Some(5) => l::t("the game is private"),
        Some(6) => l::t("only the creator's friends can play it"),
        Some(7) => l::t("only the group's members can play it"),
        Some(8 | 22) => l::t("it can't be played on this device"),
        Some(10 | 21) => l::t("the game has to be bought first"),
        Some(11) => l::t("Roblox has restricted this account"),
        Some(12) => l::t("the game is temporarily unavailable"),
        Some(14..=16) => l::t("it isn't available in this region"),
        Some(17 | 18 | 27) => l::t("parental controls block it"),
        Some(19 | 20 | 24 | 26) => l::t("it needs an age check"),
        Some(23) => l::t("the game has no age rating yet"),
        Some(29) => l::t("it needs a trusted friend"),
        Some(30) => l::t("it needs Roblox Plus"),
        Some(31..=33) => l::t("only the game's editors or testers can play it"),
        _ => match &answer.text {
            Some(said) => said.trim_end_matches('.').to_string(),
            None => {
                let status = answer.status.map(|s| s.to_string()).unwrap_or_else(|| "?".into());
                l::f("Roblox says no (status {0})", &[&status])
            }
        },
    }
}

impl App {
    pub fn on_session_ended(&self, account_id: Guid, launch: &Rc<PendingLaunch>, last_log: Option<ClientState>) {
        self.note_session_end(account_id, launch, last_log.as_ref()); // before auto-rejoin reads (and forgets) whether Roblox closed it
        self.controller.offline_strikes.borrow_mut().remove(&account_id);
        // A game ban in its last lines, unseen before the window closed: noted all the same, or the next preset would
        // open the account into Join Error 600.
        if let Some(log) = last_log.as_ref().filter(|l| l.banned_by_game)
            && self.find_account(account_id).is_some()
        {
            let place = log.chosen_place.or(launch.target.as_ref().map(|t| t.place_id)).or(log.place_id);
            self.note_game_ban(account_id, place, log.kick_message.clone(), launch.started_at.get());
        }
        // Closed for good: a scheduled stop no longer owns the account, so a client opened by hand later stays open.
        if !self.queue_rejoin(account_id, launch, last_log.as_ref()) {
            self.forget_scheduled_open(account_id);
        }
    }

    /// True when auto-rejoin (or the preset's planned restart) will bring the client back.
    pub(crate) fn queue_rejoin(&self, account_id: Guid, launch: &Rc<PendingLaunch>, last_log: Option<&ClientState>) -> bool {
        let out_of_memory = self.rejoin.ran_out_of_memory.borrow_mut().remove(&account_id);
        let restarting = self.presets.restarts.borrow_mut().remove(&account_id);
        let restart = restarting.is_some();
        let Some(account) = self.should_rejoin(account_id, launch, last_log, restart) else {
            self.controller.rejoins_in_a_row.borrow_mut().remove(&account_id); // closed for good: a client opened later counts from one again
            return false;
        };
        self.controller.rejoin_queue.borrow_mut().retain(|r| r.account_id != account_id);
        if let Some((restart_target, _)) = restarting {
            // Not a drop: not counted, and goes ahead whatever auto-rejoin says.
            let place = restart_target.place_id;
            self.controller.rejoin_queue.borrow_mut().push(RejoinRequest {
                account_id,
                target: restart_target,
                due: Stamp::utc_now().plus_secs(secs(REJOIN_DELAY)),
                network_retries: 0,
                preset_id: launch.preset_id,
                restart: true,
                open_retries: 0,
            });
            log::note(&format!("Rejoin: account {} reopens in {} s (a restart), place {place}.", short(account_id), REJOIN_DELAY.as_secs()));
            return true;
        }
        let in_a_row = self.controller.rejoins_in_a_row.borrow().get(&account_id).copied().unwrap_or(0);
        self.controller.rejoins_in_a_row.borrow_mut().insert(account_id, in_a_row + 1);
        self.join_player.follow_wait_told.borrow_mut().remove(&account_id); // a new drop: waiting for its followed player is said again
        let wait = REJOIN_DELAY;
        // The same game or private server, not the exact server, which may be gone. A game the person picked in the
        // window since wins over the one opened. A followed player is looked up again (follow_target).
        let opened = launch.target.clone().unwrap_or_default();
        let target = match last_log.and_then(|l| l.chosen_place) {
            Some(chosen) if chosen != opened.place_id => GameTarget { place_id: chosen, follow: opened.follow.clone(), ..Default::default() },
            _ => GameTarget { job_id: None, ..opened },
        };
        self.controller.rejoin_queue.borrow_mut().push(RejoinRequest {
            account_id,
            target: target.clone(),
            due: Stamp::utc_now().plus_secs(secs(wait)),
            network_retries: 0,
            preset_id: launch.preset_id,
            restart: false,
            open_retries: 0,
        });
        log::note(&format!(
            "Rejoin: account {} queued in {} s, drop {} in a row, place {}{}{} (last log: {}{}).",
            short(account_id),
            wait.as_secs(),
            in_a_row + 1,
            target.place_id,
            if target.follow.is_some() { ", following a player" } else { "" },
            if out_of_memory { ", ran out of memory" } else { "" },
            last_log.map(|l| l.status.as_str()).unwrap_or("none"),
            last_log.and_then(|l| l.reason).map(|c| format!(", code {c}")).unwrap_or_default()
        ));
        let n = self.name(&account);
        let w = wait.as_secs();
        if in_a_row == KEEPS_DROPPING {
            // Said once; it goes on trying all the same.
            self.notify(&l::f("{0} keeps getting disconnected — Bloxkeep keeps rejoining every {1}s.", &[&n, &w]), "warn", "rejoinSlow", account.id, true, None);
        }
        // Its rejoin waits for room (run_due_rejoin).
        else if out_of_memory {
            self.notify(&l::f("{0} ran out of memory and Roblox crashed — rejoining in {1}s…", &[&n, &w]), "warn", "drop", account.id, true, None);
        } else {
            self.notify(&l::f("{0} left the game — rejoining in {1}s…", &[&n, &w]), "info", "drop", account.id, true, None);
        }
        true
    }

    /// The account when its window goes back; None (logged why) when it doesn't.
    pub(crate) fn should_rejoin(&self, account_id: Guid, launch: &Rc<PendingLaunch>, last_log: Option<&ClientState>, restart: bool) -> Option<Account> {
        let no = |reason: &str| -> Option<Account> {
            log::note(&format!("Rejoin: account {}'s window ended, not rejoined ({reason}).", short(account_id)));
            None
        };
        if self.controller.user_closed.borrow_mut().remove(&account_id) {
            return no("closed on purpose");
        }
        // Roblox closed it when another window opened: a rejoin would only close the next one (roblox_updates.rs).
        if self.roblox_updates.closed_by_roblox.borrow_mut().remove(&account_id) {
            return no("Roblox closed it as another opened");
        }
        // Roblox refused the account its game (rejoin_from_logs): a new window would only be refused again.
        if remove_launch(&mut self.rejoin.join_refused.borrow_mut(), launch) {
            return no("Roblox refused the join");
        }
        let found = self.find_account(account_id);
        let Some(found) = found.filter(|_| launch.target.is_some()) else {
            return no(if launch.target.is_none() { "opened without a game" } else { "account gone" });
        };
        // Banned: a new window would only show the ban again, every few minutes for good.
        if found.banned == Some(true) {
            return no("banned on Roblox");
        }
        // Banned by the game: only Join Error 600 again.
        if last_log.is_some_and(|l| l.banned_by_game) || self.game_ban_of(&found, Some(rejoin_place(launch, last_log))).is_some() {
            return no("banned by the game");
        }
        if restart {
            return Some(found);
        }
        if !self.rejoin_on(&found, launch.preset_id) {
            return no("auto-rejoin off");
        }
        // A leave by choice, a sign-in from another device, or a code Roblox won't reconnect after: not a drop.
        let drop = !last_log.is_some_and(|l| l.status == "left" || l.status == "signedout")
            && !last_log.is_some_and(|l| l.status == "disconnected" && (signed_in_elsewhere(l.reason) || roblox_wont_reconnect(l.reason)));
        if drop {
            Some(found)
        } else {
            no(&format!(
                "log says {}{}",
                last_log.map(|l| l.status.as_str()).unwrap_or(""),
                last_log.and_then(|l| l.reason).map(|c| format!(", code {c}")).unwrap_or_default()
            ))
        }
    }

    pub fn forget_settled_rejoins(&self, now: Stamp) {
        if self.controller.rejoins_in_a_row.borrow().is_empty() {
            return;
        }
        for (id, info) in running(self) {
            if !self.controller.rejoins_in_a_row.borrow().contains_key(&id) {
                continue;
            }
            // From the window's start, not its last server: a game that teleports every few minutes would never
            // settle, and three drops in a day would read as "keeps getting disconnected".
            let long_enough = now.secs_since(info.1.started_at.get()) >= secs(STAY_RESETS_REJOINS);
            let settled = match log_of(self, info.0) {
                Some(log) => log.status == "ingame" && long_enough,
                None => {
                    long_enough
                        && self
                            .find_account(id)
                            .is_some_and(|account| self.controller.presence.borrow().get(&account.user_id).is_some_and(|p| p.r#type == 2))
                }
            };
            if settled {
                self.controller.rejoins_in_a_row.borrow_mut().remove(&id);
            }
        }
    }

    pub fn cancel_rejoin(&self, id: Guid) -> serde_json::Value {
        self.controller.rejoins_in_a_row.borrow_mut().remove(&id);
        let under_way = self.cancel_rejoin_under_way(id);
        let removed = {
            let mut q = self.controller.rejoin_queue.borrow_mut();
            let before = q.len();
            q.retain(|r| r.account_id != id);
            before - q.len()
        };
        if removed > 0 || under_way {
            log::note(&format!("Rejoin: account {}'s rejoin cancelled by hand.", short(id)));
            self.forget_scheduled_open(id); // closed for good now
        }
        self.push_state();
        serde_json::Value::Null
    }

    /// Acts on a kick or lost connection as soon as the client logs it, rather than after two presence checks, and on
    /// a game start that never got a server (GET_IN_TIMEOUT).
    pub fn rejoin_from_logs(self: &Rc<Self>) {
        // First, also with nothing open: a process id Windows gives a later client mustn't come with an old one's answer.
        let alive: HashSet<i32> = running(self).iter().map(|(_, r)| r.0).collect();
        self.rejoin.join_checks.borrow_mut().retain(|pid, _| alive.contains(pid));
        self.rejoin.game_ban_noted.borrow_mut().retain(|pid| alive.contains(pid));
        self.rejoin.no_rejoin_told.borrow_mut().retain(|pid| alive.contains(pid));
        self.rejoin.rejoin_held_noted.borrow_mut().retain(|pid| alive.contains(pid));
        self.rejoin.in_line.borrow_mut().retain(|pid, _| alive.contains(pid));
        let launches: Vec<Rc<PendingLaunch>> = running(self).into_iter().map(|(_, r)| r.1).collect();
        self.rejoin.join_refused.borrow_mut().retain(|l| has_launch(&launches, l));
        if vault_now(self).is_none() || self.controller.running.borrow().is_empty() || self.settling() {
            return;
        }
        let now = Stamp::utc_now();
        for (id, info) in running(self) {
            let Some(state) = log_of(self, info.0) else {
                continue;
            };
            if self.rejoin.closing_for_rejoin.borrow().contains(&info.0) {
                continue;
            }
            // A rejoin of a hidden window: hidden again once in its game.
            if state.status == "ingame" {
                let marked = self.rejoin.hide_when_in.borrow_mut().remove(&id);
                if marked && info.1.rejoin {
                    roblox_processes::hide_windows(&[info.0]);
                    self.push_state();
                }
            }
            // Got a server since, or the person started a game: no longer held as banned.
            if state.status == "ingame" || (state.status == "joining" && (state.job_id.is_some() || state.person_started)) {
                self.rejoin.game_ban_noted.borrow_mut().remove(&info.0);
                self.rejoin.join_checks.borrow_mut().remove(&info.0);
            }
            // Banned by the game (code 267 as for any kick): a new window would only meet Join Error 600. The window
            // stays, with Roblox's message. The ban is noted for the game a rejoin would open, not the place it was in.
            if state.status == "disconnected" && state.banned_by_game {
                let newly = self.rejoin.game_ban_noted.borrow_mut().insert(info.0);
                if newly && self.find_account(id).is_some() {
                    let place = state.chosen_place.or(info.1.target.as_ref().map(|t| t.place_id)).or(state.place_id);
                    self.note_game_ban(id, place, state.kick_message.clone(), info.1.started_at.get());
                }
                continue;
            }
            let stuck_joining = state.status == "joining" && state.job_id.is_none() && !state.person_started;
            // Held for Roblox's bot check (RobloxLogWatcher's CHALLENGED_MARK): closing and reopening it looped into the same
            // check for good. The window stays, why is said once, and it isn't rejoined once closed.
            if stuck_joining && self.controller.logs.borrow().join_challenged_at(info.0).is_some() {
                let newly_held = add_launch(&mut self.rejoin.join_refused.borrow_mut(), &info.1);
                if newly_held {
                    log::note(&format!("Rejoin: window {} held on Roblox's bot check; left open, not rejoined.", info.0));
                }
                if newly_held
                    && let Some(challenged) = self.find_account(id)
                    && self.rejoin_on(&challenged, info.1.preset_id)
                {
                    let n = self.name(&challenged);
                    self.notify(
                        &l::f(
                            "{0} can't get into the game: Roblox wants a check that it isn't a bot first, so Bloxkeep leaves the window open and doesn't try again.",
                            &[&n],
                        ),
                        "warn",
                        "drop",
                        challenged.id,
                        true,
                        None,
                    );
                }
                continue;
            }
            // Not in after join_check_after: Roblox says why only in the window (a game's ban is Join Error 600) and logs
            // nothing, so its join API is asked. A ban or refusal stops auto-rejoin; a place in a full game's line keeps
            // the window; any other answer leaves it to GET_IN_TIMEOUT.
            if stuck_joining
                && let Some(wanted) = info.1.target.clone()
                && let Some(asking) = self.find_account(id).filter(|a| !a.cookie.is_empty())
                && now.secs_since(state.at) >= secs(self.join_check_after(&asking, &wanted))
                && !self.rejoin.game_ban_noted.borrow().contains(&info.0)
            {
                // Asked again after JOIN_CHECK_RETRY without an answer (often no network after a wake), and every
                // QUEUE_RECHECK while in line.
                let asked = self.rejoin.join_checks.borrow().get(&info.0).cloned();
                let answer_now: Option<Option<JoinAnswer>> = asked.as_ref().and_then(|a| a.answer.borrow().clone());
                let in_line_answer = match &answer_now {
                    Some(Some(r)) if r.queue_position.is_some_and(|q| q > 0) => Some(r.clone()),
                    _ => None,
                };
                let asked_at = asked.as_ref().map(|a| a.at);
                let since_asked = asked_at.map(|at| now.secs_since(at)).unwrap_or(0.0);
                if asked.is_none()
                    || (matches!(answer_now, Some(None)) && since_asked >= secs(JOIN_CHECK_RETRY))
                    || (in_line_answer.is_some() && since_asked >= secs(QUEUE_RECHECK))
                {
                    match &in_line_answer {
                        Some(r) => {
                            self.rejoin.in_line.borrow_mut().insert(info.0, r.clone());
                        }
                        None => {
                            self.rejoin.in_line.borrow_mut().remove(&info.0);
                        }
                    }
                    let why = if in_line_answer.is_some() { "A join in a full game's line, asked again" } else { "A join that never got in, asked again" };
                    let slot = self.spawn_check_join(asking.clone(), wanted.clone(), why, Cancel::new());
                    self.rejoin.join_checks.borrow_mut().insert(info.0, JoinCheck { answer: slot, at: now });
                    continue;
                }
                if let Some(Some(r)) = &answer_now
                    && r.banned()
                {
                    self.rejoin.game_ban_noted.borrow_mut().insert(info.0);
                    self.note_game_ban(asking.id, Some(wanted.place_id), r.reason(), info.1.started_at.get());
                    continue;
                }
                if let Some(result) = &answer_now {
                    self.forget_ban_if_let_in(asking.id, wanted.place_id, result.as_ref());
                }
                // A private server's join is asked as its public game's, so only a ban counts from that answer.
                if let Some(Some(said)) = &answer_now
                    && wanted.link_code.is_none()
                    && wanted.share_code.is_none()
                    && wanted.access_code.is_none()
                {
                    // A new window would only be refused again, every minute for good. The window stays; why is said once.
                    if said.refused() {
                        let newly_refused = add_launch(&mut self.rejoin.join_refused.borrow_mut(), &info.1);
                        if newly_refused {
                            log::note(&format!(
                                "Rejoin: window {} refused by Roblox's join answer (status {}); left open, not rejoined.",
                                info.0,
                                said.status.map(|s| s.to_string()).unwrap_or_default()
                            ));
                        }
                        if newly_refused && self.rejoin_on(&asking, info.1.preset_id) {
                            let n = self.name(&asking);
                            let why = join_refusal_words(said.status, said.message.as_deref());
                            self.notify(&l::f("{0} can't get into the game, so Bloxkeep doesn't try again: {1}", &[&n, &why]), "warn", "drop", asking.id, true, None);
                        }
                        continue;
                    }
                    // In a full game's line: a new window would start at its end.
                    if said.queue_position.is_some_and(|q| q > 0) {
                        continue;
                    }
                }
            }
            // Stuck on a game's ban (Join Error 600): left as it is, not closed by the timeout below.
            if self.rejoin.game_ban_noted.borrow().contains(&info.0) {
                continue;
            }
            // Roblox's error box has no Reconnect for this code: the window stays, and why is said once (not for a drop
            // from before this run).
            if state.status == "disconnected"
                && let Some(refused) = state.reason
                && roblox_wont_reconnect(Some(refused))
            {
                let newly_kept = self.rejoin.no_rejoin_told.borrow_mut().insert(info.0);
                if newly_kept {
                    log::note(&format!("Rejoin: window {} disconnected with code {refused}, which Roblox doesn't reconnect after; left open.", info.0));
                }
                if newly_kept
                    && state.at >= self.relaunch.started.get()
                    && let Some(kept) = self.find_account(id)
                    && self.rejoin_on(&kept, info.1.preset_id)
                {
                    let n = self.name(&kept);
                    let why = super::activity::kick_reason(refused);
                    self.notify(&l::f("{0} isn't rejoined ({1}): Roblox doesn't allow reconnecting after this.", &[&n, &why]), "warn", "drop", kept.id, true, None);
                }
                continue;
            }
            let dropped = state.status == "disconnected" && !signed_in_elsewhere(state.reason) && now.secs_since(state.at) >= 3.0;
            // One the person started in the window is theirs: closing it would reopen the game Bloxkeep opened instead.
            let never_got_in = stuck_joining && now.secs_since(state.at) >= secs(GET_IN_TIMEOUT);
            let account = self.find_account(id);
            let go = (dropped || never_got_in)
                && account.as_ref().is_some_and(|a| {
                    self.rejoin_on(a, info.1.preset_id)
                        && info.1.target.is_some()
                        // No rejoin follows (should_rejoin): the window stays rather than closing for nothing.
                        && a.banned != Some(true)
                        && self.game_ban_of(a, Some(rejoin_place(&info.1, Some(&state)))).is_none()
                });
            if !go {
                // Once per window: why a drop doesn't lead to a rejoin.
                if (dropped || never_got_in) && self.rejoin.rejoin_held_noted.borrow_mut().insert(info.0) {
                    let why = match self.find_account(id) {
                        None => "account gone",
                        Some(held) if !self.rejoin_on(&held, info.1.preset_id) => "auto-rejoin off",
                        Some(_) if info.1.target.is_none() => "opened without a game",
                        Some(held) if held.banned == Some(true) => "banned on Roblox",
                        Some(_) => "banned by the game",
                    };
                    log::note(&format!(
                        "Rejoin: window {} {}, left open ({why}).",
                        info.0,
                        if dropped {
                            format!("disconnected (code {})", state.reason.map(|c| c.to_string()).unwrap_or_else(|| "none".into()))
                        } else {
                            "never got into its game".to_string()
                        }
                    ));
                }
                continue;
            }
            let account = account.expect("checked above");
            let target = info.1.target.clone().expect("checked above");
            // Not trying again before Roblox's answer is in (asked at join_check_after, above).
            if never_got_in
                && let Some(pending) = self.rejoin.join_checks.borrow().get(&info.0)
                && pending.answer.borrow().is_none()
            {
                continue;
            }
            log::note(&format!(
                "Rejoin: closing window {} to rejoin, {}.",
                info.0,
                if dropped {
                    format!(
                        "disconnected (code {}{})",
                        state.reason.map(|c| c.to_string()).unwrap_or_else(|| "none".into()),
                        if state.kick_message.as_deref().is_some_and(|m| !m.is_empty()) { ", kicked with a message" } else { "" }
                    )
                } else {
                    format!("no server after {} s", GET_IN_TIMEOUT.as_secs())
                }
            ));
            self.rejoin.closing_for_rejoin.borrow_mut().insert(info.0);
            // Said as what it was: on 2026-10-21 a game's "DataStore lock lost, please rejoin the game." went out as
            // "Lost connection".
            let n = self.name(&account);
            let text = if !dropped {
                if target.job_id.is_some() {
                    l::f("{0} couldn't get into that server (full or closed) — joining another one…", &[&n])
                } else {
                    l::f("{0} couldn't get into the game — trying again…", &[&n])
                }
            } else if let Some(words) = state.kick_message.as_deref().filter(|m| !m.is_empty()) {
                l::f("{0} was kicked by the game, rejoining: {1}", &[&n, &words])
            } else if state.reason == Some(267) {
                l::f("{0} was kicked by the game — rejoining…", &[&n])
            } else if state.reason == Some(SERVER_SHUTDOWN) {
                l::f("{0}'s server shut down — joining another one…", &[&n])
            } else if let Some(lost) = state.reason.filter(|c| lost_connection(*c)) {
                let _ = lost;
                l::f("{0} lost connection — restarting Roblox…", &[&n])
            } else if let Some(other) = state.reason {
                let why = super::activity::kick_reason(other);
                l::f("{0} was disconnected ({1}) — restarting Roblox…", &[&n, &why])
            } else {
                l::f("{0} dropped out of the game, reason unknown — restarting Roblox…", &[&n])
            };
            self.notify(&text, "info", "drop", account.id, true, None);
            roblox_processes::close(info.0, false);
        }
    }

    /// The accounts Roblox says can't play the game, with why. An unknown game, no sign-in or no answer in time: none.
    pub async fn unplayable(self: &Rc<Self>, accounts: &[Account], place_id: i64, ct: &Cancel) -> Res<Vec<(Account, String)>> {
        let asking: Vec<Account> = accounts.iter().filter(|a| !a.cookie.is_empty() && a.cookie_valid != Some(false)).cloned().collect();
        if asking.is_empty() {
            return Ok(Vec::new());
        }
        let limit = linked(ct, PLAYABILITY_LIMIT);
        let timed_out = |limit: &Cancel| limit.is_cancelled() && !ct.is_cancelled();
        if !self.has_game(place_id, false) {
            // LoadGamesAsync(...).WaitAsync(limit): given up on (not called off) when the limit comes first.
            let load = self.load_games(vec![(place_id, 0)], false);
            let wait = {
                let l2 = limit.clone();
                rt::blocking(move || {
                    while !l2.is_cancelled() {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                })
            };
            futures::pin_mut!(load);
            futures::pin_mut!(wait);
            match futures::future::select(load, wait).await {
                futures::future::Either::Left((done, _)) => done?,
                futures::future::Either::Right(_) => {
                    if timed_out(&limit) {
                        log::note(&format!("Playability of place {place_id}: no answer within {} s; not asked.", PLAYABILITY_LIMIT.as_secs()));
                        return Ok(Vec::new());
                    }
                    return Err(Fail::other("cancelled"));
                }
            }
        }
        let universe = self.controller.games.borrow().get(&place_id).and_then(|g| g.as_ref()).map(|g| g.universe_id);
        let Some(universe) = universe.filter(|u| *u > 0) else {
            return Ok(Vec::new());
        };
        let ask = self.rejoin.ask_playability.borrow().clone();
        // Eight at a time (SemaphoreSlim(8)); answers in the accounts' order.
        let asked: Vec<(Account, Res<Option<Playability>>)> = futures::stream::iter(asking.iter().cloned())
            .map(|account| {
                let (ask, limit, outer) = (ask.clone(), limit.clone(), ct.clone());
                async move {
                    if limit.is_cancelled() {
                        let answer = if outer.is_cancelled() { Err(Fail::other("cancelled")) } else { Ok(None) };
                        return (account, answer);
                    }
                    let a = account.clone();
                    let answer = rt::blocking(move || match ask {
                        Some(f) => f(&a, universe, &limit),
                        None => roblox_api::get_playability(&a.cookie, &a.realm, universe, &limit),
                    })
                    .await;
                    let answer = match answer {
                        Ok(p) => Ok(p),
                        // Expected errors, or the limit (not the caller) calling it off: no answer.
                        Err(ex) if roblox_api::is_expected_error(&ex) && !(is_fail_kind(&ex, roblox_api::TASK_CANCELED_EXCEPTION) && outer.is_cancelled()) => Ok(None),
                        Err(ex) if is_fail_kind(&ex, roblox_api::TASK_CANCELED_EXCEPTION) && !outer.is_cancelled() => Ok(None),
                        Err(ex) => Err(ex),
                    };
                    (account, answer)
                }
            })
            .buffered(8)
            .collect()
            .await;
        let mut answers: Vec<(Account, Option<Playability>)> = Vec::new();
        for (account, answer) in asked {
            answers.push((account, answer?));
        }
        let unanswered = answers.iter().filter(|x| x.1.is_none()).count();
        let refused_here = answers.iter().filter(|x| x.1.as_ref().is_some_and(|a| !a.playable)).count();
        if unanswered > 0 || refused_here > 0 {
            let mut statuses: Vec<String> = Vec::new();
            for s in answers.iter().filter_map(|x| x.1.as_ref().filter(|a| !a.playable)).map(|a| a.status.map(|s| s.to_string()).unwrap_or_else(|| "?".into())) {
                if !statuses.contains(&s) {
                    statuses.push(s);
                }
            }
            log::note(&format!(
                "Playability of place {place_id}: {refused_here} of {} account(s) can't play{}, {unanswered} unanswered.",
                asking.len(),
                if refused_here > 0 { format!(" (status {})", statuses.join(", ")) } else { String::new() }
            ));
        }
        Ok(answers.into_iter().filter_map(|(acc, a)| a.filter(|a| !a.playable).map(|a| (acc, playability_why(&a)))).collect())
    }

    /// check_join run on its own; its answer lands in the slot (as C#'s Task kept with the window).
    pub fn spawn_check_join(self: &Rc<Self>, account: Account, target: GameTarget, why: &str, ct: Cancel) -> JoinSlot {
        let slot: JoinSlot = Rc::new(RefCell::new(None));
        let (app, s, why) = (self.clone(), slot.clone(), why.to_string());
        rt::spawn(async move {
            let answer = app.check_join(&account, &target, &why, &ct).await;
            *s.borrow_mut() = Some(answer);
        });
        slot
    }

    /// Asked as a Roblox window asks when joining: a game's ban comes back as its message. The answer is logged after
    /// `why` (no account, no ticket) as proof of what Roblox answers.
    pub async fn check_join(self: &Rc<Self>, account: &Account, target: &GameTarget, why: &str, ct: &Cancel) -> Option<JoinAnswer> {
        let ask = self.rejoin.ask_join.borrow().clone();
        let (a, t, c) = (account.clone(), target.clone(), ct.clone());
        let asked = rt::blocking(move || match ask {
            Some(f) => f(&a, &t, &c),
            None => roblox_api::join_check(&a.cookie, t.place_id, t.job_id.as_deref(), &a.realm, &c, None),
        })
        .await;
        match asked {
            Ok(mut answer) => {
                // A server named and refused without a ban: that server is gone (JoinAnswer::server_gone).
                if target.job_id.as_deref().is_some_and(|j| !j.is_empty()) && answer.status == Some(12) && !answer.banned() {
                    answer.server_gone = true;
                }
                log::note(&format!(
                    "{why}: Roblox answered {}, status {}: {}{}{}{}",
                    answer.http,
                    answer.status.map(|s| s.to_string()).unwrap_or_default(),
                    answer.message.as_deref().unwrap_or("(no message)"),
                    answer.data.as_ref().map(|d| format!(" · statusData {d}")).unwrap_or_default(),
                    if answer.banned() { " (a ban by the game)" } else { "" },
                    if answer.server_gone { " (that server is gone)" } else { "" }
                ));
                // No status: the request itself was refused (rate limit, server error, sign-in), so it is asked again.
                if answer.status.is_none() && !answer.banned() { None } else { Some(answer) }
            }
            Err(ex) => {
                log::note(&format!("{why}: no answer ({}).", roblox_api::fail_type_name(&ex)));
                None
            }
        }
    }

    /// Status 2 (a join under way): a ban noted for this game is over.
    fn forget_ban_if_let_in(&self, account_id: Guid, place_id: i64, answer: Option<&JoinAnswer>) -> bool {
        if !answer.is_some_and(|a| !a.banned() && a.status == Some(2)) {
            return false;
        }
        let Some(account) = self.find_account(account_id) else {
            return false;
        };
        let gone: Vec<i64> = account.game_bans.iter().filter(|b| self.same_game(b.place_id, place_id)).map(|b| b.place_id).collect();
        if gone.is_empty() {
            return false;
        }
        let _ = self.update_account(account_id, |a| a.game_bans.retain(|b| !gone.contains(&b.place_id)));
        log::note(&format!("Game ban: account {}'s ban from place {place_id} is over (Roblox lets it join).", short(account_id)));
        self.persist_soon();
        self.push_state();
        true
    }

    /// A game's ban may last a day: before opening, Roblox's join answer says whether a noted one is over. No answer
    /// within GAME_BAN_RECHECK_LIMIT keeps the ban.
    pub async fn recheck_game_bans(self: &Rc<Self>, accounts: &[Account], target: &GameTarget, ct: &Cancel) -> Res<()> {
        let asking: Vec<Account> = accounts.iter().filter(|acc| self.game_ban_of(acc, Some(target.place_id)).is_some() && !acc.cookie.is_empty()).cloned().collect();
        if asking.is_empty() {
            return Ok(());
        }
        let limit = linked(ct, GAME_BAN_RECHECK_LIMIT);
        self.progress(Some(&l::t("Asking Roblox whether the game's ban still stands…")));
        let answers = futures::future::join_all(asking.iter().map(|acc| self.check_join(acc, target, "A game's ban noted before, asked before opening", &limit))).await;
        self.progress(None);
        for (acc, answer) in asking.iter().zip(answers.iter()) {
            self.forget_ban_if_let_in(acc.id, target.place_id, answer.as_ref());
        }
        Ok(())
    }

    /// Noted with the account so auto-rejoin stops trying that game, and notified. Getting in again later drops it
    /// (forget_lifted_game_bans).
    /// `since`: when the window opened: a ban noted since is this one, seen again after Bloxkeep restarted with the
    /// window still open, and isn't told again.
    pub fn note_game_ban(&self, account_id: Guid, place_id: Option<i64>, message: Option<String>, since: Stamp) {
        let Some(place) = place_id.filter(|p| *p > 0) else {
            return;
        };
        let Some(account) = self.find_account(account_id) else {
            return;
        };
        let known = self.game_ban_of(&account, Some(place));
        if let Some(known) = known.as_ref().filter(|k| k.at >= since) {
            if known.message.is_none() && message.is_some() {
                let known_place = known.place_id;
                let _ = self.update_account(account_id, |a| {
                    if let Some(b) = a.game_bans.iter_mut().find(|b| b.place_id == known_place) {
                        b.message = message.clone();
                    }
                });
                self.persist_soon();
                self.push_state();
            }
            return;
        }
        // Roblox's join answer carries none of the game's words: the earlier ones are kept.
        let message = message.or_else(|| known.and_then(|k| k.message));
        let game_name = self.controller.games.borrow().get(&place).and_then(|g| g.as_ref()).map(|g| g.name.clone());
        let game = match game_name.as_deref().filter(|n| !n.is_empty()) {
            Some(name) => name.to_string(),
            None => l::f("Place {0}", &[&place]),
        };
        let gone: Vec<i64> = account.game_bans.iter().filter(|b| self.same_game(b.place_id, place)).map(|b| b.place_id).collect();
        let _ = self.update_account(account_id, |a| {
            a.game_bans.retain(|b| !gone.contains(&b.place_id));
            a.game_bans.push(GameBan { place_id: place, game_name: game_name.clone(), at: Stamp::utc_now(), message: message.clone() });
        });
        let with_words = message.as_deref().is_some_and(|m| !m.is_empty());
        log::note(&format!(
            "Game ban: account {} banned from place {place}{}; no rejoins there.",
            short(account_id),
            if with_words { ", with the game's message" } else { "" }
        ));
        self.persist_soon();
        let n = self.name(&account);
        let text = match message.as_deref().filter(|m| !m.is_empty()) {
            Some(m) => l::f("{0} was banned from {1} by the game: {2}", &[&n, &game, &m]),
            None => l::f("{0} was banned from {1} by the game.", &[&n, &game]),
        };
        self.notify(&text, "error", "gameBan", account_id, true, None);
        self.push_state();
    }

    /// Only a window the join answer showed stuck on the ban (Join Error 600): one opened into a game that banned the
    /// account once may well get in.
    pub fn banned_window(&self, account: &Account, run: &Run) -> Option<GameBan> {
        let stuck = self.rejoin.game_ban_noted.borrow().contains(&run.0)
            && log_of(self, run.0).is_some_and(|l| l.status == "joining" && l.job_id.is_none() && !l.person_started);
        if stuck { self.game_ban_of(account, run.1.target.as_ref().map(|t| t.place_id)) } else { None }
    }

    /// Any place of the game: a game's ban holds in all of them.
    pub fn game_ban_of(&self, account: &Account, place_id: Option<i64>) -> Option<GameBan> {
        let place = place_id?;
        account.game_bans.iter().find(|b| self.same_game(b.place_id, place)).cloned()
    }

    pub fn forget_lifted_game_bans(&self) {
        for (id, info) in running(self) {
            let Some(here) = log_of(self, info.0).filter(|l| l.status == "ingame").and_then(|l| l.place_id) else {
                continue;
            };
            let Some(account) = self.find_account(id).filter(|a| !a.game_bans.is_empty()) else {
                continue;
            };
            // The game it is in now, not the one it was opened into.
            let gone: Vec<i64> = account.game_bans.iter().filter(|b| self.same_game(b.place_id, here)).map(|b| b.place_id).collect();
            if !gone.is_empty() {
                let _ = self.update_account(id, |a| a.game_bans.retain(|b| !gone.contains(&b.place_id)));
                log::note(&format!("Game ban: account {}'s ban from place {here} is over (in the game again).", short(id)));
                self.persist_soon();
                self.push_state();
            }
        }
    }

    pub fn run_due_rejoin(self: &Rc<Self>, now: Stamp) {
        // One client at a time, after the wait between accounts. A multi-account launch lets rejoins go while it waits
        // between two, except of the accounts it is about to open.
        let launching = self.controller.launch_cts.borrow().is_some();
        let launch_waiting = self.controller.launch_waiting.borrow().clone();
        let sweeping = now < self.controller.sweep_background_until.get();
        let update_reopen = self.roblox_updates.update_reopen_since.get().is_some();
        if self.controller.rejoin_queue.borrow().is_empty()
            || self.controller.rejoining.get()
            || self.tray.opening_home.get()
            || (launching && launch_waiting.is_none())
            || sweeping
            || !self.controller.pending.borrow().is_empty()
            || self.settling()
            || update_reopen
        {
            // Logged when the reason a due rejoin waits changes, not every tick; one window at a time is the usual pace, not said.
            let held_by = if self.tray.opening_home.get() {
                Some("a home screen opening")
            } else if launching && launch_waiting.is_none() {
                Some("a launch under way")
            } else if sweeping {
                Some("Roblox still closing")
            } else if self.settling() {
                Some("the PC just woke")
            } else if update_reopen {
                Some("Roblox updating (Close, update, reopen)")
            } else {
                None
            };
            let any_due = self.controller.rejoin_queue.borrow().iter().any(|r| r.due <= now);
            if any_due && let Some(held_by) = held_by {
                self.note_rejoin_held(Some(held_by));
            }
            return;
        }
        let due = self
            .controller
            .rejoin_queue
            .borrow()
            .iter()
            .find(|r| r.due <= now && !launch_waiting.as_ref().is_some_and(|w| w.contains(&r.account_id)))
            .cloned();
        let Some(due) = due else {
            return;
        };
        if now.secs_since(self.controller.last_session_start.get()) < secs(self.rejoin_spacing(&due)) {
            return;
        }
        // Commit limit all but used up: a new window would crash as it opens (2026-10-22: "cannot commit OS memory",
        // OutOfMemoryGraphics) and rejoin into the same crash every 10 s. It waits for room; said once.
        let warnings_on = self.vault(|d| d.settings.memory_warnings).ok() != Some(false);
        let reader = self.rejoin.read_memory.borrow().clone();
        let short_of_memory = warnings_on && {
            let memory = match reader {
                Some(f) => f(),
                None => system_memory::read(),
            };
            let players = self.processes.last_players.borrow().as_ref().map(|p| p.0.clone()).unwrap_or_default();
            App::virtual_memory_short(&memory, 1, App::client_commit_mb(&players))
        };
        if short_of_memory {
            {
                let mut q = self.controller.rejoin_queue.borrow_mut();
                if let Some(r) = q.iter_mut().find(|r| **r == due) {
                    r.due = now.plus_secs(secs(MEMORY_HOLD_RECHECK));
                }
            }
            let newly_held = self.rejoin.memory_held_told.borrow_mut().insert(due.account_id);
            if newly_held {
                log::note(&format!("Rejoin: account {} waits, Windows' virtual memory is almost full.", short(due.account_id)));
            }
            if newly_held && let Some(held) = self.find_account(due.account_id) {
                let n = self.name(&held);
                self.notify(
                    &l::f(
                        "{0} waits to rejoin: Windows' virtual memory is almost full, so a new Roblox window would crash as it opens. It rejoins once there's room.",
                        &[&n],
                    ),
                    "warn",
                    "rejoinFail",
                    held.id,
                    true,
                    None,
                );
            }
            self.push_state();
            return;
        }
        self.rejoin.memory_held_told.borrow_mut().remove(&due.account_id);
        self.note_rejoin_held(None);
        {
            let mut q = self.controller.rejoin_queue.borrow_mut();
            if let Some(i) = q.iter().position(|r| *r == due) {
                q.remove(i);
            }
        }
        rt::spawn({
            let app = self.clone();
            async move { app.rejoin(due).await }
        });
    }

    fn note_rejoin_held(&self, why: Option<&str>) {
        if self.rejoin.rejoin_held_why.borrow().as_deref() == why {
            return;
        }
        *self.rejoin.rejoin_held_why.borrow_mut() = why.map(str::to_string);
        if let Some(why) = why {
            let now = Stamp::utc_now();
            let due = self.controller.rejoin_queue.borrow().iter().filter(|r| r.due <= now).count();
            log::note(&format!("Rejoin: {due} due rejoin(s) wait: {why}."));
        }
    }

    /// The preset's wait between accounts, else Settings'; 3 s at least.
    fn rejoin_spacing(&self, request: &RejoinRequest) -> Duration {
        let seconds = self
            .preset_for(request.preset_id)
            .and_then(|p| p.delay_seconds)
            .or_else(|| self.vault(|d| d.settings.launch_delay_seconds).ok())
            .unwrap_or(0);
        Duration::from_secs(seconds.max(3) as u64)
    }

    /// Calls off the account's rejoin under way, so Close (an account, a preset, a scheduled stop) wins over it: it opens
    /// nothing and doesn't go back in the queue. Before, only Close all and the lock could stop it.
    /// Returns whether one was under way.
    pub fn cancel_rejoin_under_way(&self, account_id: Guid) -> bool {
        let in_flight = self.rejoin.rejoin_in_flight.borrow().as_ref().map(|r| r.account_id);
        let cts = self.controller.rejoin_cts.borrow().clone();
        match cts {
            Some(cts) if in_flight == Some(account_id) && !cts.is_cancelled() => {
                cts.cancel();
                true
            }
            _ => false,
        }
    }

    pub(crate) async fn rejoin(self: Rc<Self>, request: RejoinRequest) {
        let vault = vault_now(&self);
        let account = self.find_account(request.account_id);
        // server_hop.rs: taken here, even by a rejoin that stops below.
        let hopping = self.server_hop.hopping.borrow_mut().remove(&request.account_id).is_some();
        let already_open = |app: &App, id: Guid| is_running(app, id) || app.controller.pending.borrow().iter().any(|p| p.account_id == id);
        let stop = match (&vault, &account) {
            (None, _) => Some("locked"),
            (_, None) => Some("account gone"),
            (Some(_), Some(account)) => {
                if account.banned == Some(true) {
                    Some("banned on Roblox")
                } else if !request.restart && !self.rejoin_on(account, request.preset_id) {
                    Some("auto-rejoin off")
                } else if already_open(&self, account.id) {
                    Some("already open")
                // Banned by that game since it was queued: only Join Error 600 again. A followed player's game is checked
                // once they are found (follow_target).
                } else if request.target.follow.is_none() && self.game_ban_of(account, Some(request.target.place_id)).is_some() {
                    Some("banned by the game")
                } else {
                    None
                }
            }
        };
        if let Some(why) = stop {
            log::note(&format!("Rejoin: account {} dropped from the queue ({why}).", short(request.account_id)));
            return;
        }
        let (Some(_vault), Some(account)) = (vault, account) else {
            return;
        };
        self.controller.rejoining.set(true);
        log::note(&format!(
            "Rejoin: account {} starting{}{}{}.",
            short(account.id),
            if request.restart {
                " (a restart)"
            } else if hopping {
                " (a server switch)"
            } else {
                ""
            },
            if request.network_retries > 0 { format!(", network try {}", request.network_retries + 1) } else { String::new() },
            if request.open_retries > 0 { format!(", open try {}", request.open_retries + 1) } else { String::new() }
        ));
        let request = Rc::new(request);
        *self.rejoin.rejoin_in_flight.borrow_mut() = Some(request.clone());
        let cts = Cancel::new();
        *self.controller.rejoin_cts.borrow_mut() = Some(cts.clone());
        // PORT: `_rejoinCts == cts` below is a reference compare; base::Cancel doesn't expose its Arc, so a count of the
        // rejoins that took `rejoin_cts` stands in.
        let generation = self.rejoin.rejoin_generation.get() + 1;
        self.rejoin.rejoin_generation.set(generation);
        let ct: Cancel = cts.clone();
        // Back in the queue to try again later, unless it was called off meanwhile.
        let again = |app: &App, next: RejoinRequest| {
            if !ct.is_cancelled() {
                app.controller.rejoin_queue.borrow_mut().push(next);
            }
        };
        let outcome: Res<()> = async {
            // Where a followed player is now. Not found (offline, hidden or full server): waits and looks again.
            let mut target = request.target.clone();
            if target.follow.is_some() {
                match self.follow_target(&account, &request, &ct).await? {
                    Some(found) => target = found,
                    None => return Ok(()),
                }
            }
            // The preset's "Rejoin server": the server another of its accounts plays in, if it has room.
            else if !hopping && let Some(teammate) = self.teammate_target(&account, &request, &ct).await? {
                log::note(&format!("Rejoin: account {} goes to another account's server (Rejoin into).", short(account.id)));
                target = teammate;
            }
            // Banned while it played (the daily check would say so hours later): a new window would only show the ban
            // every few minutes. No clear answer lets the rejoin go on.
            match self.check_ban(&account).await {
                Ok(Some(true)) => {
                    log::note(&format!("Rejoin: account {} not rejoined, banned on Roblox.", short(account.id)));
                    if let Some(now) = self.find_account(account.id) {
                        self.notify_banned(vec![now]);
                    }
                    self.persist();
                    self.push_state();
                    return Ok(());
                }
                Ok(_) => {}
                Err(ex) if roblox_api::is_expected_error(&ex) => {
                    log::note(&format!("Rejoin: account {}'s ban check got no answer ({}); rejoining anyway.", short(account.id), launch_fail_kind(&ex)));
                }
                Err(ex) => return Err(ex),
            }
            ct.check()?;
            let players = roblox_processes::get_player_pids();
            let multi_on = self.vault(|d| d.settings.multi_instance).unwrap_or(false);
            if multi_on && !multi_instance::armed() && players.is_empty() {
                multi_instance::enable();
            }
            // Without Multi-Roblox the new client closes the open one, which would then rejoin and close this one.
            if !multi_instance::armed() && roblox_processes::visible_windows(&players.iter().copied().collect::<Vec<_>>()).values().any(|w| !w.is_empty()) {
                log::note(&format!("Rejoin: account {} not rejoined, Multi-Roblox not ready with {} Roblox open.", short(account.id), players.len()));
                let n = self.name(&account);
                self.notify(
                    &l::f("Didn't rejoin {0}: Multi-Roblox isn't ready, so a new Roblox window would close the one that's open.", &[&n]),
                    "warn",
                    "rejoinFail",
                    account.id,
                    true,
                    None,
                );
                return Ok(());
            }
            let via = self.vault(|d| d.settings.launch_via.clone())?;
            self.start_session(&account, Some(target), &via, &ct, true, request.preset_id, request.restart, request.open_retries).await?;
            self.persist();
            self.push_state();
            Ok(())
        }
        .await;

        if let Err(ex) = outcome {
            let network = is_fail_kind(&ex, roblox_api::HTTP_REQUEST_EXCEPTION) || is_fail_kind(&ex, roblox_api::TASK_CANCELED_EXCEPTION) || ex.rate_limited() || ex.server_error();
            if ct.is_cancelled() {
                // Close all, or Close of this account: whatever it ran into meanwhile, nothing to say.
                log::note(&format!("Rejoin: account {}'s rejoin called off.", short(account.id)));
            } else if matches!(ex, Fail::User(_)) {
                // The account was deleted while the ticket was fetched.
                log::note(&format!("Rejoin: account {} not rejoined, deleted meanwhile.", short(request.account_id)));
            } else if network && request.network_retries < MAX_NETWORK_RETRIES {
                // No connection (often right after sleep) or Roblox busy: try again, each time later; not counted as a
                // rejoin. A refusal is reported below instead: silent retries wouldn't change it.
                let delay = retry_delay(request.network_retries);
                log::note(&format!(
                    "Rejoin: account {} got no answer ({}); try {} in {} s.",
                    short(account.id),
                    launch_fail_kind(&ex),
                    request.network_retries + 2,
                    delay.as_secs()
                ));
                again(&self, RejoinRequest { due: Stamp::utc_now().plus_secs(secs(delay)), network_retries: request.network_retries + 1, ..(*request).clone() });
            } else if is_start_error(&ex) && request.open_retries < MAX_OPEN_RETRIES {
                // Roblox wouldn't start: most likely its updater swapping the build folder under the launch. Tried again later,
                // as a client that quits before its window is (retry_open); before, the account was left out for good while a
                // drop in game is rejoined (review, 2026-10-30).
                log::note(&format!("A rejoin's Roblox didn't start, tried again later: {}", super::controller::start_error_log(&ex)));
                let delay = retry_delay(request.open_retries);
                again(&self, RejoinRequest { due: Stamp::utc_now().plus_secs(secs(delay)), open_retries: request.open_retries + 1, ..(*request).clone() });
            } else if roblox_api::is_expected_error(&ex) || is_start_error(&ex) {
                log::note(&format!("Rejoin: account {} failed ({}).", short(account.id), launch_fail_kind(&ex)));
                let mut why: Option<String> = None;
                let mut said = true;
                if ex.cookie_invalid() {
                    // One refused ticket isn't proof of a sign-out (an outage answers 401 for everyone): retried a few
                    // times, then said as a refusal, auto-rejoin staying on.
                    let signed_out = self.confirm_signed_out(account.id, &Cancel::new(), false).await;
                    match signed_out {
                        false => {
                            if request.network_retries < MAX_TICKET_RETRIES {
                                let delay = retry_delay(request.network_retries);
                                log::note(&format!(
                                    "Rejoin: account {}'s ticket refused but still signed in; try {} in {} s.",
                                    short(account.id),
                                    request.network_retries + 2,
                                    delay.as_secs()
                                ));
                                again(&self, RejoinRequest { due: Stamp::utc_now().plus_secs(secs(delay)), network_retries: request.network_retries + 1, ..(*request).clone() });
                                said = false;
                            } else {
                                why = Some(super::controller::refused_text());
                            }
                        }
                        true => {
                            log::note(&format!("Rejoin: account {} is signed out; auto-rejoin off until it signs in again.", short(account.id)));
                            // Back on by itself once signed in again (signed_in_again).
                            let _ = self.update_account(account.id, |a| {
                                if a.auto_rejoin {
                                    a.rejoin_off_by_sign_out = true;
                                }
                                a.auto_rejoin = false;
                            });
                            self.persist();
                            self.push_state();
                        }
                    }
                }
                if said {
                    let n = self.name(&account);
                    let text = why.unwrap_or_else(|| if is_start_error(&ex) { super::controller::start_error_text(&ex) } else { App::error_text(&ex) });
                    self.notify(&l::f("Couldn't rejoin {0}: {1}", &[&n, &text]), "error", "rejoinFail", account.id, true, None);
                }
            } else {
                // Said and logged rather than the account quietly not coming back.
                let n = self.name(&account);
                let text = App::error_text(&ex);
                self.notify(&l::f("Couldn't rejoin {0}: {1}", &[&n, &text]), "error", "rejoinFail", account.id, true, None);
            }
        }
        // finally
        self.controller.rejoining.set(false);
        let mine = self.rejoin.rejoin_in_flight.borrow().as_ref().is_some_and(|r| Rc::ptr_eq(r, &request));
        if mine {
            *self.rejoin.rejoin_in_flight.borrow_mut() = None;
        }
        let own_cts = self.rejoin.rejoin_generation.get() == generation && self.controller.rejoin_cts.borrow().is_some();
        if own_cts {
            *self.controller.rejoin_cts.borrow_mut() = None;
        }
    }

    /// The log is the better source while current; one stuck in "joining" is left to Roblox's status again, unless
    /// the person started that game (the status check would close it and reopen Bloxkeep's).
    pub fn log_followed(&self, pid: i32) -> bool {
        log_of(self, pid).is_some_and(|log| !(log.status == "joining" && !log.person_started && log.at.age_secs() > secs(JOINING_TIMEOUT)))
    }

    /// Bloxkeep's own close makes Roblox log a leave first; that leave is not the person's, so auto-rejoin still brings
    /// the client back. 2026-10-17 (0.743): five clients closed this way logged leaveUGCGameInternal and none came back.
    pub fn ended_state(&self, pid: i32) -> Option<ClientState> {
        let log = self.controller.logs.borrow_mut().last_state_of(pid);
        match log {
            Some(l) if l.status == "left" && self.rejoin.closing_for_rejoin.borrow().contains(&pid) => {
                Some(ClientState { status: "disconnected".into(), reason: None, ..l })
            }
            other => other,
        }
    }

    /// A client open while its account was out of a game for two presence checks is usually on a kick screen: closed
    /// so auto-rejoin starts a fresh one.
    /// `reliable`: accounts whose status came from their own cookie or shows them in a game.
    pub fn detect_disconnected_sessions(&self, reliable: &HashSet<i64>) {
        // Right after sleep everyone looks offline until the network is back; that says nothing about the clients.
        if self.settling() {
            self.controller.offline_strikes.borrow_mut().clear();
            return;
        }
        for (id, info) in running(self) {
            let account = self.find_account(id);
            // The log is the better source (rejoin_from_logs); a strike from before it was found says nothing.
            if self.log_followed(info.0) {
                self.controller.offline_strikes.borrow_mut().remove(&id);
                continue;
            }
            // A status hidden by privacy settings or a failed request proves nothing; wait for a clear answer.
            if let Some(a) = &account
                && !reliable.contains(&a.user_id)
            {
                continue;
            }
            // Nor one a game banned (Join Error 600, log silent): on 2026-10-21 one was closed here as "Lost connection"
            // two minutes after its ban was told.
            let presence = account.as_ref().and_then(|a| self.controller.presence.borrow().get(&a.user_id).cloned());
            let leave_alone = match &account {
                None => true,
                Some(a) => {
                    !self.rejoin_on(a, info.1.preset_id)
                        || a.banned == Some(true)
                        || info.1.target.is_none()
                        || Stamp::utc_now().secs_since(info.1.started_at.get()) < secs(DISCONNECT_GRACE)
                        || self.game_ban_of(a, Some(rejoin_place(&info.1, log_of(self, info.0).as_ref()))).is_some()
                        || presence.as_ref().is_none_or(|p| p.r#type == 2 || p.r#type == 4)
                }
            };
            if leave_alone {
                self.controller.offline_strikes.borrow_mut().remove(&id);
                continue;
            }
            let account = account.expect("checked above");
            let strikes = {
                let mut s = self.controller.offline_strikes.borrow_mut();
                let n = s.get(&id).copied().unwrap_or(0) + 1;
                s.insert(id, n);
                n
            };
            if strikes < 2 {
                continue;
            }
            self.controller.offline_strikes.borrow_mut().remove(&id);
            log::note(&format!("Rejoin: closing window {} to rejoin, its account was out of a game on two status checks (no log to go by).", info.0));
            self.rejoin.closing_for_rejoin.borrow_mut().insert(info.0); // Bloxkeep's own close: never taken for Roblox closing it
            let n = self.name(&account);
            self.notify(&l::f("{0} dropped out of the game, reason unknown — restarting Roblox…", &[&n]), "info", "drop", account.id, true, None);
            roblox_processes::close(info.0, false);
        }
    }
}
