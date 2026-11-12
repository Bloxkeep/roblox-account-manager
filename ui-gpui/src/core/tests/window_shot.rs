// Tests/Bloxkeep.Tests/WindowShotSuite.cs: /pc screenshot's picture of a window (window_shot.rs). The C# pictured a red
// window of its own in view, minimized and hidden: those need a window and are skipped here. What runs: a picture all
// white is taken for nothing drawn yet (one like that red window, with its blue spot, isn't), and no window gives no
// picture.
use super::Checks;
use crate::core::{win_kit::Picture, window_shot};

pub async fn run(c: &mut Checks, _dir: &str) {
    for what in [
        "a window in view: pictured whole, its game only (no title bar)",
        "a minimized window: pictured whole all the same",
        "…minimized again after, with its own look back (not unseen, not click-through)",
        "a window Bloxkeep hid: pictured whole",
        "…hidden again after, with its own look back",
    ] {
        c.skip(what, "needs a window");
    }

    let mut white = Picture::new(400, 300);
    white.pixels.iter_mut().for_each(|p| *p = 0xFFFF_FFFF);
    let white_blank = window_shot::blank(&white);
    // PORT: the C# used its picture of the red window; one drawn the same way stands in (red, a blue spot at 20,40).
    let mut drawn = Picture::new(400, 300);
    for y in 0..300 {
        for x in 0..400 {
            drawn.pixels[(y * 400 + x) as usize] = if (20..80).contains(&x) && (40..100).contains(&y) { 0xFF00_00FF } else { 0xFFFF_0000 };
        }
    }
    let drawn_blank = window_shot::blank(&drawn);
    c.check(
        "a picture all white is taken for nothing drawn yet, one with something on it isn't",
        white_blank && !drawn_blank,
        format!("white {white_blank}, drawn {drawn_blank}"),
    );

    c.check("no window: no picture", window_shot::capture(0, 1920).is_none(), ());
}
