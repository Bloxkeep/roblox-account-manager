// AppController.RejoinPlace.cs: a rejoin's window goes back where the account's last one was. A window arranged into a
// tile that crashed, was kicked or lost its connection came back at Roblox's own size (or maximised) over the others, and
// had to be arranged again. Each account's window place is noted while it is an ordinary window; a maximised one is left
// to Roblox and Settings' Maximise. A window that was minimised is minimised again once its rejoin is in the game. One
// rule for every rejoin: its window comes back where and as the account's last one was. A minimised window's place is
// the one it opens at (Windows keeps it: its normal position), so one minimised in its tile has that tile to go back to.
//
// The rule is window_plan.rs's (`rejoin_start`, `rejoin_place`, and the put-away of a rejoin's window); the places are
// noted and the windows moved by the window desk (window_desk.rs). What is left here are the entry points.
use std::{cell::RefCell, collections::HashMap};

use super::App;
use crate::core::{
    base::{Guid, Stamp},
    window_plan::Last,
};

#[derive(Default)]
pub struct RejoinPlaceState {
    /// Where and how each account's window last stood, noted by the window desk for its next rejoin's window.
    pub last: RefCell<HashMap<Guid, Last>>,
}

impl App {
    /// One of the window desk's entry points: notes each account's window place and puts rejoins' windows back.
    pub fn keep_rejoin_places(&self, now: Stamp, windows: &HashMap<i32, Vec<isize>>) {
        self.window_desk(now, windows);
    }

    /// A rejoin's window going back to its account's place: Settings' Maximise leaves it alone.
    pub fn going_back(&self, pid: i32) -> bool {
        self.desk_going_back(pid)
    }
}
