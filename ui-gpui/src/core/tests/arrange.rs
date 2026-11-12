// Tests/Bloxkeep.Tests/ArrangeSuite.cs: Arrange windows (roblox_processes): the math of it is checked here (a window coming
// back from maximised, a Roblox window leaving full screen, where windows too big for their tiles go, a grid's cells, a
// size of one's own, an edge dragged, the layouts taken). The C#'s checks on real windows of its own (tiled, spread, their
// order among other apps' windows) are skipped: they would make and move windows.
use serde_json::json;

use super::{call, unlock, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    base::Stamp,
    roblox_processes::{after_drag, grid_cells, is_layout, overlap_spots, restored_place},
    vault::Vault,
    win_kit::{Point, Rect, Size},
    window_size_keeper::WindowSizeKeeper,
};

pub async fn run(c: &mut Checks, _dir: &str) {
    // Bloxkeep's own window coming back from maximised: as measured on the owner's PC, 2026-11-01.
    let (work, small, snapped) = (Rect::new(2560, 0, 2560, 1380), Rect::new(2560, 20, 1857, 1078), Rect::new(2551, -9, 1298, 1398));
    let wanted = Size::new(1650, 1050);
    c.check("restored from maximised: a size that fits stays", restored_place(small, work, 9, wanted) == small, restored_place(small, work, 9, wanted));
    c.check("…a window snapped to an edge stays", restored_place(snapped, work, 9, wanted) == snapped, restored_place(snapped, work, 9, wanted));
    let too_big = restored_place(Rect::new(2560, 20, 2560, 1398), work, 9, wanted);
    c.check(
        "…one too tall and running past the screen comes back smaller, inside it",
        too_big == Rect::new(3015, 165, 1650, 1050),
        too_big,
    );

    // A Roblox window leaving full screen.
    let mut sizes = WindowSizeKeeper::new();
    let (mine, monitor, roblox) = (Rect::new(100, 100, 700, 500), Rect::new(0, 0, 2560, 1440), Rect::new(200, 200, 1224, 957));
    let clock = Stamp::utc_now();
    let at = |s: f64| clock.plus_secs(s);
    c.check("a window's size is noted, nothing moved", sizes.look(1, mine, false, true, clock).is_none(), ());
    c.check("…going full screen moves nothing", sizes.look(1, monitor, true, false, at(2.0)).is_none(), ());
    c.check("…back at Roblox's default: back to its own size", sizes.look(1, roblox, false, true, at(4.0)) == Some(mine), ());
    c.check("…Roblox sizing it again as it settles: again", sizes.look(1, roblox, false, true, at(6.0)) == Some(mine), ());
    c.check("…once there, nothing more", sizes.look(1, mine, false, true, at(7.0)).is_none(), ());
    let dragged = Rect::new(100, 100, 900, 600);
    let a = sizes.look(1, dragged, false, true, at(20.0)).is_none();
    let b = sizes.look(1, monitor, true, false, at(21.0)).is_none();
    let d = sizes.look(1, roblox, false, true, at(22.0)) == Some(dragged);
    c.check("…a size dragged later is the person's", a && b && d, (a, b, d));
    let mut never_seen = WindowSizeKeeper::new();
    let a = never_seen.look(2, monitor, true, false, clock).is_none();
    let b = never_seen.look(2, roblox, false, true, at(1.0)).is_none();
    c.check("a window opened full screen has no size to go back to", a && b, (a, b));
    sizes.look(1, monitor, true, false, at(40.0));
    sizes.clear();
    c.check("arranged meanwhile: the tile stays", sizes.look(1, roblox, false, true, at(41.0)).is_none(), ());

    // Picked in the page: a layout the app knows is kept, anything else left as it was; the size held to what it takes.
    Vault::create(PASSWORD, None).expect("vault");
    {
        let (app, _said) = app();
        unlock(&app).await;
        let _ = call(&app, "setSettings", json!({ "windowLayout": "4x4" })).await;
        let _ = call(&app, "setSettings", json!({ "windowLayout": "9x9" })).await;
        let layout = app.vault(|d| d.settings.window_layout.clone()).unwrap_or_default();
        c.check("a grid picked is kept; one the app doesn't know is left out", layout == "4x4", &layout);
        let _ = call(&app, "setSettings", json!({ "windowLayout": "custom", "windowWidth": 50, "windowHeight": 99999 })).await;
        let (layout, w, h) = app.vault(|d| (d.settings.window_layout.clone(), d.settings.window_width, d.settings.window_height)).unwrap_or_default();
        c.check(
            "a size of one's own, held to 160 × 120 at least and 7680 × 4320 at most",
            layout == "custom" && w == 160 && h == 4320,
            format!("{layout} {w} × {h}"),
        );
    }

    // Where 5 Roblox windows (1224 × 957 pixels each) go on that screen's 2560 × 1528 work area.
    let spots = overlap_spots(5, Rect::new(0, 0, 2560, 1528), Size::new(1224, 957));
    c.check(
        "5 windows too big for their tiles: 3 and 2, spread evenly from corner to corner",
        spots == [Point::new(0, 0), Point::new(668, 0), Point::new(1336, 0), Point::new(0, 571), Point::new(668, 571)],
        &spots,
    );
    c.check("…every one of them on the screen", spots.iter().all(|p| p.x + 1224 <= 2560 && p.y + 957 <= 1528), ());
    let two = overlap_spots(2, Rect::new(100, 0, 2560, 1528), Size::new(1224, 957));
    c.check("2 that fit side by side: side by side, not spread apart", two == [Point::new(100, 0), Point::new(1324, 0)], &two);

    // The layouts: a grid's cells for 2 windows as for 16; a size of one's own, as many across as fit, held to the screen.
    let screen = Size::new(2560, 1528);
    let none = Size::new(0, 0);
    c.check("a 4 × 4 grid: 2 windows get a sixteenth of the screen each, not half", grid_cells(2, screen, "4x4", none) == (4, 640, 382), grid_cells(2, screen, "4x4", none));
    c.check(
        "a size of one's own: 640 × 480, 4 across",
        grid_cells(2, screen, "custom", Size::new(640, 480)) == (4, 640, 480),
        grid_cells(2, screen, "custom", Size::new(640, 480)),
    );
    c.check(
        "…no smaller than 160 × 120 and no larger than the screen",
        grid_cells(1, screen, "custom", Size::new(20, 10)) == (16, 160, 120) && grid_cells(1, screen, "custom", Size::new(9000, 9000)) == (1, 2560, 1528),
        (grid_cells(1, screen, "custom", Size::new(20, 10)), grid_cells(1, screen, "custom", Size::new(9000, 9000))),
    );
    // An edge dragged by the person, Roblox keeping 1224 × 957 (its least size on a 150 % screen): the size the mouse asked.
    let from = Rect::new(100, 100, 1224, 957);
    let right = after_drag(from, from, Point::new(1322, 500), Point::new(822, 500), 12);
    c.check(
        "the right edge dragged 500 in, Roblox keeping its least size: 724 wide, the left edge where it was",
        right == Some(Rect::new(100, 100, 724, 957)),
        right,
    );
    let corner = after_drag(from, from, Point::new(101, 101), Point::new(801, 601), 12);
    c.check("…the top-left corner dragged in: the bottom-right corner stays", corner == Some(Rect::from_ltrb(800, 600, 1324, 1057)), corner);
    let tiny = after_drag(from, from, Point::new(1322, 1055), Point::new(0, 0), 12);
    c.check("…no smaller than 160 × 120", tiny == Some(Rect::new(100, 100, 160, 120)), tiny);
    c.check(
        "…made bigger, as Windows did it: left alone",
        after_drag(from, Rect::new(100, 100, 1424, 957), Point::new(1322, 500), Point::new(1522, 500), 12).is_none(),
        (),
    );
    c.check(
        "…moved by its title bar: left to Windows",
        after_drag(from, Rect::new(300, 300, 1300, 1000), Point::new(600, 120), Point::new(800, 320), 12).is_none(),
        (),
    );

    c.check(
        "layouts the app takes: fill, the grids up to 8 × 8, custom and the old fixed tiles; nothing else",
        ["fill", "2x1", "4x4", "8x8", "custom", "small", "medium", "large"].iter().all(|l| is_layout(l))
            && !["", "9x9", "0x2", "2x0", "4x", "x4", "44", "4x4x", "huge"].iter().any(|l| is_layout(l)),
        (),
    );

    // Real windows of the test's own, tiled and spread: none are made here.
    for what in [
        "windows with a least size bigger than their tiles, as Roblox's: given their tiles all the same, side by side",
        "windows that grow back to their least size: spread evenly over the area, none past its edge",
        "…each over the one before it, so every title bar shows",
        "windows that take a new scale's size as they move: their tiles all the same",
        "…its answer coming after the size: given its tile again",
        "a 3 × 2 grid with 2 windows: its first two cells, not half the area each",
        "…more windows than cells: their cell's size, spread evenly over the area",
        "a size of one's own (200 × 150): side by side from the top-left",
        "a window with a least size, its edge dragged in past it: the size the mouse asked, its place kept",
        "tiled after a launch: another app's window stays over them",
        "clicked in Bloxkeep: over every other app, each over the one before it, Bloxkeep's window under them",
    ] {
        c.skip(what, "needs a window");
    }
}
