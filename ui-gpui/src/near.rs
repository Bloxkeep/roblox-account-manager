//! A long run of rows inside a page's scroll (Accounts' list and cards, Friends' groups, the Roblox tab's games, a
//! game's servers, the Marketplace): only the rows in view are made and drawn, the others are empty space of their
//! exact height, so the page scrolls and its bar measures as before. 200 accounts or 1,000 friends then cost what a
//! screenful does on every frame (the page is made again on every change the app sends, and on every frame of a
//! scroll).
//!
//! Where the run stands comes from the frame before: a mark at its top notes, as it is painted, where it was in the
//! window, how far its page was scrolled then and what of the window the page shows. Given the page's scroll handle,
//! the run's place now is that place moved by what the page scrolled since (the handle has this frame's offset before
//! the page is made), so the rows drawn are those in view and a few more; without it, the place last painted, with a
//! wide margin. A mark that finds the run elsewhere than it was reckoned (the page above it changed, a first frame)
//! asks for the next frame at once, drawn from where it really is.
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ops::Range,
    rc::Rc,
};

use gpui_kit::{prelude::*, *};

/// Runs of rows measured as they are drawn (`measured`, `tiles`) shorter than this are drawn whole (as everything was
/// before): a row not drawn yet only has a guessed height.
pub const FROM: usize = 40;
/// Runs of rows of one height in a page's scroll as long as this are cut: a row's height is known (given, or measured
/// from the first row painted), so cutting changes nothing on the screen.
pub const ROWS_FROM: usize = 8;
/// Drawn beyond the view each way when the run's place now is reckoned from its page's scroll (on the side rows come
/// in from while it scrolls: their pictures are asked for before they show).
const MARGIN_IN: f32 = 160.;
/// Drawn beyond the view on the side rows leave by while the page scrolls: every row kept there is made again on every
/// frame of the scroll, for nothing.
const BEHIND: f32 = 40.;
/// Drawn beyond the view each way otherwise (no scroll handle, or nothing painted yet).
const MARGIN: f32 = 900.;

/// Where a run stood as it was last painted, and where it was reckoned to be when made.
#[derive(Clone, Copy, Default)]
struct Seen {
    /// Painted yet.
    known: bool,
    /// Its top in the window.
    top: f32,
    /// Its page's scroll offset (down is negative) then.
    scrolled: f32,
    /// What of the window its page showed: top and bottom.
    view: (f32, f32),
    /// Its top as reckoned when the rows were picked, and the margin they were picked with.
    guess: f32,
    margin: f32,
}

thread_local! {
    /// Each run's last place (`Seen`), by the run's id.
    static AT: RefCell<HashMap<&'static str, Rc<Cell<Seen>>>> = RefCell::new(HashMap::new());
}

fn at(id: &'static str) -> Rc<Cell<Seen>> {
    AT.with(|m| m.borrow_mut().entry(id).or_default().clone())
}

thread_local! {
    /// The window's scale as the last run was painted.
    static SCALE: Cell<f32> = const { Cell::new(1.) };
}

/// A length as layout gives it: GPUI rounds a length it is given to whole device pixels, halves toward zero (its
/// taffy.rs), so at 150 % a 1 px hairline takes 1 device pixel and a row of 56 px and its hairline take 85, not 85.5.
/// Room for rows not drawn is counted from the rounded lengths, else 43 rows' room came out 21 device pixels taller
/// than the rows.
fn snap(x: f32) -> f32 {
    let s = SCALE.with(Cell::get);
    let d = x * s;
    (d.abs() - 0.5).ceil().copysign(d) / s
}

/// What of a run to draw: from and to, in px from the run's top (the view and its margin). Notes the reckoning in
/// `mark` for the mark to check.
fn band(mark: &Rc<Cell<Seen>>, scroll: Option<&ScrollHandle>) -> (f32, f32) {
    let mut s = mark.get();
    // (The offset as the page will clamp it: a glide past the end stops there.)
    let offset = |h: &ScrollHandle| f32::from(h.offset().y).clamp(-f32::from(h.max_offset().y).max(0.), 0.);
    let (top, view, (above, below)) = match (s.known, scroll) {
        (true, Some(h)) => {
            let moved = offset(h) - s.scrolled;
            // (Scrolling, the full margin is the side rows come in from; the side they leave by keeps a little.)
            let margins = if moved < 0. { (BEHIND, MARGIN_IN) } else if moved > 0. { (MARGIN_IN, BEHIND) } else { (MARGIN_IN, MARGIN_IN) };
            (s.top + moved, s.view, margins)
        }
        (true, None) => (s.top, s.view, (MARGIN, MARGIN)),
        // Nothing painted yet: as if the run began the page's scroll.
        (false, Some(h)) if h.bounds().size.height > px(0.) => {
            let b = h.bounds();
            (f32::from(b.origin.y) + offset(h), (f32::from(b.origin.y), f32::from(b.bottom())), (MARGIN, MARGIN))
        }
        (false, _) => (0., (0., 1200.), (MARGIN, MARGIN)),
    };
    s.guess = top;
    s.margin = above.min(below);
    mark.set(s);
    (view.0 - above - top, view.1 + below - top)
}

/// The mark at a run's top (no height): notes where the run stands as it is painted, and asks for the next frame when
/// that isn't where its rows were picked for.
fn mark_el(mark: Rc<Cell<Seen>>, scroll: Option<ScrollHandle>) -> AnyElement {
    canvas(
        move |b, window, _| {
            let scale = window.scale_factor();
            if SCALE.with(|s| s.replace(scale)) != scale {
                window.request_animation_frame();
            }
            let mut s = mark.get();
            let top = f32::from(b.origin.y);
            let off = (top - s.guess).abs() > s.margin / 3.;
            let first = !s.known;
            s.known = true;
            s.top = top;
            match &scroll {
                Some(h) if h.bounds().size.height > px(0.) => {
                    let v = h.bounds();
                    s.scrolled = f32::from(h.offset().y);
                    s.view = (f32::from(v.origin.y), f32::from(v.bottom()));
                }
                _ => {
                    s.scrolled = 0.;
                    s.view = (0., f32::from(window.viewport_size().height));
                }
            }
            mark.set(s);
            // (GPUI's refresh does nothing while a frame is drawn: the view being drawn is asked for the next one.)
            if first || off {
                window.request_animation_frame();
            }
        },
        |_, _, _, _| {},
    )
    .w_full()
    .h(px(0.))
    .flex_none()
    .into_any_element()
}

/// The id of a run named after something of the person's (a group of accounts): `prefix` and the name, kept for the
/// program's life, once a name (a run's id is `&'static`; there are as many as names ever seen, a handful).
pub fn named(prefix: &str, name: &str) -> &'static str {
    thread_local! {
        static NAMES: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
    }
    let key = format!("{prefix}{name}");
    NAMES.with(|m| *m.borrow_mut().entry(key.clone()).or_insert_with(|| Box::leak(key.into_boxed_str())))
}

/// A part of a run: rows drawn, or the room of rows not drawn.
pub enum Part {
    Rows(Range<usize>),
    Space(f32),
}

/// What of a run to draw, top to bottom.
pub struct Near {
    pub parts: Vec<Part>,
    mark: Option<(Rc<Cell<Seen>>, Option<ScrollHandle>)>,
}

impl Near {
    /// The rows of `count` to draw, each `tall` high and `gap` apart (the gap only between rows: drawn, a part of rows
    /// has gaps between its own rows only). `must`: a row drawn wherever it is (an account being revealed, or the one
    /// the arrow keys are on: it has to be drawn to be scrolled to). Drawn whole when `on` is false.
    pub fn new(on: bool, id: &'static str, count: usize, tall: f32, gap: f32, must: Option<usize>) -> Self {
        Self::new_in(None, on, id, count, tall, gap, must)
    }

    /// As `new`, in the page that `scroll` scrolls (the rows picked are those in view: see the module's words).
    pub fn new_in(scroll: Option<&ScrollHandle>, on: bool, id: &'static str, count: usize, tall: f32, gap: f32, must: Option<usize>) -> Self {
        if !on || count == 0 || tall + gap <= 0. {
            return Self { parts: vec![Part::Rows(0..count)], mark: None };
        }
        let (tall, gap) = (snap(tall), snap(gap));
        let pitch = tall + gap;
        let mark = at(id);
        let (from_y, to_y) = band(&mark, scroll);
        let from = ((from_y / pitch).floor().max(0.) as usize).min(count);
        let to = ((to_y / pitch).ceil().max(0.) as usize).clamp(from, count);
        let mut runs: Vec<Range<usize>> = Vec::new();
        if let Some(m) = must.filter(|m| *m < count && !(from..to).contains(m)) {
            runs.push(m..m + 1);
        }
        if from < to {
            runs.push(from..to);
        }
        runs.sort_by_key(|r| r.start);
        let mut parts = Vec::new();
        let mut end = 0usize;
        for (i, r) in runs.into_iter().enumerate() {
            // Before the first part of rows, the rows above it; between two, the rows between and the gap after the
            // first part's last row.
            let space = (r.start - end) as f32 * pitch + if i > 0 { gap } else { 0. };
            if space > 0. {
                parts.push(Part::Space(space));
            }
            end = r.end;
            parts.push(Part::Rows(r));
        }
        // (After the last part, the gap after its last row and the rows after it; with no row drawn, the rows and
        // the gaps between them only: one gap fewer than rows.)
        if end < count {
            parts.push(Part::Space((count - end) as f32 * pitch - if end == 0 { gap } else { 0. }));
        }
        Self { parts, mark: Some((mark, scroll.cloned())) }
    }

    /// The mark to put at the run's very top (no height): notes where the run stands as it is painted.
    pub fn mark(&self) -> Option<AnyElement> {
        let (at, scroll) = self.mark.clone()?;
        Some(mark_el(at, scroll))
    }
}

/// Room of `h`, as a part of a column.
pub fn space(h: f32) -> AnyElement {
    div().w_full().h(px(h)).flex_none().into_any_element()
}

/// A grid of `items` cards, `per` a row, each `tall` high, `gap` apart both ways (kit::grid's layout), only its rows
/// near the view made once it has [`FROM`] cards: `make(i)` makes card `i`. `must`: a card drawn wherever it is.
pub fn grid(id: &'static str, items: usize, per: usize, tall: f32, gap: f32, must: Option<usize>, make: impl FnMut(usize) -> AnyElement) -> Div {
    grid_in(None, items >= FROM, id, items, per, tall, gap, must, make)
}

/// As `grid`, in the page that `scroll` scrolls, cut when `on`.
#[allow(clippy::too_many_arguments)]
pub fn grid_in(scroll: Option<&ScrollHandle>, on: bool, id: &'static str, items: usize, per: usize, tall: f32, gap: f32, must: Option<usize>, mut make: impl FnMut(usize) -> AnyElement) -> Div {
    let per = per.max(1);
    let near = Near::new_in(scroll, on, id, items.div_ceil(per), tall, gap, must.map(|m| m / per));
    let mut col = div().w_full().flex().flex_col().children(near.mark());
    for part in near.parts {
        col = col.child(match part {
            Part::Space(h) => space(h),
            Part::Rows(r) => crate::kit::grid(gap).children((r.start * per..(r.end * per).min(items)).map(&mut make).collect::<Vec<_>>()).into_any_element(),
        });
    }
    col
}

// ── Rows of different heights ───────────────────────────────────────────────────────────────────────────────────
// Activity's lines (the words wrap), a server's or a shop's tiles in lines: a row's height is known once it has been
// drawn (measured as it is painted); the rows not drawn yet count as the average of those measured.

/// A run's measured rows: their heights with the gap after (by row), the run's width, and what they were measured for.
#[derive(Default)]
struct Tall {
    sig: u64,
    h: HashMap<usize, f32>,
    width: f32,
}

thread_local! {
    static TALL: RefCell<HashMap<&'static str, Rc<RefCell<Tall>>>> = RefCell::new(HashMap::new());
}

fn tall_of(id: &'static str) -> Rc<RefCell<Tall>> {
    TALL.with(|m| m.borrow_mut().entry(id).or_default().clone())
}

/// `n` rows of different heights, `gap` between them, only those near the view made once there are [`FROM`]:
/// `make(i)` makes row `i`. `sig`: what the rows are (other rows, a changed filter): a new one measures again. `guess`: a
/// row's height before any has been measured.
pub fn measured(id: &'static str, n: usize, sig: u64, guess: f32, gap: f32, make: impl FnMut(usize) -> AnyElement) -> Div {
    measured_in(None, FROM, id, n, sig, guess, gap, make)
}

/// As `measured`, in the page that `scroll` scrolls, cut from `from` rows (rows that are all one height are measured
/// right from the first one painted: [`ROWS_FROM`]). A row not yet measured is counted as tall as the measured ones are
/// on average.
#[allow(clippy::too_many_arguments)]
pub fn measured_in(scroll: Option<&ScrollHandle>, from: usize, id: &'static str, n: usize, sig: u64, guess: f32, gap: f32, make: impl FnMut(usize) -> AnyElement) -> Div {
    run(scroll, from, id, n, sig, guess, gap, false, make)
}

/// The rows of [`measured_in`]; `exact`: every row not yet measured is made too, wherever it is, so the room of the
/// rows not made is always their own height (the first frame of a new `sig` makes them all, then only those in view).
#[allow(clippy::too_many_arguments)]
fn run(scroll: Option<&ScrollHandle>, from: usize, id: &'static str, n: usize, sig: u64, guess: f32, gap: f32, exact: bool, mut make: impl FnMut(usize) -> AnyElement) -> Div {
    let col = div().w_full().flex().flex_col();
    if n < from {
        return col.children((0..n).map(|i| div().w_full().when(i + 1 < n, |d| d.pb(px(gap))).child(make(i)).into_any_element()).collect::<Vec<_>>());
    }
    let tall = tall_of(id);
    let (pitches, mark) = {
        let mut t = tall.borrow_mut();
        if t.sig != sig {
            t.sig = sig;
            t.h.clear();
        }
        let avg = if t.h.is_empty() { guess + gap } else { t.h.values().sum::<f32>() / t.h.len() as f32 };
        ((0..n).map(|i| t.h.get(&i).copied().ok_or(avg)).collect::<Vec<Result<f32, f32>>>(), at(id))
    };
    let (from, to) = band(&mark, scroll);
    let sig = tall.borrow().sig;
    let gap = snap(gap);
    let mut col = col.child(mark_el(mark, scroll.cloned()));
    // Each row is counted with the gap after it, the last one too, which has none: room that reaches the run's end
    // leaves that gap out.
    let (mut y, mut room) = (0f32, 0f32);
    for (i, p) in pitches.iter().enumerate() {
        let (p, known) = match *p {
            Ok(p) => (p, true),
            Err(p) => (p, false),
        };
        let shown = (y + p >= from && y <= to) || (exact && !known);
        y += p;
        if !shown {
            room += p;
            continue;
        }
        if room > 0. {
            col = col.child(space(room));
            room = 0.;
        }
        let t = tall.clone();
        col = col.child(
            div()
                .relative()
                .w_full()
                .when(i + 1 < n, |d| d.pb(px(gap)))
                .child(make(i))
                .child(
                    canvas(
                        move |b, window, _| {
                            let h = f32::from(b.size.height);
                            let mut t = t.borrow_mut();
                            if t.sig != sig {
                                return;
                            }
                            // (The last row has no gap after it: counted with one, as the others.)
                            let h = if i + 1 < n { h } else { h + gap };
                            // Another height than it was counted as: the rows after it moved, drawn again.
                            if t.h.insert(i, h).is_none_or(|old| (old - h).abs() > 0.5) {
                                window.request_animation_frame();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                ),
        );
    }
    col.when(room - gap > 0., |d| d.child(space(room - gap)))
}

/// Tiles `tile_w` wide in lines (a wrapping grid as wide as its place, `gap` apart both ways), only the lines near the
/// view made once there are [`FROM`] tiles: `make(i)` makes tile `i`. `top`: a line's tiles keep their own heights
/// (kit::grid's), else they are as tall as the line's tallest (a plain wrap's).
#[allow(clippy::too_many_arguments)]
pub fn tiles(id: &'static str, items: usize, tile_w: f32, gap: f32, sig: u64, top: bool, guess: f32, make: impl FnMut(usize) -> AnyElement) -> Div {
    tiles_in(None, id, items, tile_w, gap, sig, top, guess, make)
}

/// As `tiles`, in the page that `scroll` scrolls.
#[allow(clippy::too_many_arguments)]
pub fn tiles_in(scroll: Option<&ScrollHandle>, id: &'static str, items: usize, tile_w: f32, gap: f32, sig: u64, top: bool, guess: f32, mut make: impl FnMut(usize) -> AnyElement) -> Div {
    if items < FROM {
        return div().w_full().flex().flex_wrap().when(top, |d| d.items_start()).gap(px(gap)).children((0..items).map(make).collect::<Vec<_>>());
    }
    // How many go in a line, from the width the run was drawn at (a line a guess the first time, then right).
    let tall = tall_of(id);
    let width = tall.borrow().width;
    let per = if width > 0. { (((width + gap) / (tile_w + gap)).floor() as usize).max(1) } else { 4 };
    let lines = items.div_ceil(per);
    let noted = tall.clone();
    div()
        .w_full()
        .relative()
        .child(
            canvas(
                move |b, window, _| {
                    let w = f32::from(b.size.width);
                    let mut t = noted.borrow_mut();
                    if (t.width - w).abs() > 0.5 {
                        t.width = w;
                        window.request_animation_frame();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(0.)),
        )
        // (Cut by its lines whatever their number: the tiles are past [`FROM`]. Counted against FROM themselves, the lines
        // of 100 servers or 200 things in the Marketplace, 4 to 6 a line, were all drawn. Each line is measured before
        // it is left out: a line's height comes from its tiles' names, one line or two, and an average of the others
        // put the lines after it a few pixels off.)
        .child(run(scroll, 0, id, lines, sig ^ (per as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15), guess, gap, true, |l| {
            div().w_full().flex().when(top, |d| d.items_start()).gap(px(gap)).children((l * per..((l + 1) * per).min(items)).map(&mut make).collect::<Vec<_>>()).into_any_element()
        }))
}

/// A number for what a run's rows are, from anything hashable (their ids, their count, a filter).
pub fn sig(of: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    of.hash(&mut h);
    h.finish()
}

/// BLOXKEEP_SHOT_MANY=N: a hidden shot's list (the shop's, the free things', a shelf's, the Roblox tab's games) made N
/// long, with copies of what came or cut to its first N, to see a long one without paging through Roblox (and a short
/// one beside it, to time the two).
#[cfg(feature = "shot")]
pub fn shot_many<T: Clone>(v: &mut Vec<T>) {
    let Some(n) = std::env::var("BLOXKEEP_SHOT_MANY").ok().and_then(|v| v.parse::<usize>().ok()) else { return };
    let base = v.clone();
    if base.is_empty() {
        return;
    }
    v.truncate(n);
    while v.len() < n {
        v.push(base[v.len() % base.len()].clone());
    }
}
