// The overlay over a game, and the colour picker over a monitor (OverlayView.tsx, app.css .ov-*, part for part). Edit:
// a macro's points (numbered dots) and areas (labelled boxes with corner handles) over the game, dimmed; a dot is
// dragged, a box dragged by its inside and sized by its corners. Areas: the boxes only. Pick and Screen: the picture
// frozen, a magnifier at the pointer, a click takes the place (the core reads its colour from the same picture). A bar
// at the top says what it is for; Esc or Enter ends it. Places go to the core as parts of the picture area (0–1), when
// a drag is let go.
use std::sync::Arc;

use base64::Engine;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use super::{Body, ToolPage};
use crate::{kit::*, theme::*};

/// As the WinForms overlay had them: a corner's reach, a box's smallest side, a dot's radius.
const GRIP: f32 = 8.;
const MIN: f32 = 12.;
const DOT: f32 = 11.;
/// The magnifier: this many pixels each way around the pointer, each this big.
const SPAN: i32 = 9;
const ZOOM: f32 = 12.;

#[derive(Clone)]
struct Area {
    name: String,
    label: String,
    colour: u32,
    rect: [f32; 4],
}

#[derive(Clone, Copy, PartialEq)]
enum Grip {
    Move,
    Tl,
    Tr,
    Bl,
    Br,
}

#[derive(Clone, Copy)]
enum Drag {
    Dot { index: usize, moved: bool },
    Area { index: usize, grip: Grip, start: [f32; 4], from: Point<Pixels>, moved: bool },
}

pub struct Overlay {
    data: Value,
    dots: Vec<(i64, f32, f32)>,
    areas: Vec<Area>,
    drag: Option<Drag>,
    at: Option<Point<Pixels>>,
    cursor: CursorStyle,
    /// Pick and Screen: the picture (as drawn, and its pixels in BGRA for the magnifier), and its address it came as.
    picture: Option<(Arc<RenderImage>, image::RgbaImage)>,
    picture_src: String,
}

fn hex_of(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0xffc400)
}

/// Black or white words on a colour, whichever reads (as the WinForms overlay chose them).
fn ink(rgb: u32) -> Hsla {
    let (r, g, b) = ((rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    if (mx + mn) as f32 / 510. > 0.55 { rgb_hsla(0x000000) } else { rgb_hsla(0xffffff) }
}

fn rgb_hsla(rgb: u32) -> Hsla {
    gpui_kit::rgb(rgb).into()
}

fn round4(v: f32) -> f32 {
    (v * 10000.).round() / 10000.
}

/// The box after dragging a corner or its inside by dx, dy: inside the window, at least MIN each way (AreaGeometry.Drag).
fn drag_box(s: [f32; 4], grip: Grip, mut dx: f32, mut dy: f32, w: f32, h: f32) -> [f32; 4] {
    let [mut l, mut t, mut r, mut b] = s;
    if grip == Grip::Move {
        dx = dx.clamp(-l, w - r);
        dy = dy.clamp(-t, h - b);
        return [l + dx, t + dy, r + dx, b + dy];
    }
    if matches!(grip, Grip::Tl | Grip::Bl) {
        l = (l + dx).clamp(0., (r - MIN).max(0.));
    }
    if matches!(grip, Grip::Tr | Grip::Br) {
        r = (r + dx).clamp(l + MIN, w.max(l + MIN));
    }
    if matches!(grip, Grip::Tl | Grip::Tr) {
        t = (t + dy).clamp(0., (b - MIN).max(0.));
    }
    if matches!(grip, Grip::Bl | Grip::Br) {
        b = (b + dy).clamp(t + MIN, h.max(t + MIN));
    }
    [l, t, r, b]
}

impl Overlay {
    pub fn new(data: Value) -> Overlay {
        let mut o = Overlay { data: Value::Null, dots: Vec::new(), areas: Vec::new(), drag: None, at: None, cursor: CursorStyle::Crosshair, picture: None, picture_src: String::new() };
        o.set(data);
        o
    }

    /// The core's dots and areas; kept here while one is dragged (the core's next draft waits until it is let go).
    pub fn set(&mut self, data: Value) {
        if self.drag.is_none() {
            self.dots = data
                .get("dots")
                .and_then(Value::as_array)
                .map(|l| l.iter().map(|d| (d.get("step").and_then(Value::as_i64).unwrap_or(0), f(d, "x"), f(d, "y"))).collect())
                .unwrap_or_default();
            self.areas = data
                .get("areas")
                .and_then(Value::as_array)
                .map(|l| {
                    l.iter()
                        .map(|a| {
                            let r = a.get("rect").cloned().unwrap_or(Value::Null);
                            Area {
                                name: s(a, "name"),
                                label: s(a, "label"),
                                colour: hex_of(&s(a, "colour")),
                                rect: [f(&r, "x"), f(&r, "y"), f(&r, "w"), f(&r, "h")],
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
        }
        let src = s(&data, "picture");
        if src != self.picture_src {
            self.picture_src = src.clone();
            self.picture = decode(&src);
        }
        self.data = data;
    }

    fn mode(&self) -> &str {
        self.data.get("mode").and_then(Value::as_str).unwrap_or("edit")
    }
    fn picking(&self) -> bool {
        matches!(self.mode(), "pick" | "screen")
    }

    /// The colour of the picture's pixel under a point of the window (the picture is stretched over it).
    fn pixel_at(&self, p: Point<Pixels>, w: f32, h: f32) -> Option<(i32, i32, u32)> {
        let (_, px) = self.picture.as_ref()?;
        let (pw, ph) = (px.width() as i32, px.height() as i32);
        let x = ((f32::from(p.x) / w * pw as f32).floor() as i32).clamp(0, pw - 1);
        let y = ((f32::from(p.y) / h * ph as f32).floor() as i32).clamp(0, ph - 1);
        let c = px.get_pixel(x as u32, y as u32).0;
        Some((x, y, ((c[2] as u32) << 16) | ((c[1] as u32) << 8) | c[0] as u32))
    }

    /// What is under a point: a dot, else an area's corner or inside (the last drawn first).
    fn hit(&self, p: Point<Pixels>, w: f32, h: f32) -> Option<Drag> {
        let (x, y) = (f32::from(p.x), f32::from(p.y));
        for (i, &(_, dx, dy)) in self.dots.iter().enumerate().rev() {
            if ((x - dx * w).powi(2) + (y - dy * h).powi(2)).sqrt() <= DOT + 5. {
                return Some(Drag::Dot { index: i, moved: false });
            }
        }
        for (i, a) in self.areas.iter().enumerate().rev() {
            let [l, t, r, b] = [a.rect[0] * w, a.rect[1] * h, (a.rect[0] + a.rect[2]) * w, (a.rect[1] + a.rect[3]) * h];
            let near = |cx: f32, cy: f32| (x - cx).abs() <= GRIP && (y - cy).abs() <= GRIP;
            let grip = if near(l, t) {
                Some(Grip::Tl)
            } else if near(r, t) {
                Some(Grip::Tr)
            } else if near(l, b) {
                Some(Grip::Bl)
            } else if near(r, b) {
                Some(Grip::Br)
            } else if x >= l && x <= r && y >= t && y <= b {
                Some(Grip::Move)
            } else {
                None
            };
            if let Some(grip) = grip {
                return Some(Drag::Area { index: i, grip, start: [l, t, r, b], from: p, moved: false });
            }
        }
        None
    }
}

fn f(v: &Value, k: &str) -> f32 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.) as f32
}
fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// A PNG data address as a picture to draw and its pixels (both in BGRA, GPUI's order).
fn decode(src: &str) -> Option<(Arc<RenderImage>, image::RgbaImage)> {
    let b64 = src.split_once("base64,")?.1;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let mut px = image::load_from_memory(&bytes).ok()?.to_rgba8();
    for p in px.pixels_mut() {
        p.0.swap(0, 2);
    }
    Some((Arc::new(RenderImage::new(vec![image::Frame::new(px.clone())])), px))
}

fn ov(t: &mut ToolPage) -> Option<&mut Overlay> {
    match &mut t.body {
        Body::Overlay(o) => Some(o),
        _ => None,
    }
}

/// Esc or Enter ends it (Done; Cancel while picking).
pub fn key(t: &mut ToolPage, key: &str, _cx: &mut Context<ToolPage>) -> bool {
    if key != "escape" && key != "enter" {
        return false;
    }
    let link = t.link;
    if let Body::Overlay(o) = &t.body {
        link.act(if o.picking() { "cancel" } else { "done" }, json!({}));
    }
    true
}

fn down(t: &mut ToolPage, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<ToolPage>) {
    let link = t.link;
    let (w, h) = view(window);
    let Some(o) = ov(t) else { return };
    if o.picking() {
        if e.button == MouseButton::Right {
            link.act("cancel", json!({}));
        }
        return;
    }
    if e.button != MouseButton::Left {
        return;
    }
    o.drag = o.hit(e.position, w, h);
    cx.notify();
}

fn moved(t: &mut ToolPage, e: &MouseMoveEvent, window: &mut Window, cx: &mut Context<ToolPage>) {
    let (w, h) = view(window);
    let Some(o) = ov(t) else { return };
    if o.picking() {
        o.at = Some(e.position);
        cx.notify();
        return;
    }
    let (x, y) = (f32::from(e.position.x), f32::from(e.position.y));
    match o.drag {
        None => {
            let cursor = match o.hit(e.position, w, h) {
                Some(Drag::Dot { .. }) | Some(Drag::Area { grip: Grip::Move, .. }) => CursorStyle::ClosedHand,
                Some(Drag::Area { grip: Grip::Tl | Grip::Br, .. }) => CursorStyle::ResizeUpLeftDownRight,
                Some(Drag::Area { .. }) => CursorStyle::ResizeUpRightDownLeft,
                None => CursorStyle::Crosshair,
            };
            if cursor != o.cursor {
                o.cursor = cursor;
                cx.notify();
            }
        }
        Some(Drag::Dot { index, .. }) => {
            if let Some(d) = o.dots.get_mut(index) {
                d.1 = round4(x.clamp(0., w - 1.) / w);
                d.2 = round4(y.clamp(0., h - 1.) / h);
            }
            o.drag = Some(Drag::Dot { index, moved: true });
            cx.notify();
        }
        Some(Drag::Area { index, grip, start, from, .. }) => {
            let b = drag_box(start, grip, x - f32::from(from.x), y - f32::from(from.y), w, h);
            if let Some(a) = o.areas.get_mut(index) {
                a.rect = [round4(b[0] / w), round4(b[1] / h), round4((b[2] - b[0]) / w), round4((b[3] - b[1]) / h)];
            }
            o.drag = Some(Drag::Area { index, grip, start, from, moved: true });
            cx.notify();
        }
    }
}

fn up(t: &mut ToolPage, e: &MouseUpEvent, window: &mut Window, cx: &mut Context<ToolPage>) {
    let link = t.link;
    let (w, h) = view(window);
    let Some(o) = ov(t) else { return };
    if o.picking() {
        if e.button == MouseButton::Left {
            link.act("pick", json!({ "x": round4((f32::from(e.position.x) / w).clamp(0., 1.)), "y": round4((f32::from(e.position.y) / h).clamp(0., 1.)) }));
        }
        return;
    }
    match o.drag.take() {
        Some(Drag::Dot { index, moved: true }) => {
            if let Some(&(step, x, y)) = o.dots.get(index) {
                link.act("dot", json!({ "step": step, "x": x, "y": y }));
            }
        }
        Some(Drag::Area { index, moved: true, .. }) => {
            if let Some(a) = o.areas.get(index) {
                link.act("area", json!({ "name": a.name, "x": a.rect[0], "y": a.rect[1], "w": a.rect[2], "h": a.rect[3] }));
            }
        }
        _ => {}
    }
    cx.notify();
}

fn view(window: &Window) -> (f32, f32) {
    let v = window.viewport_size();
    (f32::from(v.width).max(1.), f32::from(v.height).max(1.))
}

pub fn render(t: &ToolPage, window: &mut Window, cx: &mut Context<ToolPage>) -> AnyElement {
    let Body::Overlay(o) = &t.body else { return div().into_any_element() };
    let link = t.link;
    let (w, h) = view(window);
    let picking = o.picking();
    let mode = o.mode().to_string();
    let name = s(&o.data, "name");

    let mut root = div()
        .id("ov")
        .size_full()
        .relative()
        .overflow_hidden()
        .cursor(if picking { CursorStyle::Crosshair } else { o.cursor })
        .on_mouse_down(MouseButton::Left, cx.listener(down))
        .on_mouse_down(MouseButton::Right, cx.listener(down))
        .on_mouse_move(cx.listener(moved))
        .on_mouse_up(MouseButton::Left, cx.listener(up));

    // The game dimmed (editing), or the frozen picture over it whole (picking).
    root = match (&o.picture, picking) {
        (Some((pic, _)), true) => root.child(img(pic.clone()).absolute().inset_0().size_full().object_fit(ObjectFit::Fill)),
        (None, true) => root.child(div().absolute().inset_0().bg(rgb_hsla(0x000000))),
        _ => root.child(div().absolute().inset_0().bg(hsla(0., 0., 0., 0.62))),
    };

    let held = o.drag;
    for (i, a) in o.areas.iter().enumerate() {
        let is_held = matches!(held, Some(Drag::Area { index, .. }) if index == i);
        let col = rgb_hsla(a.colour);
        let (l, tp, aw, ah) = (a.rect[0] * w, a.rect[1] * h, a.rect[2] * w, a.rect[3] * h);
        let grip = |x: f32, y: f32| {
            div().absolute().left(px(x - 4.)).top(px(y - 4.)).size(px(8.)).bg(rgb_hsla(0xffffff)).shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., 0.45),
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(1.),
                inset: false,
            }])
        };
        root = root.child(
            div()
                .absolute()
                .left(px(l))
                .top(px(tp))
                .w(px(aw))
                .h(px(ah))
                .border_2()
                .border_color(col)
                .bg(col.opacity(if is_held { 0.47 } else { 0.27 }))
                .child(
                    div()
                        .absolute()
                        .left(px(-2.))
                        .bottom_full()
                        .max_w(px(aw + 4.))
                        .px(px(5.))
                        .py(px(1.))
                        .truncate()
                        .text_size(px(T_XS))
                        .line_height(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .bg(col)
                        .text_color(ink(a.colour))
                        .child(a.label.clone()),
                )
                .child(grip(0., 0.))
                .child(grip(aw - 4., 0.).left(px(aw - 4. - 4.)))
                .child(grip(0., ah - 4.).top(px(ah - 4. - 4.)))
                .child(grip(aw - 4., ah - 4.).left(px(aw - 8.)).top(px(ah - 8.))),
        );
    }
    for (i, &(step, x, y)) in o.dots.iter().enumerate() {
        let is_held = matches!(held, Some(Drag::Dot { index, .. }) if index == i);
        root = root.child(
            div()
                .absolute()
                .left(px(x * w - DOT))
                .top(px(y * h - DOT))
                .size(px(DOT * 2.))
                .rounded_full()
                .border_2()
                .border_color(rgb_hsla(0xffffff))
                .bg(rgb_hsla(if is_held { 0xffc400 } else { 0x2563eb }))
                .text_color(rgb_hsla(if is_held { 0x000000 } else { 0xffffff }))
                .text_size(px(T_XS))
                .font_weight(FontWeight::BOLD)
                .flex()
                .items_center()
                .justify_center()
                .child((step + 1).to_string()),
        );
    }

    // What it is for, at the top (the app's own look, above the markings).
    let title = match mode.as_str() {
        "edit" => t.i18n.tv("ov.editing", &[("name", &if name.is_empty() { t.t("ov.newMacro") } else { name.clone() })]),
        "areas" => t.i18n.tv("ov.areas", &[("name", &name)]),
        "pick" => t.t("ov.pick"),
        _ => t.t("ov.screen"),
    };
    let hint = (!picking).then(|| t.t(if mode != "edit" { "ov.areasHint" } else if !o.dots.is_empty() || !o.areas.is_empty() { "ov.dragHint" } else { "ov.emptyHint" }));
    let button = if picking {
        Button::new("ov-cancel", t.t("ov.cancel")).small().quiet().on_click(move |_, _, _| link.act("cancel", json!({})))
    } else {
        Button::new("ov-done", t.t("ov.done")).small().primary().on_click(move |_, _, _| link.act("done", json!({})))
    };
    let bar = div()
        .id("ov-bar")
        .absolute()
        .top(px(8.))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            div()
                .id("ov-bar-in")
                .max_w(px((w - 16.).max(0.)))
                .flex()
                .items_center()
                .gap(px(12.))
                .pl(px(14.))
                .pr(px(5.))
                .py(px(5.))
                .rounded(px(R_M))
                .border_1()
                .border_color(c(LINE))
                .bg(crate::theme::solid_panel())
                .text_color(c(TEXT))
                .cursor(CursorStyle::Arrow)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_move(|_, _, cx| cx.stop_propagation())
                .child(div().min_w_0().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(title))
                .children(hint.filter(|_| w > 520.).map(|hn| div().flex_none().text_size(px(T_S)).text_color(c(TEXT_2)).child(hn)))
                .child(button),
        );
    root = root.child(bar);

    // The magnifier beside the pointer, on the other side when it would run off the window.
    if picking {
        if let (Some(at), Some((_, pixels))) = (o.at, o.picture.as_ref()) {
            if let Some((cx0, cy0, colour)) = o.pixel_at(at, w, h) {
                let lens_w = SPAN as f32 * ZOOM;
                let lens_h = lens_w + 28.;
                let (ax, ay) = (f32::from(at.x), f32::from(at.y));
                let left = if ax + 24. + lens_w > w { ax - 24. - lens_w } else { ax + 24. };
                let top = if ay + 24. + lens_h > h { ay - 24. - lens_h } else { ay + 24. };
                let half = (SPAN - 1) / 2;
                let mut grid = div().flex().flex_col().size(px(lens_w)).bg(rgb_hsla(0x000000));
                for yy in -half..=half {
                    let mut row = div().flex();
                    for xx in -half..=half {
                        let (x, y) = (cx0 + xx, cy0 + yy);
                        let col = if x >= 0 && y >= 0 && (x as u32) < pixels.width() && (y as u32) < pixels.height() {
                            let p = pixels.get_pixel(x as u32, y as u32).0;
                            rgb_hsla(((p[2] as u32) << 16) | ((p[1] as u32) << 8) | p[0] as u32)
                        } else {
                            rgb_hsla(0x000000)
                        };
                        row = row.child(div().size(px(ZOOM)).bg(col));
                    }
                    grid = grid.child(row);
                }
                let hex = format!("#{colour:06x}");
                root = root.child(
                    div()
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(lens_w + 2.))
                        .border_1()
                        .border_color(rgb_hsla(0x000000))
                        .bg(rgb_hsla(0x000000))
                        .shadow_lg()
                        .child(
                            div()
                                .relative()
                                .child(grid)
                                .child(div().absolute().left(px(half as f32 * ZOOM)).top(px(half as f32 * ZOOM)).size(px(ZOOM)).border_1().border_color(rgb_hsla(0xffffff))),
                        )
                        .child(div().h(px(28.)).flex().items_center().justify_center().bg(rgb_hsla(colour)).text_color(ink(colour)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).child(hex)),
                );
            }
        }
    }
    root.into_any_element()
}
