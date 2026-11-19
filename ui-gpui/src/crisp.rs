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
/// Below this many pixels a side the tilted mark can't be drawn: its eyes (4 of 51) and smile (2.6) come out under a
/// pixel each, blended with the yellow into brown smudges, and its tilted edges into a blur: the status bar's 14 px mark
/// read pale and muddy beside the rail's (the owner, 2026-11-19: "logo bloxkeep ở đầu sau nhạt hơn hẳn so với logo
/// gốc"). There `small_mark` draws it on the pixel grid instead.
const SMALL_BELOW: u32 = 28;

/// The mark for `dots` pixels a side, built on whole pixels (as small icons are hinted): the same two blocks and face,
/// upright, each edge, eye and the smile's bar on a pixel line, so the yellow stays solid and the face dark. Laid out on
/// a 14-step grid (the status bar's 14 px at 100 %) and scaled to `dots`, rounded.
fn small_mark(dots: u32, back: u32) -> String {
    let n = dots as f32;
    let u = n / 14.;
    let r = |v: f32| v.round();
    let (sx, sh) = (r(4. * u), r(10. * u));
    let (fy, fw) = (r(3. * u), r(11. * u));
    let cx = fw / 2.;
    // (The smile one pixel thick at every size here: two made it a block, a mouth shouting.)
    let (ew, eh, t) = (r(u).max(1.), r(2. * u).max(2.), 1.);
    let (le, re) = (r(cx - 1.5 * u) - ew, r(cx + 1.5 * u));
    let ey = fy + r(3. * u);
    let (bl, br, by) = (r(cx - 2.5 * u), r(cx + 2.5 * u), fy + r(8. * u));
    let ink = "#15181e";
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {n} {n}"><rect x="{sx}" y="0" width="{sw}" height="{sh}" rx="{sr}" fill="#{back:06x}"/><rect x="0" y="{fy}" width="{fw}" height="{fh}" rx="{fr}" fill="#f5c933"/><g fill="{ink}" shape-rendering="crispEdges"><rect x="{le}" y="{ey}" width="{ew}" height="{eh}"/><rect x="{re}" y="{ey}" width="{ew}" height="{eh}"/><rect x="{bl}" y="{by}" width="{bw}" height="{t}"/><rect x="{blt}" y="{byt}" width="{t}" height="{t}"/><rect x="{br}" y="{byt}" width="{t}" height="{t}"/></g></svg>"##,
        sw = n - sx,
        sr = 2. * u,
        fh = n - fy,
        fr = 2.5 * u,
        bw = br - bl,
        blt = bl - t,
        byt = by - t,
    )
}

/// Sizes kept (the rail's, the lock screen's, each at the screens' scales): a size past that is drawn the old way rather
/// than kept forever.
const MAX_KEPT: usize = 12;

thread_local! {
    static DRAWN: RefCell<HashMap<u32, Arc<RenderImage>>> = RefCell::new(HashMap::new());
    /// The rail's mark by its size and its back block's colour (`back_block`).
    static MARKS: RefCell<HashMap<(u32, u32), Arc<RenderImage>>> = RefCell::new(HashMap::new());
    /// Pictures that were replaced (the hero at a new size) and are still on the windows' atlases: given back by `retired`.
    static RETIRED: RefCell<Vec<Arc<RenderImage>>> = const { RefCell::new(Vec::new()) };
}

/// Lets go of the pictures replaced since the last call, from every window's atlas. A window dragged wider asked for the
/// hero at a new size every frame, and each old one stayed on the atlas for good (a texture of the window's size each).
/// Called between frames, from the app's own timer (images.rs).
pub fn retired(cx: &mut gpui_kit::App) {
    let gone = RETIRED.with(|r| std::mem::take(&mut *r.borrow_mut()));
    for pic in gone {
        cx.drop_image(pic, None);
    }
}

/// The kept size nearest `dots`, for when the sizes kept are all taken: a size beyond them is shown from that one, as
/// the graphics card scales it, rather than drawn again every frame (each new picture took a place on the atlas for good).
fn nearest<V: Clone>(kept: &HashMap<u32, V>, dots: u32) -> Option<V> {
    kept.iter().min_by_key(|(k, _)| k.abs_diff(dots)).map(|(_, v)| v.clone())
}

/// The colour of the block behind the face (the logo's slate, 3b4250) made to stand apart from the rail it is drawn
/// on: on a theme whose rail is near that slate the block sank into it and only the yellow face was left (the owner,
/// 2026-11-18: "đôi khi chỉ còn mặt cười vàng"). Lightened on a dark rail, darkened on a light one, only as far as
/// it takes.
fn back_block() -> u32 {
    use crate::theme::{GROUND, RAIL_BG, c, contrast, over};
    let ground = over(c(RAIL_BG), Hsla { a: 1., ..c(GROUND) });
    let mut block: Hsla = rgb(0x3b4250).into();
    let step = if ground.l < 0.5 { 0.05 } else { -0.05 };
    for _ in 0..12 {
        if contrast(block, ground) >= 1.8 {
            break;
        }
        block.l = (block.l + step).clamp(0.04, 0.96);
    }
    let r = block.to_rgb();
    let dot = |v: f32| (v * 255.).round().clamp(0., 255.) as u32;
    (dot(r.r) << 16) | (dot(r.g) << 8) | dot(r.b)
}

/// The logo's picture at `dots` pixels a side: kept, or drawn now.
fn picture(dots: u32) -> Option<Arc<RenderImage>> {
    let kept = DRAWN.with(|d| d.borrow().get(&dots).cloned());
    if kept.is_some() {
        return kept;
    }
    if DRAWN.with(|d| d.borrow().len() >= MAX_KEPT) {
        if let Some(near) = DRAWN.with(|d| nearest(&d.borrow(), dots)) {
            return Some(near);
        }
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
    let back = back_block();
    snapped(side, move |dots| {
        let kept = MARKS.with(|d| d.borrow().get(&(dots, back)).cloned());
        kept.or_else(|| {
            // (All the places taken: the nearest size of this colour is shown, not a new picture each frame.)
            let full = MARKS.with(|d| d.borrow().len() >= MAX_KEPT * 2);
            if full {
                let same: HashMap<u32, Arc<RenderImage>> = MARKS.with(|d| d.borrow().iter().filter(|((_, b), _)| *b == back).map(|((n, _), p)| (*n, p.clone())).collect());
                if let Some(near) = nearest(&same, dots) {
                    return Some(near);
                }
            }
            let svg = if dots < SMALL_BELOW { small_mark(dots, back) } else { String::from_utf8_lossy(MARK).replace("#3b4250", &format!("#{back:06x}")) };
            let pic = Arc::new(draw(svg.as_bytes(), dots)?);
            MARKS.with(|d| {
                let mut d = d.borrow_mut();
                if d.len() >= MAX_KEPT * 2 {
                    // Full with none of this colour: the others are an earlier theme's, let go (else this one would be
                    // drawn anew every frame).
                    let old: Vec<(u32, u32)> = d.keys().filter(|(_, b)| *b != back).copied().collect();
                    RETIRED.with(|r| r.borrow_mut().extend(old.iter().filter_map(|k| d.remove(k))));
                }
                if d.len() < MAX_KEPT * 2 {
                    d.insert((dots, back), pic.clone());
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
    draw_wh(svg, px, px)
}

/// Bloxkeep's banner, drawn to fill `w`×`h` logical px on this screen in the theme's own colours: the Home hero of a
/// vault that has played nothing yet (a game's picture was someone else's; the owner, 2026-11-18). Three windows, each
/// a scene (a city at dusk, mountains over water, a canyon under the sun in front), Bloxkeep's icon (logo.svg, untouched) at the front one's
/// corner: the owner's mock-up (2026-11-19, "đây chính là hero banner chính thức") that replaced the dark windows of
/// faces. Every colour comes from the theme's ground (a picture theme's colour under its picture) and its accent, so it
/// sits on any theme; checked on Sunset, Dark, Aurora, City, Halloween, Christmas, Rose, Snow and Light.
pub fn hero(w: f32, h: f32) -> Option<ImageSource> {
    use crate::theme::{FACE, GROUND, c};
    thread_local! {
        static KEPT: RefCell<Option<((u32, u32, u32, u32), Arc<RenderImage>)>> = const { RefCell::new(None) };
    }
    let ground = crate::theme::backdrop().map_or_else(|| c(GROUND), |b| b.under);
    let accent = c(FACE);
    let scale = crate::kit::screen_scale().max(1.);
    let key = ((w * scale).round().clamp(1., 4096.) as u32, (h * scale).round().clamp(1., 4096.) as u32, hex_of(ground), hex_of(accent));
    if let Some(pic) = KEPT.with(|k| k.borrow().as_ref().filter(|(at, _)| *at == key).map(|(_, p)| p.clone())) {
        return Some(ImageSource::Render(pic));
    }
    // Fitted, never cropped: the whole scene shows even where the box is shorter than 16:9.
    let pic = Arc::new(draw_fit(hero_svg(ground, accent).as_bytes(), key.0, key.1, false)?);
    if let Some((_, old)) = KEPT.with(|k| k.borrow_mut().replace((key, pic.clone()))) {
        RETIRED.with(|r| r.borrow_mut().push(old));
    }
    Some(ImageSource::Render(pic))
}

fn hex_of(x: Hsla) -> u32 {
    let r = x.to_rgb();
    let dot = |v: f32| (v * 255.).round().clamp(0., 255.) as u32;
    (dot(r.r) << 16) | (dot(r.g) << 8) | dot(r.b)
}

/// The banner's SVG for a theme. The back windows take the ground's hue (the accent's when the ground is grey); the
/// front scene runs from the accent toward the ground, round by red and purple for a warm accent and the short way for
/// a cool one (the short way turned warm canyons green); a light theme gets light steps.
fn hero_svg(ground: Hsla, accent: Hsla) -> String {
    use std::fmt::Write as _;
    let (gh, gs, gl) = (ground.h * 360., ground.s, ground.l);
    let ah = accent.h * 360.;
    let col = |h: f32, s: f32, l: f32| format!("#{:06x}", hex_of(hsla(h.rem_euclid(360.) / 360., s.clamp(0., 1.), l.clamp(0., 1.), 1.)));
    let mixh = |a: f32, b: f32, t: f32| a + ((b - a + 540.).rem_euclid(360.) - 180.) * t;
    let hb = if gs > 0.2 { gh } else { ah };
    let sb = (gs.max(0.22) * 0.85).clamp(0.12, 0.5);
    let dark = gl < 0.5;
    let lay = |t: f32| {
        if gs > 0.3 && dark {
            if !(75. ..=300.).contains(&ah) { ah - t * (ah - hb).rem_euclid(360.) } else { mixh(ah, hb, t) }
        } else {
            ah - t * 12.
        }
    };
    // Lightness of: frame, edge, title dots, sky, far, mid, near, lit windows, front sky, layers 1-4, river, sun.
    let l: [f32; 15] = if dark {
        [0.15, 0.40, 0.50, 0.42, 0.34, 0.27, 0.20, 0.72, 0.70, 0.55, 0.45, 0.35, 0.24, 0.62, 0.80]
    } else {
        [0.93, 0.72, 0.62, 0.84, 0.76, 0.68, 0.60, 0.97, 0.82, 0.72, 0.64, 0.56, 0.48, 0.90, 0.90]
    };
    let frame = col(hb, sb, l[0]);
    let edge = col(hb, sb + 0.15, l[1]);
    let dot = col(hb, sb + 0.1, l[2]);
    let sky = col(hb, sb + 0.05, l[3]);
    let far = col(hb, sb, l[4]);
    let mid = col(hb, sb, l[5]);
    let near = col(hb, sb, l[6]);
    let lit = col(ah, 0.85, l[7]);
    let fsky = col(ah, 0.8, l[8]);
    let (f1, f2, f3, f4) = (col(lay(0.3), 0.55, l[9]), col(lay(0.55), 0.45, l[10]), col(lay(0.8), 0.38, l[11]), col(hb, sb + 0.05, l[12]));
    let river = col(ah, 0.75, l[13]);
    let sun = col(ah + 12., 0.95, l[14]);
    let msun = col(ah + 12., 0.9, l[14] - 0.05);

    let mut city = format!(
        r#"<path d="M140 380 H190 V355 H240 V420 H140Z" fill="{far}"/><rect x="205" y="308" width="60" height="140" fill="{mid}"/><rect x="323" y="262" width="80" height="200" fill="{mid}"/><rect x="352" y="242" width="22" height="22" fill="{mid}"/><rect x="359" y="190" width="7" height="55" fill="{mid}"/><rect x="283" y="350" width="45" height="120" fill="{mid}"/><rect x="418" y="388" width="90" height="100" fill="{mid}"/><rect x="455" y="340" width="50" height="60" fill="{mid}"/><rect x="140" y="355" width="50" height="200" fill="{near}"/><rect x="230" y="395" width="70" height="160" fill="{near}"/><rect x="380" y="410" width="60" height="150" fill="{near}"/><rect x="500" y="370" width="230" height="190" fill="{far}"/>"#
    );
    for (x, y) in [(222, 328), (343, 281), (371, 281), (371, 313), (298, 375), (343, 375), (343, 408), (437, 407), (437, 437), (463, 437), (468, 362), (492, 362)] {
        let _ = write!(city, r#"<rect x="{x}" y="{y}" width="12" height="12" fill="{lit}"/>"#);
    }
    let _ = write!(city, r#"<path d="M140 470 L360 470 L440 445 L540 480 L730 470 V550 H140Z" fill="{near}"/>"#);
    let mut hills = format!(
        r#"<circle cx="1545" cy="230" r="34" fill="{msun}"/><path d="M1048 360 L1150 330 L1250 300 L1395 225 L1460 270 L1530 300 L1636 350 V460 H1048Z" fill="{far}"/><path d="M1300 270 L1395 225 L1440 255 L1395 260 L1360 280Z" fill="{edge}" opacity=".7"/><path d="M1048 410 L1170 360 L1280 385 L1390 330 L1470 375 L1560 355 L1636 380 V470 H1048Z" fill="{mid}"/><rect x="1048" y="460" width="588" height="95" fill="{near}"/>"#
    );
    for (x, y, w) in [(1295, 485, 120), (1430, 490, 110), (1520, 492, 90), (1315, 512, 90), (1435, 518, 140), (1380, 538, 150)] {
        let _ = write!(hills, r#"<rect x="{x}" y="{y}" width="{w}" height="11" rx="5.5" fill="{far}"/>"#);
    }
    let canyon = format!(
        r#"<circle cx="830" cy="488" r="40" fill="{sun}"/><path d="M533 560 L620 545 L700 560 L780 545 L830 575 L880 548 L960 560 L1040 540 L1237 552 V772 H533Z" fill="{f1}"/><path d="M533 505 H650 L660 535 L705 545 L720 600 L760 610 L790 650 L700 660 L600 640 L533 650Z" fill="{f2}"/><path d="M1237 430 H1075 L1060 470 L1045 520 L1005 545 L985 600 L940 625 L905 660 L1000 670 L1100 640 L1237 650Z" fill="{f2}"/><path d="M533 600 L600 595 L640 640 L700 660 L760 700 L800 772 H533Z" fill="{f3}"/><path d="M1237 560 L1150 570 L1100 600 L1040 640 L980 680 L940 720 L1000 772 H1237Z" fill="{f3}"/><path d="M800 690 C860 680 900 700 910 715 C960 720 1060 735 1110 772 H860 C840 740 800 720 760 705Z" fill="{river}"/><path d="M533 690 L600 680 L680 720 L740 772 H533Z" fill="{f4}"/><path d="M1237 680 L1150 690 L1090 730 L1060 772 H1237Z" fill="{f4}"/>"#
    );

    let mut o = String::with_capacity(10_000);
    let windows = [
        ((115, 85, 640, 490, 42), (140, 165, 590, 385), "ca", &sky, &city),
        ((1025, 85, 635, 495, 42), (1048, 165, 588, 387), "cc", &sky, &hills),
        ((505, 262, 760, 545, 44), (533, 347, 704, 425), "cb", &fsky, &canyon),
    ];
    for ((x, y, w, h, rx), (ix, iy, iw, ih), id, top, scene) in windows {
        let _ = write!(
            o,
            r#"<clipPath id="{id}"><rect x="{ix}" y="{iy}" width="{iw}" height="{ih}" rx="26"/></clipPath><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{frame}" stroke="{edge}" stroke-width="6"/>"#
        );
        for i in 0..3 {
            let _ = write!(o, r#"<circle cx="{}" cy="{}" r="13" fill="{dot}"/>"#, x + 52 + i * 43, y + 42);
        }
        let _ = write!(o, r#"<g clip-path="url(#{id})"><rect x="{ix}" y="{iy}" width="{iw}" height="{ih}" fill="{top}"/>{scene}</g>"#);
    }
    // (No Bloxkeep icon on it: the owner took it off, 2026-11-19, "bỏ logo bloxkeep ở đây luôn đi cũng được"; the
    // hero's words already carry the mark.)
    // 16:9 like the banner's box, round all three windows with room to spare (they span x 112–1663, y 82–810): a 2:1
    // view laid over the 16:9 box cut the side windows off (the owner, 2026-11-19: "sao bạn lại ăn bớt vậy?").
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="52 -24 1671 940">{o}</svg>"#)
}

/// An SVG drawn to cover `w`×`h` pixels: at one scale both ways, its middle kept (a square one in a square, whole).
fn draw_wh(svg: &[u8], w: u32, h: u32) -> Option<RenderImage> {
    draw_fit(svg, w, h, true)
}

/// `svg` drawn `w`×`h` in the middle: `cover` fills the box (its overflow cut off), else the whole picture fits in it.
fn draw_fit(svg: &[u8], w: u32, h: u32, cover: bool) -> Option<RenderImage> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let (sx, sy) = (w as f32 / size.width(), h as f32 / size.height());
    let s = if cover { sx.max(sy) } else { sx.min(sy) };
    let (dx, dy) = ((w as f32 - size.width() * s) / 2., (h as f32 - size.height() * s) / 2.);
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s).post_translate(dx, dy), &mut pixmap.as_mut());
    let px = w;
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
    let buffer = image::RgbaImage::from_raw(px, h, dots)?;
    Some(RenderImage::new(vec![image::Frame::new(buffer)]))
}
