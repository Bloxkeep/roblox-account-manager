// Tests/Bloxkeep.Tests/WindowSuite.cs: the macOS themes' window buttons: the page's "window" command reaches the window
// with the right action, nothing else gets through; and a saved window place brought back inside the screen.
use std::{cell::RefCell, rc::Rc};

use serde_json::json;

use super::{app, call, user_error, Checks};
use crate::core::{
    ui,
    win_kit::{Point, Rect},
};

pub async fn run(c: &mut Checks, _dir: &str) {
    let (app, _said) = quiet_app();
    let got: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let keep = got.clone();
    *app.relaunch.window_control.borrow_mut() = Some(Rc::new(move |a: &str| keep.borrow_mut().push(a.to_string())));
    for action in ["close", "minimize", "zoom"] {
        let _ = call(&app, "window", json!({ "action": action })).await;
    }
    let joined = got.borrow().join(",");
    c.check("close, minimize and zoom reach the window", joined == "close,minimize,zoom", &joined);
    let refused = user_error(&call(&app, "window", json!({ "action": "quit" })).await).is_some();
    let joined = got.borrow().join(",");
    c.check("an unknown action is refused", refused && got.borrow().len() == 3, &joined);

    // A saved place brought back inside the screen: a 1920 × 1032 work area, 8 px of Windows' resize border allowed.
    let inside = |r: Rect| ui::inside(r, Rect::new(0, 0, 1920, 1032), 8);
    c.check(
        "saved at x = 1000, held to 1920 wide: moved back onto the screen",
        inside(Rect::new(1000, 100, 1920, 900)) == Point::new(8, 100),
        inside(Rect::new(1000, 100, 1920, 900)),
    );
    c.check(
        "…saved low on a taller screen: moved up to show its bottom",
        inside(Rect::new(1500, 600, 380, 700)) == Point::new(1500, 340),
        inside(Rect::new(1500, 600, 380, 700)),
    );
    c.check(
        "…snapped to an edge (its invisible border past it) or well inside: left where it was",
        inside(Rect::new(-7, 0, 974, 1039)) == Point::new(-7, 0) && inside(Rect::new(200, 150, 1320, 840)) == Point::new(200, 150),
        (),
    );
}

/// The harness's App, its process check held off for a day (shared by suite C's files). The C# suites ran without a
/// message loop, so the WinForms timer behind OnProcTimer never ticked there; here it would look at the PC's real Roblox
/// windows every 1.5 s and act on them with the suite's settings (cores, RAM limits). Must be called with no await
/// between it and `app()`: the timer's first wait reads the interval when it starts.
pub fn quiet_app() -> (std::rc::Rc<crate::core::app::App>, super::Said) {
    let (app, said) = app();
    app.controller.proc_interval_ms.set(86_400_000);
    (app, said)
}
