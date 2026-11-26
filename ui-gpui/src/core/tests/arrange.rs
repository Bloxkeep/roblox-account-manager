// Arrange windows (roblox_processes, carried out by the window desk): the math of it (a window coming back from
// maximised, a Roblox window leaving full screen, where windows too big for their tiles go, a grid's cells, a size of
// one's own, an edge dragged, the layouts taken), then real windows of the test's own standing in for Roblox's, tiled,
// spread and stacked among other apps' windows by the desk's Arrange, on a desktop of their own that is never shown
// (hidden_desktop.rs).
use std::rc::Rc;

use serde_json::json;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, WS_OVERLAPPEDWINDOW};

use super::{
    call,
    hidden_desktop::{self as hd, Kind, Rec, Windows, make_windows, near, put, rank, rect_of, set_kind, settle, stacking, top},
    rejoin::{no_network, quiet_app, seed},
    unlock,
    window::quiet_app as app,
    Checks, PASSWORD,
};
use crate::core::{
    app::{App, drag::keep_dragged_size},
    base::Stamp,
    roblox_processes::{self as rp, after_drag, grid_cells, is_layout, overlap_spots, restored_place, tiles},
    vault::Vault,
    win_kit::{self, Point, Rect, Size},
    window_desk::{Arranged, Arrangement, Lift},
    window_size_keeper::WindowSizeKeeper,
};

pub async fn run(c: &mut Checks, dir: &str) {
    if hd::in_view() {
        math(c).await;
        hd::run_there(c, dir, "arrange", 180_000);
        return;
    }
    let mut r = Rec::new(c);
    windows(&mut r).await;
    r.save(dir);
}

async fn math(c: &mut Checks) {
    // Bloxkeep's own window coming back from maximised: as measured on a real PC.
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

    // An Arrange's tiles, as the window desk gives them.
    let area = Rect::new(0, 0, 2400, 1200);
    let grid = tiles(2, area, "3x2", none);
    c.check(
        "a 3 × 2 grid with 2 windows: its first two cells, not half the area each",
        grid.side_by_side && grid.every == [Rect::new(0, 0, 800, 600), Rect::new(800, 0, 800, 600)],
        &grid,
    );
    let grid = tiles(9, area, "2x2", none);
    c.check(
        "…more windows than cells: their cell's size, spread evenly over the area",
        !grid.side_by_side && grid.every.len() == 9 && grid.every.iter().all(|r| r.size() == Size::new(1200, 600) && area.contains_rect(*r)),
        &grid,
    );
    let grid = tiles(3, area, "custom", Size::new(200, 150));
    c.check(
        "a size of one's own (200 × 150): side by side from the top-left",
        grid.every == [Rect::new(0, 0, 200, 150), Rect::new(200, 0, 200, 150), Rect::new(400, 0, 200, 150)],
        &grid,
    );
}

// ───────── real windows, on a desktop of their own ─────────

/// Five stand-ins of `kind`, each a little below and right of the one before, over the area's top-left.
fn open(kind: Kind, area: Rect) -> Windows {
    let ws = make_windows(5, WS_OVERLAPPEDWINDOW);
    for (i, &w) in ws.all.iter().enumerate() {
        set_kind(w, kind);
        put(w, Rect::new(area.x + 40 * i as i32, area.y + 30 * i as i32, 450, 330));
    }
    settle();
    ws
}

/// An Arrange of `windows` in this order on `area`, carried out by the window desk (made-up process ids).
async fn lay(app: &Rc<App>, windows: &[isize], area: Rect, lift: Lift, clicked: bool) -> Arranged {
    let slots: Vec<(i32, isize)> = windows.iter().enumerate().map(|(i, &w)| (960_001 + i as i32, w)).collect();
    let a = Arrangement { windows: slots.clone(), slots, area, layout: "fill".into(), custom: Size::new(0, 0), open: Vec::new(), clicked, lift };
    let laid = app.desk_arrange(move |_| a).await.ok().expect("the input turn");
    settle();
    laid
}

/// What shows of a window: its frame without Windows' unseen borders, where Windows can say it.
fn seen(w: isize) -> Rect {
    rp::seen_frame(w).unwrap_or_else(|| rect_of(w))
}

async fn windows(r: &mut Rec<'_>) {
    no_network();
    seed(Vec::new());
    let (app, _said) = quiet_app();
    unlock(&app).await;
    let screens = win_kit::all_screens();
    let Some(screen) = screens.iter().find(|s| s.primary).or(screens.first()).cloned() else {
        r.skip("windows with a least size bigger than their tiles, as Roblox's: given their tiles all the same, side by side", "no screen");
        return;
    };
    let work = screen.working_area;
    let area = Rect::new(work.x + 20, work.y + 20, 900, 700);
    let fill = tiles(5, area, "fill", Size::new(0, 0)).every;
    let least = Size::new(400, 300);

    // Tiles of 300 × 350 for 3 and 2 in 900 × 700, 100 narrower than these windows' least size, as Roblox's window has
    // one: given all the same, side by side.
    {
        let ws = open(Kind { least: Some(least), ..Kind::default() }, area);
        let laid = lay(&app, &ws.all, area, Lift::No, false).await;
        let got: Vec<Rect> = ws.all.iter().map(|&w| seen(w)).collect();
        r.check(
            "windows with a least size bigger than their tiles, as Roblox's: given their tiles all the same, side by side",
            laid.done == 5 && fill.iter().all(|t| t.width < least.width) && got.iter().zip(&fill).all(|(g, t)| near(*g, *t, 1)),
            (&got, &fill, &laid.note),
        );
    }

    // Windows that grow back to their least size whatever they are given: 400 × 350 each (their least width, their
    // tile's height), laid over each other evenly over the area.
    {
        let ws = open(Kind { least: Some(least), grows_back: true, ..Kind::default() }, area);
        let laid = lay(&app, &ws.all, area, Lift::No, false).await;
        let size = Size::new(least.width, fill[0].height);
        let spread: Vec<Rect> = overlap_spots(5, area, size).iter().map(|&p| Rect::at(p, size)).collect();
        let got: Vec<Rect> = ws.all.iter().map(|&w| rect_of(w)).collect();
        r.check(
            "windows that grow back to their least size: spread evenly over the area, none past its edge",
            got == spread && got.iter().all(|g| area.contains_rect(*g)),
            (&got, &spread, &laid.note),
        );
        let order = stacking();
        let ranks: Vec<Option<usize>> = ws.all.iter().map(|&w| rank(&order, w)).collect();
        r.check(
            "…each over the one before it, so every title bar shows",
            ranks.iter().all(Option::is_some) && ranks.windows(2).all(|p| p[0] > p[1]),
            &ranks,
        );
    }

    // Windows moved onto a screen at another scale take Windows' size for it as they get there: placed first, then
    // sized, they keep their tiles; one whose answer comes after its size is given it again.
    for later in [false, true] {
        let ws = open(Kind::default(), area);
        // Not on its tile already: the first window's move into it is a move too.
        put(ws.all[0], Rect::new(area.x + 25, area.y + 25, 450, 330));
        settle();
        for &w in &ws.all {
            set_kind(w, Kind { shrinks_on_move: Some(later), ..Kind::default() });
        }
        let laid = lay(&app, &ws.all, area, Lift::No, false).await;
        let got: Vec<Rect> = ws.all.iter().map(|&w| seen(w)).collect();
        r.check(
            if later { "…its answer coming after the size: given its tile again" } else { "windows that take a new scale's size as they move: their tiles all the same" },
            got.iter().zip(&fill).all(|(g, t)| near(*g, *t, 1)),
            (&got, &fill, &laid.note),
        );
    }

    // A window that keeps a least size (as Roblox's), its right edge dragged 150 in: given that size.
    {
        let ws = make_windows(1, WS_OVERLAPPEDWINDOW);
        let w = ws.all[0];
        set_kind(w, Kind { least: Some(least), ..Kind::default() });
        put(w, Rect::new(area.x, area.y, least.width, least.height));
        settle();
        let start = rect_of(w);
        keep_dragged_size(w, start, Point::new(start.right() - 2, start.y + 150), Point::new(start.right() - 152, start.y + 150));
        settle();
        let meant = Rect::new(start.x, start.y, start.width - 150, start.height);
        r.check(
            "a window with a least size, its edge dragged in past it: the size the mouse asked, its place kept",
            start.size() == least && rect_of(w) == meant,
            (start, rect_of(w), meant),
        );
    }

    // Another app's window over them, Bloxkeep's over that.
    {
        let ws = open(Kind::default(), area);
        let two = make_windows(2, WS_OVERLAPPEDWINDOW);
        let (other, ours) = (two.all[0], two.all[1]);
        hd::set_title(other, "Arrange check other app");
        hd::set_title(ours, "Arrange check Bloxkeep");
        let over = || {
            for w in [other, ours] {
                put(w, Rect::new(area.x + 300, area.y + 200, 400, 300));
                top(w);
            }
            settle();
        };
        let ranks = |order: &[isize]| -> Vec<Option<usize>> { ws.all.iter().map(|&w| rank(order, w)).collect() };
        let under = |ranks: &[Option<usize>], w: isize, order: &[isize]| rank(order, w).is_some_and(|o| ranks.iter().all(|x| x.is_some_and(|x| x < o)));

        // Clicked with Bloxkeep out of view (no lift): the PC's order stays, the other app's window over them.
        over();
        let _ = lay(&app, &ws.all, area, Lift::No, true).await;
        let order = stacking();
        let now = ranks(&order);
        let other_at = rank(&order, other);
        r.check(
            "an Arrange clicked with Bloxkeep out of view: another app's window stays over them",
            other_at.is_some_and(|o| now.iter().all(|x| x.is_some_and(|x| x > o))),
            (&now, other_at),
        );

        // After a launch: over every other app, each over the one before it, the keyboard left where it was.
        over();
        // SAFETY: plain query.
        let front_before = unsafe { GetForegroundWindow() } as isize;
        let _ = lay(&app, &ws.all, area, Lift::Quietly, false).await;
        // SAFETY: plain query.
        let front_after = unsafe { GetForegroundWindow() } as isize;
        let order = stacking();
        let now = ranks(&order);
        r.check(
            "tiled after a launch: over every other app, each over the one before it, the keyboard left where it was",
            under(&now, other, &order) && under(&now, ours, &order) && now.windows(2).all(|p| p[0] > p[1]) && front_before == front_after,
            (&now, rank(&order, other), rank(&order, ours), front_before, front_after),
        );

        // Clicked in Bloxkeep: over every app, each over the one before it, Bloxkeep's window under them.
        over();
        let _ = lay(&app, &ws.all, area, Lift::Keyboard, true).await;
        let order = stacking();
        let now = ranks(&order);
        r.check(
            "clicked in Bloxkeep: over every other app, each over the one before it, Bloxkeep's window under them",
            under(&now, other, &order) && under(&now, ours, &order) && now.windows(2).all(|p| p[0] > p[1]),
            (&now, rank(&order, other), rank(&order, ours)),
        );
    }
}
