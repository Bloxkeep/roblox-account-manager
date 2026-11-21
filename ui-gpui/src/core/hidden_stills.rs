//! Pictures of the Roblox windows that are hidden or minimized, for their tiles in the Accounts page's windows view:
//! Windows draws no live picture for such a window, and the tile showed only the game's picture with "Window hidden",
//! so nobody could tell how an account was doing without showing every window (elguestvr on Discord, 2026-11-21: "i
//! cant know if one account happened something because i need to open all and hide all").
//!
//! Each window is pictured as /pc screenshot pictures it (window_shot: shown unseen for a moment, then put back), one
//! window at a time, each about every EVERY, and only while its tile is drawn (the page asks again at each frame; the
//! windows view is drawn every 1.5 s while it shows). A turn already taken (anti-AFK, a macro, Arrange) is never waited
//! on: the picture waits for the next look.
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::threads::Hold;

use super::{input_turn, log, roblox_processes as players, win_kit::Picture, window_shot};

/// How often each window is pictured again.
const EVERY: Duration = Duration::from_secs(30);
/// Between two windows' pictures: the PC (and the game shown unseen) gets its breath.
const GAP: Duration = Duration::from_secs(2);
/// A window whose tile wasn't drawn for this long isn't pictured any more.
const WANT_FOR: Duration = Duration::from_secs(6);
/// The pictures' width: a tile is at most about 460 px wide on a large screen.
const WIDTH: i32 = 640;

/// A window's latest picture: `serial` changes with each new one.
#[derive(Clone)]
pub struct Still {
    pub serial: u64,
    pub picture: Arc<Picture>,
}

#[derive(Default)]
struct State {
    /// Windows whose tile was drawn, and when last.
    wanted: HashMap<isize, Instant>,
    stills: HashMap<isize, Still>,
    /// When each window was last pictured or tried.
    tried: HashMap<isize, Instant>,
    /// Windows whose failed picture went to error.log (once a window).
    said: HashSet<isize>,
    serial: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Window `h`'s tile is drawn now: it gets pictured (and kept pictured while asked). Its latest picture, if any.
pub fn want(h: isize) -> Option<Still> {
    if h == 0 {
        return None;
    }
    let mut g = STATE.hold();
    let st = g.get_or_insert_with(State::default);
    st.wanted.insert(h, Instant::now());
    if !RUNNING.swap(true, Ordering::SeqCst) {
        crate::threads::run(work);
    }
    st.stills.get(&h).cloned()
}

/// The next window due for a picture, or None while none is; Err once nothing is wanted (the worker ends).
fn due() -> Result<Option<isize>, ()> {
    let mut g = STATE.hold();
    let st = g.get_or_insert_with(State::default);
    st.wanted.retain(|_, at| at.elapsed() < WANT_FOR);
    if st.wanted.is_empty() {
        // (Said under the lock: a `want` meanwhile either sees the worker still running or starts a new one.)
        RUNNING.store(false, Ordering::SeqCst);
        st.stills.clear();
        st.tried.clear();
        return Err(());
    }
    let wanted = &st.wanted;
    st.stills.retain(|h, _| wanted.contains_key(h));
    st.tried.retain(|h, _| wanted.contains_key(h));
    let tried = &st.tried;
    Ok(wanted
        .keys()
        .copied()
        .filter(|h| tried.get(h).is_none_or(|t| t.elapsed() >= EVERY))
        .min_by_key(|h| tried.get(h).copied()))
}

fn work() {
    loop {
        let h = match due() {
            Err(()) => return,
            Ok(None) => {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            Ok(Some(h)) => h,
        };
        // Shown again meanwhile (its live picture is drawn instead), or gone.
        if !(players::is_hidden(h) || players::is_minimised(h)) {
            mark(h, None, None);
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        // Someone has the windows (anti-AFK, a macro, Arrange): looked at again in a moment.
        let Some(turn) = input_turn::try_enter("hidden picture") else {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        let (picture, how) = window_shot::still(h, WIDTH);
        drop(turn);
        mark(h, picture, Some(how));
        std::thread::sleep(GAP);
    }
}

/// Window `h` was tried now: its new picture kept, a failure said once.
fn mark(h: isize, picture: Option<Picture>, how: Option<String>) {
    let mut g = STATE.hold();
    let st = g.get_or_insert_with(State::default);
    st.tried.insert(h, Instant::now());
    match picture {
        Some(p) => {
            st.serial += 1;
            st.stills.insert(h, Still { serial: st.serial, picture: Arc::new(p) });
        }
        None => {
            if let Some(how) = how
                && st.said.insert(h)
            {
                log::note_file(&format!("Hidden window picture: none for window {h:#x}: {how}."));
            }
        }
    }
}
