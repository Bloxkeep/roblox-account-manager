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
    with_bar(id, &h, div().id(id).size_full().overflow_y_scroll().track_scroll(&h).child(body))
}

/// A scrolling element that already has its own handle, put in a box with the bar beside it.
pub fn with_bar(id: &'static str, h: &ScrollHandle, scroller: impl IntoElement) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .size_full()
        .relative()
        .child(scroller)
        .child(crate::pages::auto::scroll_bar_named(SharedString::from(format!("{id}-bar")), h))
}
