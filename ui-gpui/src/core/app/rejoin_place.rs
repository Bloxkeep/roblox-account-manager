// AppController.RejoinPlace.cs: a rejoin's window goes back where the account's last one was. A window arranged into a
// tile that crashed, was kicked or lost its connection came back at Roblox's own size (or maximised) over the others, and
// had to be arranged again (a user's request, 2026-11-11). Each account's window place is noted while it is an ordinary
// window; a maximised one is left to Roblox and Settings' Maximise.
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    time::Duration,
};

use super::App;
use crate::core::{
    base::{Guid, Stamp},
    log, roblox_processes,
    win_kit::{self, Rect},
};

/// Roblox sizes its window again as it joins its game: put back once in it (or up this long), a moment after.
const GO_BACK_WITHOUT_GAME: Duration = Duration::from_secs(60);
const GO_BACK_SETTLE: Duration = Duration::from_secs(3);
const GO_BACK_CHECK: Duration = Duration::from_secs(6);

/// A rejoin's window going back: the place, when the window was first seen, and when it was put there (None until then).
#[derive(Clone, Copy)]
struct Back {
    place: Rect,
    seen: Stamp,
    put_at: Option<Stamp>,
}

#[derive(Default)]
pub struct RejoinPlaceState {
    /// Where each account's window last stood as an ordinary window (not maximised, minimised or hidden).
    last_place: RefCell<HashMap<Guid, Rect>>,
    /// Rejoins' windows (by process id) going back to their account's place.
    going_back: RefCell<HashMap<i32, Back>>,
    /// Windows whose place is noted (theirs, or one already gone back).
    place_noted: RefCell<HashSet<i32>>,
    /// When each window going back was first seen in its game (it settles a moment first).
    in_game_since: RefCell<HashMap<i32, Stamp>>,
}

fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

fn on_a_screen(place: Rect) -> bool {
    win_kit::all_screens().iter().any(|s| intersects(s.working_area, place))
}

impl App {
    pub fn keep_rejoin_places(&self, now: Stamp, windows: &HashMap<i32, Vec<isize>>) {
        let st = &self.rejoin_place;
        st.going_back.borrow_mut().retain(|pid, _| windows.contains_key(pid));
        st.place_noted.borrow_mut().retain(|pid| windows.contains_key(pid));
        {
            let going = st.going_back.borrow();
            st.in_game_since.borrow_mut().retain(|pid, _| going.contains_key(pid));
        }
        // `_vault is null`: the lock screen (vault still open) doesn't stop it, so a rejoin while away goes back too.
        if self.vault_ref().is_none() || self.windows_in_turn() {
            return;
        }
        let runs: Vec<(Guid, i32, bool)> = self.controller.running.borrow().iter().map(|(id, (pid, l))| (*id, *pid, l.rejoin)).collect();
        for (id, pid, rejoin) in runs {
            let Some(list) = windows.get(&pid).filter(|l| !l.is_empty()) else { continue };
            let main = roblox_processes::main_window_of(list);
            if main == 0 {
                continue;
            }
            // A rejoin's new window: first seen, it takes the place the account's last window had.
            if rejoin && !st.place_noted.borrow().contains(&pid) && !st.going_back.borrow().contains_key(&pid) {
                let last = st.last_place.borrow().get(&id).copied();
                match last.filter(|p| on_a_screen(*p)) {
                    Some(place) => {
                        st.going_back.borrow_mut().insert(pid, Back { place, seen: now, put_at: None });
                    }
                    // Nothing to go back to (it was maximised, or off every screen now).
                    None => {
                        st.place_noted.borrow_mut().insert(pid);
                    }
                }
            }
            let back = st.going_back.borrow().get(&pid).copied();
            if let Some(back) = back {
                self.go_back(pid, main, back, now);
                continue;
            }
            st.place_noted.borrow_mut().insert(pid);
            if roblox_processes::is_hidden(main) || roblox_processes::is_minimised(main) {
                continue; // put away: the place it had stays
            }
            match roblox_processes::window_rect(main).filter(|_| roblox_processes::is_plain_window(main)) {
                Some(rect) => {
                    st.last_place.borrow_mut().insert(id, rect);
                }
                // Maximised or full screen: the next window is left to Roblox and Maximise.
                None => {
                    st.last_place.borrow_mut().remove(&id);
                }
            }
        }
    }

    fn go_back(&self, pid: i32, main: isize, back: Back, now: Stamp) {
        let st = &self.rejoin_place;
        if let Some(put_at) = back.put_at {
            // Checked once more: Roblox can size it again a moment after joining.
            if now.secs_since(put_at) < secs(GO_BACK_CHECK) {
                return;
            }
            if roblox_processes::is_plain_window(main)
                && let Some(after) = roblox_processes::window_rect(main)
                && after != back.place
            {
                log::note(&format!("Rejoin place: window {pid} moved again to {}×{} after going back; put back once more.", after.width, after.height));
                roblox_processes::place_window(main, back.place);
            }
            st.going_back.borrow_mut().remove(&pid);
            st.place_noted.borrow_mut().insert(pid);
            // Where it was put stays: Settings' Maximise doesn't take it after.
            self.presets.maximised.borrow_mut().insert(pid, "ingame".into());
            return;
        }
        let in_game = self.controller.logs.borrow().state_of(pid).is_some_and(|s| s.status == "ingame");
        if !in_game && now.secs_since(back.seen) < secs(GO_BACK_WITHOUT_GAME) {
            return;
        }
        let since = *st.in_game_since.borrow_mut().entry(pid).or_insert(now);
        if in_game && now.secs_since(since) < secs(GO_BACK_SETTLE) {
            return;
        }
        st.in_game_since.borrow_mut().remove(&pid);
        // Put away by then (minimised, hidden by a hidden rejoin): left as it is, its place kept for the next one.
        let hidden = roblox_processes::is_hidden(main);
        if hidden || roblox_processes::is_minimised(main) {
            log::note(&format!("Rejoin place: window {pid} left as it is, {}.", if hidden { "hidden" } else { "minimised" }));
            st.going_back.borrow_mut().remove(&pid);
            st.place_noted.borrow_mut().insert(pid);
            return;
        }
        // Maximised by Roblox (its last close): back to an ordinary window first.
        roblox_processes::unmaximise_quietly(main);
        roblox_processes::place_window(main, back.place);
        log::note(&format!(
            "Rejoin place: window {pid} put back at {}×{}, where the account's last window was{}.",
            back.place.width,
            back.place.height,
            if in_game { String::new() } else { format!(" (not in its game after {} s)", GO_BACK_WITHOUT_GAME.as_secs()) }
        ));
        st.going_back.borrow_mut().insert(pid, Back { put_at: Some(now), ..back });
    }

    /// A rejoin's window going back to its account's place: Settings' Maximise leaves it alone.
    pub fn going_back(&self, pid: i32) -> bool {
        self.rejoin_place.going_back.borrow().contains_key(&pid)
    }

    /// Arrange placed every window since: one still waiting to go back stays in its new tile.
    pub fn forget_going_back(&self) {
        let st = &self.rejoin_place;
        let pids: Vec<i32> = st.going_back.borrow_mut().drain().map(|(pid, _)| pid).collect();
        st.place_noted.borrow_mut().extend(pids);
    }
}
