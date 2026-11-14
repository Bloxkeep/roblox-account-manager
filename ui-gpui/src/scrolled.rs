//! A page body that scrolls with its scroll bar beside it, always there while the content is taller than the window,
//! so a row cut at the foot reads as "more below" (the shots had lists, cards and pictures cut at the bottom edge with
//! nothing saying the page goes on).
use std::{cell::RefCell, collections::HashMap};

use gpui_kit::*;

thread_local! {
    /// One scroll handle per body, by its id: the ids are fixed names in the code, so the map stays small.
    static HANDLES: RefCell<HashMap<&'static str, ScrollHandle>> = RefCell::new(HashMap::new());
}

/// The scroll handle kept for a body's id (made the first time it is asked for).
pub fn handle(id: &'static str) -> ScrollHandle {
    HANDLES.with(|h| h.borrow_mut().entry(id).or_insert_with(ScrollHandle::new).clone())
}

/// `body` in a box that fills what is left of a column (or all of its parent), scrolling up and down, its bar on the
/// right edge.
pub fn body(id: &'static str, body: impl IntoElement) -> Div {
    let h = handle(id);
    // BLOXKEEP_SHOT_BODY=<id> with BLOXKEEP_SHOT_SCROLL=<px>: a hidden shot of that body scrolled down that far.
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_BODY").is_ok_and(|b| b == id)
        && let Ok(y) = std::env::var("BLOXKEEP_SHOT_SCROLL").map(|v| v.parse::<f32>().unwrap_or(0.))
        && (h.offset().y + px(y)).abs() > px(1.)
    {
        h.set_offset(point(px(0.), px(-y)));
    }
    with_bar(id, &h, div().id(id).size_full().overflow_y_scroll().track_scroll(&h).child(body))
}

/// A scrolling element that already has its own handle, put in a box with the bar beside it.
pub fn with_bar<H: gpui_kit::component::scroll::ScrollbarHandle + Clone>(id: &'static str, h: &H, scroller: impl IntoElement) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .size_full()
        .relative()
        // The body runs to the panel's foot and fades out over its last 28 px: a row cut flush against the rounded
        // edge looked like a scratch (the owner, Profile page, 2026-11-12), and stopping 14 px short left an open strip
        // under a hard cut that looked cut off too (the Roblox tab, 2026-11-15). A list's own bottom padding is under the
        // fade at its end, so nothing of it is hidden there.
        .child(scroller)
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(28.))
                .bg(linear_gradient(180., linear_color_stop(crate::theme::ca(crate::theme::PANEL, 0.), 0.), linear_color_stop(crate::theme::c(crate::theme::PANEL), 1.))),
        )
        .child(crate::pages::auto::scroll_bar_named(SharedString::from(format!("{id}-bar")), h))
}
