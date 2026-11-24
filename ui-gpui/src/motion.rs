//! Small things that move inside the page (the playing bars, the live mark's ring), drawn in a layer of their own over
//! the app instead of inside it. Inside it, each of their frames (30 a second) made the part of the app they are in
//! over again: each part is kept as it was drawn (shell/parts.rs) only while nothing in it asks to be drawn again, and
//! anything that moves in it asks every frame. Measured 2026-11-20 on Home with two accounts playing: 4.4 ms a frame for the bars
//! alone (13 % of a core), 6.5 ms with an animated theme under them (where the theme alone costs 3.3).
//!
//! In the app each one is a place of its own size (`spot`), noted as it is drawn: where it is, what it is clipped to,
//! how faded. The layer (`layer`, in the root over the app) draws them all at every frame, at the time of that frame,
//! and asks for the next frame on the shared clock (`on_frame_step`), with the animated theme's. What covers a place (a
//! dialog, a menu over it: something that keeps the mouse from it) hides its motion too.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use gpui_kit::*;

/// 60 frames a second, the step of the shared clock the page's other motion and the animated theme use (30 looked
/// stepped on a fast screen: the owner, 2026-11-21, "motion cứ 30fps kiểu gì ấy").
const STEP: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// How a place's motion is drawn: in its bounds, at `t` seconds of the motion's clock, or standing still (`None`: the
/// window behind, or motion turned down in Windows).
type Paint = Rc<dyn Fn(Bounds<Pixels>, Option<f32>, &mut Window, &mut App)>;

struct Place {
    bounds: Bounds<Pixels>,
    mask: ContentMask<Pixels>,
    opacity: f32,
    /// The rounded boxes it lies in (a tile's round corners), rounded again when the layer draws it.
    rounded: Vec<(Bounds<Pixels>, Corners<Pixels>)>,
    hitbox: HitboxId,
    paint: Paint,
    /// Moves on the shared clock (the layer asks for its frames); false: asks for its own (a playing clip, video.rs).
    clocked: bool,
    /// The part of the app it was painted in (shell/parts.rs): kept while that part is, cleared when it is painted
    /// anew. None: painted outside the parts (a dialog), anew at every frame, so it is drawn once and dropped.
    owner: Option<EntityId>,
}

thread_local! {
    /// The places in the main window as last drawn, each part's noted again each time that part is painted anew.
    static PLACES: RefCell<Vec<Place>> = const { RefCell::new(Vec::new()) };
    /// The main window (the one with the layer); a place in another window draws itself still.
    static MAIN: Cell<Option<WindowId>> = const { Cell::new(None) };
    /// A frame is asked for at the next step.
    static ASKED: Cell<bool> = const { Cell::new(false) };
    static START: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// The main window: said each time it is drawn (the root's render, before anything is painted).
pub fn main_window(window: &Window) {
    MAIN.with(|m| m.set(Some(window.window_handle().window_id())));
}

/// Part `owner` is painted anew: its places are noted again as it is.
pub fn clear_of(owner: EntityId) {
    PLACES.with(|p| p.borrow_mut().retain(|p| p.owner != Some(owner)));
}

/// Only the parts `shown` are in the app this frame: the places of any other part go with it.
pub fn keep(shown: &[EntityId]) {
    PLACES.with(|p| p.borrow_mut().retain(|p| p.owner.is_none_or(|o| shown.contains(&o))));
}

/// Whether `window` is the main one (the one with the layer).
pub fn is_main(window: &Window) -> bool {
    MAIN.with(|m| m.get()) == Some(window.window_handle().window_id())
}

/// The main window shown again with every part kept as drawn (`Window::redraw`): what changed is only in a texture it
/// paints (the 3D avatar's, pages/avatar.rs) or in this layer.
pub fn redraw_main(cx: &mut App) {
    for handle in cx.windows() {
        let _ = handle.update(cx, |_, window, _| {
            if is_main(window) {
                window.redraw();
            }
        });
    }
}

/// A place `w` × `h` whose motion `paint` draws (see `Paint`).
pub fn spot(w: f32, h: f32, paint: impl Fn(Bounds<Pixels>, Option<f32>, &mut Window, &mut App) + 'static) -> Div {
    div().w(px(w)).h(px(h)).flex_none().child(placed(Rc::new(paint), true))
}

/// A place filling its box, drawn by the layer whenever the window is drawn but not moved on by its clock: what it
/// shows asks for frames itself (`Window::redraw`, which keeps the app as drawn). A playing clip's frames: each made the
/// whole app over again inside it (88 frames a second at 6 ms each for a 60 fps clip on Home, 2026-11-21).
pub fn place(paint: impl Fn(Bounds<Pixels>, Option<f32>, &mut Window, &mut App) + 'static) -> Div {
    div().size_full().child(placed(Rc::new(paint), false))
}

fn placed(paint: Paint, clocked: bool) -> impl IntoElement {
    {
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |bounds, hitbox, window, cx| {
                // (In another window, still. Covered (a dialog's dimmed backdrop over it): drawn here, where what
                // covers it dims it, standing as it was when covered, as the app is kept until the cover goes.)
                if MAIN.with(|m| m.get()) != Some(window.window_handle().window_id()) {
                    paint(bounds, None, window, cx);
                    return;
                }
                if !window.hitbox_reached(hitbox.id, bounds.center()) {
                    paint(bounds, now(cx), window, cx);
                    return;
                }
                let place = Place { bounds, mask: window.content_mask(), opacity: window.element_opacity(), rounded: window.rounded_clips(), hitbox: hitbox.id, paint: paint.clone(), clocked, owner: crate::shell::parts::owner(window) };
                PLACES.with(|p| p.borrow_mut().push(place));
            },
        )
        .size_full()
    }
}

/// The layer over the app that draws every place's motion (see the top of the file).
pub fn layer() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |_, _, window, cx| {
            let t = now(cx);
            let moving = t.is_some();
            let places: Vec<_> = PLACES.with(|p| p.borrow().iter().map(|p| (p.bounds, p.mask.clone(), p.opacity, p.rounded.clone(), p.hitbox, p.paint.clone())).collect());
            let clocked = PLACES.with(|p| p.borrow().iter().any(|p| p.clocked));
            for (bounds, mask, opacity, rounded, hitbox, paint) in &places {
                if !window.hitbox_reached(*hitbox, bounds.center()) {
                    continue;
                }
                window.with_content_mask(Some(mask.clone()), |window| {
                    within_rounded(window, rounded, &mut |window| window.with_element_opacity(Some(*opacity), |window| paint(*bounds, t, window, cx)))
                });
            }
            // (Those painted outside the parts are painted again before the next frame's layer.)
            PLACES.with(|p| p.borrow_mut().retain(|p| p.owner.is_some()));
            if moving && clocked && !ASKED.with(|a| a.replace(true)) {
                let handle = window.window_handle();
                on_frame_step(cx, STEP, move |cx| {
                    ASKED.with(|a| a.set(false));
                    let _ = handle.update(cx, |_, window, _| window.redraw());
                });
            }
        },
    )
    .absolute()
    .inset_0()
}

/// `f` painted inside each of `rounded` (outermost first), as it would be where its place lies.
fn within_rounded(window: &mut Window, rounded: &[(Bounds<Pixels>, Corners<Pixels>)], f: &mut dyn FnMut(&mut Window)) {
    match rounded.split_first() {
        None => f(window),
        Some(((bounds, radii), rest)) => window.with_rounded_content_mask(Some(ContentMask { bounds: *bounds }), *radii, |window| within_rounded(window, rest, f)),
    }
}

/// The motion's clock now, in seconds; `None` while it stands still (the window behind, or motion turned down).
fn now(cx: &App) -> Option<f32> {
    (crate::kit::looping() && !cx.reduce_motion()).then(|| {
        START.with(|s| {
            let start = s.get().unwrap_or_else(Instant::now);
            s.set(Some(start));
            start.elapsed().as_secs_f32()
        })
    })
}

/// A rounded box filled with `color`.
pub fn fill_round(window: &mut Window, bounds: Bounds<Pixels>, radius: f32, color: Hsla) {
    window.paint_quad(fill(bounds, color).corner_radii(px(radius)));
}
