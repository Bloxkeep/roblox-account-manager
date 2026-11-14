//! The logo drawn at the exact pixels it takes on screen. GPUI turns an SVG <img> into a picture twice its own size
//! (128 px for the logo) and shrinks that on the graphics card, which has no smaller copies to shrink from: at 32 px on
//! a 150 % screen (48 px) it came out jagged and smudged (owner's screenshot, 2026-11-12). Here resvg draws it at
//! size × scale once, kept per size; the picture then lands pixel for pixel. And it must stand on whole pixels too: laid
//! out by the page it can land a fraction of a pixel off (the rail's 68 less its 1 px border leaves the 32 px logo at
//! 17.5, a quarter pixel off at 150 %), and the graphics card then blends every pixel with its neighbour, the tilted
//! edges came out bent (owner 2026-11-13: "vẫn bị méo"). `mark` draws it itself, snapped.
use std::{cell::RefCell, collections::HashMap, sync::Arc};

use gpui_kit::*;

const LOGO: &[u8] = include_bytes!("../assets/logo.svg");
/// The logo's two blocks alone, without the app icon's dark tile round them, filling the square: what the rail shows.
/// On the rail's own dark ground the tile was a box in a box, and it left the face 24 px of the 48 (at 150 %): eyes
/// and smile a few pixels each, tilted, read as bent (owner 2026-11-13). Here the face is half as large again.
const MARK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="6.5 5 51 51"><g transform="rotate(-8 32 32)"><rect x="25" y="10" width="28" height="28" rx="6" fill="#3b4250"/><rect x="12" y="20" width="32" height="32" rx="7" fill="#f5c933"/><rect x="19.5" y="30" width="4.2" height="6.5" rx="2.1" fill="#15181e"/><rect x="32.3" y="30" width="4.2" height="6.5" rx="2.1" fill="#15181e"/><path d="M20.5 41.5c4.6 4.2 10.4 4.2 15 0" fill="none" stroke="#15181e" stroke-width="2.6" stroke-linecap="round"/></g></svg>"##;
/// Sizes kept (the rail's, the lock screen's, each at the screens' scales): a size past that is drawn the old way rather
/// than kept forever.
const MAX_KEPT: usize = 12;

thread_local! {
    static DRAWN: RefCell<HashMap<u32, Arc<RenderImage>>> = RefCell::new(HashMap::new());
    static MARKS: RefCell<HashMap<u32, Arc<RenderImage>>> = RefCell::new(HashMap::new());
}

/// The logo's picture at `dots` pixels a side: kept, or drawn now.
fn picture(dots: u32) -> Option<Arc<RenderImage>> {
    let kept = DRAWN.with(|d| d.borrow().get(&dots).cloned());
    if kept.is_some() {
        return kept;
    }
    let pic = Arc::new(draw(LOGO, dots)?);
    DRAWN.with(|d| {
        let mut d = d.borrow_mut();
        if d.len() < MAX_KEPT {
            d.insert(dots, pic.clone());
        }
    });
    Some(pic)
}

/// The logo at `size` (logical px) for this window's scale; GPUI's own drawing of the file when it can't be drawn here.
/// For an element that moves or grows (the lock screen's pop); one that stands still takes `mark`.
pub fn logo(size: f32, window: &Window) -> ImageSource {
    let dots = (size * window.scale_factor()).round().clamp(1., 1024.) as u32;
    match picture(dots) {
        Some(pic) => ImageSource::Render(pic),
        None => "logo.svg".into(),
    }
}

/// The logo's blocks alone (no tile) in a box of `side` logical px, as `mark` draws the whole logo: for the rail.
pub fn blocks(side: f32) -> impl IntoElement {
    snapped(side, |dots| {
        let kept = MARKS.with(|d| d.borrow().get(&dots).cloned());
        kept.or_else(|| {
            let pic = Arc::new(draw(MARK, dots)?);
            MARKS.with(|d| {
                let mut d = d.borrow_mut();
                if d.len() < MAX_KEPT {
                    d.insert(dots, pic.clone());
                }
            });
            Some(pic)
        })
    })
}

/// The logo standing still in a box of `side` logical px: drawn at its exact pixels and on whole pixels, centred in the
/// box, wherever the page's layout put the box.
pub fn mark(side: f32) -> impl IntoElement {
    snapped(side, picture)
}

fn snapped(side: f32, picture: impl Fn(u32) -> Option<Arc<RenderImage>> + 'static) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let scale = window.scale_factor();
            let dots = (side * scale).round().clamp(1., 1024.) as u32;
            let Some(pic) = picture(dots) else { return };
            let snap = |v: f32| (v * scale).round() / scale;
            let shown = dots as f32 / scale;
            let x = snap(f32::from(bounds.origin.x) + (f32::from(bounds.size.width) - shown) / 2.);
            let y = snap(f32::from(bounds.origin.y) + (f32::from(bounds.size.height) - shown) / 2.);
            let at = Bounds::new(point(px(x), px(y)), size(px(shown), px(shown)));
            let _ = window.paint_image(at, at, Corners::all(px(0.)), pic, 0, false);
        },
    )
    .size(px(side))
    .flex_none()
}

/// An SVG drawn square at `px` pixels, as GPUI keeps pictures (blue, green, red, alpha; not premultiplied).
fn draw(svg: &[u8], px: u32) -> Option<RenderImage> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    let size = tree.size();
    let (sx, sy) = (px as f32 / size.width(), px as f32 / size.height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(sx, sy), &mut pixmap.as_mut());
    let mut dots = pixmap.take();
    for p in dots.chunks_exact_mut(4) {
        p.swap(0, 2);
        let a = p[3];
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
            }
        }
    }
    let buffer = image::RgbaImage::from_raw(px, px, dots)?;
    Some(RenderImage::new(vec![image::Frame::new(buffer)]))
}
