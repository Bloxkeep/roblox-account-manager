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
/// (device px, 0: none) and saturate (%). `whole`: the window's backdrop, drawn in the picture's own shape to cover
/// `w`×`h` and cut by the window as it shows it; else cut to `w`×`h` exactly (a theme's tile).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Key {
    file: String,
    whole: bool,
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
    /// The window as it looked before the theme now coming in, laid over it while it fades out (`fade_from`): the
    /// picture, when it was taken, and when the fade began (once the new theme's picture is drawn).
    fade: Option<(Arc<RenderImage>, std::time::Instant, Option<std::time::Instant>)>,
    /// The frosted picture's darkest and lightest parts, its mean, and the darkest and lightest parts of its top strip
    /// and of its foot (`extremes`), for `reading_veil`, `carries_words`, `top_reads` and `foot_reads`.
    extremes: Option<[f32; 7]>,
    drawing: Option<Key>,
    /// A theme's picture drawn ahead, while the mouse rests on its tile (`warm`): taken as it is picked, so the window
    /// changes at once instead of after the draw (the five scenes with a big blur take over a second at 2560 px), and
    /// the one being drawn so.
    warm: Option<(Key, Arc<RenderImage>, Option<Arc<RenderImage>>, Option<[f32; 7]>)>,
    warming: Option<Key>,
    /// When the picture drawn ahead was drawn: let go unpicked after WARM_KEEP.
    warm_at: Option<std::time::Instant>,
    /// The last picture that couldn't be drawn (a custom theme's file gone or unreadable): not asked again for that key,
    /// or every frame started another read and decode of it.
    failed: Option<Key>,
    thumbs: HashMap<Key, Option<Arc<RenderImage>>>,
    /// The custom theme's video playing behind the page, by its name: made once, let go with the theme.
    video: Option<(String, gpui_kit::Entity<crate::video::Video>)>,
    /// Wallpaper Engine's wallpaper playing live (wallpaper_engine.rs), by its window: made once per window.
    live: Option<(isize, gpui_kit::Entity<crate::video::Video>)>,
    /// What keeps an animated theme moving (live.rs), by its id: started with it, let go with it.
    ticker: Option<(&'static str, Task<()>)>,
    /// The pictures keep their pixels in memory too: one was wanted again where it wasn't (drawn in two windows, or
    /// the graphics card started over), and letting go again would draw it again and again.
    keep_pixels: bool,
    /// A picture of the theme was wanted again where its pixels weren't, once (see `lost`).
    lost_once: bool,
}

impl Global for Scene {}

/// The window's background layers for the theme in use, under everything else: the picture's colour, the picture, and
/// the theme's wash over it. None on a theme without a picture (the root's own ground shows).
pub fn layer(window: &mut Window, cx: &mut Context<Root>) -> Option<AnyElement> {
    // (A picture drawn ahead and not picked: let go, it is the size of the window's own.)
    let s = cx.default_global::<Scene>();
    if s.warm.is_some() && s.warm_at.is_some_and(|t| t.elapsed() >= WARM_KEEP) {
        if let Some((_, p, f, _)) = s.warm.take() {
            cx.drop_image(p, Some(window));
            if let Some(f) = f {
                cx.drop_image(f, Some(window));
            }
        }
    }
    let Some(b) = crate::theme::backdrop() else {
        // Back to a plain theme: the picture is let go, and a video's player with it.
        cx.default_global::<Scene>().video = None;
        cx.default_global::<Scene>().live = None;
        if cx.default_global::<Scene>().ticker.take().is_some() {
            crate::live::stop();
        }
        if let Some((_, old, frosted)) = cx.default_global::<Scene>().shown.take() {
            cx.drop_image(old, Some(window));
            if let Some(f) = frosted {
                cx.drop_image(f, Some(window));
            }
        }
        return None;
    };
    let key = wanted(&b, window);
    // (Wallpaper Engine's second copy plays at this size, and only while the window is seen: wallpaper_engine.rs.)
    if window.is_visible() {
        crate::wallpaper_engine::seen(key.w, key.h);
    }
    let pic =if b.file.is_empty() { None } else { picture(key, &b, window, cx) };
    // A video behind the page: its player kept while this video is the theme's, started the first time it is drawn.
    let clip = match &b.video {
        Some(name) => {
            let kept = cx.default_global::<Scene>().video.as_ref().filter(|(n, _)| n == name).map(|(_, v)| v.clone());
            match kept {
                Some(v) => Some(v),
                None => match crate::images::user(name) {
                    Some(path) => {
                        let v = cx.new(|cx| crate::video::Video::behind(path, cx));
                        cx.default_global::<Scene>().video = Some((name.clone(), v.clone()));
                        Some(v)
                    }
                    None => {
                        crate::net::note_failed("Theme video", name, "its file isn't in Bloxkeep's pictures folder");
                        None
                    }
                },
            }
        }
        None => {
            cx.default_global::<Scene>().video = None;
            None
        }
    };
    // Wallpaper Engine's wallpaper live (Settings › Appearance › Follow Wallpaper Engine, with Wallpaper Engine running):
    // its own window's picture over the theme's picture.
    // (Only behind the custom theme, whose backdrop the wallpaper is: with another theme picked while the following
    // was on, it went on playing behind that theme's glass, the owner's Sunset over autumn leaves, 2026-11-18.)
    let live = match crate::wallpaper_engine::live().filter(|_| crate::theme::showing_custom()) {
        Some(h) => {
            let kept = cx.default_global::<Scene>().live.as_ref().filter(|(k, _)| *k == h).map(|(_, v)| v.clone());
            Some(kept.unwrap_or_else(|| {
                let v = cx.new(|cx| crate::video::Video::live(h, cx));
                cx.default_global::<Scene>().live = Some((h, v.clone()));
                v
            }))
        }
        None => {
            cx.default_global::<Scene>().live = None;
            None
        }
    };
    // An animated theme: its moving things over the picture, kept drawing while the window is in use.
    let moving = crate::live::of(&b.file).filter(|_| b.video.is_none());
    if cx.default_global::<Scene>().ticker.as_ref().map(|(id, _)| *id) != moving {
        crate::live::stop();
        let ticker = moving.map(|id| (id, crate::live::ticker(window, cx)));
        cx.default_global::<Scene>().ticker = ticker;
    }
    Some(
        div()
            .absolute()
            .inset_0()
            .bg(b.under)
            .when_some(pic, |d, p| d.child(img(ImageSource::Render(p)).absolute().inset_0().size_full().object_fit(ObjectFit::Cover)))
            .when_some(clip, |d, v| d.child(div().absolute().inset_0().child(v)))
            .when_some(live, |d, v| d.child(div().absolute().inset_0().child(v)))
            .child(div().absolute().inset_0().bg(b.wash))
            // (Over the wash: the page's canvas was over the picture and its tint, lights stay bright.)
            .when_some(moving, |d, id| d.child(crate::live::layer(id)))
            .into_any_element(),
    )
}

/// Pictures whose pixels went to the graphics card and are wanted again where they aren't (another window, or the card
/// started over): the backdrop and the theme tiles among them are let go, and drawn again as the window next asks.
pub fn lost(ids: &[gpui_kit::ImageId], cx: &mut App) {
    let Some(s) = cx.try_global::<Scene>() else { return };
    let mine = |p: &Arc<RenderImage>| ids.contains(&p.id);
    let backdrop = s.shown.as_ref().is_some_and(|(_, p, f)| mine(p) || f.as_ref().is_some_and(mine));
    let tiles: Vec<Key> = s.thumbs.iter().filter(|(_, p)| p.as_ref().is_some_and(mine)).map(|(k, _)| k.clone()).collect();
    if !backdrop && tiles.is_empty() {
        return;
    }
    let s = cx.default_global::<Scene>();
    // (Lost once, as when the graphics card starts over, it is drawn again and lets go of its pixels again: kept for
    // good from the first loss, the theme's picture held 14.6 MB at 2560 px for the rest of the run, the owner's
    // error.log 2026-11-20. Lost again, it is wanted in two places, and kept.)
    if s.lost_once {
        s.keep_pixels = true;
    }
    s.lost_once = true;
    let mut gone = Vec::new();
    if backdrop {
        if let Some((_, p, f)) = s.shown.take() {
            gone.push(p);
            gone.extend(f);
        }
    }
    for k in tiles {
        if let Some(Some(p)) = s.thumbs.remove(&k) {
            gone.push(p);
        }
    }
    for p in gone {
        cx.drop_image(p, None);
    }
}

/// How long the old look takes to fade out over a new theme (the C# app's page: 0.2 s, ease-out).
const FADE: f32 = 0.2;
/// How long a new theme's picture is waited for before the fade starts without it (a slow file must not hold the pick).
/// (0.7 s first: Aurora's picture took longer at 2560 px, so the old look faded to a bare ground and the picture
/// popped in after it, the owner's recording of 2026-11-19. It comes at half size first now: `picture`.)
const FADE_WAIT: std::time::Duration = std::time::Duration::from_millis(1500);

/// Keeps the window as it looked in the frame last drawn, to fade out over the theme now coming in (called as the
/// next frame begins, before anything of it is drawn: theme::take_fade). The C# app's page changed themes so (a view
/// transition); the port swapped the look at once (the owner, 2026-11-18).
pub fn fade_from(window: &mut Window, cx: &mut App) {
    // (The frame last drawn, as GPUI gives it: red, green, blue; kept as GPUI draws pictures: blue, green, red.)
    let Ok(mut frame) = window.render_to_image() else { return };
    for dot in frame.chunks_exact_mut(4) {
        dot.swap(0, 2);
    }
    let pic = Arc::new(RenderImage::new(vec![image::Frame::new(frame)]));
    gpui_kit::label_picture(pic.id, "theme fade");
    if let Some((old, ..)) = cx.default_global::<Scene>().fade.replace((pic, std::time::Instant::now(), None)) {
        cx.drop_image(old, Some(window));
    }
}

/// The old look over the window while a theme fades in, drawn last; None once it is over. The fade starts when the
/// new theme's picture is drawn, so the picture doesn't pop in after it (app.js themePictureReady).
pub fn fade_layer(window: &mut Window, cx: &mut Context<Root>) -> Option<AnyElement> {
    cx.try_global::<Scene>()?.fade.as_ref()?;
    let ready = match crate::theme::backdrop() {
        Some(b) if !b.file.is_empty() => {
            let key = wanted(&b, window);
            let s = cx.default_global::<Scene>();
            s.shown.as_ref().is_some_and(|(k, _, _)| alike(k, &key)) || s.failed.as_ref() == Some(&key)
        }
        _ => true,
    };
    let s = cx.default_global::<Scene>();
    let (pic, asked, since) = s.fade.as_mut()?;
    if since.is_none() && (ready || asked.elapsed() >= FADE_WAIT) {
        *since = Some(std::time::Instant::now());
    }
    let k = since.map_or(0., |t| t.elapsed().as_secs_f32() / FADE);
    if k >= 1. {
        let (old, ..) = s.fade.take()?;
        cx.drop_image(old, Some(window));
        return None;
    }
    let pic = pic.clone();
    window.request_animation_frame();
    // (Ease-out: most of the change at once, the rest settling.)
    let left = (1. - k) * (1. - k);
    Some(img(ImageSource::Render(pic)).absolute().inset_0().size_full().opacity(left).into_any_element())
}

/// What the window wants drawn for backdrop `b` at its size now.
fn wanted(b: &Backdrop, window: &Window) -> Key {
    let scale = window.scale_factor();
    let size = window.viewport_size();
    // (Rounded up in steps of a quarter: a window dragged a little wider asks for no other picture.)
    let step = |dots: f32| {
        let mut to = 480f32;
        while to < dots && to < 8192. {
            to *= 1.25;
        }
        to.round().min(8192.) as u32
    };
    Key {
        file: b.file.clone(),
        whole: true,
        w: step(f32::from(size.width) * scale),
        h: step(f32::from(size.height) * scale),
        blur: (b.blur * scale).round() as u32,
        veil: b.veil.map(pack),
        frost: b.frost.map_or(0, |(r, _)| (r * scale).round().max(1.) as u32),
        sat: b.frost.map_or(100, |(_, s)| (s * 100.).round() as u32),
    }
}

/// The same picture as `b` (maybe at another size: it shows, cut by the window as the other will be, until that
/// one is drawn).
fn alike(a: &Key, b: &Key) -> bool {
    a.file == b.file && a.whole == b.whole && a.blur == b.blur && a.veil == b.veil && a.frost == b.frost && a.sat == b.sat
}

/// Fills its parent, a see-through panel with corners of `radius`, with what the web page's backdrop-filter showed
/// through it: the frosted picture, placed where the picture lies behind the window, and the theme's wash over it. None
/// where the web page left the picture sharp (or until the frosted copy is drawn).
pub fn frost(radius: f32, window: &Window, cx: &App) -> Option<AnyElement> {
    frost_corners(Corners::all(px(radius)), window, cx)
}

/// The same, each corner with its own radius (the status bar: round at its top only).
pub fn frost_corners(r: Corners<Pixels>, window: &Window, cx: &App) -> Option<AnyElement> {
    frost_under(r, crate::theme::c(crate::theme::PANEL), window, cx)
}

/// `frost_corners` under a piece whose own fill is `pane` (a panel's see-through fill; nothing for the top bar).
fn frost_under(r: Corners<Pixels>, pane: Hsla, window: &Window, cx: &App) -> Option<AnyElement> {
    let b = crate::theme::backdrop()?;
    b.frost?;
    let key = wanted(&b, window);
    let frosted = cx.try_global::<Scene>()?.shown.as_ref().filter(|(k, _, _)| alike(k, &key))?.2.clone()?;
    let wash = b.wash;
    // Under the panel's own fill (clear with glass on the blocks only), as much more of the theme's ground as its words
    // need on this picture: a custom theme's glass is as thin as its fade says, and at a low fade over a light picture
    // its words were lost (a fixed floor stood in for this while following Wallpaper Engine: too thick on a dark
    // wallpaper, the owner 2026-11-18, and not there at all without the following).
    let veil = reading_veil(&b, pane, cx);
    Some(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                // Where the picture behind the window lies (object-fit: cover over the whole window).
                let (scale, view) = (window.scale_factor(), window.viewport_size());
                let dots = frosted.size(0);
                let (iw, ih) = (dots.width.0.max(1) as f32 / scale, dots.height.0.max(1) as f32 / scale);
                let (vw, vh) = (f32::from(view.width), f32::from(view.height));
                let s = (vw / iw).max(vh / ih);
                let at = Bounds::new(point(px((vw - iw * s) / 2.), px((vh - ih * s) / 2.)), size(px(iw * s), px(ih * s)));
                let _ = window.paint_image(bounds, at, r, frosted.clone(), 0, false);
                window.paint_quad(fill(bounds, wash).corner_radii(r));
                if let Some(veil) = veil {
                    window.paint_quad(fill(bounds, veil).corner_radii(r));
                }
            },
        )
        .absolute()
        .inset_0()
        .into_any_element(),
    )
}

thread_local! {
    /// What is being painted lies on the page (between `on_page`'s two marks), with the picture behind it.
    static ON_PAGE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A mark painted before the page's panels (`on`) and one after them: a glass block (`frost_lazy`) between the two has
/// the picture behind it. What is painted after (a dialog, a menu, a popup: all drawn over the page later) has a box
/// behind it, and a frosted picture there read as a hole cut through the box.
pub fn on_page(on: bool) -> AnyElement {
    canvas(|_, _, _| {}, move |_, _, _, _| ON_PAGE.with(|p| p.set(on))).absolute().size_0().into_any_element()
}

/// A frosted picture's darkest and lightest parts: the luminance (0 to 1, as WCAG's contrast takes it) of the darkest
/// and the lightest of its 16×9 fields, each field its dots' mean; and the mean of them all. A block of glass is
/// about a field's size, so the lightest field is the worst ground white words can meet on it. Then the same two of
/// the top row of fields alone (what the top bar stands on), and of the last row (the lock screen's foot line).
fn extremes(pic: &image::RgbaImage) -> [f32; 7] {
    const COLS: u32 = 16;
    const ROWS: u32 = 9;
    let (w, h) = pic.dimensions();
    let lin = |v: u8| {
        let v = v as f32 / 255.;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let (mut low, mut high, mut all, mut fields) = (1f32, 0f32, 0f32, 0u32);
    let (mut top_low, mut top_high) = (1f32, 0f32);
    let (mut foot_low, mut foot_high) = (1f32, 0f32);
    for row in 0..ROWS {
        for col in 0..COLS {
            let (x0, x1) = (w * col / COLS, (w * (col + 1) / COLS).max(w * col / COLS + 1).min(w));
            let (y0, y1) = (h * row / ROWS, (h * (row + 1) / ROWS).max(h * row / ROWS + 1).min(h));
            let (mut sum, mut n) = (0f32, 0u32);
            // (Every fourth dot each way: the picture is blurred, its neighbours say the same.)
            for y in (y0..y1).step_by(4) {
                for x in (x0..x1).step_by(4) {
                    // (As GPUI keeps pictures: blue, green, red.)
                    let p = pic.get_pixel(x, y).0;
                    sum += 0.0722 * lin(p[0]) + 0.7152 * lin(p[1]) + 0.2126 * lin(p[2]);
                    n += 1;
                }
            }
            if n > 0 {
                let mean = sum / n as f32;
                (low, high) = (low.min(mean), high.max(mean));
                if row == 0 {
                    (top_low, top_high) = (top_low.min(mean), top_high.max(mean));
                }
                if row == ROWS - 1 {
                    (foot_low, foot_high) = (foot_low.min(mean), foot_high.max(mean));
                }
                all += mean;
                fields += 1;
            }
        }
    }
    [low.min(high), high, if fields > 0 { all / fields as f32 } else { high }, top_low.min(top_high), top_high, foot_low.min(foot_high), foot_high]
}

/// A grey of luminance `lum`, standing for the picture where it is that light.
fn grey_of(lum: f32) -> Hsla {
    let l = if lum <= 0.0031308 { lum * 12.92 } else { 1.055 * lum.powf(1. / 2.4) - 0.055 };
    Hsla { h: 0., s: 0., l: l.clamp(0., 1.), a: 1. }
}

/// Whether the theme's words read on the picture's top strip under the theme's wash alone (4.5:1 against its worst
/// part): the top bar stands there with nothing under it. True until the picture is drawn and on a theme without one.
pub fn top_reads(cx: &App) -> bool {
    strip_reads(3, cx)
}

/// The same for the picture's foot: the lock screen's line of who this is stands there.
pub fn foot_reads(cx: &App) -> bool {
    strip_reads(5, cx)
}

/// `at`: where in `extremes` the strip's darkest part is (its lightest follows).
fn strip_reads(at: usize, cx: &App) -> bool {
    use crate::theme::{TEXT, c, contrast, over};
    let Some(b) = crate::theme::backdrop() else { return true };
    let Some(e) = cx.try_global::<Scene>().and_then(|s| s.extremes) else { return true };
    let worst = if crate::theme::light() { e[at] } else { e[at + 1] };
    contrast(c(TEXT), over(b.wash, grey_of(worst))) >= 4.5
}

/// The top bar's glass, where its words don't read on the picture (`top_reads`): the frosted picture and what its
/// words need of the theme's ground, round at its foot. None where they read, and where the theme frosts nothing.
pub fn top_glass(window: &Window, cx: &App) -> Option<AnyElement> {
    if top_reads(cx) {
        return None;
    }
    let foot = px(crate::theme::R_M);
    frost_under(Corners { top_left: px(0.), top_right: px(0.), bottom_left: foot, bottom_right: foot }, Hsla::transparent_black(), window, cx)
}

/// Whether the theme's words read standing on the picture itself, under the theme's wash alone (4.5:1 against the
/// picture's mean): a page's title and headings and the rail's icons do, with glass on the blocks only. True until the
/// picture is drawn, and on a theme without one.
pub fn carries_words(cx: &App) -> bool {
    use crate::theme::{TEXT, c, contrast, over};
    let Some(b) = crate::theme::backdrop() else { return true };
    let Some(e) = cx.try_global::<Scene>().and_then(|s| s.extremes) else { return true };
    contrast(c(TEXT), over(b.wash, grey_of(e[2]))) >= 4.5
}

/// What glass lays over the frosted picture besides the theme's wash and its own `pane`, so the theme's words read on
/// it: more of the theme's ground, only as much as it takes for 4.5:1 against the picture's worst part (its lightest
/// under light words, its darkest under dark ones). A theme made thin (the custom one at 0 % fade, over a sunlit
/// wallpaper) left a block as a tenth of white on the bare picture (the owner, 2026-11-18: "mù mắt"); a dark picture
/// needs nothing more and keeps its look (Sunset).
fn reading_veil(b: &crate::theme::Backdrop, pane: Hsla, cx: &App) -> Option<Hsla> {
    use crate::theme::{MUTED, TEXT, c, contrast, over};
    let e = cx.try_global::<Scene>()?.extremes?;
    let picture = grey_of(if crate::theme::light() { e[0] } else { e[1] });
    let ground = Hsla { a: 1., ..b.wash };
    // The words at 4.5:1, and the quiet ones (a chart's hours, a figure's label) at 3:1 at least: held to the words
    // alone, a block lying over Sunset's sun kept its title while the hours under its chart were gone (the shots of
    // every theme and size, 2026-11-18).
    let reads = |veil: f32| {
        let under = over(pane, over(Hsla { a: veil, ..ground }, over(b.wash, picture)));
        contrast(c(TEXT), under) >= 4.5 && contrast(Hsla { a: 1., ..c(MUTED) }, under) >= 3.
    };
    if reads(0.) {
        return None;
    }
    let mut veil = 0.04;
    while veil < 0.9 && !reads(veil) {
        veil += 0.04;
    }
    Some(Hsla { a: veil, ..ground })
}

/// `frost` for a piece built without the window at hand (kit::surface): the frosted picture is looked for as it is
/// painted, the theme's wash and the block's own pane laid over it. `pane(true)` is that pane, asked for as it is
/// painted (a pressable block's brightens under the mouse); `pane(false)` the block's plain ground, all that is
/// painted when it is not on the page (`on_page`).
pub fn frost_lazy(r: Corners<Pixels>, pane: impl Fn(bool) -> Hsla + 'static) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, cx| {
            let Some(b) = crate::theme::backdrop() else { return };
            if !ON_PAGE.with(|p| p.get()) {
                window.paint_quad(fill(bounds, pane(false)).corner_radii(r));
                return;
            }
            if let Some(frosted) = cx.try_global::<Scene>().and_then(|s| s.shown.as_ref()).and_then(|(_, _, f)| f.clone()) {
                let (scale, view) = (window.scale_factor(), window.viewport_size());
                let dots = frosted.size(0);
                let (iw, ih) = (dots.width.0.max(1) as f32 / scale, dots.height.0.max(1) as f32 / scale);
                let (vw, vh) = (f32::from(view.width), f32::from(view.height));
                let s = (vw / iw).max(vh / ih);
                let at = Bounds::new(point(px((vw - iw * s) / 2.), px((vh - ih * s) / 2.)), size(px(iw * s), px(ih * s)));
                let _ = window.paint_image(bounds, at, r, frosted, 0, false);
            }
            window.paint_quad(fill(bounds, b.wash).corner_radii(r));
            let pane = pane(true);
            if let Some(veil) = reading_veil(&b, pane, cx) {
                window.paint_quad(fill(bounds, veil).corner_radii(r));
            }
            window.paint_quad(fill(bounds, pane).corner_radii(r));
        },
    )
    .absolute()
    .inset_0()
    .into_any_element()
}

/// The picture for `key`: kept, or being drawn (the last one meanwhile when it is the same file at another size).
fn picture(key: Key, b: &Backdrop, window: &mut Window, cx: &mut Context<Root>) -> Option<Arc<RenderImage>> {
    let s = cx.default_global::<Scene>();
    let same_file = s.shown.as_ref().filter(|(k, _, _)| alike(k, &key)).map(|(_, p, _)| p.clone());
    // (The one kept is this picture at this size or a bigger one: shown as it is, the window cuts it. Made small
    // again after being big, the window asks for nothing.)
    if s.shown.as_ref().is_some_and(|(k, _, _)| alike(k, &key) && k.w >= key.w && k.h >= key.h) {
        return same_file;
    }
    // Drawn ahead as its tile was pointed at: taken now.
    if s.warm.as_ref().is_some_and(|(k, ..)| alike(k, &key) && k.w >= key.w && k.h >= key.h) {
        let (k, p, f, extremes) = s.warm.take()?;
        s.extremes = extremes;
        let pic = p.clone();
        if let Some((_, old, frosted)) = s.shown.replace((k, p, f)) {
            cx.drop_image(old, Some(window));
            if let Some(f) = frosted {
                cx.drop_image(f, Some(window));
            }
        }
        return Some(pic);
    }
    // (Being drawn ahead: waited for, not drawn twice.)
    if s.warming.as_ref().is_some_and(|k| alike(k, &key) && k.w >= key.w && k.h >= key.h) {
        return same_file;
    }
    // A picture not shown yet comes at half size first (a quarter of the dots: Aurora's took ~1.4 s whole at 2560 px,
    // the owner's recording of 2026-11-19), shown stretched while the whole one is drawn after it.
    let key = if same_file.is_none() && key.w > 960 { Key { w: key.w / 2, h: key.h / 2, ..key } } else { key };
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
                    Some((p, f, extremes)) => {
                        s.failed = None;
                        s.extremes = extremes;
                        s.shown.replace((key, Arc::new(p), f.map(Arc::new)))
                    }
                    None => {
                        crate::net::note_failed("Theme picture", &key.file, "couldn't be read or drawn");
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

/// How long a picture drawn ahead is kept unpicked.
const WARM_KEEP: std::time::Duration = std::time::Duration::from_secs(20);

/// Draws theme `id`'s picture ahead for the window as it is now (the mouse rests on its tile in Appearance), so a
/// pick shows it at once. One at a time; the last one drawn so is kept until it is picked or another is drawn.
pub fn warm(id: &str, window: &mut Window, cx: &mut Context<Root>) {
    let Some(b) = crate::theme::palette_of(id).backdrop else { return };
    if b.file.is_empty() || b.video.is_some() {
        return;
    }
    let key = wanted(&b, window);
    let s = cx.default_global::<Scene>();
    let has = |k: &Key| alike(k, &key) && k.w >= key.w && k.h >= key.h;
    if s.shown.as_ref().is_some_and(|(k, ..)| has(k)) || s.warm.as_ref().is_some_and(|(k, ..)| has(k)) || s.warming.is_some() || s.failed.as_ref() == Some(&key) {
        return;
    }
    s.warming = Some(key.clone());
    let task = draw_task(key.clone(), b.under, b.veil, cx);
    cx.spawn_in(window, async move |this, cx| {
        let drawn = task.await;
        let _ = this.update_in(cx, |_, window, cx| {
            let s = cx.default_global::<Scene>();
            s.warming = None;
            let Some((p, f, extremes)) = drawn else { return };
            s.warm_at = Some(std::time::Instant::now());
            if let Some((_, old, frosted, _)) = s.warm.replace((key, Arc::new(p), f.map(Arc::new), extremes)) {
                cx.drop_image(old, Some(window));
                if let Some(f) = frosted {
                    cx.drop_image(f, Some(window));
                }
            }
            // (Picked meanwhile: the window takes it now.)
            cx.notify();
        });
        // (Looked at again once it may go: `layer` lets it go.)
        cx.background_executor().timer(WARM_KEEP + std::time::Duration::from_millis(50)).await;
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

/// A theme tile's picture, `w`×`h` device pixels; None until it is drawn (or when it can't be).
pub fn thumb(file: &str, under: Hsla, veil: Option<Hsla>, w: u32, h: u32, cx: &mut Context<Root>) -> Option<Arc<RenderImage>> {
    if file.is_empty() {
        return None;
    }
    let key = Key { file: file.to_string(), whole: false, w, h, blur: 0, veil: veil.map(pack), frost: 0, sat: 100 };
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
        let drawn = task.await.map(|(p, _, _)| p);
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
fn draw_task(key: Key, under: Hsla, veil: Option<Hsla>, cx: &mut Context<Root>) -> Task<Option<(RenderImage, Option<RenderImage>, Option<[f32; 7]>)>> {
    let user = if crate::theme::scene_bytes(&key.file).is_none() { crate::images::user(&key.file) } else { None };
    let (under, veil) = (under.to_rgb(), veil.map(|v| v.to_rgb()));
    let keep = cx.default_global::<Scene>().keep_pixels;
    let done = move |r: RenderImage, what: &'static str| {
        gpui_kit::label_picture(r.id, what);
        if keep { r } else { r.discard_after_upload() }
    };
    cx.background_executor().spawn(async move {
        #[cfg(feature = "core")]
        let started = std::time::Instant::now();
        let bytes: std::borrow::Cow<'static, [u8]> = match crate::theme::scene_bytes(&key.file) {
            Some(b) => b.into(),
            None => std::fs::read(user?).ok()?.into(),
        };
        let pic = draw(&bytes, key.file.to_ascii_lowercase().ends_with(".svg"), key.whole, key.w, key.h, key.blur, under, veil)?;
        let frosted = (key.frost > 0).then(|| frosted(&pic, key.frost, key.sat as f32 / 100.));
        let extremes = frosted.as_ref().map(extremes);
        // (A slow one is said: the fade waits for it, and a pick that waits long is felt.)
        #[cfg(feature = "core")]
        if key.whole && started.elapsed() >= std::time::Duration::from_millis(1200) {
            crate::core::log::note_file(&format!("Theme picture {} took {} ms to draw at {}x{}", key.file, started.elapsed().as_millis(), key.w, key.h));
        }
        // (Their pixels go once they are on the graphics card: kept there, not twice. Lost there, `lost` draws them again.)
        Some((
            done(RenderImage::new(vec![image::Frame::new(pic)]), if keep { "theme picture (kept)" } else { "theme picture" }),
            frosted.map(|f| done(RenderImage::new(vec![image::Frame::new(f)]), if keep { "theme frost (kept)" } else { "theme frost" })),
            extremes,
        ))
    })
}

/// The picture covering `w`×`h` (cut to fill, centred, as CSS's `center / cover`) over its colour, blurred, under the
/// veil; as GPUI keeps pictures (blue, green, red, alpha).
///
/// `whole`: not cut to `w`×`h` but drawn in its own shape at the size that covers them, for the window to cut as it
/// shows it (each side at most 1.6 times the window's: a very wide or tall picture is cut that far, in its middle).
/// Cut to the window's exact size, every size had a picture framed its own way, and a window changing size showed
/// the old one stretched and then jumped to the new one (the owner's recording, 2026-11-18).
fn draw(bytes: &[u8], svg: bool, whole: bool, w: u32, h: u32, blur: u32, under: Rgba, veil: Option<Rgba>) -> Option<image::RgbaImage> {
    // The size drawn: `w`×`h`, or the whole picture's at the scale `s` that covers them.
    let drawn = |sw: f32, sh: f32, s: f32| -> (u32, u32) {
        if !whole {
            return (w, h);
        }
        let most = |side: f32, window: u32| (side.round() as u32).clamp(window, (window as f32 * 1.6).round() as u32);
        (most(sw * s, w), most(sh * s, h))
    };
    let mut pic = if svg {
        let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default()).ok()?;
        let size = tree.size();
        let s = (w as f32 / size.width()).max(h as f32 / size.height());
        let (w, h) = drawn(size.width(), size.height(), s);
        let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
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
        let (w, h) = drawn(sw as f32, sh as f32, s);
        let (cw, ch) = (((w as f32 / s).round() as u32).clamp(1, sw), ((h as f32 / s).round() as u32).clamp(1, sh));
        let cut = image::imageops::crop_imm(&src, (sw - cw) / 2, (sh - ch) / 2, cw, ch).to_image();
        image::imageops::resize(&cut, w, h, image::imageops::FilterType::Triangle)
    };
    let (w, h) = pic.dimensions();
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
/// standard deviation) and saturated (CSS saturate's matrix; 1 = as is). Blurred small and kept small: a strong blur
/// at full size would take seconds, it has no detail left to lose, and the graphics card stretches it as it draws it
/// (brought back up to the window's size it took ~20 MB of memory for the same look; placed by its shape alone, `frost`).
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
    small
}
