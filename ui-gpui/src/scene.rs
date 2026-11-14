//! The picture behind the window on a picture theme (the web page's scene-*.svg and photo-*.jpg, built into the
//! program; the custom theme's own picture from the app's pictures folder), drawn at the window's exact pixels off the
//! UI thread and kept for that theme and size. The one it replaces is taken off the graphics card. Settings' theme
//! tiles get small copies the same way. The web page moved some scenes (livescene.js); here they stand still. Where the
//! web page frosted its panels (backdrop-filter: blur, saturate), a frosted copy is drawn with it, and each see-through
//! panel paints the part of it that is behind that panel (`frost`).
use std::{collections::HashMap, sync::Arc};

use gpui_kit::{prelude::*, *};

use crate::{shell::Root, theme::Backdrop};

/// What a picture is drawn for: the file, its pixels, the blur (device px), the veil over it, and the frosted copy's blur
/// (device px, 0: none) and saturate (%).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Key {
    file: String,
    w: u32,
    h: u32,
    blur: u32,
    veil: Option<u32>,
    frost: u32,
    sat: u32,
}

/// Tiles kept before they are all let go (98 themes, a few sizes).
const MAX_THUMBS: usize = 128;

#[derive(Default)]
struct Scene {
    /// The picture behind the window (and its frosted copy), and the one being drawn now (only one at a time: a resize
    /// asks again once it's done).
    shown: Option<(Key, Arc<RenderImage>, Option<Arc<RenderImage>>)>,
    drawing: Option<Key>,
    /// The last picture that couldn't be drawn (a custom theme's file gone or unreadable): not asked again for that key,
    /// or every frame started another read and decode of it.
    failed: Option<Key>,
    thumbs: HashMap<Key, Option<Arc<RenderImage>>>,
}

impl Global for Scene {}

/// The window's background layers for the theme in use, under everything else: the picture's colour, the picture, and
/// the theme's wash over it. None on a theme without a picture (the root's own ground shows).
pub fn layer(window: &mut Window, cx: &mut Context<Root>) -> Option<AnyElement> {
    let Some(b) = crate::theme::backdrop() else {
        // Back to a plain theme: the picture is let go.
        if let Some((_, old, frosted)) = cx.default_global::<Scene>().shown.take() {
            cx.drop_image(old, Some(window));
            if let Some(f) = frosted {
                cx.drop_image(f, Some(window));
            }
        }
        return None;
    };
    let key = wanted(&b, window);
    let pic = if b.file.is_empty() { None } else { picture(key, &b, window, cx) };
    Some(
        div()
            .absolute()
            .inset_0()
            .bg(b.under)
            .when_some(pic, |d, p| d.child(img(ImageSource::Render(p)).absolute().inset_0().size_full().object_fit(ObjectFit::Cover)))
            .child(div().absolute().inset_0().bg(b.wash))
            .into_any_element(),
    )
}

/// What the window wants drawn for backdrop `b` at its size now.
fn wanted(b: &Backdrop, window: &Window) -> Key {
    let scale = window.scale_factor();
    let size = window.viewport_size();
    Key {
        file: b.file.clone(),
        w: (f32::from(size.width) * scale).round().clamp(1., 8192.) as u32,
        h: (f32::from(size.height) * scale).round().clamp(1., 8192.) as u32,
        blur: (b.blur * scale).round() as u32,
        veil: b.veil.map(pack),
        frost: b.frost.map_or(0, |(r, _)| (r * scale).round().max(1.) as u32),
        sat: b.frost.map_or(100, |(_, s)| (s * 100.).round() as u32),
    }
}

/// The same picture as `b` (maybe at another size: it shows, stretched, until that one is drawn).
fn alike(a: &Key, b: &Key) -> bool {
    a.file == b.file && a.blur == b.blur && a.veil == b.veil && a.frost == b.frost && a.sat == b.sat
}

/// Fills its parent, a see-through panel with corners of `radius`, with what the web page's backdrop-filter showed
/// through it: the frosted picture, placed where the picture lies behind the window, and the theme's wash over it. None
/// where the web page left the picture sharp (or until the frosted copy is drawn).
pub fn frost(radius: f32, window: &Window, cx: &App) -> Option<AnyElement> {
    let b = crate::theme::backdrop()?;
    b.frost?;
    let key = wanted(&b, window);
    let frosted = cx.try_global::<Scene>()?.shown.as_ref().filter(|(k, _, _)| alike(k, &key))?.2.clone()?;
    let wash = b.wash;
    Some(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let r = Corners::all(px(radius));
                // Where the picture behind the window lies (object-fit: cover over the whole window).
                let (scale, view) = (window.scale_factor(), window.viewport_size());
                let dots = frosted.size(0);
                let (iw, ih) = (dots.width.0.max(1) as f32 / scale, dots.height.0.max(1) as f32 / scale);
                let (vw, vh) = (f32::from(view.width), f32::from(view.height));
                let s = (vw / iw).max(vh / ih);
                let at = Bounds::new(point(px((vw - iw * s) / 2.), px((vh - ih * s) / 2.)), size(px(iw * s), px(ih * s)));
                let _ = window.paint_image(bounds, at, r, frosted.clone(), 0, false);
                window.paint_quad(fill(bounds, wash).corner_radii(r));
            },
        )
        .absolute()
        .inset_0()
        .into_any_element(),
    )
}

/// The picture for `key`: kept, or being drawn (the last one meanwhile when it is the same file at another size).
fn picture(key: Key, b: &Backdrop, window: &mut Window, cx: &mut Context<Root>) -> Option<Arc<RenderImage>> {
    let s = cx.default_global::<Scene>();
    let same_file = s.shown.as_ref().filter(|(k, _, _)| alike(k, &key)).map(|(_, p, _)| p.clone());
    if s.shown.as_ref().is_some_and(|(k, _, _)| *k == key) {
        return same_file;
    }
    if s.drawing.is_none() && s.failed.as_ref() != Some(&key) {
        s.drawing = Some(key.clone());
        let task = draw_task(key.clone(), b.under, b.veil, cx);
        cx.spawn_in(window, async move |this, cx| {
            let drawn = task.await;
            let _ = this.update_in(cx, |_, window, cx| {
                let s = cx.default_global::<Scene>();
                s.drawing = None;
                // (Kept even when another size is wanted by now: it shows, stretched, until that one is drawn.)
                let old = match drawn {
                    Some((p, f)) => {
                        s.failed = None;
                        s.shown.replace((key, Arc::new(p), f.map(Arc::new)))
                    }
                    None => {
                        s.failed = Some(key);
                        None
                    }
                };
                if let Some((_, old, frosted)) = old {
                    cx.drop_image(old, Some(window));
                    if let Some(f) = frosted {
                        cx.drop_image(f, Some(window));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    same_file
}

/// A theme tile's picture, `w`×`h` device pixels; None until it is drawn (or when it can't be).
pub fn thumb(file: &str, under: Hsla, veil: Option<Hsla>, w: u32, h: u32, cx: &mut Context<Root>) -> Option<Arc<RenderImage>> {
    if file.is_empty() {
        return None;
    }
    let key = Key { file: file.to_string(), w, h, blur: 0, veil: veil.map(pack), frost: 0, sat: 100 };
    let s = cx.default_global::<Scene>();
    if let Some(kept) = s.thumbs.get(&key) {
        return kept.clone();
    }
    if s.thumbs.len() >= MAX_THUMBS {
        let gone: Vec<_> = s.thumbs.drain().filter_map(|(_, p)| p).collect();
        for p in gone {
            cx.drop_image(p, None);
        }
    }
    cx.default_global::<Scene>().thumbs.insert(key.clone(), None);
    let task = draw_task(key.clone(), under, veil, cx);
    cx.spawn(async move |this, cx| {
        let drawn = task.await.map(|(p, _)| p);
        let _ = this.update(cx, |_, cx| {
            if let (Some(p), Some(slot)) = (drawn, cx.default_global::<Scene>().thumbs.get_mut(&key)) {
                *slot = Some(Arc::new(p));
            }
            cx.notify();
        });
    })
    .detach();
    None
}

fn pack(h: Hsla) -> u32 {
    let c = h.to_rgb();
    let b = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
    (b(c.r) << 24) | (b(c.g) << 16) | (b(c.b) << 8) | b(c.a)
}

/// The picture drawn on the background executor: the file read (a user's picture from disk) and drawn there.
fn draw_task(key: Key, under: Hsla, veil: Option<Hsla>, cx: &mut Context<Root>) -> Task<Option<(RenderImage, Option<RenderImage>)>> {
    let user = if crate::theme::scene_bytes(&key.file).is_none() { crate::images::user(&key.file) } else { None };
    let (under, veil) = (under.to_rgb(), veil.map(|v| v.to_rgb()));
    cx.background_executor().spawn(async move {
        let bytes: std::borrow::Cow<'static, [u8]> = match crate::theme::scene_bytes(&key.file) {
            Some(b) => b.into(),
            None => std::fs::read(user?).ok()?.into(),
        };
        let pic = draw(&bytes, key.file.to_ascii_lowercase().ends_with(".svg"), key.w, key.h, key.blur, under, veil)?;
        let frosted = (key.frost > 0).then(|| frosted(&pic, key.frost, key.sat as f32 / 100.));
        Some((RenderImage::new(vec![image::Frame::new(pic)]), frosted.map(|f| RenderImage::new(vec![image::Frame::new(f)]))))
    })
}

/// The picture covering `w`×`h` (cut to fill, centred, as CSS's `center / cover`) over its colour, blurred, under the
/// veil; as GPUI keeps pictures (blue, green, red, alpha).
fn draw(bytes: &[u8], svg: bool, w: u32, h: u32, blur: u32, under: Rgba, veil: Option<Rgba>) -> Option<image::RgbaImage> {
    let mut pic = if svg {
        let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default()).ok()?;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
        let size = tree.size();
        let s = (w as f32 / size.width()).max(h as f32 / size.height());
        let (dx, dy) = ((w as f32 - size.width() * s) / 2., (h as f32 - size.height() * s) / 2.);
        resvg::render(&tree, resvg::tiny_skia::Transform::from_row(s, 0., 0., s, dx, dy), &mut pixmap.as_mut());
        // tiny-skia's pixels are premultiplied: back to straight alpha.
        let mut dots = pixmap.take();
        for p in dots.chunks_exact_mut(4) {
            let a = p[3];
            if a > 0 && a < 255 {
                for c in &mut p[..3] {
                    *c = ((*c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                }
            }
        }
        image::RgbaImage::from_raw(w, h, dots)?
    } else {
        let src = image::load_from_memory(bytes).ok()?.to_rgba8();
        let (sw, sh) = src.dimensions();
        if sw == 0 || sh == 0 {
            return None;
        }
        let s = (w as f32 / sw as f32).max(h as f32 / sh as f32);
        let (cw, ch) = (((w as f32 / s).round() as u32).clamp(1, sw), ((h as f32 / s).round() as u32).clamp(1, sh));
        let cut = image::imageops::crop_imm(&src, (sw - cw) / 2, (sh - ch) / 2, cw, ch).to_image();
        image::imageops::resize(&cut, w, h, image::imageops::FilterType::Triangle)
    };
    if blur > 0 {
        // Blurred small and brought back up: a strong blur at full size would take seconds. A slight one is done
        // at full size (shrinking alone would soften the picture more than the blur asked for).
        let f = (blur / 3).clamp(1, 4);
        let (sw, sh) = ((w / f).max(1), (h / f).max(1));
        let small = image::imageops::resize(&pic, sw, sh, image::imageops::FilterType::Triangle);
        let small = image::imageops::fast_blur(&small, (blur as f32 / f as f32).max(0.5));
        pic = image::imageops::resize(&small, w, h, image::imageops::FilterType::Triangle);
    }
    let to = |v: f32| (v.clamp(0., 1.) * 255.).round() as f32;
    let (ur, ug, ub) = (to(under.r), to(under.g), to(under.b));
    let veil = veil.map(|v| (to(v.r), to(v.g), to(v.b), v.a.clamp(0., 1.)));
    for p in pic.pixels_mut() {
        let a = p[3] as f32 / 255.;
        let mut px = [p[0] as f32 * a + ur * (1. - a), p[1] as f32 * a + ug * (1. - a), p[2] as f32 * a + ub * (1. - a)];
        if let Some((vr, vg, vb, va)) = veil {
            px = [px[0] * (1. - va) + vr * va, px[1] * (1. - va) + vg * va, px[2] * (1. - va) + vb * va];
        }
        // Blue first, as GPUI keeps them.
        *p = image::Rgba([px[2].round() as u8, px[1].round() as u8, px[0].round() as u8, 255]);
    }
    Some(pic)
}

/// The drawn picture as the web's backdrop-filter shows it through a panel: blurred (`sigma` device px, CSS blur's
/// standard deviation) and saturated (CSS saturate's matrix; 1 = as is). Blurred small and brought back up: a strong
/// blur at full size would take seconds, and it has no detail left to lose.
fn frosted(pic: &image::RgbaImage, sigma: u32, sat: f32) -> image::RgbaImage {
    let (w, h) = pic.dimensions();
    let f = (sigma / 3).clamp(1, 8);
    let (sw, sh) = ((w / f).max(1), (h / f).max(1));
    let small = image::imageops::resize(pic, sw, sh, image::imageops::FilterType::Triangle);
    let mut small = image::imageops::fast_blur(&small, (sigma as f32 / f as f32).max(0.5));
    if (sat - 1.).abs() > 0.001 {
        let m = [
            [0.213 + 0.787 * sat, 0.715 - 0.715 * sat, 0.072 - 0.072 * sat],
            [0.213 - 0.213 * sat, 0.715 + 0.285 * sat, 0.072 - 0.072 * sat],
            [0.213 - 0.213 * sat, 0.715 - 0.715 * sat, 0.072 + 0.928 * sat],
        ];
        for p in small.pixels_mut() {
            // (Blue first, as GPUI keeps them.)
            let (r, g, b) = (p[2] as f32, p[1] as f32, p[0] as f32);
            let ch = |row: [f32; 3]| (row[0] * r + row[1] * g + row[2] * b).round().clamp(0., 255.) as u8;
            *p = image::Rgba([ch(m[2]), ch(m[1]), ch(m[0]), 255]);
        }
    }
    image::imageops::resize(&small, w, h, image::imageops::FilterType::Triangle)
}
