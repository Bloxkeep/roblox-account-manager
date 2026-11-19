//! A long run of equal rows inside a page's scroll (Accounts' list and cards, Friends' groups): only the rows near the
//! view are made and drawn, the others are empty space of their exact height, so the page scrolls and its bar measures
//! as before. 200 accounts or 1,000 friends then cost what a screenful does on every frame (the page is made again on
//! every change the app sends: a window's RAM, a friend's presence).
//!
//! Where the run stands in the window comes from the frame before (a mark at its top notes it while painting); a scroll
//! further than a third of the margin asks for another frame at once, so a fast scroll never shows blank space.
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ops::Range,
    rc::Rc,
};

use gpui_kit::{prelude::*, *};

/// Runs shorter than this are drawn whole (as everything was before): most people's lists never change.
pub const FROM: usize = 40;
/// Drawn beyond the view each way.
const MARGIN: f32 = 900.;

thread_local! {
    /// Each run's top in the window and the window's height, as the last frame painted them, by the run's id.
    static AT: RefCell<HashMap<&'static str, Rc<Cell<(f32, f32)>>>> = RefCell::new(HashMap::new());
}

fn at(id: &'static str) -> Rc<Cell<(f32, f32)>> {
    AT.with(|m| m.borrow_mut().entry(id).or_insert_with(|| Rc::new(Cell::new((0., 0.)))).clone())
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
    mark: Option<Rc<Cell<(f32, f32)>>>,
}

impl Near {
    /// The rows of `count` to draw, each `tall` high and `gap` apart (the gap only between rows: drawn, a part of rows
    /// has gaps between its own rows only). `must`: a row drawn wherever it is (an account being revealed, or the one
    /// the arrow keys are on: it has to be drawn to be scrolled to). Drawn whole when `on` is false.
    pub fn new(on: bool, id: &'static str, count: usize, tall: f32, gap: f32, must: Option<usize>) -> Self {
        let pitch = tall + gap;
        if !on || count == 0 || pitch <= 0. {
            return Self { parts: vec![Part::Rows(0..count)], mark: None };
        }
        let mark = at(id);
        let (top, view) = mark.get();
        let view = if view > 0. { view } else { 1200. };
        let from = (((-top - MARGIN) / pitch).floor().max(0.) as usize).min(count);
        let to = (((-top + view + MARGIN) / pitch).ceil().max(0.) as usize).clamp(from, count);
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
        if end < count {
            parts.push(Part::Space((count - end) as f32 * pitch));
        }
        Self { parts, mark: Some(mark) }
    }

    /// The mark to put at the run's very top (no height): notes where the run stands as it is painted.
    pub fn mark(&self) -> Option<AnyElement> {
        let at = self.mark.clone()?;
        Some(
            canvas(
                move |b, window, _| {
                    let (old_top, old_view) = at.get();
                    let now = (f32::from(b.origin.y), f32::from(window.viewport_size().height));
                    at.set(now);
                    if (now.0 - old_top).abs() > MARGIN / 3. || now.1 != old_view {
                        window.refresh();
                    }
                },
                |_, _, _, _| {},
            )
            .w_full()
            .h(px(0.))
            .flex_none()
            .into_any_element(),
        )
    }
}

/// Room of `h`, as a part of a column.
pub fn space(h: f32) -> AnyElement {
    div().w_full().h(px(h)).flex_none().into_any_element()
}

/// A grid of `items` cards, `per` a row, each `tall` high, `gap` apart both ways (kit::grid's layout), only its rows
/// near the view made once it has [`FROM`] cards: `make(i)` makes card `i`. `must`: a card drawn wherever it is.
pub fn grid(id: &'static str, items: usize, per: usize, tall: f32, gap: f32, must: Option<usize>, mut make: impl FnMut(usize) -> AnyElement) -> Div {
    let per = per.max(1);
    let near = Near::new(items >= FROM, id, items.div_ceil(per), tall, gap, must.map(|m| m / per));
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
pub fn measured(id: &'static str, n: usize, sig: u64, guess: f32, gap: f32, mut make: impl FnMut(usize) -> AnyElement) -> Div {
    let col = div().w_full().flex().flex_col();
    if n < FROM {
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
        ((0..n).map(|i| t.h.get(&i).copied().unwrap_or(avg)).collect::<Vec<f32>>(), at(id))
    };
    let (top, view) = mark.get();
    let view = if view > 0. { view } else { 1200. };
    let (from, to) = (-top - MARGIN, -top + view + MARGIN);
    let mut y = 0f32;
    let (mut first, mut last, mut above) = (None, 0usize, 0f32);
    for (i, p) in pitches.iter().enumerate() {
        if y + p >= from && y <= to {
            if first.is_none() {
                first = Some(i);
                above = y;
            }
            last = i + 1;
        }
        y += p;
    }
    let first = first.unwrap_or(n);
    let last = last.max(first);
    let below: f32 = pitches[last..].iter().sum();
    let mut col = col.child(Near { parts: Vec::new(), mark: Some(mark) }.mark().expect("a mark")).when(above > 0., |d| d.child(space(above)));
    for i in first..last {
        let t = tall.clone();
        let sig = t.borrow().sig;
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
                            if t.h.insert(i, h).is_none_or(|old| (old - h).abs() > 0.5) {
                                window.refresh();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                ),
        );
    }
    col.when(below > 0., |d| d.child(space(below)))
}

/// Tiles `tile_w` wide in lines (a wrapping grid as wide as its place, `gap` apart both ways), only the lines near the
/// view made once there are [`FROM`] tiles: `make(i)` makes tile `i`. `top`: a line's tiles keep their own heights
/// (kit::grid's), else they are as tall as the line's tallest (a plain wrap's).
pub fn tiles(id: &'static str, items: usize, tile_w: f32, gap: f32, sig: u64, top: bool, guess: f32, mut make: impl FnMut(usize) -> AnyElement) -> Div {
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
                        window.refresh();
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
        .child(measured(id, lines, sig ^ (per as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15), guess, gap, |l| {
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

/// BLOXKEEP_SHOT_MANY=N: a hidden shot's list (the shop's, the free things', a shelf's) made N long with copies of
/// what came, to see a long one without paging through Roblox.
#[cfg(feature = "shot")]
pub fn shot_many<T: Clone>(v: &mut Vec<T>) {
    let Some(n) = std::env::var("BLOXKEEP_SHOT_MANY").ok().and_then(|v| v.parse::<usize>().ok()) else { return };
    let base = v.clone();
    if base.is_empty() {
        return;
    }
    while v.len() < n {
        v.push(base[v.len() % base.len()].clone());
    }
}
