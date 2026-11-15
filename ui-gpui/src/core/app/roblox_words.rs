// Roblox's own words in error.log. Bloxkeep's log says what Bloxkeep did; why Roblox showed an error box, quit as it
// started or left a game is only in Roblox's own log, which users rarely send (owner, 2026-11-15: "error log should
// cover it all"). So each window that ends on its own or by the person in Roblox (not closed by Bloxkeep) and each client
// that quits before its window gets the lines of its Roblox log that say why (roblox_log_watcher::keep_line), with
// names, ids, tokens and the Windows user's folder taken out as a bug report's log is (feedback.rs scrub).
use crate::core::base::{Guid, Stamp};
use std::cell::Cell;
use std::sync::LazyLock;

use regex::Regex;

use super::App;
use super::controller::PendingLaunch;
use crate::core::log;

thread_local! {
    /// Clients in a row that quit before their window while other Roblox windows stayed open (see note_quit).
    static QUITS_IN_A_ROW: Cell<u32> = const { Cell::new(0) };
}

/// A Windows user folder in a path (C:\Users\<name>\…): the scrub misses names under three letters.
static USER_FOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\\users\\[^\\/\s]+").expect("user-folder pattern"));

impl App {
    fn scrub_roblox_words(&self, text: &str) -> String {
        self.scrub(&USER_FOLDER.replace_all(text, r"\Users\[user]"))
    }

    /// The kept lines of window `pid`'s Roblox log, into error.log only (not the Console).
    pub fn note_roblox_lines(&self, pid: i32, why: &str) {
        let lines = self.controller.logs.borrow_mut().lines_of(pid);
        let text = if lines.is_empty() {
            format!("Roblox's log of window {pid} ({why}): not found, nothing to add.")
        } else {
            format!("Roblox's log of window {pid} ({why}):\r\n    {}", lines.join("\r\n    "))
        };
        log::note_file(&self.scrub_roblox_words(&text));
    }

    /// A drop whose window Bloxkeep is about to close itself, to rejoin, or leaves open (rejoin off): Roblox's own
    /// lines of it, into error.log. When such a window ends its lines are skipped (the close is Bloxkeep's own), and
    /// Bloxkeep's line alone ("code 256", "code none") never says why Roblox dropped it: with Error Code 256 the answer
    /// is only in Roblox's lines before the drop (the owner, 2026-11-15: "error log có cover được hết không"; it didn't).
    /// At most once in ten minutes an account: one that keeps dropping would fill the log with the same lines.
    pub fn note_drop_lines(&self, account: Guid, pid: i32, why: &str) {
        let now = Stamp::utc_now();
        {
            let mut said = self.rejoin.drop_lines_at.borrow_mut();
            if said.get(&account).is_some_and(|at| now.secs_since(*at) < 600.0) {
                return;
            }
            said.insert(account, now);
        }
        self.note_roblox_lines(pid, why);
    }

    /// A window came up: the run of quits (note_quit) is over.
    pub fn window_came_up(&self) {
        QUITS_IN_A_ROW.with(|q| q.set(0));
    }

    /// A launch's client quit before its window: its Roblox log's lines (followed, or found afterwards by when it began).
    /// Returns how many quit in a row with other Roblox windows open, counting this one (0 with none open). A user's log
    /// (2026-11-10 to 11-14, five times): once that starts, every new client quits for any account until all windows are
    /// closed; closing them all is the only recovery the log shows.
    pub fn note_quit(&self, launch: &PendingLaunch) -> u32 {
        let others_open = !self.controller.running.borrow().is_empty();
        let in_a_row = QUITS_IN_A_ROW.with(|q| {
            q.set(if others_open { q.get() + 1 } else { 0 });
            q.get()
        });
        let mut found: Vec<(String, Vec<String>)> = self
            .controller
            .logs
            .borrow()
            .ended_since(launch.started_at.get(), &launch.before)
            .into_iter()
            .take(2)
            .map(|(pid, lines)| (format!("client {pid}"), lines))
            .collect();
        if found.is_empty() {
            found = self.controller.logs.borrow().unfollowed_logs_since(launch.started_at.get()).into_iter().map(|(name, lines)| (format!("log {name}"), lines)).collect();
        }
        if found.is_empty() {
            log::note_file("Roblox's log of the client that quit: none found (no Player log begun since the start).");
        }
        for (what, lines) in found {
            let text = if lines.is_empty() {
                format!("Roblox's log of {what}, which quit before its window: nothing in it.")
            } else {
                format!("Roblox's log of {what}, which quit before its window:\r\n    {}", lines.join("\r\n    "))
            };
            log::note_file(&self.scrub_roblox_words(&text));
        }
        in_a_row
    }
}
