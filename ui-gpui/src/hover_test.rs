//! An element with a hover style is redrawn when the mouse comes onto it and when it leaves, with an id or without one,
//! and only then. Without an id GPUI had no element state to keep its hover in, so the mouse crossing it asked for no
//! frame: its hover showed only when something else redrew the window (Statistics › Activity's lines waited ~1.5 s
//! for the status bar's figures, 2026-11-24; the rule is in vendor/gpui-pre/src/elements/div.rs, paint).
//!
//! (What a hover style changes when drawn was never wrong: a painted look, as the app's hovers all are (a ground, a
//! border's colour), takes the hover the mouse has at the time. A hover that changes the layout (a width, a padding)
//! needs an id: without one the layout is made before the mouse is known. No hover in the app does.)
//!
//! `cargo test --release --features shot hover_styles` (RUST_MIN_STACK=268435456: the test build of the whole app
//! overflows rustc's own stack otherwise).

use std::cell::Cell;
use std::rc::Rc;

use ::gpui::TestAppContext;
use gpui_kit::{prelude::*, *};

struct Boxes {
    renders: Rc<Cell<usize>>,
}

impl Render for Boxes {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let hover = |s: StyleRefinement| s.bg(rgb(0xffffff));
        div()
            .size_full()
            .child(div().absolute().left(px(0.)).top(px(0.)).w(px(10.)).h(px(20.)).hover(hover))
            .child(div().id("with-id").absolute().left(px(40.)).top(px(0.)).w(px(10.)).h(px(20.)).hover(hover))
    }
}

#[::core::prelude::v1::test] // (gpui_kit::* brings GPUI's own `test` macro, which shadows the built-in one.)
fn hover_styles_redraw_on_coming_and_leaving_with_or_without_an_id() {
    let mut cx = TestAppContext::single();
    let renders = Rc::new(Cell::new(0));
    let window: AnyWindowHandle = cx
        .add_window({
            let renders = renders.clone();
            move |_, _| Boxes { renders }
        })
        .into();
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    let mouse = |cx: &mut TestAppContext, x: f32, y: f32| {
        cx.update_window(window, |_, w, cx| w.simulate_mouse_move(point(px(x), px(y)), cx)).unwrap();
        cx.run_until_parked();
    };
    mouse(&mut cx, 100., 100.);

    for (which, x) in [("no id", 5.), ("an id", 45.)] {
        let before = renders.get();
        mouse(&mut cx, x, 5.);
        assert_eq!(renders.get(), before + 1, "{which}: drawn when the mouse comes onto it");
        mouse(&mut cx, x, 15.);
        assert_eq!(renders.get(), before + 1, "{which}: not drawn again while the mouse stays on it");
        mouse(&mut cx, 100., 100.);
        assert_eq!(renders.get(), before + 2, "{which}: drawn when the mouse leaves it");
        mouse(&mut cx, 120., 110.);
        assert_eq!(renders.get(), before + 2, "{which}: not drawn for a move away from it");
    }
}
