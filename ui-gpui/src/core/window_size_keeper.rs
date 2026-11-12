//! A Roblox window's place and size from before it went full screen, put back when it leaves it: a user's report says
//! Roblox comes back at its default size (2026-11-01). Where it doesn't, the place already matches and nothing moves.
use std::collections::HashMap;

use super::{base::Stamp, win_kit::Rect};

/// Roblox sizes its window again as it settles, so the size is held for a moment (TimeSpan.FromSeconds(5)).
const HOLD_SECS: f64 = 5.0;

struct Seen {
    place: Rect,
    full: bool,
    hold_until: Option<Stamp>,
}

#[derive(Default)]
pub struct WindowSizeKeeper {
    seen: HashMap<i32, Seen>,
}

impl WindowSizeKeeper {
    pub fn new() -> WindowSizeKeeper {
        WindowSizeKeeper::default()
    }

    /// The place to put the window back to, if any.
    /// `full`: covering its screen, without a title bar.
    /// `plain`: an ordinary window: not full screen, minimized, maximized or hidden.
    pub fn look(&mut self, pid: i32, now: Rect, full: bool, plain: bool, at: Stamp) -> Option<Rect> {
        let Some(seen) = self.seen.get_mut(&pid) else {
            if plain {
                self.seen.insert(pid, Seen { place: now, full: false, hold_until: None });
            }
            return None;
        };
        if full {
            seen.full = true;
            return None;
        }
        if !plain {
            return None;
        }
        if seen.full {
            seen.full = false;
            seen.hold_until = Some(at.plus_secs(HOLD_SECS));
        }
        // default(DateTime) for HoldUntil: no moment is before it.
        if seen.hold_until.is_some_and(|until| at < until) {
            return if now == seen.place { None } else { Some(seen.place) };
        }
        seen.place = now;
        None
    }

    pub fn forget(&mut self, alive: &[i32]) {
        self.seen.retain(|pid, _| alive.contains(pid));
    }

    /// Bloxkeep is about to place the windows itself: what it gives them is the size to keep.
    pub fn clear(&mut self) {
        self.seen.clear();
    }
}
