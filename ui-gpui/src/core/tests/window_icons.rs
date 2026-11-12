// Tests/Bloxkeep.Tests/WindowIconsSuite.cs: the account's picture as its window's icon (WindowIcons). The C# put the
// icons on a window of its own and read them back: those checks need a window and are skipped. Which client is checked
// when, a client's icons forgotten when it closes, and the initials picture are pure and run here, on clients with no
// window listed.
use std::{collections::HashSet, sync::atomic::{AtomicI64, Ordering}};

use super::Checks;
use crate::core::window_icons;

static NOW: AtomicI64 = AtomicI64::new(1_000_000);

fn now() -> i64 {
    NOW.load(Ordering::SeqCst)
}

pub async fn run(c: &mut Checks, _dir: &str) {
    *window_icons::CLOCK.lock().unwrap() = Some(now);
    let none: &[isize] = &[];

    c.skip("the account's picture is put on its window", "needs a window");
    c.skip("…and kept, not drawn again, while it is the same", "needs a window");
    c.skip("a window isn't asked about its icon again within half a minute", "needs a window");
    c.skip("a window that dropped it gets it again at its next check", "needs a window");
    c.skip("a new picture (the avatar has loaded) replaces the initials", "needs a window");

    // Two clients due at once: one is asked a process check, the one asked longest ago first, then the other. As in the
    // C#, 4242 was last given its picture at 1 030 000 and 4244 a second later.
    NOW.store(1_030_000, Ordering::SeqCst);
    window_icons::apply(4242, none, "https://tr.rbxcdn.com/avatar.png", &|| window_icons::initials("Avatar"), false);
    NOW.fetch_add(1_000, Ordering::SeqCst);
    window_icons::apply(4244, none, "initials:Account 3", &|| window_icons::initials("Account 3"), false);
    NOW.fetch_add(30_000, Ordering::SeqCst);
    let turns = [window_icons::next_to_check(&[4242, 4244]), window_icons::next_to_check(&[4242, 4244]), window_icons::next_to_check(&[4242, 4244])];
    c.check(
        "one client checked at a time, the one checked longest ago first, then the other",
        turns == [Some(4242), Some(4244), None],
        turns,
    );

    window_icons::remove(4242, none);
    c.skip("turned off: the window's own icon back", "needs a window");

    window_icons::apply(4243, none, "initials:Account 2", &|| window_icons::initials("Account 2"), false);
    window_icons::forget(&HashSet::new());
    c.check("a client that closed: its icons go", !window_icons::pids().contains(&4243), window_icons::pids());

    c.skip("the windows given icons are written down", "needs a window");
    c.skip("the next start takes a crashed run's icons off its windows", "needs a window");
    c.skip("…and none of them is put back later as a window's own", "needs a window");

    let some = window_icons::initials("");
    c.check("a name without letters still gets a picture", some.width == 96, some.width);
}
