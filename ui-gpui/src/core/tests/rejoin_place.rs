// Tests/Bloxkeep.Tests/RejoinPlaceSuite.cs: a rejoin's window goes back where the account's last window stood (noted
// while the old one is open, put back once the new one is in its game and has settled, checked once more after, left
// alone when Arrange placed it meanwhile). Every check moves and reads real windows of the test's own, so none of them
// runs here; app/rejoin_place.rs was compared line by line with AppController.RejoinPlace.cs instead (2026-11-14).
use super::Checks;

pub async fn run(c: &mut Checks, _dir: &str) {
    for what in [
        "the account's window place is noted while it is open",
        "the rejoin's window waits to go back",
        "not moved while it joins",
        "the account's place isn't overwritten by the new window's Roblox size",
        "not moved the moment it gets in",
        "put back where the old window stood",
        "sized again after joining, it is put back once more",
        "done going back",
        "Settings' Maximise leaves it where it went",
        "a window moved by hand: its new place is noted",
        "the next rejoin waits to go back",
        "arranged meanwhile: not taken back to the old place",
    ] {
        c.skip(what, "needs a window");
    }
}
