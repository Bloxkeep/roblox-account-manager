//! What a Roblox window of Bloxkeep's should be: open, minimised or maximised, and where it stands. Decided from what is
//! known of the window now (`Facts`, read once a tick by window_desk.rs) and what Bloxkeep asked of it before
//! (`Memory`). No Windows calls, no clock and no memory of its own: data in, data out, so every case is a plain check
//! (suite "window-plan"). The desk carries out the difference and writes back what Windows showed.
//!
//! Nothing here ever asks a minimised window to open: only a clicked or hotkey Arrange opens windows (window_desk.rs
//! `desk_arrange`), and `arranged` then leaves them as the person has them.
use std::time::Duration;

use crate::core::{
    app::presets::{MAXIMISE_WITHOUT_LOG, ON_JOIN_LOADING, ON_JOIN_RECENT, PUT_AWAY_WAIT},
    base::Stamp,
    win_kit::Rect,
};

/// A rejoin's window goes back where its account's last window stood once it is in its game, or after this without one.
pub const GO_BACK_WITHOUT_GAME: Duration = Duration::from_secs(60);
/// Roblox sizes its window again as it joins its game: a rejoin's window is put back a moment after.
pub const GO_BACK_SETTLE: Duration = Duration::from_secs(3);
/// A window put back is looked at once more this long after (Roblox can size it again a moment after joining).
pub const GO_BACK_CHECK: Duration = Duration::from_secs(6);
/// How many times one thing is asked of a window that doesn't take it before it is left as it is.
pub const ASKS: u8 = 2;
/// A window in use when it was due to be minimised is looked at again each tick for this long after it was due
/// (`ON_JOIN_RECENT` after its join line, for the join rule), then left up.
pub const IN_USE_WAIT: Duration = Duration::from_secs(60);
/// How far a window may stand from its tile and still be in it: Windows' unseen resize borders, by which an Arrange
/// lays each window over its tile (9 px a side at 150 %, more at higher scales).
pub const TILE_SLACK: i32 = 24;

fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

/// One thing Bloxkeep wants of a window, and how far it got.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Never,
    /// Asked at `at`, `tries` times so far; not seen to come about yet.
    Asked { at: Stamp, tries: u8 },
    /// Seen to come about (or found so already).
    Done { at: Stamp },
    /// Not done, on purpose: why, in a word or two. It isn't asked again.
    LeftUp(&'static str),
}

impl Step {
    /// Nothing more to ask.
    pub fn settled(self) -> bool {
        matches!(self, Step::Done { .. } | Step::LeftUp(_))
    }
}

/// How far a window has come since it opened, for Settings' Maximise: it is maximised once up a while ("shown"), and
/// again once in its game, as Roblox sizes its window again as it joins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    #[default]
    None,
    Shown,
    InGame,
}

/// Its game as its Roblox log says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Game {
    /// Nothing logged yet (still starting, Roblox's home screen, or a client that writes no log).
    Unknown,
    Joining { at: Stamp },
    InGame { at: Stamp },
    /// Left, kicked, disconnected or signed out.
    Out,
}

/// The window as Windows has it now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    pub exists: bool,
    pub minimised: bool,
    pub maximised: bool,
    /// Hidden by Bloxkeep (Ctrl+Alt+H, a hidden rejoin).
    pub hidden: bool,
    /// It has a title bar (Roblox's full screen has none).
    pub caption: bool,
    pub rect: Option<Rect>,
    /// Minimised: where it opens again as an ordinary window (None when it opens maximised or has no title bar).
    pub resting: Option<Rect>,
    /// Minimised: it opens maximised.
    pub opens_maximised: bool,
    /// Its window is the one in front.
    pub front: bool,
    /// "Not responding".
    pub hung: bool,
}

impl Seen {
    /// Shown as an ordinary window, title bar and all.
    pub fn plain(&self) -> bool {
        self.exists && !self.hidden && !self.minimised && !self.maximised && self.caption
    }
}

/// Where and how an account's last window stood, noted while it was open (for its rejoin's window).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Last {
    /// Its place as an ordinary window (where it opens, while minimised); None when it was maximised or full screen.
    pub place: Option<Rect>,
    pub minimised: bool,
}

/// A rejoin's window on its way back to where its account's last window stood.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Back {
    pub place: Rect,
    /// When the rejoin's window was first seen.
    pub since: Stamp,
    /// When it was first seen in its game (it settles a moment first).
    pub in_game_since: Option<Stamp>,
    /// When it was put there (None until then).
    pub put_at: Option<Stamp>,
    /// Places to open at asked of it while minimised.
    pub tries: u8,
}

/// What Bloxkeep asked of one window and when; kept by the desk per process id, dropped when the process ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    /// When its window was first there.
    pub first_seen: Stamp,
    /// Minimised as it joined (the join rule), or as its account's last window was (a rejoin).
    pub put_away: Step,
    /// The join rule has had its first look at it (a window already playing a while is left up).
    pub put_away_judged: bool,
    /// Settings' Maximise, at `maximised_for`.
    pub maximised: Step,
    pub maximised_for: Stage,
    /// A rejoin's window going back to its account's place.
    pub back: Option<Back>,
    /// A rejoin's window to minimise once loaded, as its account's last window was.
    pub rejoin_minimise: bool,
    /// A rejoin's window whose account's last window was read.
    pub rejoin_judged: bool,
    /// Its tile from the last Arrange: where it stands while open, and where it opens while minimised (the spot it was
    /// laid at, when the windows were too big for their tiles). None when the last Arrange gave it none.
    pub tile: Option<Rect>,
    /// Seen in that tile (or keeping it as the place it opens at) since the Arrange gave it: from then on the window is
    /// the person's to move, and the tile isn't given again until an Arrange gives new ones.
    pub tile_given: Step,
}

impl Memory {
    pub fn new(first_seen: Stamp) -> Memory {
        Memory {
            first_seen,
            put_away: Step::Never,
            put_away_judged: false,
            maximised: Step::Never,
            maximised_for: Stage::None,
            back: None,
            rejoin_minimise: false,
            rejoin_judged: false,
            tile: None,
            tile_given: Step::Never,
        }
    }
}

/// One window of Bloxkeep's as it is now and what is known about it, read once per tick.
#[derive(Clone, Copy, Debug)]
pub struct Facts {
    pub pid: i32,
    pub now: Stamp,
    /// A main account's (the window played on).
    pub main: bool,
    /// Its preset is a light-AFK one.
    pub afk_profile: bool,
    /// Opened by a rejoin.
    pub rejoin: bool,
    /// Opened since Bloxkeep started (one picked up after a restart was sized by the person already).
    pub opened_this_run: bool,
    /// "When a window joins a game" says Minimise (its preset's, else Settings'; always for a light-AFK preset).
    pub on_join_minimise: bool,
    /// Settings' Maximise applies to it (`maximise_on_open`).
    pub maximise_on_open: bool,
    pub game: Game,
    pub seen: Seen,
    /// Input in the last minute that wasn't Bloxkeep's.
    pub person_active: bool,
    /// A borrower has it (window_loan.rs).
    pub lent: bool,
    /// Its live view is open.
    pub watched: bool,
    /// Made big for a sharper live view just now: not where the account's window stands.
    pub sharp: bool,
    /// A rejoin's window not judged yet: where and how its account's last window stood (a place on no screen now is
    /// left out by the desk).
    pub last: Option<Last>,
    pub memory: Memory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    Minimise,
    Maximise,
}

/// What the window should be after this tick. The desk asks the difference: `place` (an ordinary window, unmaximised
/// first), then `show`, then for a minimised window `opens_at` and `opens_maximised`.
#[derive(Clone, Debug, PartialEq)]
pub struct Wanted {
    pub show: Option<Show>,
    /// Where it stands, open.
    pub place: Option<Rect>,
    /// Its place as an ordinary window while it stays minimised.
    pub opens_at: Option<Rect>,
    /// It stays minimised and opens maximised.
    pub opens_maximised: bool,
    /// As it is to be remembered; an ask is `Step::Asked` here, and the desk makes it `Done` once Windows shows it.
    pub memory: Memory,
    /// Which rule asked `show`: "join", "afk", "rejoin", "rejoin-late" (minimise); "shown", "ingame" (maximise).
    pub why: &'static str,
    /// Which rule asked `place` or `opens_at`: "rejoin", "rejoin-late", "rejoin-again", "tile".
    pub place_why: &'static str,
    /// Lines for the log about what was decided without an ask (left up, and why).
    pub notes: Vec<String>,
}

impl Wanted {
    /// Anything to ask of Windows.
    pub fn asks(&self) -> bool {
        self.show.is_some() || self.place.is_some() || self.opens_at.is_some() || self.opens_maximised
    }
}

/// Whether Settings' Maximise applies to a window: a light-AFK preset's stays down, a main account's is the one
/// played on (left as the person has it, as the tiles leave it), and a preset's own choice of how its windows open
/// (arranged, or left as Roblox opens them) is taken over Settings'.
pub fn maximise_on_open(setting: bool, preset_arrange: Option<bool>, afk_profile: bool, main: bool) -> bool {
    setting && preset_arrange.is_none() && !afk_profile && !main
}

/// The rule's word in the log.
pub fn label(why: &str) -> &'static str {
    match why {
        "afk" => "Light AFK",
        "join" => "On joining",
        "shown" | "ingame" => "Maximise",
        _ => "Rejoin place",
    }
}

pub fn plan(f: &Facts) -> Wanted {
    let mut w = Wanted { show: None, place: None, opens_at: None, opens_maximised: false, memory: f.memory, why: "", place_why: "", notes: Vec::new() };
    // Gone, frozen, or lent to a borrower (a nudge, a macro's turn, a picture): nothing asked, nothing changed. This
    // window alone; the others go on.
    if !f.seen.exists || f.lent || f.seen.hung {
        return w;
    }
    rejoin_start(f, &mut w);
    rejoin_place(f, &mut w);
    put_away(f, &mut w);
    tile(f, &mut w);
    open_maximised(f, &mut w);
    w
}

/// A window stands in `tile` (or opens there), give or take its unseen borders.
pub fn in_tile(r: Rect, tile: Rect) -> bool {
    let near = |a: i32, b: i32| (a - b).abs() <= TILE_SLACK;
    near(r.x, tile.x) && near(r.y, tile.y) && near(r.right(), tile.right()) && near(r.bottom(), tile.bottom())
}

/// What an Arrange did with a window's tile: `tile` None, it gave it none (any tile before is forgotten); `seen`, the
/// window was seen there (or keeps it as the place it opens at). Not seen there (asked and missed, or not asked: lent
/// to a borrower, not answering), it is asked on the next tick it can be, as `tile` says.
pub fn given_tile(m: &mut Memory, tile: Option<Rect>, seen: bool, now: Stamp) {
    m.tile = tile;
    m.tile_given = if tile.is_some() && seen { Step::Done { at: now } } else { Step::Never };
}

/// After an Arrange. A clicked or hotkey one tiled this window open (it opened the minimised ones), so the join rule
/// leaves it as the person now has it, put away before or not. A rejoin's window still on its way back stays in its new
/// tile, after any Arrange. (Its tile itself: `given_tile`.)
pub fn arranged(m: &mut Memory, clicked: bool) {
    m.back = None;
    if clicked && !m.put_away.settled() {
        m.put_away = Step::LeftUp("arranged");
        m.put_away_judged = true;
    }
}

/// What to do about one thing asked before: ask (the how-manieth time), wait for the last ask, or leave it.
enum Retry {
    Ask(u8),
    Wait,
    GiveUp,
}

fn retry(step: Step, now: Stamp) -> Retry {
    match step {
        Step::Asked { at, .. } if now >= at && now.secs_since(at) < secs(PUT_AWAY_WAIT) => Retry::Wait,
        Step::Asked { tries, .. } if tries >= ASKS => Retry::GiveUp,
        Step::Asked { tries, .. } => Retry::Ask(tries + 1),
        _ => Retry::Ask(1),
    }
}

/// A rejoin's new window, first looked at: it takes the place its account's last window had, and is minimised again
/// if that one was ("When a window joins a game" on Minimise minimises every window anyway).
fn rejoin_start(f: &Facts, w: &mut Wanted) {
    let m = &mut w.memory;
    if !f.rejoin || m.rejoin_judged || f.sharp {
        return;
    }
    m.rejoin_judged = true;
    let Some(last) = f.last else { return };
    m.rejoin_minimise = last.minimised && !f.on_join_minimise;
    if let Some(place) = last.place {
        m.back = Some(Back { place, since: f.now, in_game_since: None, put_at: None, tries: 0 });
    }
}

/// Done going back: where it was put stays (Settings' Maximise doesn't take it after).
fn end_back(m: &mut Memory) {
    m.back = None;
    m.maximised_for = Stage::InGame;
    m.maximised = Step::LeftUp("placed by its rejoin");
}

/// A rejoin's window goes back where its account's last one stood, once in its game and settled (or after
/// `GO_BACK_WITHOUT_GAME` without one), and is looked at once more `GO_BACK_CHECK` later. One minimised by then is given
/// the place to open at and is done once Windows keeps it; one hidden by a hidden rejoin is left as it is.
fn rejoin_place(f: &Facts, w: &mut Wanted) {
    let Some(mut back) = w.memory.back else { return };
    if f.sharp {
        return;
    }
    let now = f.now;
    if let Some(put_at) = back.put_at {
        if now.secs_since(put_at) < secs(GO_BACK_CHECK) {
            return;
        }
        if f.seen.plain() && f.seen.rect.is_some_and(|r| r != back.place) {
            w.place = Some(back.place);
            w.place_why = "rejoin-again";
        }
        end_back(&mut w.memory);
        return;
    }
    let in_game = matches!(f.game, Game::InGame { .. });
    if !in_game && now.secs_since(back.since) < secs(GO_BACK_WITHOUT_GAME) {
        return;
    }
    if in_game {
        let since = *back.in_game_since.get_or_insert(now);
        w.memory.back = Some(back);
        if now.secs_since(since) < secs(GO_BACK_SETTLE) {
            return;
        }
    }
    let why = if in_game { "rejoin" } else { "rejoin-late" };
    if f.seen.hidden {
        w.notes.push(format!("Rejoin place: window {} left as it is, hidden.", f.pid));
        w.memory.back = None;
        return;
    }
    if f.seen.minimised {
        if f.seen.resting == Some(back.place) {
            end_back(&mut w.memory);
        } else if back.tries >= ASKS {
            w.notes.push(format!("Rejoin place: window {} didn't keep the place it opens at; left as it is.", f.pid));
            end_back(&mut w.memory);
        } else {
            back.tries += 1;
            w.memory.back = Some(back);
            w.opens_at = Some(back.place);
            w.place_why = why;
        }
        return;
    }
    // (Maximised by Roblox, as its last one closed: the desk makes it an ordinary window first.)
    back.put_at = Some(now);
    back.in_game_since = None;
    w.memory.back = Some(back);
    w.place = Some(back.place);
    w.place_why = why;
}

/// A window whose "When a window joins a game" is Minimise (or a light-AFK preset's) is minimised once, `ON_JOIN_LOADING`
/// after its join line, so its game has loaded. A rejoin's window whose account's last window was minimised is too, once
/// it has gone back (or after `GO_BACK_WITHOUT_GAME` without a game). Only once: a window brought back up stays up.
fn put_away(f: &Facts, w: &mut Wanted) {
    if w.memory.put_away.settled() {
        return;
    }
    let now = f.now;
    let joined = match f.game {
        Game::InGame { at } => Some(at),
        _ => None,
    };
    let (why, due_at) = if f.on_join_minimise {
        // The first look with the rule on: a window that has played a while already isn't put away by a choice made
        // just now (a light-AFK one is). Looked at once, so a window kept busy past that isn't lost to the rule.
        if !w.memory.put_away_judged {
            w.memory.put_away_judged = true;
            if !f.afk_profile && joined.is_some_and(|at| now.secs_since(at) > secs(ON_JOIN_RECENT)) {
                w.memory.put_away = Step::LeftUp("already playing");
                return;
            }
        }
        let Some(at) = joined else { return };
        (if f.afk_profile { "afk" } else { "join" }, at.plus_secs(secs(ON_JOIN_LOADING)))
    } else if w.memory.rejoin_minimise && w.memory.back.is_none() {
        match joined {
            Some(at) => ("rejoin", at.plus_secs(secs(ON_JOIN_LOADING))),
            None => ("rejoin-late", w.memory.first_seen.plus_secs(secs(GO_BACK_WITHOUT_GAME))),
        }
    } else {
        return;
    };
    if now < due_at || f.watched {
        return;
    }
    let who = label(why);
    // A main account's window is the one played on: left as the person has it (a light-AFK preset's still goes).
    if why == "join" && f.main {
        w.memory.put_away = Step::LeftUp("main");
        w.notes.push(format!("{who}: window {} left as it is, a main account's.", f.pid));
        return;
    }
    if f.seen.minimised || f.seen.hidden {
        // An ask that landed after its wait was over was said as not held: said now that it did, as one that holds
        // at once is.
        if f.seen.minimised && matches!(w.memory.put_away, Step::Asked { .. }) {
            w.notes.push(format!("{who}: window {} minimised, in its game.", f.pid));
        }
        w.memory.put_away = Step::Done { at: now };
        return;
    }
    // In use: looked at again each tick while it is, and left up once it was in use the whole time.
    if f.seen.front && f.person_active {
        if now.secs_since(due_at) > secs(IN_USE_WAIT) {
            w.memory.put_away = Step::LeftUp("in use");
            w.notes.push(format!("{who}: window {} in its game left up, in use.", f.pid));
        }
        return;
    }
    match retry(w.memory.put_away, now) {
        Retry::Wait => {}
        Retry::GiveUp => {
            w.memory.put_away = Step::LeftUp("not taken");
            w.notes.push(format!("{who}: window {} left up, Windows didn't minimise it.", f.pid));
        }
        Retry::Ask(tries) => {
            w.show = Some(Show::Minimise);
            w.why = why;
            w.memory.put_away = Step::Asked { at: now, tries };
        }
    }
}

/// A window an Arrange gave a tile and that isn't seen there yet (lent while the Arrange ran, sized back by Roblox, or
/// gone down meanwhile): open, it is placed there; minimised, or going down this tick, the tile is where it opens (it
/// isn't opened for it). Seen there once, it is left where the person puts it. A hidden window, one without a title bar
/// (Roblox's full screen), one made big for a live view and a rejoin's still going back are looked at again later.
fn tile(f: &Facts, w: &mut Wanted) {
    let Some(tile) = w.memory.tile else { return };
    if w.memory.tile_given.settled() || w.memory.back.is_some() || f.sharp || f.seen.hidden || !f.seen.caption || w.place.is_some() || w.opens_at.is_some() {
        return;
    }
    let down = f.seen.minimised || w.show == Some(Show::Minimise);
    let there = if f.seen.minimised {
        f.seen.resting.is_some_and(|r| in_tile(r, tile))
    } else {
        !down && !f.seen.maximised && f.seen.rect.is_some_and(|r| in_tile(r, tile))
    };
    if there {
        w.memory.tile_given = Step::Done { at: f.now };
        return;
    }
    match retry(w.memory.tile_given, f.now) {
        Retry::Wait => {}
        Retry::GiveUp => {
            w.memory.tile_given = Step::LeftUp("not taken");
            w.notes.push(format!("Arrange: window {} didn't take its tile; left where it is.", f.pid));
        }
        Retry::Ask(tries) => {
            w.memory.tile_given = Step::Asked { at: f.now, tries };
            if down {
                w.opens_at = Some(tile);
            } else {
                w.place = Some(tile);
            }
            w.place_why = "tile";
        }
    }
}

/// Settings' Maximise for windows Bloxkeep opened this run: once up `MAXIMISE_WITHOUT_LOG` or in a game, and again on
/// joining if Roblox sized it back. Maximised, not full screen, so windows don't fight over Roblox's one full-screen
/// switch. A minimised window stays so and opens maximised; one the join rule minimises is put away first and then
/// opens maximised. Hidden, full screen (no title bar to maximise), or behind what the person is doing: looked at again
/// next tick, not marked done. A window an Arrange gave a tile stays in it.
fn open_maximised(f: &Facts, w: &mut Wanted) {
    if !f.maximise_on_open || f.main || !f.opened_this_run || w.memory.back.is_some() || w.memory.tile.is_some() {
        return;
    }
    let now = f.now;
    let stage = if matches!(f.game, Game::InGame { .. }) {
        Stage::InGame
    } else if now.secs_since(w.memory.first_seen) >= secs(MAXIMISE_WITHOUT_LOG) {
        Stage::Shown
    } else {
        Stage::None
    };
    let m = &mut w.memory;
    if stage > m.maximised_for {
        m.maximised_for = stage;
        m.maximised = Step::Never;
    } else if m.maximised_for == Stage::None || m.maximised.settled() {
        return;
    }
    if f.on_join_minimise && !f.seen.minimised && !f.seen.hidden {
        // Put away this tick, in its game: it opens maximised once down.
        if w.show == Some(Show::Minimise) && m.maximised_for == Stage::InGame {
            if let Retry::Ask(tries) = retry(m.maximised, now) {
                m.maximised = Step::Asked { at: now, tries };
                w.opens_maximised = true;
            }
            return;
        }
        // Not maximised in view before it is put away (it would come to the front only to go). Put away and opened
        // again, or left up: maximised as any other.
        if m.maximised_for != Stage::InGame || !m.put_away.settled() {
            return;
        }
    }
    if f.seen.minimised {
        m.maximised_for = Stage::InGame;
        if f.seen.opens_maximised {
            m.maximised = Step::Done { at: now };
            return;
        }
        match retry(m.maximised, now) {
            Retry::Wait => {}
            Retry::GiveUp => {
                m.maximised = Step::LeftUp("not taken");
                w.notes.push(format!("Maximise: window {} left minimised, Windows didn't keep it opening maximised.", f.pid));
            }
            Retry::Ask(tries) => {
                m.maximised = Step::Asked { at: now, tries };
                w.opens_maximised = true;
            }
        }
        return;
    }
    if f.seen.hidden || !f.seen.caption {
        return;
    }
    if f.seen.maximised {
        m.maximised = Step::Done { at: now };
        return;
    }
    if !f.seen.front && f.person_active {
        return; // the next tick: not over what the person is doing
    }
    match retry(m.maximised, now) {
        Retry::Wait => {}
        Retry::GiveUp => {
            m.maximised = Step::LeftUp("not taken");
            w.notes.push(format!("Maximise: window {} left as it is, Windows didn't maximise it.", f.pid));
        }
        Retry::Ask(tries) => {
            m.maximised = Step::Asked { at: now, tries };
            w.show = Some(Show::Maximise);
            w.why = if m.maximised_for == Stage::InGame { "ingame" } else { "shown" };
        }
    }
}
