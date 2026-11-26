// window_plan.rs: what a Roblox window should be, decided from what is known of it now and what was asked of it before.
// Pure: every case is made up here (no window, no clock), one tick after another, with what Windows "shows" written
// into the facts of the next tick.
use super::Checks;
use crate::core::{
    base::Stamp,
    win_kit::Rect,
    window_plan::{Back, Facts, Game, Last, Memory, Seen, Show, Stage, Step, Wanted, arranged, given_tile, in_tile, maximise_on_open, plan},
};

fn t(s: f64) -> Stamp {
    Stamp::from_unix_secs(1_800_000_000).plus_secs(s)
}

const HOME: Rect = Rect { x: 100, y: 100, width: 800, height: 600 };
const TILE: Rect = Rect { x: 900, y: 0, width: 640, height: 480 };

fn open() -> Seen {
    Seen { exists: true, caption: true, rect: Some(HOME), ..Default::default() }
}

fn down(resting: Option<Rect>) -> Seen {
    Seen { exists: true, caption: true, minimised: true, resting, rect: Some(Rect::new(-32000, -32000, 160, 28)), ..Default::default() }
}

fn facts(now: f64) -> Facts {
    Facts {
        pid: 7,
        now: t(now),
        main: false,
        afk_profile: false,
        rejoin: false,
        opened_this_run: true,
        on_join_minimise: false,
        maximise_on_open: false,
        game: Game::Unknown,
        seen: open(),
        person_active: false,
        lent: false,
        watched: false,
        sharp: false,
        last: None,
        memory: Memory::new(t(0.)),
    }
}

/// The next tick's facts: what was remembered, at `now`, the window as `seen`.
fn next(f: &Facts, w: &Wanted, now: f64, seen: Seen) -> Facts {
    Facts { now: t(now), seen, memory: w.memory, last: None, ..*f }
}

fn nothing(w: &Wanted) -> bool {
    !w.asks()
}

pub async fn run(c: &mut Checks, _dir: &str) {
    join_rule(c);
    open_rule(c);
    rejoin(c);
    arrange(c);
    tile(c);
    left_alone(c);
}

fn join_rule(c: &mut Checks) {
    let mut f = facts(5.);
    f.on_join_minimise = true;
    f.game = Game::Joining { at: t(0.) };
    let w = plan(&f);
    c.check("join rule: not before its game", nothing(&w) && w.memory.put_away == Step::Never, &w);
    let f = Facts { game: Game::InGame { at: t(10.) }, ..next(&f, &w, 20., open()) };
    let w = plan(&f);
    c.check("…not while its game still loads", nothing(&w), &w);
    let f = next(&f, &w, 41., open());
    let w = plan(&f);
    c.check("…minimised once loaded", w.show == Some(Show::Minimise) && w.why == "join" && matches!(w.memory.put_away, Step::Asked { tries: 1, .. }), &w);
    let asked = w.clone();
    let f2 = next(&f, &asked, 42., down(Some(HOME)));
    let w = plan(&f2);
    c.check("…the ask is done once it is seen minimised, not before", nothing(&w) && matches!(w.memory.put_away, Step::Done { .. }), &w);
    let w = plan(&next(&f2, &w, 100., open()));
    c.check("…opened again by the person: it stays open (never twice)", nothing(&w) && matches!(w.memory.put_away, Step::Done { .. }), &w);

    // Not seen minimised: asked once more after a while, then left up with a line.
    let w = plan(&next(&f, &asked, 43., open()));
    c.check("an ask not seen yet: not asked again at once", nothing(&w) && matches!(w.memory.put_away, Step::Asked { tries: 1, .. }), &w);
    let f3 = next(&f, &w, 46.5, open());
    let w = plan(&f3);
    c.check("…asked again after 5 s", w.show == Some(Show::Minimise) && matches!(w.memory.put_away, Step::Asked { tries: 2, .. }), &w);
    let w = plan(&next(&f3, &w, 52., open()));
    c.check("…then left up, said in the log, never marked done", nothing(&w) && w.memory.put_away == Step::LeftUp("not taken") && w.notes.len() == 1, &w);

    // In use: asked again until 90 s after the join, not marked done the first time.
    let in_use = Seen { front: true, ..open() };
    let fu = Facts { person_active: true, ..next(&f, &Wanted { memory: f.memory, ..asked.clone() }, 41., in_use) };
    let w = plan(&fu);
    c.check("in front and in use when due: not minimised, not marked done", nothing(&w) && w.memory.put_away == Step::Never, &w);
    let w2 = plan(&Facts { person_active: false, ..next(&fu, &w, 60., open()) });
    c.check("…minimised once the person leaves it, before 90 s", w2.show == Some(Show::Minimise), &w2);
    let w3 = plan(&next(&fu, &w, 99., in_use));
    c.check("…still in use at 89 s after its join: still looked at", nothing(&w3) && w3.memory.put_away == Step::Never, &w3);
    let w4 = plan(&next(&fu, &w, 101., in_use));
    c.check("…in use the whole time: left up past 90 s, said once", nothing(&w4) && w4.memory.put_away == Step::LeftUp("in use") && w4.notes.len() == 1, &w4);

    // A main account's window; a light-AFK one of a main account.
    let fm = Facts { main: true, ..f };
    let w = plan(&fm);
    c.check("a main account's window is left as it is", nothing(&w) && w.memory.put_away == Step::LeftUp("main") && w.notes.len() == 1, &w);
    let w = plan(&Facts { afk_profile: true, ..fm });
    c.check("…a light-AFK preset's main window is minimised all the same", w.show == Some(Show::Minimise) && w.why == "afk", &w);

    // Already playing a while when first looked at with the rule on.
    let fl = Facts { game: Game::InGame { at: t(0.) }, ..facts(200.) };
    let fl = Facts { on_join_minimise: true, ..fl };
    let w = plan(&fl);
    c.check("a window in its game long before the rule: left up", nothing(&w) && w.memory.put_away == Step::LeftUp("already playing"), &w);
    let w = plan(&Facts { afk_profile: true, ..fl });
    c.check("…a light-AFK one is minimised", w.show == Some(Show::Minimise), &w);

    // Looked at while joining, then not again until long after (the pass was busy): not lost.
    let mut fj = facts(5.);
    fj.on_join_minimise = true;
    fj.game = Game::Joining { at: t(0.) };
    let w = plan(&fj);
    let w = plan(&Facts { game: Game::InGame { at: t(10.) }, ..next(&fj, &w, 150., open()) });
    c.check("a window first looked at while joining, next looked at long after: still minimised", w.show == Some(Show::Minimise), &w);

    let w = plan(&Facts { watched: true, ..f });
    c.check("its live view open: waits", nothing(&w) && w.memory.put_away == Step::Never, &w);
    let w = plan(&Facts { seen: Seen { hidden: true, ..open() }, ..f });
    c.check("hidden by Bloxkeep when due: done, nothing asked", nothing(&w) && matches!(w.memory.put_away, Step::Done { .. }), &w);
}

fn open_rule(c: &mut Checks) {
    c.check(
        "Maximise applies: not a main account's, not light-AFK, not a preset that arranges or leaves its windows",
        maximise_on_open(true, None, false, false)
            && !maximise_on_open(true, None, false, true)
            && !maximise_on_open(true, None, true, false)
            && !maximise_on_open(true, Some(true), false, false)
            && !maximise_on_open(true, Some(false), false, false)
            && !maximise_on_open(false, None, false, false),
        (),
    );
    let f = Facts { maximise_on_open: true, ..facts(5.) };
    let w = plan(&f);
    c.check("Maximise: not in its first seconds", nothing(&w), &w);
    let f = next(&f, &w, 11., open());
    let w = plan(&f);
    c.check("…maximised once up 10 s", w.show == Some(Show::Maximise) && w.why == "shown" && w.memory.maximised_for == Stage::Shown, &w);
    let asked = w.clone();
    let w = plan(&next(&f, &asked, 12., Seen { maximised: true, ..open() }));
    c.check("…done once seen maximised", nothing(&w) && matches!(w.memory.maximised, Step::Done { .. }), &w);
    let w = plan(&Facts { game: Game::InGame { at: t(20.) }, ..next(&f, &w, 21., open()) });
    c.check("…and again in its game when Roblox sized it back", w.show == Some(Show::Maximise) && w.why == "ingame", &w);

    // Refused: not marked done.
    let w = plan(&next(&f, &asked, 13., open()));
    c.check("a maximise not seen: not marked done, not asked again at once", nothing(&w) && matches!(w.memory.maximised, Step::Asked { tries: 1, .. }), &w);
    let f2 = next(&f, &w, 16.5, open());
    let w = plan(&f2);
    c.check("…asked again after 5 s", w.show == Some(Show::Maximise) && matches!(w.memory.maximised, Step::Asked { tries: 2, .. }), &w);
    let w = plan(&next(&f2, &w, 22., open()));
    c.check("…then left as it is with a line, never done", nothing(&w) && w.memory.maximised == Step::LeftUp("not taken") && w.notes.len() == 1, &w);

    let w = plan(&Facts { seen: Seen { hidden: true, ..open() }, ..f });
    c.check("hidden: waits, not done", nothing(&w) && w.memory.maximised == Step::Never, &w);
    let w = plan(&Facts { seen: Seen { caption: false, ..open() }, ..f });
    c.check("full screen (no title bar): waits, not done", nothing(&w) && w.memory.maximised == Step::Never, &w);
    let w = plan(&Facts { person_active: true, ..f });
    c.check("behind what the person is doing: waits", nothing(&w) && w.memory.maximised == Step::Never, &w);
    let w = plan(&Facts { main: true, ..f });
    c.check("a main account's window isn't maximised", nothing(&w), &w);
    let w = plan(&Facts { opened_this_run: false, ..f });
    c.check("one opened before Bloxkeep started isn't", nothing(&w), &w);

    // Minimised already: it stays so and opens maximised.
    let fd = Facts { seen: down(Some(HOME)), ..f };
    let w = plan(&fd);
    c.check("minimised: stays so, asked to open maximised", w.show.is_none() && w.opens_maximised && w.memory.maximised_for == Stage::InGame, &w);
    let w = plan(&next(&fd, &w, 12., Seen { opens_maximised: true, ..down(None) }));
    c.check("…done once Windows keeps it", nothing(&w) && matches!(w.memory.maximised, Step::Done { .. }), &w);

    // With the join rule: not maximised in view; put away, then opens maximised.
    let fb = Facts { maximise_on_open: true, on_join_minimise: true, game: Game::InGame { at: t(10.) }, ..facts(25.) };
    let w = plan(&fb);
    c.check("with Minimise: not maximised in view while its game loads", nothing(&w), &w);
    let w = plan(&next(&fb, &w, 41., open()));
    c.check("…minimised once loaded, and it opens maximised", w.show == Some(Show::Minimise) && w.opens_maximised, &w);
}

fn rejoin(c: &mut Checks) {
    let f = Facts { rejoin: true, last: Some(Last { place: Some(TILE), minimised: false }), ..facts(1.) };
    let w = plan(&f);
    c.check("a rejoin's window waits to go back where its account's last one stood", nothing(&w) && w.memory.back.is_some_and(|b| b.place == TILE), &w);
    let w = plan(&next(&f, &w, 30., open()));
    c.check("…not moved while it joins", nothing(&w), &w);
    let fg = Facts { game: Game::InGame { at: t(40.) }, ..next(&f, &w, 40., open()) };
    let w = plan(&fg);
    c.check("…not the moment it gets in", nothing(&w), &w);
    let fp = next(&fg, &w, 43.5, open());
    let w = plan(&fp);
    c.check("…put back once settled", w.place == Some(TILE) && w.place_why == "rejoin" && w.memory.back.is_some_and(|b| b.put_at.is_some()), &w);
    let put = w.clone();
    let w = plan(&next(&fp, &put, 46., Seen { rect: Some(TILE), ..open() }));
    c.check("…looked at again only later", nothing(&w) && w.memory.back.is_some(), &w);
    let w = plan(&next(&fp, &put, 50., Seen { rect: Some(TILE), ..open() }));
    c.check("…done going back; Maximise leaves it there", nothing(&w) && w.memory.back.is_none() && w.memory.maximised.settled(), &w);
    let w = plan(&next(&fp, &put, 50., open()));
    c.check("…sized again after: put back once more", w.place == Some(TILE) && w.place_why == "rejoin-again" && w.memory.back.is_none(), &w);

    let w = plan(&next(&f, &plan(&f), 61.5, open()));
    c.check("no game after 60 s: put back all the same", w.place == Some(TILE) && w.place_why == "rejoin-late", &w);

    // Minimised by then: given the place to open at, done once Windows keeps it, not asked every tick.
    let fm = Facts { seen: down(Some(HOME)), ..next(&f, &plan(&f), 61.5, open()) };
    let w = plan(&fm);
    c.check("minimised when due: it opens at the place, stays minimised", w.opens_at == Some(TILE) && w.show.is_none() && w.place.is_none(), &w);
    let w = plan(&next(&fm, &w, 62.5, down(Some(TILE))));
    c.check("…done once Windows keeps that place: not asked again", nothing(&w) && w.memory.back.is_none(), &w);
    let w1 = plan(&fm);
    let w2 = plan(&next(&fm, &w1, 62.5, down(Some(HOME))));
    let w3 = plan(&next(&fm, &w2, 63.5, down(Some(HOME))));
    c.check("…not kept: asked twice, then left with a line", w2.opens_at.is_some() && nothing(&w3) && w3.memory.back.is_none() && w3.notes.len() == 1, (&w2, &w3));
    let w = plan(&Facts { seen: Seen { hidden: true, ..open() }, ..next(&f, &plan(&f), 61.5, open()) });
    c.check("hidden by a hidden rejoin: left as it is", nothing(&w) && w.memory.back.is_none() && w.notes.len() == 1, &w);

    // Its account's last window was minimised: minimised again once loaded, after going back.
    let fm = Facts { rejoin: true, last: Some(Last { place: None, minimised: true }), game: Game::InGame { at: t(10.) }, ..facts(11.) };
    let w = plan(&fm);
    c.check("last window minimised, no place: nothing to go back to", nothing(&w) && w.memory.back.is_none() && w.memory.rejoin_minimise, &w);
    let w = plan(&next(&fm, &w, 30., open()));
    c.check("…not minimised while its game loads", nothing(&w), &w);
    let in_use = Seen { front: true, ..open() };
    let wu = plan(&Facts { person_active: true, ..next(&fm, &w, 41., in_use) });
    c.check("…in use when due: not minimised, not given up", nothing(&wu) && wu.memory.put_away == Step::Never, &wu);
    let fa = next(&fm, &wu, 45., open());
    let w = plan(&fa);
    c.check("…minimised once the person leaves it", w.show == Some(Show::Minimise) && w.why == "rejoin", &w);
    let w = plan(&next(&fa, &w, 51., open()));
    c.check("…not seen minimised: asked again", w.show == Some(Show::Minimise) && matches!(w.memory.put_away, Step::Asked { tries: 2, .. }), &w);
    let fl = Facts { game: Game::Joining { at: t(1.) }, ..fm };
    let w = plan(&next(&fl, &plan(&fl), 61., open()));
    c.check("…no game after 60 s: minimised all the same", w.show == Some(Show::Minimise) && w.why == "rejoin-late", &w);
    let w = plan(&Facts { on_join_minimise: true, ..fm });
    c.check("…Minimise for every window: the join rule does it", !w.memory.rejoin_minimise, &w);
    let fb = Facts { last: Some(Last { place: Some(TILE), minimised: true }), ..fm };
    let w = plan(&next(&fb, &plan(&fb), 45., open()));
    c.check("…with a place: put back first, minimised after", w.place == Some(TILE) && w.show.is_none(), &w);
}

fn arrange(c: &mut Checks) {
    let mut f = facts(20.);
    f.on_join_minimise = true;
    f.game = Game::InGame { at: t(10.) };
    let mut m = plan(&f).memory;
    arranged(&mut m, true);
    let w = plan(&Facts { memory: m, ..next(&f, &plan(&f), 41., open()) });
    c.check("Arrange clicked while it loaded: the join rule doesn't close it after", nothing(&w) && w.memory.put_away == Step::LeftUp("arranged"), &w);
    let done = Memory { put_away: Step::Done { at: t(41.) }, ..m };
    let mut d = done;
    arranged(&mut d, true);
    let w = plan(&Facts { memory: d, ..next(&f, &plan(&f), 60., open()) });
    c.check("…one it opened stays open", nothing(&w) && matches!(w.memory.put_away, Step::Done { .. }), &w);
    let mut a = plan(&f).memory;
    a.back = Some(Back { place: TILE, since: t(0.), in_game_since: None, put_at: None, tries: 0 });
    arranged(&mut a, false);
    c.check("Arrange after a launch changes no rule; a rejoin's window going back stays in its tile", a.put_away == Step::Never && a.back.is_none(), &a);

    // A minimise asked just before an Arrange, landing while it runs.
    let asked = plan(&next(&f, &plan(&f), 41., open()));
    let mut m = asked.memory;
    arranged(&mut m, false);
    let w = plan(&Facts { memory: m, ..next(&f, &asked, 43., down(Some(TILE))) });
    c.check("a minimise that lands during an Arrange: stays down, done, nothing asked to open it", nothing(&w) && matches!(w.memory.put_away, Step::Done { .. }), &w);
}

fn tile(c: &mut Checks) {
    let tiled = Memory { tile: Some(TILE), ..Memory::new(t(0.)) };
    let f = Facts { memory: tiled, ..facts(20.) };
    let w = plan(&f);
    c.check(
        "a tile given, the window not in it: placed there",
        w.place == Some(TILE) && w.place_why == "tile" && w.show.is_none() && matches!(w.memory.tile_given, Step::Asked { tries: 1, .. }),
        &w,
    );
    // Over its tile by Windows' unseen borders, as Arrange lays a window.
    let there = Seen { rect: Some(Rect::new(TILE.x - 9, TILE.y, TILE.width + 18, TILE.height + 9)), ..open() };
    let w2 = plan(&next(&f, &w, 21., there));
    c.check("…seen there, give or take its unseen borders: done, nothing asked", nothing(&w2) && matches!(w2.memory.tile_given, Step::Done { .. }), &w2);
    let w3 = plan(&next(&f, &w2, 60., open()));
    c.check("…moved by the person after: left where it is, not given again every tick", nothing(&w3), &w3);
    let w4 = plan(&next(&f, &w, 22., open()));
    c.check("…not seen there yet: not asked again at once", nothing(&w4) && matches!(w4.memory.tile_given, Step::Asked { tries: 1, .. }), &w4);
    let f5 = next(&f, &w4, 26., open());
    let w5 = plan(&f5);
    c.check("…asked again after 5 s", w5.place == Some(TILE) && matches!(w5.memory.tile_given, Step::Asked { tries: 2, .. }), &w5);
    let w6 = plan(&next(&f5, &w5, 32., open()));
    c.check("…then left where it is, said in the log", nothing(&w6) && w6.memory.tile_given == Step::LeftUp("not taken") && w6.notes.len() == 1, &w6);

    // Minimised: its tile is where it opens; it isn't opened for it.
    let fd = Facts { seen: down(Some(HOME)), ..f };
    let w = plan(&fd);
    c.check("minimised with a tile: the tile is where it opens, nothing opens it", w.opens_at == Some(TILE) && w.place.is_none() && w.show.is_none() && w.place_why == "tile", &w);
    let w = plan(&next(&fd, &w, 21., down(Some(TILE))));
    c.check("…done once Windows keeps that place", nothing(&w) && matches!(w.memory.tile_given, Step::Done { .. }), &w);
    let fj = Facts { on_join_minimise: true, game: Game::InGame { at: t(0.) }, memory: tiled, ..facts(41.) };
    let w = plan(&fj);
    c.check(
        "put away by the join rule this tick: minimised, and its tile is where it opens",
        w.show == Some(Show::Minimise) && w.opens_at == Some(TILE) && w.place.is_none(),
        &w,
    );

    for (what, g) in [
        ("hidden by Bloxkeep", Facts { seen: Seen { hidden: true, ..open() }, ..f }),
        ("full screen (no title bar)", Facts { seen: Seen { caption: false, ..open() }, ..f }),
        ("lent to a borrower", Facts { lent: true, ..f }),
        ("made big for a live view", Facts { sharp: true, ..f }),
    ] {
        let w = plan(&g);
        c.check(&format!("a tile not taken yet, {what}: waits, not marked done"), nothing(&w) && w.memory.tile_given == Step::Never, &w);
    }
    let w = plan(&Facts { maximise_on_open: true, ..next(&f, &plan(&f), 21., there) });
    c.check("Maximise leaves a window in its tile", w.show.is_none(), &w);
    let back = Memory { back: Some(Back { place: HOME, since: t(0.), in_game_since: None, put_at: None, tries: 0 }), ..tiled };
    let w = plan(&Facts { memory: back, ..f });
    c.check("a rejoin's window still going back isn't given an old tile", w.place.is_none() && w.memory.tile_given == Step::Never, &w);

    let mut m = Memory::new(t(0.));
    given_tile(&mut m, Some(TILE), true, t(5.));
    let seen = matches!(m.tile_given, Step::Done { .. });
    given_tile(&mut m, Some(TILE), false, t(6.));
    let missed = m.tile_given == Step::Never && plan(&Facts { memory: m, ..facts(7.) }).place == Some(TILE);
    given_tile(&mut m, None, true, t(8.));
    let none = m.tile.is_none() && m.tile_given == Step::Never;
    c.check(
        "an Arrange's tile: seen there is done; missed or lent, it is asked on the next tick; none given forgets the last",
        seen && missed && none,
        (seen, missed, none),
    );
    c.check(
        "in a tile: its unseen borders (24 px) allowed, no more",
        in_tile(Rect::new(TILE.x - 24, TILE.y, TILE.width + 48, TILE.height), TILE) && !in_tile(Rect::new(TILE.x + 30, TILE.y, TILE.width, TILE.height), TILE),
        (),
    );
}

fn left_alone(c: &mut Checks) {
    let mut f = facts(41.);
    f.on_join_minimise = true;
    f.maximise_on_open = true;
    f.game = Game::InGame { at: t(10.) };
    for (what, g) in [
        ("lent to a borrower", Facts { lent: true, ..f }),
        ("not responding", Facts { seen: Seen { hung: true, ..open() }, ..f }),
        ("gone", Facts { seen: Seen::default(), ..f }),
    ] {
        let w = plan(&g);
        c.check(&format!("{what}: nothing asked, nothing remembered"), nothing(&w) && w.memory == g.memory, &w);
    }
    // Nothing in the plan ever opens a minimised window.
    let mut opened = false;
    for game in [Game::Unknown, Game::Joining { at: t(0.) }, Game::InGame { at: t(0.) }, Game::Out] {
        for now in [5., 20., 41., 80., 200.] {
            for rejoin in [false, true] {
                let g = Facts {
                    now: t(now),
                    game,
                    rejoin,
                    last: Some(Last { place: Some(TILE), minimised: true }),
                    seen: down(Some(HOME)),
                    memory: Memory { tile: Some(TILE), ..f.memory },
                    ..f
                };
                let w = plan(&g);
                opened |= w.place.is_some() || w.show == Some(Show::Maximise);
            }
        }
    }
    c.check("no rule asks a minimised window to open", !opened, ());
}
