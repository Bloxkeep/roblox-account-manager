//! The app's parts as views of their own, as Zed builds a GPUI app. Before, all of the app over the theme was one
//! kept view: anything that moved in it (the status bar's card opening, a notice, a menu's fade, a hover's spring, a
//! page coming in) asked for its view to be drawn again, and that was the whole app, the page under it too. The
//! pop-up's frames cost 5.1 ms each on Home and 5.8 on a game's page, past a 240 Hz screen's 4.2: the heavier the
//! page, the choppier anything over it moved.
//!
//! Now Root draws a frame that keeps nothing itself (`Root::frame`), and in it each part is a view kept from frame to
//! frame (`AnyView::cached`): the rail, the top bar, the page, the accounts' column, the status bar, and what floats
//! over them (the status bar's cards, the notices). What moves in one part asks for that part alone (GPUI's
//! `request_animation_frame` notifies the view being drawn, and the views above it, never the ones beside it); the
//! others are put down again as they were. Root notified (its data changed) draws them all again, as before.
//!
//! Rules that come with it:
//! - The frame never animates: an animation there notifies Root, which draws every part again at each of its frames.
//! - A kept view inside a kept view is drawn again whenever its parent is (GPUI draws a kept view's insides with
//!   `refreshing` on), so the parts sit side by side under the frame, never inside each other.
//! - A kept view is laid out by its style alone, not by what it holds: each part's style gives its size, and what it
//!   holds fills it.
//! - What a part notes while it is laid out or painted, for others to read (motion.rs's places, the live pictures'
//!   boxes), is kept per part: a part put down again as it was leaves its notes as they were, a part drawn again
//!   clears its own first (`begin`), and a part not shown any more has its notes dropped (`Root::frame`).
//! - What one part shows from another's state without Root being notified (the status bar's pill hidden under its
//!   open card, capsule.rs) asks for the reading part to be drawn again when that state changes.
use std::cell::{Cell, RefCell};

use gpui_kit::{prelude::*, *};

use super::{Live, Root};

macro_rules! part {
    ($name:ident, $draw:ident, $doc:literal) => {
        #[doc = $doc]
        pub struct $name {
            pub(super) root: WeakEntity<Root>,
        }

        impl Render for $name {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let me = cx.entity_id();
                self.root.update(cx, |r, cx| r.$draw(me, window, cx)).unwrap_or_else(|_| div().into_any_element())
            }
        }
    };
}

part!(RailPart, rail_part, "The rail at the left: the logo and the pages' buttons.");
part!(TopPart, top_part, "The top bar: back and forward, the quick search, the window's buttons.");
part!(PagePart, page_part, "The page shown (or the lock screen, or the line lost), with the strip of dropped accounts over it.");
part!(SidePart, side_part, "The accounts' column beside Home.");
part!(StatusPart, status_part, "The status bar's pills.");
part!(OverPart, over_part, "What floats over the rest: the status bar's cards, the notices (and a shot's tray menu).");

thread_local! {
    /// The part being laid out or painted anew (between its `begin` and `end` marks).
    static NOW: Cell<Option<EntityId>> = const { Cell::new(None) };
    /// Every part's view, to tell a part from another view.
    static IDS: RefCell<Vec<EntityId>> = const { RefCell::new(Vec::new()) };
}

/// Root's parts, made at its first frame.
#[derive(Clone)]
pub struct Parts {
    pub rail: Entity<RailPart>,
    pub top: Entity<TopPart>,
    pub page: Entity<PagePart>,
    pub side: Entity<SidePart>,
    pub status: Entity<StatusPart>,
    pub over: Entity<OverPart>,
}

impl Parts {
    pub fn new(root: WeakEntity<Root>, cx: &mut App) -> Self {
        let parts = Self {
            rail: cx.new(|_| RailPart { root: root.clone() }),
            top: cx.new(|_| TopPart { root: root.clone() }),
            page: cx.new(|_| PagePart { root: root.clone() }),
            side: cx.new(|_| SidePart { root: root.clone() }),
            status: cx.new(|_| StatusPart { root: root.clone() }),
            over: cx.new(|_| OverPart { root }),
        };
        IDS.with(|i| {
            *i.borrow_mut() = vec![
                parts.rail.entity_id(),
                parts.top.entity_id(),
                parts.page.entity_id(),
                parts.side.entity_id(),
                parts.status.entity_id(),
                parts.over.entity_id(),
            ]
        });
        parts
    }
}

/// What the frame worked out for the parts this frame (they are drawn after it, in the same frame, or kept).
#[derive(Clone, Copy, Default)]
pub struct Frame {
    pub locked: bool,
    pub lost: bool,
    pub with_side: bool,
    /// The accounts' column may slide in over Home (a narrow window): `Root::no_side`.
    pub slide: bool,
    pub main_w: f32,
    /// The top bar has its glass (scene::top_glass): it keeps a gap under it.
    pub top_glass: bool,
}

/// The part that what is laid out or painted now belongs to: the one between its marks, or the one a menu or a
/// popup drawn after everything was made in (GPUI draws those with the view they were made in). None: outside the
/// parts (a dialog over the app).
pub fn owner(window: &Window) -> Option<EntityId> {
    NOW.with(|n| n.get()).or_else(|| {
        let view = window.current_view();
        IDS.with(|i| i.borrow().contains(&view)).then_some(view)
    })
}

/// A part's box, filling the part, opening with its mark: laid out anew, the part's live pictures' notes are cleared;
/// painted anew, its motion places are. What follows is the part's own until `end`.
pub(super) fn begin(me: EntityId, live: &Live) -> Div {
    let live = live.clone();
    div().size_full().relative().child(
        canvas(
            move |_, _, _| {
                NOW.with(|n| n.set(Some(me)));
                live.clear_of(me);
            },
            move |_, _, _, _| {
                NOW.with(|n| n.set(Some(me)));
                crate::motion::clear_of(me);
            },
        )
        .absolute()
        .size_0(),
    )
}

/// The last thing in a part's box (after `begin`).
pub(super) fn end() -> AnyElement {
    canvas(|_, _, _| NOW.with(|n| n.set(None)), |_, _, _, _| NOW.with(|n| n.set(None))).absolute().size_0().into_any_element()
}
