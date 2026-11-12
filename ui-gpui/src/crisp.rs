//! The logo drawn at the exact pixels it takes on screen. GPUI turns an SVG <img> into a picture twice its own size
//! (128 px for the logo) and shrinks that on the graphics card, which has no smaller copies to shrink from: at 32 px on
//! a 150 % screen (48 px) it came out jagged and smudged (owner's screenshot, 2026-11-12). Here resvg draws it at
//! size × scale once, kept per size; the picture then lands pixel for pixel.
use std::{cell::RefCell, collections::HashMap, sync::Arc};

use gpui_kit::*;

const LOGO: &[u8] = include_bytes!("../assets/logo.svg");
/// Sizes kept (the rail's, the lock screen's, each at the screens' scales): a size past that is drawn the old way rather
/// than kept forever.
const MAX_KEPT: usize = 12;

thread_local! {
    static DRAWN: RefCell<HashMap<u32, Arc<RenderImage>>> = RefCell::new(HashMap::new());
}

/// The logo at `size` (logical px) for this window's scale; GPUI's own drawing of the file when it can't be drawn here.
pub fn logo(size: f32, window: &Window) -> ImageSource {
    let px = (size * window.scale_factor()).round().clamp(1., 1024.) as u32;
    let kept = DRAWN.with(|d| d.borrow().get(&px).cloned());
    if let Some(pic) = kept {
        return ImageSource::Render(pic);
    }
    let Some(pic) = draw(LOGO, px) else {
        return "logo.svg".into();
    };
    let pic = Arc::new(pic);
    DRAWN.with(|d| {
        let mut d = d.borrow_mut();
        if d.len() < MAX_KEPT {
            d.insert(px, pic.clone());
        }
    });
    ImageSource::Render(pic)
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
