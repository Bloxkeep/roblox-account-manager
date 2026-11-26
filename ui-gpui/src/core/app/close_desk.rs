//! The one place that closes a Roblox client: every part that wants one closed asks here (`ask_close`), close_plan.rs
//! decides from what is known of it now, and the desk keeps one record per client, closes it through one hook, forces
//! it once if it lingers and gives up on it in time (`close_desk_tick`), and says what follows once it ended
//! (`close_ended`: closed for good, auto-rejoin's drop path, or opened again into its target).
//!
//! The record replaces the marks each closer kept before: the person's close, the restart's target, the client closing
//! for a rejoin, the account moving to another server, the schedule's stop. The parts read them back through the
//! queries here (`closing_of`, `closing_account`, `own_close`, `reopening`, `hopping`).
//!
//! Not through here, on purpose: Roblox's windowless copies in the notification area, ended by their own functions
//! (roblox_processes::close_tray_copies; the tray sweep and the closed client's child in processes.rs; the windowless
//! Roblox no account owns in launching.rs `end_handoff_targets`; the sign-in window's copies in tray.rs
//! `release_home_session_if_idle`). They are no account's client and nothing follows their end.
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use super::{
    App,
    launching::{log_of, pending, run_of},
    presets::{secs, short},
    rejoin::{rejoin_place, rejoin_target},
    server_hop::HOP_TIMEOUT,
};
use crate::core::{
    anti_afk,
    base::{Guid, Stamp},
    close_plan::{self, Answer, Ask, AskFacts, Cause, Closing, End, EndFacts, Hold, No, NoOpen, Open, OpenFacts, Step, Then, Who, Why},
    l, log, multi_instance,
    roblox_log_watcher::ClientState,
    roblox_processes,
};

#[derive(Default)]
pub struct CloseDesk {
    /// The closes under way, by process id.
    pub closing: RefCell<HashMap<i32, Closing>>,
    /// Accounts on their way to another server (a server switch, Rejoin now), and since when: from the ask until the
    /// account's next window is in its game, the account is closed for good, its reopen is dropped, or HOP_TIMEOUT.
    pub moving: RefCell<HashMap<Guid, Stamp>>,
    /// Accounts the person or a schedule closed, and when: a launch asked before that doesn't open them (close_plan.rs
    /// O2). A launch the person starts later lets them go.
    pub held_closed: RefCell<HashMap<Guid, Stamp>>,
    /// Refusals and waits already said, by process id.
    told: RefCell<HashSet<(i32, String)>>,
    /// Closes a client (a field, so the checks can see what is closed without closing anything). None:
    /// roblox_processes::close.
    pub close_hook: RefCell<Option<Rc<dyn Fn(i32, bool)>>>,
}

/// A server switch on its way since `since` is over by time: after HOP_TIMEOUT. One stamped later in the tick whose
/// `now` this is (asked while the tick ran) is not; only one far ahead (the clock set back further than the app's clock
/// mending moved it) is.
pub fn moving_over(now: Stamp, since: Stamp) -> bool {
    now.secs_since(since).abs() > secs(HOP_TIMEOUT)
}

impl CloseDesk {
    /// The clock was set back by `back`: the desk's times move with it (processes.rs move_clock_back).
    pub fn move_back(&self, back: std::time::Duration) {
        use super::processes::moved_back;
        for c in self.closing.borrow_mut().values_mut() {
            c.asked = moved_back(c.asked, back);
        }
        for at in self.moving.borrow_mut().values_mut() {
            *at = moved_back(*at, back);
        }
        for at in self.held_closed.borrow_mut().values_mut() {
            *at = moved_back(*at, back);
        }
    }
}

/// What came of an ask.
#[derive(Clone, Debug, PartialEq)]
pub enum Asked {
    Closing,
    Refused(No),
    Waiting(Hold),
}

impl Asked {
    pub fn closing(&self) -> bool {
        matches!(self, Asked::Closing)
    }
}

impl CloseDesk {
    pub fn closing_of(&self, pid: i32) -> Option<Closing> {
        self.closing.borrow().get(&pid).cloned()
    }

    /// The close under way of the account's client, if any.
    pub fn closing_account(&self, id: Guid) -> Option<Closing> {
        self.closing.borrow().values().find(|c| c.account == Some(id)).cloned()
    }

    /// Closing for Bloxkeep's own reasons (`Cause::own`): a reopen, a rejoin, an update.
    pub fn own_close(&self, pid: i32) -> bool {
        self.closing.borrow().get(&pid).is_some_and(|c| c.cause.own())
    }

    /// Some client is closing to be opened again into its target.
    pub fn reopening(&self) -> bool {
        self.closing.borrow().values().any(|c| matches!(c.then, Then::Reopen(_)))
    }

    /// Where the account's client goes back once closed, if it is closing to be opened again.
    pub fn reopen_of(&self, id: Guid) -> Option<crate::core::game_target::GameTarget> {
        self.closing.borrow().values().find(|c| c.account == Some(id)).and_then(|c| match &c.then {
            Then::Reopen(t) => Some(t.clone()),
            _ => None,
        })
    }

    pub fn any(&self) -> bool {
        !self.closing.borrow().is_empty()
    }

    /// Some client is closing for Bloxkeep's own reasons.
    pub fn any_own(&self) -> bool {
        self.closing.borrow().values().any(|c| c.cause.own())
    }

    /// The close stays as Roblox left it: its record goes (a switch whose window didn't close in time).
    pub fn withdraw(&self, pid: i32, cause: Cause) -> bool {
        let mut closing = self.closing.borrow_mut();
        if closing.get(&pid).is_some_and(|c| c.cause == cause) {
            closing.remove(&pid);
            true
        } else {
            false
        }
    }

    /// The account is opened again: the person's Close and an account switch's close of it go (as the old "closed
    /// by the person" mark did). A restart, a drop's or an update's close of its earlier window stays, so that window's
    /// end still reads as what it was; a server switch stays until the new window is in its game.
    pub fn forget_closes(&self, id: Guid) {
        self.closing.borrow_mut().retain(|_, c| c.account != Some(id) || !matches!(c.cause, Cause::User | Cause::Switch));
    }

    /// Everything of the account's closes goes.
    fn forget_all_closes(&self, id: Guid) {
        self.closing.borrow_mut().retain(|_, c| c.account != Some(id));
    }

    /// Everything of the account goes (deleted).
    pub fn forget_account(&self, id: Guid) {
        self.forget_all_closes(id);
        self.moving.borrow_mut().remove(&id);
        self.held_closed.borrow_mut().remove(&id);
    }

    /// Closed by the person or a schedule now: a launch asked before doesn't open it.
    pub fn hold_closed(&self, id: Guid) {
        self.held_closed.borrow_mut().insert(id, Stamp::utc_now());
    }

    /// A launch the person starts now: its accounts open again.
    pub fn release_closed(&self, ids: impl IntoIterator<Item = Guid>) {
        let mut held = self.held_closed.borrow_mut();
        for id in ids {
            held.remove(&id);
        }
    }

    pub fn moving_of(&self, id: Guid) -> bool {
        self.moving.borrow().contains_key(&id)
    }

    /// Its server switch is over (its reopen dropped, closed for good).
    pub fn stop_moving(&self, id: Guid) {
        self.moving.borrow_mut().remove(&id);
    }

    /// Every server switch is over (locked: their reopens went with the vault).
    pub fn stop_moving_all(&self) {
        self.moving.borrow_mut().clear();
    }

    /// Nothing under way any more; returns how many closes were.
    pub fn clear(&self) -> usize {
        let n = self.closing.borrow().len();
        self.closing.borrow_mut().clear();
        self.moving.borrow_mut().clear();
        self.held_closed.borrow_mut().clear();
        self.told.borrow_mut().clear();
        n
    }

    fn tell_once(&self, pid: i32, what: String) -> bool {
        self.told.borrow_mut().insert((pid, what))
    }

    fn send(&self, pid: i32, force: bool) {
        let hook = self.close_hook.borrow().clone();
        match hook {
            Some(f) => f(pid, force),
            None => roblox_processes::close(pid, force),
        }
    }
}

impl App {
    /// Closes a client if close_plan.rs says it may; see `ask_close_with`.
    pub fn ask_close(&self, id: Option<Guid>, pid: i32, cause: Cause, then: Then, force: bool) -> Asked {
        self.ask_close_with(id, pid, cause, then, force, || {})
    }

    /// Asks for a client to close. Allowed, it is recorded, `before` runs (the caller's own line and notice, said
    /// before the close as they were), then it is closed. Refused or held, nothing is closed; the desk says why once
    /// per window, unless the caller says it in its own words (`Cause::says_own_refusals`; a restart's wait is silent).
    pub fn ask_close_with(&self, id: Option<Guid>, pid: i32, cause: Cause, then: Then, force: bool, before: impl FnOnce()) -> Asked {
        let ask = Ask { pid, account: id, cause, then, force, send: true };
        match self.decide_close(&ask) {
            Answer::Close { record, force } => {
                self.keep_close(record);
                before();
                self.close_desk.send(pid, force);
                Asked::Closing
            }
            Answer::Refuse(no) => {
                if !cause.says_own_refusals() && self.close_desk.tell_once(pid, format!("{cause:?} {no:?}")) {
                    log::note(&format!("Close: window {pid} not closed for {}: {}.", cause.word(), no.words()));
                }
                Asked::Refused(no)
            }
            Answer::Wait(hold) => {
                if !matches!(cause, Cause::Restart(_)) && self.close_desk.tell_once(pid, format!("{cause:?} {hold:?}")) {
                    log::note(&format!("Close: window {pid}'s close for {} waits: {}.", cause.word(), hold.words()));
                }
                Asked::Waiting(hold)
            }
        }
    }

    /// A close Roblox makes itself (a launch without Multi-Roblox replaces the other windows): recorded, nothing sent.
    /// False when the client was already closing for as much or more. Whoever waits for it takes it back with
    /// `withdraw` if the window stays.
    pub fn expect_close(&self, id: Option<Guid>, pid: i32, cause: Cause, then: Then) -> bool {
        let ask = Ask { pid, account: id, cause, then, force: false, send: false };
        match self.decide_close(&ask) {
            Answer::Close { record, .. } => {
                self.keep_close(record);
                true
            }
            _ => false,
        }
    }

    /// A launch still opening (or late) is closed as soon as its window shows up, as the person's close.
    pub fn close_when_it_shows(&self, launch: &Rc<super::controller::PendingLaunch>) -> bool {
        super::launching::closing_add(self, launch)
    }

    /// Close all: every Roblox on this PC, Bloxkeep's as the person's close, the others as ones it didn't open. The
    /// number of Roblox processes asked to close.
    pub fn close_everything(&self, force: bool) -> i32 {
        let tracked: HashMap<i32, Guid> = self.controller.running.borrow().iter().map(|(id, (pid, _))| (*pid, *id)).collect();
        let pids = roblox_processes::get_player_pids();
        for &pid in &pids {
            match tracked.get(&pid) {
                Some(id) => self.ask_close(Some(*id), pid, Cause::User, Then::StayClosed, force),
                None => self.ask_close(None, pid, Cause::Untracked, Then::StayClosed, force),
            };
        }
        pids.len() as i32
    }

    fn decide_close(&self, ask: &Ask) -> Answer {
        // A1: an older client's record of the same account is stale.
        let records: Vec<(i32, Option<Guid>)> = self.close_desk.closing.borrow().values().map(|c| (c.pid, c.account)).collect();
        for old in close_plan::stale(ask.account, ask.pid, &records) {
            if self.close_desk.closing.borrow_mut().remove(&old).is_some() {
                log::note(&format!(
                    "Close: an older close of account {}'s window {old} dropped (another window of it is asked now).",
                    ask.account.map(|a| short(&a)).unwrap_or_default()
                ));
            }
        }
        let have = self.close_desk.closing_of(ask.pid);
        let facts = self.close_facts(ask);
        close_plan::ask(have.as_ref(), ask, &facts)
    }

    fn keep_close(&self, record: Closing) {
        let (pid, account, cause) = (record.pid, record.account, record.cause);
        let old = self.close_desk.closing.borrow_mut().insert(pid, record);
        if let Some(old) = old.filter(|o| o.cause != cause) {
            log::note(&format!("Close: window {pid} now closing for {} (was {}).", cause.word(), old.cause.word()));
        }
        if let Some(id) = account {
            match cause {
                Cause::Hop | Cause::RejoinNow => {
                    self.close_desk.moving.borrow_mut().insert(id, Stamp::utc_now());
                }
                // Closed for good: a launch asked before doesn't open it again (close_plan.rs O2).
                Cause::User | Cause::Stop => {
                    self.close_desk.stop_moving(id);
                    self.close_desk.hold_closed(id);
                }
                // Closed by an update that reopens it into its own target: no longer on its way.
                Cause::UpdateReopen => self.close_desk.stop_moving(id),
                _ => {}
            }
        }
    }

    /// What close_plan.rs needs to know of the client and the app, read now. The costly reads only for the causes that
    /// use them.
    fn close_facts(&self, ask: &Ask) -> AskFacts {
        let now = Stamp::utc_now();
        let account = ask.account.and_then(|id| self.find_account(id));
        let run = ask.account.and_then(|id| run_of(self, id)).filter(|r| r.0 == ask.pid);
        let log = run.as_ref().and_then(|r| log_of(self, r.0));
        let (game_banned, has_target) = match (&ask.then, &account) {
            (Then::Reopen(t), Some(a)) => (self.game_ban_of(a, Some(t.place_id)).is_some(), t.place_id > 0),
            (Then::Rejoin, Some(a)) => match &run {
                Some(r) => (self.game_ban_of(a, Some(rejoin_place(&r.1, log.as_ref()))).is_some(), rejoin_target(&r.1, log.as_ref()).is_some()),
                None => (false, false),
            },
            _ => (false, true),
        };
        let restart = matches!(ask.cause, Cause::Restart(_));
        let busy = ask.account.is_some_and(|id| {
            pending(self).iter().any(|p| p.account_id == id)
                || self.launching.late.borrow().iter().any(|p| p.account_id == id)
                || self.controller.rejoin_queue.borrow().iter().any(|r| r.account_id == id)
                || self.rejoin.rejoin_in_flight.borrow().as_ref().is_some_and(|r| r.account_id == id)
        });
        AskFacts {
            now,
            update_reopen: self.roblox_updates.update_reopen_since.get().is_some(),
            account_found: account.is_some(),
            signed_out: account.as_ref().is_some_and(|a| a.cookie_valid == Some(false)),
            banned: account.as_ref().is_some_and(|a| a.banned == Some(true)),
            game_banned,
            has_target,
            multi_armed: multi_instance::armed(),
            running: self.controller.running.borrow().len(),
            // A window of an account, or one the person closes from the monitor tab: Roblox's own updater's app has no
            // game, nor does a window an update closes without following it.
            in_game: (ask.account.is_some() || ask.cause == Cause::Untracked) && self.in_game_now(ask.pid),
            main: matches!(ask.cause, Cause::Restart(Why::Heavy) | Cause::Restart(Why::Hung)) && self.main_pids().contains(&ask.pid),
            front_in_use: matches!(ask.cause, Cause::Restart(Why::Hours) | Cause::Restart(Why::Heavy))
                && roblox_processes::foreground_pid() == ask.pid
                && anti_afk::person_active(),
            rejoin_on: ask.cause == Cause::Restart(Why::Hung) && account.as_ref().is_some_and(|a| self.rejoin_on(a, run.as_ref().and_then(|r| r.1.preset_id))),
            settling: self.settling(),
            busy,
            last_start: self.controller.last_session_start.get(),
            reopen_under_way: restart && self.close_desk.closing.borrow().values().any(|c| c.pid != ask.pid && matches!(c.then, Then::Reopen(_))),
        }
    }

    /// May a window open now (close_plan.rs O1-O4)? `id`: the account (None: a window of no account, the tray's New
    /// Roblox). `asked_at`: when the open was asked (a launch's start).
    pub fn may_open(&self, who: Who, id: Option<Guid>, asked_at: Stamp) -> Open {
        let now = Stamp::utc_now();
        close_plan::may_open(&OpenFacts {
            who,
            // Close, update, reopen refuses the person's launch (the update would give up on the window it opens). Roblox's
            // updater alone doesn't: start_session waits for it, as it always did.
            updating: self.roblox_updates.update_reopen_since.get().is_some() || (who == Who::Auto && self.controller.roblox_updating.get()),
            held_since: id.and_then(|id| self.close_desk.held_closed.borrow().get(&id).copied()),
            asked_at,
            sweeping: now < self.controller.sweep_background_until.get(),
            settling: self.settling(),
            closing: id.is_some_and(|id| self.close_desk.closing_account(id).is_some()),
        })
    }

    /// What the person is told of a refused open (none: it says nothing).
    pub fn open_refused_text(no: NoOpen) -> Option<String> {
        match no {
            NoOpen::Updating => Some(l::t("Roblox is updating. Try again in a moment.")),
            NoOpen::ClosedSince => None,
        }
    }

    /// From the process tick, after the ended clients were taken in: a reopen whose window lingers is given up; a
    /// record whose process is gone and nobody took goes; a server switch whose window is in its game is over.
    /// `alive`: the Roblox processes running now.
    pub fn close_desk_tick(&self, now: Stamp, alive: &HashSet<i32>) {
        let records: Vec<Closing> = self.close_desk.closing.borrow().values().cloned().collect();
        for r in records {
            match close_plan::tick(&r, now, alive.contains(&r.pid)) {
                Step::Wait => {}
                Step::Gone => {
                    self.close_desk.closing.borrow_mut().remove(&r.pid);
                }
                Step::GiveUp => {
                    self.close_desk.closing.borrow_mut().remove(&r.pid);
                    match (r.account, &r.then) {
                        (Some(id), Then::Reopen(_)) => {
                            log::note(&format!(
                                "Reopen (restart, switch or update): account {}'s window didn't close within {} s; left running.",
                                short(&id),
                                close_plan::GIVE_UP.as_secs()
                            ));
                            self.close_desk.stop_moving(id);
                        }
                        _ => log::note(&format!(
                            "Close: window {} didn't close within {} s ({}); left running.",
                            r.pid,
                            close_plan::GIVE_UP.as_secs(),
                            r.cause.word()
                        )),
                    }
                }
            }
        }
        self.close_desk.told.borrow_mut().retain(|(pid, _)| alive.contains(pid));
        // A server switch is over once the account's next window is in its game (its old one's record is gone by then).
        let moving: Vec<(Guid, Stamp)> = self.close_desk.moving.borrow().iter().map(|(k, v)| (*k, *v)).collect();
        for (id, since) in moving {
            let timed_out = moving_over(now, since);
            let arrived = self.close_desk.closing_account(id).is_none() && run_of(self, id).is_some_and(|r| log_of(self, r.0).is_some_and(|l| l.status == "ingame"));
            if timed_out || arrived {
                self.close_desk.stop_moving(id);
            }
        }
    }

    /// A client ended: what follows, and its record goes.
    pub fn close_ended(&self, id: Guid, pid: i32, log: Option<&ClientState>) -> End {
        let record = self.close_desk.closing.borrow_mut().remove(&pid);
        let end = close_plan::ended(record.as_ref(), &EndFacts { banned_by_game: log.is_some_and(|l| l.banned_by_game) });
        if matches!(end, End::Closed { .. }) {
            self.close_desk.stop_moving(id);
        }
        end
    }

    /// Some account is still on its way to another server.
    pub fn hopping(&self) -> bool {
        let now = Stamp::utc_now();
        self.close_desk.moving.borrow_mut().retain(|_, since| !moving_over(now, *since));
        !self.close_desk.moving.borrow().is_empty()
    }
}
