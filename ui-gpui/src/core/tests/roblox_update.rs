// Tests/Bloxkeep.Tests/RobloxUpdateSuite.cs: the automatic Roblox update with windows open (app/roblox_updates.rs). On
// it closed a user's window a second after unlocking and neither updated nor opened it again: the wait for
// the windows to close gave up in the very tick it began, and the window, whose log hadn't been read yet, wasn't one it
// could open again.
use std::time::Duration;

use super::Checks;
use super::play_losses::utc_day;
use crate::core::app::roblox_updates::{close_time_up, would_stay_closed};

pub async fn run(c: &mut Checks, _dir: &str) {
    let limit = Duration::from_secs(30);
    let tick = utc_day(2026, 11, 11).plus_secs(3600.0 + 25.0 * 60.0 + 20.0);

    // The wait starts 200 ms after the tick's clock was read (the automatic update runs inside the tick).
    let mut since = tick.plus_secs(0.2);
    c.check("A wait begun inside a tick isn't over in that tick", !close_time_up(tick, &mut since, limit), "");
    c.check("…and counts from the tick's clock", since == tick, since);
    c.check("…nor a second later", !close_time_up(tick.plus_secs(1.5), &mut since, limit), "");
    c.check("…it is after 30 s", close_time_up(tick.plus_secs(30.0), &mut since, limit), "");

    // The PC's clock set back an hour while windows close: the 30 s start again, they aren't waited for an hour.
    let mut before = tick;
    let back = tick.plus_secs(-3600.0);
    c.check("A clock set back starts the wait again", !close_time_up(back, &mut before, limit) && before == back, before);
    c.check("…and it ends 30 s on", close_time_up(back.plus_secs(31.0), &mut before, limit), "");

    c.check("Every window open can be opened again: none would stay closed", would_stay_closed([11, 22], [22, 11]) == 0, "");
    c.check("A window that can't (not in a game yet) would", would_stay_closed([11, 22], [22]) == 1, "");
    c.check("…and so would one Bloxkeep didn't open", would_stay_closed([11, 22, 33], [11, 22]) == 1, "");
    c.check("No window open: nothing to keep", would_stay_closed([], [11]) == 0, "");
}
