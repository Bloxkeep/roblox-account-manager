//! Who may close a Roblox client of Bloxkeep's, and what follows once it has ended. Decided from what is known of the
//! client now (`AskFacts`, read by close_desk.rs) and the close already under way for it, if any (`Closing`). No
//! Windows calls, no clock and no memory of its own: data in, data out, so every rule is a plain check (suite
//! "close-plan"). The desk keeps the records, closes, and hands the answers on.
//!
//! The rules:
//! - A1 One record per client: a record of the same account for another process is stale, and goes first.
//! - A2 Rank: a new ask on a client already closing wins only with a higher rank (`rank`); a tie or a lower one is
//!   refused. The winner's `Then` replaces the old one (the person's Close over a server hop: it stays closed).
//! - A3 While Close, update, reopen runs, every ask below its rank is refused.
//! - A4 A reopen needs a way back: the account there, its sign-in not known bad, not banned on Roblox nor by the
//!   target's game, a target, and Multi-Roblox ready or no other window open (not asked of Close, update, reopen: it
//!   closes them all). A rejoin (auto-rejoin's drop path) needs
//!   the account, no ban and a target; its sign-in and Multi-Roblox are auto-rejoin's own to check when it reopens.
//! - A5 Each cause's own waits: a restart for the hours or for its weight waits while its window is in front and in
//!   use; a heavy or frozen window is never a main account's; a frozen one is reopened only where auto-rejoin is on;
//!   restarts go one at a time, spaced; restarts and drops wait while the PC settles after sleep, and
//!   restarts and leaves while a launch or rejoin of that account is under way.
//! - A6 Force (ended at once, no WM_CLOSE): asked so, a frozen window, or one in a game (it would ask "leave the
//!   game?"), except a drop's window, which is on Roblox's error screen and closes as asked.
//! - Tick: only a close that reopens has a time limit (as the restarts' own had): still there after `GIVE_UP`, the
//!   whole record goes and the window is an ordinary running client again. Every other close stays until its window
//!   ends, as its old mark did (the close itself ends a window that doesn't answer its X within 2 s).
//! - E1-E4 What follows the end: a close that stays closed is no drop (no auto-rejoin, no "ended on its own" note, no
//!   hand-over wait); a reopen goes to its target unless the client's last log says the game banned it; a rejoin goes
//!   down auto-rejoin's drop path; no record is today's path for a client nobody closed.
//! - O1-O4 Who may open a window (`may_open`): none while Roblox updates (the person is told; an automatic open
//!   waits); not an account the person or a schedule closed after the open was asked (a launch fetching its ticket
//!   when Close came); an automatic open also waits while Roblox's background copies are swept, the PC settles, or a
//!   window of that account is still closing; anything else goes, to the checks each opener makes itself.
use std::time::Duration;

use crate::core::{
    app::presets::{RESTART_CLOSE_TIMEOUT, RESTART_SPACING},
    base::{Guid, Stamp},
    game_target::GameTarget,
};

/// A close that reopens still not gone after this is given up: the record goes, and the window runs on as any other.
pub const GIVE_UP: Duration = RESTART_CLOSE_TIMEOUT;

fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

/// Why a preset's restart closes a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Why {
    /// Its preset's "restart every N hours".
    Hours,
    /// Above Settings' "Restart a heavy window" for long enough.
    Heavy,
    /// "Not responding" for a minute.
    Hung,
}

/// Why auto-rejoin closes a window to open it again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DropWhy {
    /// Its log says disconnected or kicked.
    Dropped,
    /// Its log says joining, and no server came (or the chosen one ended or is full).
    NeverGotIn,
    /// A teleport that never reached a server.
    DeadTeleport,
    /// Roblox's status says the account is out of a game (no log to go by, or the log says in one).
    OutOfGame,
}

/// Who asks for a client to close.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cause {
    /// Close, Close all, the mini window, the remote page's Close, the tray's Close all.
    User,
    /// A schedule's stop.
    Stop,
    /// A launch moving the account into another game or server, or replacing its window (no Multi-Roblox).
    Switch,
    /// Close, update, reopen (and the automatic update).
    UpdateReopen,
    Restart(Why),
    /// Switch server, or leaving a crowded one.
    Hop,
    /// The remote page's Rejoin.
    RejoinNow,
    /// A solved bot check: the held window reopens.
    BotCheck,
    /// A macro's Rejoin step.
    Macro,
    /// Auto-rejoin of a drop.
    Drop(DropWhy),
    /// Close Roblox after leaving a game.
    Leave,
    /// A window Bloxkeep didn't open (the monitor tab's Close, the Roblox app Roblox's installer opened).
    Untracked,
}

impl Cause {
    /// For the log.
    pub fn word(&self) -> &'static str {
        match self {
            Cause::User => "the person's close",
            Cause::Stop => "a schedule's stop",
            Cause::Switch => "a launch's switch",
            Cause::UpdateReopen => "Close, update, reopen",
            Cause::Restart(Why::Hours) => "a restart (hours)",
            Cause::Restart(Why::Heavy) => "a restart (heavy)",
            Cause::Restart(Why::Hung) => "a restart (not responding)",
            Cause::Hop => "a server switch",
            Cause::RejoinNow => "Rejoin now",
            Cause::BotCheck => "a bot check",
            Cause::Macro => "a macro's rejoin",
            Cause::Drop(_) => "a rejoin",
            Cause::Leave => "close after leaving",
            Cause::Untracked => "a close of a window Bloxkeep didn't open",
        }
    }

    /// Bloxkeep's own close to open the client again or for an update: its leave isn't the person's (the log's leave
    /// reads as a disconnect), and its end isn't Roblox's doing. Not close after leaving: the person left the game
    /// themselves, and the log's leave stays a leave.
    pub fn own(&self) -> bool {
        matches!(self, Cause::UpdateReopen | Cause::Restart(_) | Cause::Hop | Cause::RejoinNow | Cause::BotCheck | Cause::Macro | Cause::Drop(_))
    }

    /// The caller says its refusals in its own words (the restart's "due but left running", the macro's step).
    pub fn says_own_refusals(&self) -> bool {
        matches!(self, Cause::Restart(_) | Cause::Macro)
    }
}

/// What follows the close.
#[derive(Clone, Debug, PartialEq)]
pub enum Then {
    StayClosed,
    /// Auto-rejoin's own rules decide, counted as a drop.
    Rejoin,
    /// Opened again into this, not counted, whatever auto-rejoin says.
    Reopen(GameTarget),
}

/// A close under way.
#[derive(Clone, Debug, PartialEq)]
pub struct Closing {
    pub pid: i32,
    pub account: Option<Guid>,
    pub cause: Cause,
    pub then: Then,
    pub asked: Stamp,
    /// Ended at once (asked so, or forced after `CLOSE_GRACE`).
    pub forced: bool,
    /// Bloxkeep sent the close. False: Roblox closes it itself (a launch without Multi-Roblox replaces the other
    /// windows), and whoever waits for that takes the record back if it stays.
    pub sent: bool,
}

/// How much a cause's close counts against another on the same client (A2).
pub fn rank(c: &Cause) -> u8 {
    match c {
        Cause::User | Cause::Stop | Cause::Untracked => 5,
        Cause::UpdateReopen => 4,
        Cause::Switch => 3,
        Cause::Restart(_) | Cause::Hop | Cause::RejoinNow | Cause::BotCheck | Cause::Macro => 2,
        Cause::Drop(_) => 1,
        Cause::Leave => 0,
    }
}

/// One ask.
#[derive(Clone, Debug, PartialEq)]
pub struct Ask {
    pub pid: i32,
    pub account: Option<Guid>,
    pub cause: Cause,
    pub then: Then,
    /// Ended at once whatever else (Close all forced, a party's server move).
    pub force: bool,
    /// False: Roblox closes it itself; only recorded.
    pub send: bool,
}

/// What is known of the client and the app when a close is asked, read once by the desk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AskFacts {
    pub now: Stamp,
    /// Close, update, reopen is under way.
    pub update_reopen: bool,
    /// The account is in the vault.
    pub account_found: bool,
    pub signed_out: bool,
    /// Banned on Roblox.
    pub banned: bool,
    /// Banned by the game it would go back into.
    pub game_banned: bool,
    /// Something to open it into (a reopen's target, or auto-rejoin's).
    pub has_target: bool,
    pub multi_armed: bool,
    /// Clients of Bloxkeep's open now.
    pub running: usize,
    /// Its log says in a game or joining one.
    pub in_game: bool,
    /// A main account's window (the one played on).
    pub main: bool,
    /// Its window is in front and the person is at the PC.
    pub front_in_use: bool,
    /// Auto-rejoin is on for it.
    pub rejoin_on: bool,
    /// The PC just woke.
    pub settling: bool,
    /// A launch or rejoin of that account is under way.
    pub busy: bool,
    /// When the last client started.
    pub last_start: Stamp,
    /// Another client is closing to be reopened.
    pub reopen_under_way: bool,
}

/// Why an ask was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum No {
    /// Closing already, for a cause of this rank or higher.
    AlreadyClosing(Cause),
    UpdateReopen,
    AccountGone,
    SignedOut,
    Banned,
    GameBanned,
    NoTarget,
    MultiNotReady,
    /// A main account's window.
    Main,
    RejoinOff,
}

impl No {
    /// For the log (the restart's words for the reasons it had before).
    pub fn words(&self) -> String {
        match self {
            No::AlreadyClosing(c) => format!("already closing for {}", c.word()),
            No::UpdateReopen => "Close, update, reopen is under way".into(),
            No::AccountGone => "account gone".into(),
            No::SignedOut => "signed out".into(),
            No::Banned => "banned on Roblox".into(),
            No::GameBanned => "banned by the game".into(),
            No::NoTarget => "nothing to open it into".into(),
            No::MultiNotReady => "Multi-Roblox not ready".into(),
            No::Main => "a main account's window".into(),
            No::RejoinOff => "auto-rejoin off".into(),
        }
    }
}

/// Why an ask waits (asked again later).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hold {
    /// Its window is in front and in use.
    InUse,
    Settling,
    /// A launch or rejoin of that account is under way.
    Busy,
    /// Another client is closing to be reopened.
    OneAtATime,
    /// A client started a moment ago.
    Spacing,
}

impl Hold {
    pub fn words(&self) -> &'static str {
        match self {
            Hold::InUse => "its window is in use",
            Hold::Settling => "the PC just woke",
            Hold::Busy => "a launch or rejoin of it is under way",
            Hold::OneAtATime => "another window is closing to reopen",
            Hold::Spacing => "a window started a moment ago",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Close { record: Closing, force: bool },
    Refuse(No),
    Wait(Hold),
}

/// A1: the records of `account` for another process than `pid`, from `(pid, account)` pairs of the records kept.
pub fn stale(account: Option<Guid>, pid: i32, records: &[(i32, Option<Guid>)]) -> Vec<i32> {
    let Some(account) = account else { return Vec::new() };
    records.iter().filter(|(p, a)| *p != pid && *a == Some(account)).map(|(p, _)| *p).collect()
}

/// Whether the client may close for this ask now, given the close already under way for it (`have`, same process).
pub fn ask(have: Option<&Closing>, a: &Ask, f: &AskFacts) -> Answer {
    // A2: a tie or a lower rank is refused; the person's forced close still ends a window closing gently.
    if let Some(have) = have {
        let forced_again = a.force && rank(&a.cause) == 5 && !have.forced;
        if rank(&a.cause) <= rank(&have.cause) && !forced_again {
            return Answer::Refuse(No::AlreadyClosing(have.cause));
        }
    }
    // A3.
    if f.update_reopen && rank(&a.cause) < rank(&Cause::UpdateReopen) {
        return Answer::Refuse(No::UpdateReopen);
    }
    // A5, the waits: asked again later.
    if let Some(hold) = hold(a, f) {
        return Answer::Wait(hold);
    }
    // A4.
    if let Some(no) = no_way_back(a, f) {
        return Answer::Refuse(no);
    }
    // A5, the causes' own refusals.
    match a.cause {
        Cause::Restart(Why::Heavy) | Cause::Restart(Why::Hung) if f.main => return Answer::Refuse(No::Main),
        Cause::Restart(Why::Hung) if !f.rejoin_on => return Answer::Refuse(No::RejoinOff),
        _ => {}
    }
    let force = force(a, f) || have.is_some_and(|h| h.forced);
    Answer::Close {
        record: Closing { pid: a.pid, account: a.account, cause: a.cause, then: a.then.clone(), asked: f.now, forced: force && a.send, sent: a.send },
        force,
    }
}

/// A5's waits.
fn hold(a: &Ask, f: &AskFacts) -> Option<Hold> {
    let restart = matches!(a.cause, Cause::Restart(_));
    // (Restarts and drops waited so before; a leave's close never did: it closes a window its account left.)
    if matches!(a.cause, Cause::Restart(_) | Cause::Drop(_)) && f.settling {
        return Some(Hold::Settling);
    }
    if matches!(a.cause, Cause::Restart(_) | Cause::Leave) && f.busy {
        return Some(Hold::Busy);
    }
    if restart && f.reopen_under_way {
        return Some(Hold::OneAtATime);
    }
    if restart && f.now.secs_since(f.last_start) < secs(RESTART_SPACING) {
        return Some(Hold::Spacing);
    }
    if matches!(a.cause, Cause::Restart(Why::Hours) | Cause::Restart(Why::Heavy)) && f.front_in_use {
        return Some(Hold::InUse);
    }
    None
}

/// A4: why there is no way back, for a close that opens the client again.
fn no_way_back(a: &Ask, f: &AskFacts) -> Option<No> {
    let reopen = match a.then {
        Then::StayClosed => return None,
        Then::Rejoin => false,
        Then::Reopen(_) => true,
    };
    if !f.account_found {
        Some(No::AccountGone)
    } else if reopen && f.signed_out {
        Some(No::SignedOut)
    } else if f.banned {
        Some(No::Banned)
    } else if f.game_banned {
        Some(No::GameBanned)
    } else if !f.has_target {
        Some(No::NoTarget)
    // Close, update, reopen closes every window first: none is open beside the one reopened.
    } else if reopen && a.cause != Cause::UpdateReopen && !f.multi_armed && f.running > 1 {
        Some(No::MultiNotReady)
    } else {
        None
    }
}

/// A6.
fn force(a: &Ask, f: &AskFacts) -> bool {
    a.force || a.cause == Cause::Restart(Why::Hung) || (f.in_game && !matches!(a.cause, Cause::Drop(_)))
}

/// What a close under way comes to on a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Wait,
    /// A reopen still there after `GIVE_UP`: the record goes.
    GiveUp,
    /// The process is gone and nobody took the record (a window Bloxkeep doesn't follow): it goes quietly.
    Gone,
}

pub fn tick(r: &Closing, now: Stamp, alive: bool) -> Step {
    if !alive {
        return Step::Gone;
    }
    // A close made by Roblox itself, and a close that stays closed or rejoins: kept until the window ends (the person's
    // Close, a schedule's stop, a drop's and a leave's close never gave up before: one that did came back as a drop to
    // rejoin, or was closed again and again).
    if !r.sent || !matches!(r.then, Then::Reopen(_)) {
        return Step::Wait;
    }
    let age = now.secs_since(r.asked);
    // Asked later in the tick whose `now` this is (a close asked while the tick ran): not due yet. Only one far ahead
    // means the clock was set back further than the app's clock mending moved it: its time says nothing any more.
    if age < 0.0 {
        return if -age >= secs(GIVE_UP) { Step::GiveUp } else { Step::Wait };
    }
    if age >= secs(GIVE_UP) {
        return Step::GiveUp;
    }
    Step::Wait
}

/// What follows a client's end.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    /// Closed for good: not rejoined. `why` as auto-rejoin's line says it.
    Closed { cause: Cause, why: &'static str },
    /// Auto-rejoin's drop path.
    Rejoin,
    /// Opened again into this, not counted.
    Reopen(GameTarget),
    /// Nobody closed it.
    Unasked,
}

pub const ON_PURPOSE: &str = "closed on purpose";
pub const GAME_BANNED: &str = "banned by the game";

impl End {
    /// Bloxkeep or the person closed it: no "ended on its own" note, no Roblox lines.
    pub fn asked(&self) -> bool {
        !matches!(self, End::Unasked)
    }

    /// Bloxkeep's own close (see `Cause::own`).
    pub fn own(&self) -> bool {
        match self {
            End::Closed { cause, .. } => cause.own(),
            End::Rejoin | End::Reopen(_) => true,
            End::Unasked => false,
        }
    }
}

/// What the client's last log says, for `ended`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EndFacts {
    pub banned_by_game: bool,
}

pub fn ended(r: Option<&Closing>, f: &EndFacts) -> End {
    let Some(r) = r else { return End::Unasked };
    match &r.then {
        Then::StayClosed => End::Closed { cause: r.cause, why: ON_PURPOSE },
        Then::Reopen(_) if f.banned_by_game => End::Closed { cause: r.cause, why: GAME_BANNED },
        Then::Reopen(t) => End::Reopen(t.clone()),
        Then::Rejoin => End::Rejoin,
    }
}

/// Who asks to open a window: the person (Play, a preset's Launch, the tray's New Roblox, the phone page) or Bloxkeep
/// on its own (auto-rejoin, a restart's reopen, a schedule).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    Person,
    Auto,
}

/// What is known when a window is asked to open (built by close_desk.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpenFacts {
    pub who: Who,
    /// Close, update, reopen is under way, or Roblox's updater runs from Bloxkeep.
    pub updating: bool,
    /// When the person or a schedule last closed this account (none for a window of no account).
    pub held_since: Option<Stamp>,
    /// When this open was asked.
    pub asked_at: Stamp,
    /// Roblox's windowless copies are being swept after a close.
    pub sweeping: bool,
    /// The PC just woke.
    pub settling: bool,
    /// A window of this account is still closing.
    pub closing: bool,
}

/// Why an open is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoOpen {
    /// O1: the person is told.
    Updating,
    /// O2: silently (the person closed it).
    ClosedSince,
}

impl NoOpen {
    pub fn words(&self) -> &'static str {
        match self {
            NoOpen::Updating => "Roblox updating",
            NoOpen::ClosedSince => "closed while the launch was under way",
        }
    }
}

/// Why an automatic open waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenHold {
    Updating,
    Sweeping,
    Settling,
    Closing,
}

impl OpenHold {
    pub fn words(&self) -> &'static str {
        match self {
            OpenHold::Updating => "Roblox updating",
            OpenHold::Sweeping => "Roblox still closing",
            OpenHold::Settling => "the PC just woke",
            OpenHold::Closing => "its window still closing",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Open {
    Go,
    Wait(OpenHold),
    Refuse(NoOpen),
}

/// O1-O4.
pub fn may_open(f: &OpenFacts) -> Open {
    // O1: a window opened now would be closed by the update, or make it give up.
    if f.updating {
        return match f.who {
            Who::Person => Open::Refuse(NoOpen::Updating),
            Who::Auto => Open::Wait(OpenHold::Updating),
        };
    }
    // O2: the person's Close (or a schedule's stop) came after this open was asked.
    if f.held_since.is_some_and(|held| held > f.asked_at) {
        return Open::Refuse(NoOpen::ClosedSince);
    }
    // O3: the person's own launch makes its own waits (the sweep, the wait between accounts).
    if f.who == Who::Auto {
        if f.sweeping {
            return Open::Wait(OpenHold::Sweeping);
        }
        if f.settling {
            return Open::Wait(OpenHold::Settling);
        }
        if f.closing {
            return Open::Wait(OpenHold::Closing);
        }
    }
    // O4.
    Open::Go
}
