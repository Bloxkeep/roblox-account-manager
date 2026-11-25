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

/// How far whatever scrolls up and down fades out at an edge it goes on past: its top once scrolled down (a page's,
/// under the bar floating over it), its foot while more lies below (over the status bar). Apple's soft scroll edge
/// (Human Interface Guidelines, Scroll views). The rule for every scrolling box in the app, a page's body or a short
/// list in a card or a menu alike (`set_scroll_fade`, at the start): cut flush, a tab's ring was halved under the
/// Roblox tab's capsule and a row of game names sliced over the status bar. A colour laid over the foot only hid the
/// cut on a plain panel, never on a theme's picture or glass.
pub const EDGE: f32 = 28.;

/// A scrolling element that already has its own handle, put in a box with the bar beside it.
pub fn with_bar<H: gpui_kit::component::scroll::ScrollbarHandle + Clone>(id: &'static str, h: &H, scroller: impl IntoElement) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .size_full()
        .relative()
        // (The scroller fades at its own edges, as every scrolling box does: EDGE.)
        .child(scroller)
        .child(crate::pages::auto::scroll_bar_named(SharedString::from(format!("{id}-bar")), h))
}
