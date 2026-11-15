// The controls every screen uses, one way each, as ui/src/kit.tsx is for the web page: Button, IconButton, Switch,
// Check, Badge, Segmented, Tab, the text Field and icons. A screen never draws its own button or switch.
//
// Every control is a tab stop: Tab and Shift+Tab walk them (main.rs binds the keys), Enter or Space presses the one
// that has the focus, and the focused one shows a light ring, only when the keyboard put it there.
//
// They behave as the web page's did (RobloxManager/wwwroot/app.css, "controls" and "Bloxkeep's springs"): a ground
// and an edge that ease in under the mouse instead of switching, a button that sinks when pressed and springs back
// past where it rests, a switch whose knob stretches toward where it goes and bounces into place, a text box whose
// edge lights up when you type in it. The colours and sizes are this interface's own (theme.rs).
use gpui_kit::base::motion::{Presence, Transition as Move};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::{prelude::*, *};

use crate::theme::*;

// ───────────────────────── touch ─────────────────────────

/// A hover, a menu opening: there in about 0.15 s, no overshoot (app.css's --dur-2, ease).
pub const EASE: SpringConfig = SpringConfig::new(1600., 80., 1.);
/// A press going down: at once (0.07 s, ease-out).
pub const SNAP: SpringConfig = SpringConfig::new(6400., 160., 1.);
/// A press let go, a knob landing: past where it rests (about a fifth) and back, in half a second (app.css's --spring).
pub const BOUNCE: SpringConfig = SpringConfig::new(430., 17.5, 1.);
/// The same for bigger things, softer (--spring-soft: 8 % past).
pub const BOUNCE_SOFT: SpringConfig = SpringConfig::new(260., 20., 1.);

/// What the mouse is doing to a control.
#[derive(Default, Clone, Copy)]
pub struct Touch {
    pub over: bool,
    pub down: bool,
}

/// A control's touch, kept between draws under its id.
pub fn touch(id: &ElementId, window: &mut Window, cx: &mut App) -> Entity<Touch> {
    window.with_element_namespace(id.clone(), |w| w.use_keyed_state("touch", cx, |_, _| Touch::default()))
}

/// Has the control tell its touch what the mouse does.
pub fn sense(d: Stateful<Div>, st: &Entity<Touch>) -> Stateful<Div> {
    let (a, b, u, o) = (st.clone(), st.clone(), st.clone(), st.clone());
    d.on_hover(move |over, _, cx| {
        a.update(cx, |t, cx| {
            t.over = *over;
            if !*over {
                t.down = false;
            }
            cx.notify();
        })
    })
    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
        b.update(cx, |t, cx| {
            t.down = true;
            cx.notify();
        })
    })
    .on_mouse_up(MouseButton::Left, move |_, _, cx| {
        u.update(cx, |t, cx| {
            t.down = false;
            cx.notify();
        })
    })
    .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
        o.update(cx, |t, cx| {
            if t.down {
                t.down = false;
                cx.notify();
            }
        })
    })
}

/// A colour part of the way from one to another (by red, green and blue: GPUI's own goes round the hues, and a grey
/// on its way to a red passed through purple).
pub fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b): (Rgba, Rgba) = (a.into(), b.into());
    let t = t.clamp(0., 1.);
    let f = |x: f32, y: f32| x + (y - x) * t;
    Rgba { r: f(a.r, b.r), g: f(a.g, b.g), b: f(a.b, b.b), a: f(a.a, b.a) }.into()
}

/// A control's ground: its fill and edge, easing between how it rests and how it is under the mouse, and sinking a
/// little while it is pressed (the web page's scale .96; the words stay, only the ground gives). The control puts it
/// in first, under its icon and words, and is `relative`.
fn ground(radius: f32, round: bool, (bg, hot): (Hsla, Hsla), (edge, edge_hot): (Hsla, Hsla), over: bool, down: bool, give: f32) -> impl IntoElement {
    let back = div()
        .size_full()
        .map(|d| if round { d.rounded_full() } else { d.rounded(px(radius)) })
        .border_1()
        .with_spring("ground", SpringAnimation::new(EASE).to(over), move |d, p| d.bg(mix(bg, hot, p.0)).border_color(mix(edge, edge_hot, p.0)));
    div().absolute().inset_0().child(back).with_spring("sink", SpringAnimation::new(if down { SNAP } else { BOUNCE }).to(if down { 1f32 } else { 0. }), move |d, v| {
        d.top(px(v * give * 0.5)).bottom(px(v * give * 0.5)).left(px(v * give)).right(px(v * give))
    })
}

/// Anything else you point at and press (a row, a pill, a card, a menu's line): a div whose ground eases in under the
/// mouse and deepens while pressed, a tab stop like every control. Styled and given children and clicks as a div is.
#[derive(IntoElement)]
pub struct Hot {
    id: ElementId,
    base: Stateful<Div>,
    rest: Hsla,
    over: Hsla,
    down: Hsla,
    edge: Option<(Hsla, Hsla)>,
    /// How far it rises under the mouse, and how far off its place it stands now (kit::cascade moves it in).
    lift: f32,
    offset: f32,
}

/// See [`Hot`]. It rests clear and takes the page's hover and pressed tints (app.css's --hover and --active).
pub fn hot(id: impl Into<ElementId>) -> Hot {
    let id = id.into();
    Hot { base: focusable(div().id(id.clone())), id, rest: ca(WHITE, 0.), over: ca(WHITE, 0.06), down: ca(WHITE, 0.09), edge: None, lift: 0., offset: 0. }
}

/// The same for something you only point at (a row with its own buttons): its ground eases, but it is no tab stop
/// and takes no ring.
pub fn hover_only(id: impl Into<ElementId>) -> Hot {
    let id = id.into();
    Hot { base: div().id(id.clone()), id, rest: ca(WHITE, 0.), over: ca(WHITE, 0.06), down: ca(WHITE, 0.06), edge: None, lift: 0., offset: 0. }
}

impl Hot {
    /// Its ground at rest and under the mouse (pressed: a step further the same way).
    pub fn tones(mut self, rest: Hsla, over: Hsla) -> Self {
        self.down = mix(rest, over, 1.) ;
        self.down.a = (over.a + (over.a - rest.a) * 0.5).clamp(0., 1.);
        self.rest = rest;
        self.over = over;
        self
    }
    /// An edge that eases with the ground (a card's hairline brightening). The div needs a border width.
    pub fn edge(mut self, rest: Hsla, over: Hsla) -> Self {
        self.edge = Some((rest, over));
        self
    }
    /// It rises this far under the mouse (the web page's tiles and game cards: translateY(-2px), -3px). A tile that
    /// rises takes no tint of its own (unless `tones` gives one after): a tinted block behind a picture's caption read
    /// as a base under it.
    pub fn lift(mut self, by: f32) -> Self {
        self.lift = by;
        if self.over == ca(WHITE, 0.06) {
            (self.rest, self.over, self.down) = (ca(WHITE, 0.), ca(WHITE, 0.), ca(WHITE, 0.));
        }
        self
    }
    /// It stands this far below its place now (a move in, driven from outside).
    pub fn offset(mut self, y: f32) -> Self {
        self.offset = y;
        self
    }
}

impl Styled for Hot {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for Hot {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for Hot {}

impl ParentElement for Hot {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements)
    }
}

impl RenderOnce for Hot {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = touch(&self.id, window, cx);
        let Touch { over, down } = *st.read(cx);
        let (rest, hot, deep, edge, lift, offset) = (self.rest, self.over, self.down, self.edge, self.lift, self.offset);
        // One spring for both: 0 at rest, 1 under the mouse, 2 pressed.
        sense(self.base, &st).with_spring(self.id, SpringAnimation::new(if down { SNAP } else { EASE }).to(if down { 2f32 } else if over { 1. } else { 0. }), move |d, v| {
            // Under the mouse it rises by its lift; pressed, it gives a little (the web page's translateY(1px)).
            let d = d.top(px(offset - lift * v.clamp(0., 1.) + (v - 1.).max(0.) * 1.5));
            let d = d.bg(if v <= 1. { mix(rest, hot, v) } else { mix(hot, deep, v - 1.) });
            match edge {
                Some((a, b)) => d.border_color(mix(a, b, v)),
                None => d,
            }
        })
    }
}

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type Toggle = Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

/// One of Bloxkeep's icons (assets/icons, the same drawings as shared.js's ICONS), in a colour.
pub fn icon(name: &str, size: f32, color: Hsla) -> Svg {
    svg().path(SharedString::from(format!("icons/{name}.svg"))).size(px(size)).flex_none().text_color(color)
}

/// The icons drawn for Bloxkeep itself, in two layers (assets/icons/duo, made by dev/make_icons.py): the drawing in
/// lines, and one small part of it apart, to be in the accent colour (a door, a knob, a face, a spark). The owner,
/// 2026-11-15: stock icons "trông slop", beside Medal's own two-colour ones. The first of them; the rest of the set
/// (assets/icons) is still the common one.
const DUO: [&str; 14] = ["home", "users", "repeat", "clock", "sliders", "lock", "eye", "star", "like", "info", "moon", "download", "book", "bookmark"];

/// One of those when there is one by that name (a few go by another name in the common set), else the common icon:
/// its lines in `base`, its accent part in `accent`.
pub fn icon2(name: &str, size: f32, base: Hsla, accent: Hsla) -> Div {
    let duo = match name {
        "starFill" => "star",
        "activity" => "like",
        n => n,
    };
    let frame = div().size(px(size)).flex_none().relative();
    if !rich() || !DUO.contains(&duo) {
        return frame.child(icon(name, size, base));
    }
    let layer = |file: String, col: Hsla| svg().path(SharedString::from(file)).size(px(size)).absolute().top_0().left_0().text_color(col);
    frame.child(layer(format!("icons/duo/{duo}.svg"), base)).child(layer(format!("icons/duo/{duo}.a.svg"), accent))
}

/// "ROBLOX" as Roblox writes it (its tilted square for the first O), `h` high, in a colour: the Roblox part's title.
pub fn roblox_word(h: f32, color: Hsla) -> Svg {
    svg().path("icons/roblox-word.svg").h(px(h)).w(px(h * 5.14)).flex_none().text_color(color)
}

/// The keyboard's ring: a hairline that is there all along (clear), so showing it moves nothing.
pub fn focusable(d: Stateful<Div>) -> Stateful<Div> {
    d.tab_index(0).border_1().border_color(ca(WHITE, 0.)).focus_visible(|s| s.border_color(ca(TEXT, 0.75)))
}

/// What a button does, in words, under the mouse: icon-only buttons say it here.
pub struct Tip(pub SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.))
            .py(px(5.))
            .rounded(px(R_S))
            .bg(c(RAISED_2))
            .border_1()
            .border_color(c(LINE))
            .font_family(FONT)
            .text_size(px(T_S))
            .text_color(c(TEXT))
            .child(self.0.clone())
            .with_animation("tip", Animation::new(std::time::Duration::from_millis(110)), |d, t| d.opacity(t))
    }
}

fn with_tip(d: Stateful<Div>, tip: Option<SharedString>) -> Stateful<Div> {
    match tip {
        Some(t) => d.tooltip(move |_, cx| cx.new(|_| Tip(t.clone())).into()),
        None => d,
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// The one main action of a place: the face yellow.
    Primary,
    /// Every other button: a step above what it sits on.
    Ghost,
    /// Over a picture.
    Glass,
    /// Text only, for small acts in a row's end and "See all".
    Quiet,
    /// Deletes or stops something for good.
    Danger,
    /// Something that asks for you now (Rejoin on a dropped account): the warning orange, faint.
    Tint,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    kind: Kind,
    icon: Option<&'static str>,
    icon_end: Option<&'static str>,
    small: bool,
    big: bool,
    disabled: bool,
    tip: Option<SharedString>,
    on_click: Option<Click>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: Kind::Ghost,
            icon: None,
            icon_end: None,
            small: false,
            big: false,
            disabled: false,
            tip: None,
            on_click: None,
        }
    }
    /// What it does in more words, under the mouse (none for empty words).
    pub fn tip(mut self, t: impl Into<SharedString>) -> Self {
        self.tip = Some(t.into()).filter(|t| !t.is_empty());
        self
    }
    pub fn primary(mut self) -> Self {
        self.kind = Kind::Primary;
        self
    }
    pub fn glass(mut self) -> Self {
        self.kind = Kind::Glass;
        self
    }
    pub fn quiet(mut self) -> Self {
        self.kind = Kind::Quiet;
        self
    }
    pub fn tint(mut self) -> Self {
        self.kind = Kind::Tint;
        self
    }
    pub fn danger(mut self) -> Self {
        self.kind = Kind::Danger;
        self
    }
    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = Some(name);
        self
    }
    /// An icon after the words ("See all ›").
    pub fn icon_end(mut self, name: &'static str) -> Self {
        self.icon_end = Some(name);
        self
    }
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
    /// The main acts of a stage or a banner.
    pub fn big(mut self) -> Self {
        self.big = true;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = touch(&self.id, window, cx);
        let Touch { over, down } = *st.read(cx);
        let live = !self.disabled;
        // Its ground and edge, at rest and under the mouse, and its words.
        let (bg, edge, fg) = match self.kind {
            Kind::Primary => ((c(FACE), c(FACE_HI)), (ca(FACE, 0.), ca(FACE, 0.)), c(INK)),
            Kind::Ghost => ((c(RAISED), c(RAISED_2)), (ca(WHITE, 0.08), ca(WHITE, 0.17)), c(TEXT)),
            Kind::Glass => ((ca(WHITE, 0.1), ca(WHITE, 0.2)), (ca(WHITE, 0.), ca(WHITE, 0.)), on_picture(1.)),
            Kind::Quiet => ((ca(WHITE, 0.), ca(WHITE, 0.07)), (ca(WHITE, 0.), ca(WHITE, 0.)), if over && live { c(TEXT) } else { c(TEXT_2) }),
            Kind::Danger => ((ca(BAD, 0.12), ca(BAD, 0.22)), (ca(BAD, 0.32), ca(BAD, 0.45)), c(BAD)),
            Kind::Tint => ((ca(WARN, 0.16), ca(WARN, 0.26)), (ca(WARN, 0.), ca(WARN, 0.)), c(WARN)),
        };
        // (app.css .btn: 38 high, .sm 30, .big 48; the big one here is 44, on a picture.)
        let h = if self.small { 30. } else if self.big { 44. } else { 38. };
        let block = matches!(self.kind, Kind::Primary);
        let key = self.id.clone();
        let size = if self.small { 14. } else { 16. };
        let radius = if self.small { R_S } else { R_M };
        let mut b = focusable(div().id(self.id))
            .relative()
            .h(px(h))
            .px(px(if self.small { 10. } else if self.big { 20. } else { 14. }))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(if self.small { 6. } else { 8. }))
            .rounded(px(radius + 1.))
            .text_color(fg)
            .text_size(px(if self.big { T_H } else if self.small { T_S } else { T_M }))
            .font_weight(FontWeight::SEMIBOLD)
            .whitespace_nowrap()
            .child(ground(radius, false, bg, edge, over && live, down && live, 1.6))
            .when_some(self.icon, |d, i| d.child(icon(i, size, fg)))
            .child(self.label)
            .when_some(self.icon_end, |d, i| d.child(icon(i, size - 2., fg)));
        if let Some(t) = self.tip {
            b = b.tooltip(move |_, cx| cx.new(|_| Tip(t.clone())).into());
        }
        if self.disabled {
            b = b.opacity(0.45);
        } else {
            b = sense(b, &st).cursor_pointer();
            if let Some(f) = self.on_click {
                b = b.on_click(move |e, w, cx| f(e, w, cx));
            }
        }
        // Flat, every kind: the raised yellow block on a darker base was taken out (owner, 2026-11-12: "bỏ cái kiểu
        // nút nổi nổi đi, trông slop"). The press still shows through `ground`.
        let _ = (block, key);
        b.into_any_element()
    }
}

/// A square (or round) button with an icon only; `tip` says what it does, under the mouse.
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: &'static str,
    size: f32,
    on: bool,
    selected: bool,
    filled: bool,
    primary: bool,
    glass: bool,
    round: bool,
    off: bool,
    busy: bool,
    tip: Option<SharedString>,
    on_click: Option<Click>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: &'static str) -> Self {
        Self {
            id: id.into(),
            icon,
            size: 34.,
            on: false,
            selected: false,
            filled: false,
            primary: false,
            glass: false,
            round: false,
            off: false,
            busy: false,
            tip: None,
            on_click: None,
        }
    }
    /// What it started is under way: its icon turns, and it takes no press until that is done (the web page's
    /// .icon-btn.busy).
    pub fn busy(mut self, b: bool) -> Self {
        self.busy = b;
        self
    }
    pub fn size(mut self, s: f32) -> Self {
        self.size = s;
        self
    }
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }
    /// The place you're on, in the rail: a faint yellow square and a yellow icon.
    pub fn selected(mut self, s: bool) -> Self {
        self.selected = s;
        self
    }
    /// Over a picture: a dark see-through square.
    pub fn glass(mut self) -> Self {
        self.glass = true;
        self
    }
    /// Round instead of square (a preset's Run).
    pub fn round(mut self) -> Self {
        self.round = true;
        self
    }
    /// Greyed and deaf (Back with nowhere to go back to).
    pub fn disabled(mut self, off: bool) -> Self {
        self.off = off;
        self
    }
    /// On a raised square even when the mouse isn't over it.
    pub fn filled(mut self) -> Self {
        self.filled = true;
        self
    }
    /// In the main button's yellow: the arrow half of a split button (Add account ▾), as `Button::primary`.
    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }
    pub fn tip(mut self, t: impl Into<SharedString>) -> Self {
        self.tip = Some(t.into());
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let st = touch(&self.id, window, cx);
        let Touch { over, down } = *st.read(cx);
        let live = !self.off && !self.busy;
        let fg = if self.off {
            c(DIM)
        } else if self.primary {
            c(INK)
        } else if self.selected || self.on || self.glass || self.filled || over {
            c(TEXT)
        } else {
            c(TEXT_2)
        };
        let clear = ca(WHITE, 0.);
        // (The place you're on: its mark is a layer of its own, below.)
        let (bg, edge) = if self.selected {
            ((clear, clear), (clear, clear))
        } else if self.primary {
            ((c(FACE), c(FACE_HI)), (clear, clear))
        } else if self.glass {
            ((ca(GROUND, 0.55), ca(GROUND, 0.75)), (ca(WHITE, 0.12), ca(WHITE, 0.22)))
        } else if self.filled {
            ((c(RAISED_2), ca(WHITE, 0.16)), (clear, clear))
        } else if self.on {
            ((c(RAISED), c(RAISED_2)), (clear, clear))
        } else {
            ((clear, ca(WHITE, 0.07)), (clear, clear))
        };
        let radius = if self.size >= 40. { R_L } else { R_M };
        let d = focusable(div().id(self.id))
            .relative()
            .size(px(self.size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .when(self.round, |d| d.rounded_full())
            .when(!self.round, |d| d.rounded(px(radius + 1.)))
            // (The web page's icon buttons give more than its buttons: scale .88.)
            // The mark of the place you're on grows in on the spring and shrinks away when you leave (nav-bar).
            // (A quiet tile, not the face colour: the yellow is kept for the one main action of a place, owner 2026-11-13.)
            .child(div().absolute().rounded(px(radius)).bg(ca(TEXT, 0.12)).with_spring("here", SpringAnimation::new(BOUNCE).to(self.selected), |d, p| {
                let k = p.0.max(0.);
                d.inset(px((1. - k) * 9.)).opacity(k.min(1.))
            }))
            .child(ground(radius, self.round, bg, edge, over && live, down && live, (self.size * 0.055).max(1.4)))
            .map(|d| {
                let size = (self.size * 0.48).round().min(20.);
                if self.busy { d.child(spinner(size, fg)) } else { d.child(icon2(self.icon, size, fg, if self.selected { c(FACE) } else { fg })) }
            })
            .when(live, |d| sense(d, &st).cursor_pointer());
        with_tip(d, self.tip).when_some(self.on_click.filter(|_| live), |d, f| d.on_click(move |e, w, cx| f(e, w, cx)))
    }
}

#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    on: bool,
    disabled: bool,
    small: bool,
    on_toggle: Option<Toggle>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>, on: bool) -> Self {
        Self { id: id.into(), on, disabled: false, small: false, on_toggle: None }
    }
    /// The status bar's: 30×18 and on in the words' own light, not the accent (the one yellow is the page's main action).
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn on_toggle(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on = self.on;
        let st = touch(&self.id, window, cx);
        let Touch { over, down } = *st.read(cx);
        let live = !self.disabled;
        let down = down && live;
        // The track is 42×24 with a 16 px knob (app.css .switch); the box around it is what takes the press and the ring. The knob
        // slides with a spring and, while pressed, stretches toward the side it will go to.
        let small = self.small;
        let (w, h, k) = if small { (30., 18., 12.) } else { (42., 24., 16.) };
        let travel = w - k - 8.;
        let stretch = if small { 3. } else { 5. };
        let lit = if small { c(TEXT) } else { c(FACE) };
        let knob = div().absolute().top_0().h(px(k)).rounded_full().with_spring("knob", SpringAnimation::new(if down { SNAP } else { BOUNCE }).to(if down { 1f32 } else { 0. }), move |d, v| {
            let grow = (v * stretch).max(-2.);
            d.w(px(k + grow)).left(px(if on { -grow } else { 0. })).bg(if on { if small { crate::theme::on_text() } else { c(INK) } } else { c(KNOB) })
        });
        let slide = div().absolute().top(px((h - k) / 2. - 1.)).size(px(k)).child(knob).with_spring("slide", SpringAnimation::new(BOUNCE).to(on), move |d, p| d.left(px(3. + travel * p.0)));
        let track = div()
            .relative()
            .w(px(w))
            .h(px(h))
            .rounded_full()
            .border_1()
            .child(slide)
            .with_spring("track", SpringAnimation::new(EASE).to(on), move |d, p| {
                let off = if over && live { ca(WHITE, 0.2) } else { c(RAISED_2) };
                d.bg(mix(off, lit, p.0)).border_color(mix(ca(TEXT, 0.3), ca(WHITE, 0.), p.0))
            });
        focusable(div().id(self.id))
            .p(px(3.))
            .rounded_full()
            .flex_none()
            .child(track)
            .when(self.disabled, |d| d.opacity(0.45))
            .when(live, |d| sense(d, &st).cursor_pointer().when_some(self.on_toggle, |d, f| d.on_click(move |_, w, cx| f(&!on, w, cx))))
    }
}

#[derive(IntoElement)]
pub struct Check {
    id: ElementId,
    checked: bool,
    label: Option<SharedString>,
    on_toggle: Option<Toggle>,
}

impl Check {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self { id: id.into(), checked, label: None, on_toggle: None }
    }
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }
    pub fn on_toggle(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Check {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let checked = self.checked;
        let st = touch(&self.id, window, cx);
        let over = st.read(cx).over;
        // The web page's tick box (app.css input[type=checkbox]): a dark box with a clear edge, filled with the
        // on-colour (its accent green) and a dark tick once ticked. The box fills as it is ticked, and the tick pops in
        // past its size and settles.
        let tick = icon("check", 12., c(INK)).with_spring("tick", SpringAnimation::new(BOUNCE).to(checked), |s, p| {
            let k = p.0.max(0.);
            s.with_transformation(Transformation::scale(size(k, k)))
        });
        let bx = div()
            .size(px(17.))
            .mt(px(1.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.))
            .border_1()
            .child(tick)
            .with_spring("box", SpringAnimation::new(EASE).to(checked), move |d, p| {
                d.bg(mix(c(INSET), c(OK), p.0)).border_color(mix(if over { ca(TEXT, 0.6) } else { ca(TEXT, 0.38) }, c(OK), p.0))
            });
        // A label wraps in the room it has (a long one under a dialog's edge), its first line beside the box.
        sense(focusable(div().id(self.id)), &st)
            .px(px(2.))
            .min_w_0()
            .rounded(px(R_S))
            .flex()
            .items_start()
            .gap(px(9.))
            .cursor_pointer()
            .child(bx)
            .when_some(self.label, |d, l| d.child(div().min_w_0().flex_1().text_size(px(T_M)).line_height(px(19.)).text_color(if over { c(TEXT) } else { c(TEXT_2) }).child(l)))
            .when_some(self.on_toggle, |d, f| d.on_click(move |_, w, cx| f(&!checked, w, cx)))
    }
}

/// The mark of a picked thing (a row, a tile, a menu item): a solid yellow square with a dark tick, `size` across. One
/// place for it: five were drawn by hand at four sizes and three corner rounds.
pub fn tick_mark(size: f32) -> Div {
    div().size(px(size)).flex_none().rounded(px((size * 0.3).round())).bg(c(FACE)).flex().items_center().justify_center().child(icon("check", (size * 0.64).round(), c(INK)))
}

#[derive(Clone, Copy)]
pub enum Tone {
    Ok,
    Warn,
    Bad,
    Muted,
}

/// A small state word: "Playing", "Dropped"…
pub fn badge(text: impl Into<SharedString>, tone: Tone) -> Div {
    let col = match tone {
        Tone::Ok => OK,
        Tone::Warn => WARN,
        Tone::Bad => BAD,
        Tone::Muted => TEXT_2,
    };
    // A dot and the word in its colour; only trouble gets a ground of its own (a tinted pill on every state read as a
    // row of stickers, and nothing stood out).
    let bad = matches!(tone, Tone::Bad);
    div()
        .h(px(22.))
        .flex()
        .items_center()
        .gap(px(6.))
        .text_color(c(col))
        .text_size(px(T_S))
        .font_weight(FontWeight::SEMIBOLD)
        .when(bad, |d| d.px(px(9.)).rounded_full().bg(ca(col, 0.14)))
        .when(!bad, |d| d.child(div().size(px(6.)).rounded_full().bg(c(col))))
        .child(text.into())
}

/// A row of choices, one of them chosen: a two-way view switch.
#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    items: Vec<(SharedString, SharedString)>,
    selected: SharedString,
    on_select: Option<Box<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>>,
}

impl Segmented {
    pub fn new(id: impl Into<ElementId>, selected: impl Into<SharedString>) -> Self {
        Self { id: id.into(), items: Vec::new(), selected: selected.into(), on_select: None }
    }
    pub fn item(mut self, value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        self.items.push((value.into(), label.into()));
        self
    }
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let f = self.on_select.map(std::rc::Rc::new);
        div().id(self.id).flex().flex_none().gap(px(2.)).p(px(3.)).rounded(px(R_M)).bg(c(INSET)).border_1().border_color(c(LINE)).children(self.items.into_iter().enumerate().map(
            |(i, (value, label))| {
                let on = value == self.selected;
                let f = f.clone();
                segment(i, on).h(px(30.)).px(px(14.)).child(label)
                    .when_some(f, move |d, f| d.on_click(move |_, w, cx| f(&value, w, cx)))
            },
        ))
    }
}

/// One choice in a row of them (a Segmented's, a strip of pills): its ground eases in under the mouse, and the chosen
/// one is the light pill with dark words, as the web page's: the row's one bright thing.
pub fn segment(id: impl Into<ElementId>, on: bool) -> Hot {
    let h = hot(id).flex().flex_none().items_center().rounded(px(7.)).text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD);
    if on {
        h.tones(c(TEXT), c(TEXT)).text_color(crate::theme::on_text())
    } else {
        h.tones(ca(WHITE, 0.), ca(WHITE, 0.05)).text_color(c(TEXT_2)).cursor_pointer().hover(|d| d.text_color(c(TEXT)))
    }
}

/// A filter over a list (the web page's .chip): no track around the row; the chosen one is the light pill with dark
/// words, the one bright thing in the row, the others' ground eases in under the mouse.
pub fn chip(id: impl Into<ElementId>, on: bool) -> Hot {
    let h = hot(id).h(px(30.)).px(px(10.)).flex().flex_none().items_center().gap(px(7.)).rounded(px(9.)).text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).cursor_pointer();
    if on {
        h.tones(c(TEXT), c(TEXT)).text_color(crate::theme::on_text())
    } else {
        h.tones(ca(WHITE, 0.), ca(WHITE, 0.07)).text_color(c(TEXT_2)).hover(|d| d.text_color(c(TEXT)))
    }
}

/// The count on a [`chip`]: a small pill, ringed on the chosen one.
pub fn chip_count(n: usize, on: bool) -> Div {
    div()
        .h(px(18.))
        .min_w(px(18.))
        .px(px(6.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_size(px(T_S))
        .font_weight(FontWeight::SEMIBOLD)
        .font_features(tnum())
        .map(|d| {
            if on {
                let ink = crate::theme::on_text();
                d.bg(Hsla { a: 0.14, ..ink }).text_color(ink)
            } else {
                d.bg(ca(WHITE, 0.09)).text_color(c(TEXT_2))
            }
        })
        .child(n.to_string())
}

/// A round choice among a few (a preset's server, a log's kind): the chosen one is ringed, the others' ground eases in
/// under the mouse. `live` is false for one that can't be picked now.
pub fn pill(id: impl Into<ElementId>, on: bool, live: bool) -> Hot {
    let h = hot(id).h(px(30.)).px(px(12.)).flex().flex_none().items_center().rounded_full().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).border_1();
    if on {
        h.tones(c(TEXT), c(TEXT)).edge(ca(TEXT, 0.), ca(TEXT, 0.)).text_color(crate::theme::on_text())
    } else if live {
        h.tones(ca(WHITE, 0.04), ca(WHITE, 0.09)).edge(ca(WHITE, 0.), ca(WHITE, 0.)).text_color(c(TEXT_2)).cursor_pointer().hover(|d| d.text_color(c(TEXT)))
    } else {
        h.tones(ca(WHITE, 0.04), ca(WHITE, 0.04)).edge(ca(WHITE, 0.), ca(WHITE, 0.)).text_color(c(TEXT_2)).opacity(0.4)
    }
}

/// A text box in Bloxkeep's look around gpui-component's Input (which does the typing, IME and selection), with an
/// icon in front when it has one. Inset: a shade darker than what it sits on, with a hairline.
///
/// As the web page's did, its edge lights up while you type in it (easing in), and brightens a little under the mouse.
pub fn field(state: &Entity<InputState>, lead: Option<&'static str>) -> Field {
    Field {
        state: state.clone(),
        lead,
        end: Vec::new(),
        base: div().gap(px(8.)).h(px(38.)).px(px(12.)).flex().items_center().rounded(px(R_M)).bg(c(INSET)).border_1().border_color(ca(TEXT, FIELD_EDGE)).text_size(px(T_M)).text_color(c(TEXT)),
    }
}

/// A text box's hairline at rest (app.css --line-field: strong enough that the box itself can be seen), under the
/// mouse, and while you type in it (the web page's focus line and its 1 px ring on it).
const FIELD_EDGE: f32 = 0.26;
const FIELD_NEAR: f32 = 0.4;
const FIELD_TYPING: f32 = 0.78;

/// The edge of a text box as the mouse and the typing move it (0 rest, 1 under the mouse, 2 typing).
fn field_edge(rest: Hsla, v: f32) -> Hsla {
    // (A box that clears its resting edge, a search box on a panel, comes up from clear.)
    let near = if rest.a < 0.01 { ca(TEXT, FIELD_NEAR * 0.6) } else { ca(TEXT, FIELD_NEAR.max(rest.a)) };
    if v <= 1. { mix(rest, near, v) } else { mix(near, ca(TEXT, FIELD_TYPING), v - 1.) }
}

/// See [`field`]. Styled as a div is (a search box rounds it and clears its resting edge). Children go after the
/// words, inside the box (a password's eye, as app.css .pw puts it).
#[derive(IntoElement)]
pub struct Field {
    state: Entity<InputState>,
    lead: Option<&'static str>,
    end: Vec<AnyElement>,
    base: Div,
}

impl Styled for Field {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for Field {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.end.extend(elements)
    }
}

impl RenderOnce for Field {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = ElementId::View(self.state.entity_id());
        let typing = self.state.read(cx).focus_handle(cx).is_focused(window);
        let st = touch(&id, window, cx);
        let over = st.read(cx).over;
        let rest = self.base.style().border_color.unwrap_or(ca(WHITE, 0.));
        let lead = self.lead;
        let state = self.state;
        let o = st.clone();
        self.base
            .id(id.clone())
            .on_hover(move |over, _, cx| {
                o.update(cx, |t, cx| {
                    t.over = *over;
                    cx.notify();
                })
            })
            .when_some(lead, |d, i| d.child(icon(i, 15., if typing { c(TEXT_2) } else { c(MUTED) })))
            .child(div().flex_1().min_w_0().child(Input::new(&state).appearance(false)))
            .when(!self.end.is_empty(), |d| d.pr(px(4.)).children(self.end))
            .with_spring(id, SpringAnimation::new(EASE).to(if typing { 2f32 } else if over { 1. } else { 0. }), move |d, v| d.border_color(field_edge(rest, v)))
    }
}

/// A box for a few lines of words (a note, a message): the [`field`]'s look around gpui-component's Textarea, which
/// grows to the rows its state was made with (app.css textarea.field: 64 px at least).
pub fn text_area(state: &Entity<TextareaState>) -> TextArea {
    TextArea { state: state.clone(), base: div().min_h(px(64.)).px(px(12.)).py(px(9.)).rounded(px(R_M)).bg(c(INSET)).border_1().border_color(ca(TEXT, FIELD_EDGE)).text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT)) }
}

/// See [`text_area`].
#[derive(IntoElement)]
pub struct TextArea {
    state: Entity<TextareaState>,
    base: Div,
}

impl Styled for TextArea {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for TextArea {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = ElementId::View(self.state.entity_id());
        let typing = self.state.read(cx).focus_handle(cx).is_focused(window);
        let st = touch(&id, window, cx);
        let over = st.read(cx).over;
        let rest = self.base.style().border_color.unwrap_or(ca(WHITE, 0.));
        let o = st.clone();
        self.base
            .id(id.clone())
            .on_hover(move |over, _, cx| {
                o.update(cx, |t, cx| {
                    t.over = *over;
                    cx.notify();
                })
            })
            .child(Textarea::new(&self.state).appearance(false))
            .with_spring(id, SpringAnimation::new(EASE).to(if typing { 2f32 } else if over { 1. } else { 0. }), move |d, v| d.border_color(field_edge(rest, v)))
    }
}

/// The fuller look of blocks, tried out (the owner, 2026-11-15: the blocks "trông đơn giản quá, kém sang": a grey
/// film with a hairline, grey words in it): a solid block with a finished edge, each figure with its own mark and
/// colour, states as pills. Off until the owner takes it; a hidden shot turns it on with BLOXKEEP_SHOT_CARDS=2.
pub fn rich() -> bool {
    // (Taken, 2026-11-15: "thẻ còn đơn điệu thì làm nó rich lên". BLOXKEEP_SHOT_CARDS=1: a hidden shot of the look before.)
    static RICH: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *RICH.get_or_init(|| !(cfg!(feature = "shot") && std::env::var("BLOXKEEP_SHOT_CARDS").is_ok_and(|v| v == "1")))
}

thread_local! {
    /// The colour of the game whose page is being drawn (pages/games.rs sets it for its page, and takes it off after).
    static GAME_TINT: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}

/// A game's own colour for what is drawn from here on: its page's blocks stand on a ground faintly of it, their heads
/// and marks carry it, its wiki's links are of it. The old page did this everywhere a game was shown (app.css's --gt,
/// from the game's picture: the wiki's grounds 7 to 16 % of it, its links 60 %); without it every game's page was the
/// same grey (the owner, 2026-11-15: "các trang wiki không được màu sắc như bản cũ", the blocks "nhợt nhạt").
pub fn set_game_tint(tone: Option<u32>) {
    GAME_TINT.with(|t| t.set(tone));
}

/// That colour as it is.
pub fn game_tint() -> Option<Hsla> {
    GAME_TINT.with(|t| t.get()).map(|t| rgb(t).into())
}

/// …and made to read on a dark or a light block (a mark's colour, a link's): the same hue, never dull or too dark.
pub fn game_ink() -> Option<Hsla> {
    let t = game_tint()?;
    let dark = crate::theme::lightness(c(PANEL)) < 50.;
    Some(Hsla { h: t.h, s: t.s.clamp(0.45, 0.9), l: if dark { t.l.clamp(0.62, 0.78) } else { t.l.clamp(0.28, 0.42) }, a: 1. })
}

/// A block that is about one game among others (an account's card while it plays): its ground, its ground under the
/// mouse, its hairline and that under the mouse, faintly of that game's colour; the theme's own when it has none.
pub fn toned(tone: Option<u32>) -> (Hsla, Hsla, Hsla, Hsla) {
    match tone {
        Some(t) => {
            let t: Hsla = rgb(t).into();
            (mix(c(RAISED), t, 0.12), mix(c(RAISED_2), t, 0.16), mix(c(LINE), t, 0.24), mix(c(LINE), t, 0.5))
        }
        None => (c(RAISED), c(RAISED_2), c(LINE), ca(WHITE, 0.17)),
    }
}

/// A table's head on a game's page: a step above its block, more of the game's colour (the old wiki's --ws-head, 16 %).
pub fn game_head() -> Hsla {
    match game_tint() {
        Some(t) => mix(c(RAISED_2), Hsla { a: 1., ..t }, 0.18),
        None => ca(WHITE, 0.03),
    }
}

/// A colour of a game's picture made to read as words on the theme's blocks: its hue, never dull or too dark.
pub fn ink_of(tone: u32) -> Hsla {
    let t: Hsla = rgb(tone).into();
    let dark = crate::theme::lightness(c(PANEL)) < 50.;
    Hsla { h: t.h, s: t.s.clamp(0.5, 0.92), l: if dark { t.l.clamp(0.64, 0.78) } else { t.l.clamp(0.3, 0.42) }, a: 1. }
}

/// A game's name in the game's own colours (the owner, 2026-11-15: "tên trên thẻ nên để màu, vibe của game đó", beside
/// Blox Fruits' blue and orange logo): the colours of its picture at `url` (its icon), the name in the first and, when
/// the picture has a second colour and the name more than one word, its last word in that. One run of text, so it is
/// cut short as any name is. A game with no picture yet, or a grey one, is written as before.
pub fn game_name(name: impl Into<SharedString>, url: Option<&str>, cx: &mut App) -> StyledText {
    let name: SharedString = name.into();
    let Some(first) = url.filter(|_| rich()).and_then(|u| crate::images::tone(u, cx)) else { return StyledText::new(name) };
    let second = url.and_then(crate::images::tone2);
    let colour = |c: Hsla| HighlightStyle { color: Some(c), ..Default::default() };
    let cut = name.rfind(' ').filter(|_| second.is_some()).map(|i| i + 1).unwrap_or(name.len());
    let mut marks = vec![(0..cut, colour(ink_of(first)))];
    if let (Some(c2), true) = (second, cut < name.len()) {
        marks.push((cut..name.len(), colour(ink_of(c2))));
    }
    StyledText::new(name).with_highlights(marks)
}

/// A block's ground and hairline: the theme's, faintly of the game's colour on its page.
pub fn block_tones() -> (Hsla, Hsla) {
    match game_tint() {
        Some(t) => (mix(c(RAISED), Hsla { a: 1., ..t }, 0.11), mix(c(LINE), Hsla { a: 1., ..t }, 0.2)),
        None => (c(RAISED), c(LINE)),
    }
}

/// A block's finished edge: a hair of light along its top inside, a close dark line under it. No glow, no spread.
pub fn block_edge() -> Vec<BoxShadow> {
    vec![
        BoxShadow { color: hsla(0., 0., 0., 0.34), offset: point(px(0.), px(1.)), blur_radius: px(2.), spread_radius: px(0.), inset: false },
        BoxShadow { color: hsla(0., 0., 1., 0.05), offset: point(px(0.), px(1.)), blur_radius: px(0.), spread_radius: px(0.), inset: true },
    ]
}

/// A card on a panel: barely lighter, with a hairline. Every page's cards are this one.
pub fn surface() -> Div {
    if rich() {
        let (ground, line) = block_tones();
        return div().rounded(px(R_L)).bg(ground).border_1().border_color(line).shadow(block_edge());
    }
    div().rounded(px(R_L)).bg(ca(WHITE, 0.045)).border_1().border_color(c(LINE))
}

/// A figure's or a block's own mark: its icon in its colour, on a ground faintly of that colour.
pub fn mark_chip(name: &'static str, tone: u32, size: f32) -> Div {
    div().size(px(size)).flex_none().rounded(px((size * 0.28).round())).bg(ca(tone, 0.13)).flex().items_center().justify_center().child(icon2(name, (size * 0.56).round(), c(TEXT), c(tone)))
}

/// The same mark in the game's own colour on its page (else in `tone`): a block's head there.
pub fn game_chip(name: &'static str, tone: u32, size: f32) -> Div {
    match game_ink() {
        Some(ink) => div().size(px(size)).flex_none().rounded(px((size * 0.28).round())).bg(Hsla { a: 0.16, ..ink }).flex().items_center().justify_center().child(icon2(name, (size * 0.56).round(), c(TEXT), ink)),
        None => mark_chip(name, tone, size),
    }
}

/// A state as a pill in its colour ("In game", "Banned").
pub fn tint_pill(text: impl Into<SharedString>, tone: u32) -> Div {
    div().h(px(22.)).px(px(9.)).flex().flex_none().items_center().gap(px(6.)).rounded_full().bg(ca(tone, 0.13)).text_color(c(tone)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).child(div().size(px(6.)).rounded_full().bg(c(tone))).child(text.into())
}

/// A card you can press (a preset to run, a row that opens): the same card, its ground and hairline brightening
/// under the mouse.
pub fn card(id: impl Into<ElementId>) -> Hot {
    if rich() {
        let (ground, line) = block_tones();
        return hot(id).rounded(px(R_L)).border_1().tones(ground, mix(ground, c(TEXT), 0.05)).edge(line, mix(line, c(TEXT), 0.25));
    }
    hot(id).rounded(px(R_L)).border_1().tones(ca(WHITE, 0.045), ca(WHITE, 0.08)).edge(c(LINE), ca(WHITE, 0.17))
}

/// A line in a menu: its ground eases in under the mouse.
pub fn menu_row(id: impl Into<ElementId>) -> Hot {
    hot(id).h(px(32.)).px(px(10.)).rounded(px(R_S)).flex().items_center().gap(px(10.)).text_size(px(T_M)).text_color(c(TEXT_2)).cursor_pointer().hover(|d| d.text_color(c(TEXT)))
}

/// A panel: a step above the ground, with a hairline.
pub fn panel() -> Div {
    div().rounded(px(R_PANEL)).bg(c(PANEL)).border_1().border_color(c(LINE))
}

/// A thin dot between facts on one line.
pub fn sep() -> Div {
    div().size(px(3.)).flex_none().rounded_full().bg(c(DIM))
}

/// A short bar for how full something is: quiet until it matters, orange from 80 %, red from 90 %.
/// Its fill eases to a new figure instead of jumping (the web page's width transition); `id` is the bar's own.
pub fn bar(id: impl Into<ElementId>, part: f32, w: Option<f32>) -> Div {
    let part = part.clamp(0., 1.);
    let fill = if part >= 0.9 { c(BAD) } else if part >= 0.8 { c(WARN) } else { ca(TEXT, 0.45) };
    div()
        .when_some(w, |d, w| d.w(px(w)))
        .h(px(4.))
        .flex_none()
        .rounded_full()
        .bg(c(RAISED_2))
        .child(div().h_full().rounded_full().bg(fill).with_spring(id, SpringAnimation::new(EASE).to(part), |d, v| d.w(relative(v.clamp(0., 1.)))))
}

// ───────────────────────── page parts ─────────────────────────
// The pieces a page is laid out with (the Accounts page first, then every tab): an 8 px rhythm, sections 32 apart,
// each with its head (title and count, its actions at the right), a grid whose columns come from the room there is,
// blocks standing on their base, a calm panel for what needs a hand, and a toolbar of quiet controls of one height.

/// Between a page's sections, and the gap in its grids (8 px rhythm).
pub const SECTION_GAP: f32 = 32.;
pub const GRID_GAP: f32 = 16.;
/// The height of every control in a toolbar row (tool, view switch, Select all).
pub const TOOL_H: f32 = 32.;

/// A grid's columns: as many as fit with each at least `min` wide, and the width each then gets so a full row ends
/// exactly where the room does. Sections that ask with the same `min` line up in the same columns.
pub fn grid_fit(width: f32, min: f32, gap: f32) -> (usize, f32) {
    let cols = ((width + gap) / (min + gap)).floor().max(1.);
    (cols as usize, ((width - gap * (cols - 1.)) / cols).floor())
}

/// A grid: its items (each as wide as [`grid_fit`] said) in rows, `gap` apart both ways.
pub fn grid(gap: f32) -> Div {
    div().w_full().flex().flex_wrap().items_start().gap(px(gap))
}

/// A section's head: its title (16 px, semibold) and its count after it in muted. Its actions go after (`.child`):
/// they sit at the right edge, one tidy cluster.
pub fn section_head(title: impl Into<SharedString>, count: Option<usize>) -> Div {
    div()
        .w_full()
        .min_h(px(32.))
        .flex()
        .items_center()
        .gap(px(8.))
        .child(div().flex_none().text_size(px(T_SEC)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title.into()))
        .when_some(count, |d, n| d.child(div().flex_none().text_size(px(T_SEC)).text_color(c(MUTED)).font_features(tnum()).child(n.to_string())))
        .child(div().flex_1().min_w_0())
}

/// A group's head (a game over its windows): its picture (a block of `PIC` px), its name at 18 px and its facts in
/// muted under it. Its actions go after (`.child`), at the right edge.
pub fn group_head(picture: impl IntoElement, title: impl Into<SharedString>, facts: impl IntoElement) -> Div {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(14.))
        .child(picture)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_size(px(T_GROUP)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).truncate().child(title.into()))
                .child(facts),
        )
}
/// The size of a [`group_head`]'s picture.
pub const GROUP_PIC: f32 = 44.;

/// Facts on one line, a small dot between them, in muted tabular figures ("2 windows · 1.7 GB"); a fact with its own
/// colour (a warning) keeps it.
pub fn facts(parts: Vec<(String, Option<Hsla>)>) -> Div {
    let mut d = div().flex().min_w_0().overflow_hidden().items_center().gap(px(8.)).text_size(px(T_M)).text_color(c(MUTED)).font_features(tnum()).whitespace_nowrap();
    for (i, (text, col)) in parts.into_iter().enumerate() {
        if i > 0 {
            d = d.child(sep());
        }
        d = d.child(div().flex_none().when_some(col, |d, col| d.text_color(col)).child(text));
    }
    d
}

/// Bloxkeep's block: `el` standing on a base of `base`, `rise` px of it showing under it (the main button's base, a
/// tile's game colour). The base is as round as the block.
///
/// Flat since 2026-11-12 (owner: "bỏ cái kiểu nút nổi nổi đi", then "Flatten cards too"): nothing stands on a raised
/// base any more. What the base used to say is kept as an edge: picked (the face's base) a yellow ring, in trouble
/// (`bad_base`) a red one; a game's own colour or the plain dark base, nothing.
pub fn on_base(base: Hsla, rise: f32, radius: f32, el: impl IntoElement) -> Div {
    let _ = rise;
    let ring = if base == c(FACE_BASE) {
        Some(ca(FACE, 0.9))
    } else if base == bad_base() {
        Some(ca(BAD, 0.7))
    } else {
        None
    };
    div()
        .relative()
        .flex_none()
        .child(el)
        .when_some(ring, |d, col| d.child(div().absolute().top_0().left_0().right_0().bottom_0().rounded(px(radius)).border_2().border_color(col)))
}

/// The base under a block of this colour: the colour gone dark, as a block's side is (never the bright colour itself,
/// which read as a coloured underline).
pub fn base_of(col: Hsla) -> Hsla {
    let mut b = mix(col, c(SHADE), 0.5);
    b.a = 1.;
    b
}

/// The base of a block in trouble (a dropped window): red, a step darker than the red of the words, so it reads as
/// the block's side and not as a line.
pub fn bad_base() -> Hsla {
    let mut b = mix(c(BAD), c(SHADE), 0.25);
    b.a = 1.;
    b
}

/// The plain dark base of a block that has no colour of its own.
pub fn base_dark() -> Hsla {
    let mut b = mix(c(GROUND), c(SHADE), 0.45);
    b.a = 1.;
    b
}

/// A ground of `col` tinted toward `tint` by `k` (a picked card's warm yellow, a problem tile's red), solid.
pub fn tinted(col: u32, tint: u32, k: f32) -> Hsla {
    mix(c(col), c(tint), k)
}

/// A block to stand on a base ([`on_base`]): a [`surface`] with a solid ground, so the base under it never shows
/// through (a see-through card on its base read as a dark card with a light top edge).
pub fn block() -> Div {
    div().rounded(px(R_L)).bg(c(RAISED)).border_1().border_color(c(LINE))
}

/// A block you can press: [`card`] with solid grounds, for standing on a base.
pub fn block_card(id: impl Into<ElementId>) -> Hot {
    hot(id).rounded(px(R_L)).border_1().tones(c(RAISED), c(RAISED_2)).edge(c(LINE), ca(WHITE, 0.17))
}

/// One calm panel for what needs a hand: a ground faintly of `tone`, a head with an alert mark, the title and the
/// count. Its rows go after (`.child`), each after an [`alert_line`].
pub fn alert_panel(tone: u32, title: impl Into<SharedString>, count: usize) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .p(px(4.))
        .rounded(px(R_L + 2.))
        .bg(tinted(PANEL, tone, 0.07))
        .border_1()
        .border_color(ca(tone, 0.26))
        .child(
            div()
                .h(px(44.))
                .px(px(12.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(icon("alert", 15., c(tone)).flex_none())
                .child(div().text_size(px(T_SEC)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title.into()))
                .child(div().text_size(px(T_SEC)).text_color(c(MUTED)).font_features(tnum()).child(count.to_string())),
        )
}

/// The hairline between an [`alert_panel`]'s rows, in its tone.
pub fn alert_line(tone: u32) -> Div {
    div().h(px(1.)).mx(px(12.)).flex_none().bg(ca(tone, 0.16))
}

/// A row in an [`alert_panel`]: full width, 64 high, its ground easing in under the mouse. Face, name and what is
/// wrong go in; its fix (`alert_fix`) at the right.
pub fn alert_row(id: impl Into<ElementId>) -> Hot {
    hot(id).w_full().h(px(64.)).px(px(12.)).flex().items_center().gap(px(14.)).rounded(px(R_M)).cursor_pointer().tones(ca(WHITE, 0.), ca(WHITE, 0.045))
}

/// The column a row's fix sits in, as wide in every row so the buttons line up.
pub fn alert_fix(el: impl IntoElement) -> Div {
    div().w(px(150.)).flex_none().flex().justify_end().child(el)
}

/// A quiet control in a toolbar ("Sort  Name ▾", "Windows ▾", "Select all"): `TOOL_H` high, its icon muted, its
/// ground easing in under the mouse, a step lit while its list is open. Put its words in (`tool_words`), then
/// `tool_chev` for one that opens a list.
pub fn tool(id: impl Into<ElementId>, mark: Option<&'static str>, open: bool) -> Hot {
    let h = hot(id)
        .h(px(TOOL_H))
        .px(px(10.))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(7.))
        .rounded(px(R_S))
        .cursor_pointer()
        .whitespace_nowrap()
        .text_size(px(T_M))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if open { c(TEXT) } else { c(TEXT_2) })
        .hover(|d| d.text_color(c(TEXT)))
        .when_some(mark, |d, m| d.child(icon(m, 15., c(MUTED))));
    if open { h.tones(ca(WHITE, 0.08), ca(WHITE, 0.08)) } else { h.tones(ca(WHITE, 0.), ca(WHITE, 0.06)) }
}

/// A tool's words: its name in muted, then the value chosen ("Sort: Name"); the value alone when room is short.
pub fn tool_words(name: Option<String>, value: String) -> Div {
    div().flex().items_center().gap(px(5.)).when_some(name, |d, n| d.child(div().text_color(c(MUTED)).child(format!("{n}:")))).child(value)
}

/// The mark of a tool that opens a list.
pub fn tool_chev() -> Svg {
    icon("chevronDown", 12., c(MUTED))
}

/// A short upright hairline between groups of tools.
pub fn tool_rule() -> Div {
    div().w(px(1.)).h(px(18.)).mx(px(4.)).flex_none().bg(c(LINE))
}

/// A view switch at a toolbar's height (cards | list): its [`view_choice`]s, icons only, each with its tip and click
/// given by the caller; the chosen one is the yellow block.
pub fn view_switch(items: Vec<Hot>) -> Div {
    div().h(px(TOOL_H)).flex().flex_none().items_center().gap(px(2.)).p(px(2.)).rounded(px(R_S + 1.)).bg(c(INSET)).border_1().border_color(c(LINE)).children(items)
}

/// One choice of a [`view_switch`].
pub fn view_choice(id: impl Into<ElementId>, mark: &'static str, on: bool) -> Hot {
    segment(id, on).h(px(TOOL_H - 6.)).w(px(32.)).justify_center().rounded(px(R_S - 2.)).child(icon(mark, 15., if on { crate::theme::on_text() } else { c(TEXT_2) }))
}

// ───────────────────────── motion ─────────────────────────
// What moves, and how: a place steps in (a short rise and fade) when it replaces another, a picture fades in when it
// has loaded, a menu fades in as it opens, and what is still loading breathes instead of standing still. All short
// and eased out, so nothing is waited for; none of it holds a click back.

/// The web page's two springs as curves over a move's time (app.css's --spring and --spring-soft, the same samples):
/// past where it lands and back.
const SPRING: [f32; 25] = [0., 0.09, 0.307, 0.572, 0.826, 1.028, 1.161, 1.225, 1.23, 1.194, 1.137, 1.074, 1.019, 0.978, 0.954, 0.946, 0.949, 0.96, 0.974, 0.988, 1., 1.008, 1.012, 1.013, 1.];
const SPRING_SOFT: [f32; 25] = [0., 0.042, 0.146, 0.284, 0.435, 0.582, 0.715, 0.828, 0.919, 0.987, 1.035, 1.064, 1.079, 1.084, 1.08, 1.071, 1.059, 1.047, 1.034, 1.023, 1.014, 1.006, 1.001, 0.997, 1.];

fn sampled(curve: &'static [f32; 25]) -> impl Fn(f32) -> f32 {
    move |t| {
        let x = t.clamp(0., 1.) * 24.;
        let i = (x.floor() as usize).min(23);
        curve[i] + (curve[i + 1] - curve[i]) * (x - i as f32)
    }
}

/// The web page's --spring at `t` of a move's time (0 to 1): past where it lands and back.
pub fn spring(t: f32) -> f32 {
    sampled(&SPRING)(t)
}

/// …and its --spring-soft.
pub fn spring_soft(t: f32) -> f32 {
    sampled(&SPRING_SOFT)(t)
}

/// CSS's cubic-bezier(x1, y1, x2, y2) at `x` of a move's time.
pub fn bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let at = |a: f32, b: f32, s: f32| 3. * a * s * (1. - s) * (1. - s) + 3. * b * s * s * (1. - s) + s * s * s;
    let (mut lo, mut hi) = (0f32, 1f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.;
        if at(x1, x2, mid) < x { lo = mid } else { hi = mid }
    }
    at(y1, y2, (lo + hi) / 2.)
}

/// A move that waits its turn: nothing for `delay`, then `curve` over `run` (both in milliseconds).
fn after(delay: f32, run: f32, curve: impl Fn(f32) -> f32 + 'static) -> Animation {
    let whole = delay + run;
    Animation::new(std::time::Duration::from_millis(whole as u64)).with_easing(move |t| curve(((t * whole - delay) / run).clamp(0., 1.)))
}

/// A place that has just replaced another: it comes in from the side it lies on (`from` 1. from below, for a place
/// further down the rail; -1. from above), a little past its place and back, as the web page's views did (view-in:
/// .42 s on --spring-soft). `key` names what is shown, so the step plays when that changes and not on every draw.
pub fn step_in(key: impl Into<SharedString>, from: f32, body: impl IntoElement) -> impl IntoElement {
    div().size_full().relative().child(body).with_animation(ElementId::Name(key.into()), Animation::new(std::time::Duration::from_millis(420)).with_easing(sampled(&SPRING_SOFT)), move |d, t| {
        d.opacity(t.clamp(0., 1.)).top(px((1. - t) * 14. * from))
    })
}

/// One of many that come in together (a grid's tiles, a row's cards): each a moment after the one before it, rising
/// a little as it fades in (the web page's "pop", dealt out along the row). `scope` names what the row shows, so a
/// row showing something else comes in anew.
pub fn cascade(scope: impl Into<SharedString>, i: usize, el: Hot) -> impl IntoElement {
    el.with_animation(ElementId::NamedInteger(scope.into(), i as u64), after(i.min(16) as f32 * 22., 300., ease_out_quint()), |e, t| e.opacity(t).offset((1. - t) * 8.))
}

/// A line of a screen that builds itself (the lock screen's): after `i` others, rising into place on the soft spring.
pub fn line_in<E: Styled + IntoElement + 'static>(scope: impl Into<SharedString>, i: usize, el: E) -> impl IntoElement {
    el.with_animation(ElementId::NamedInteger(scope.into(), i as u64), after(90. + i as f32 * 70., 600., sampled(&SPRING_SOFT)), |e, t| e.opacity(t.clamp(0., 1.)).relative().top(px((1. - t) * 12.)))
}

/// One of the rail's buttons as the window first shows: in from the left, each after the one above it (intro-tab).
pub fn tab_in(i: usize, el: impl IntoElement) -> impl IntoElement {
    div().relative().child(el).with_animation(ElementId::NamedInteger("rail-in".into(), i as u64), after(60. + i as f32 * 45., 500., sampled(&SPRING_SOFT)), |d, t| d.opacity(t.clamp(0., 1.)).left(px((t - 1.) * 16.)))
}

/// Something that pops out as the screen comes up (the lock screen's logo): from small to its size, past it and back
/// (logo-pop: .7 s on --spring). It stands in a box of its size, so growing moves nothing round it.
pub fn pop_out(key: impl Into<SharedString>, size: f32, picture: impl Fn(f32) -> AnyElement + 'static) -> impl IntoElement {
    div().size(px(size)).flex_none().flex().items_center().justify_center().with_animation(ElementId::Name(key.into()), Animation::new(std::time::Duration::from_millis(700)).with_easing(sampled(&SPRING)), move |d, t| {
        d.opacity((t * 3.).clamp(0., 1.)).child(picture(size * (0.4 + 0.6 * t)))
    })
}

/// A no: left and right and still again (the web page's shake, .4 s). `times` counts the noes, so each one shakes.
pub fn shake<E: Styled + IntoElement + 'static>(times: u32, el: E) -> AnyElement {
    if times == 0 {
        return el.into_any_element();
    }
    el.with_animation(ElementId::NamedInteger("shake".into(), times as u64), Animation::new(std::time::Duration::from_millis(400)), |e, t| {
        // Out to -8 at a fifth of the way, +8 at two fifths, -8, +8, and home.
        let k = t * 5.;
        let x = match k as u32 {
            0 => -8. * k,
            1 => -8. + 16. * (k - 1.),
            2 => 8. - 16. * (k - 2.),
            3 => -8. + 16. * (k - 3.),
            _ => 8. * (5. - k).max(0.),
        };
        e.relative().left(px(x))
    })
    .into_any_element()
}

/// Something under way, as a turning mark (the web page's spin): for a button that is busy.
pub fn spinner(size: f32, color: Hsla) -> impl IntoElement {
    icon("refresh", size, color).with_animation("spin", Animation::new(std::time::Duration::from_millis(800)).repeat().with_max_fps(30.), |s, t| s.with_transformation(Transformation::rotate(radians(t * std::f32::consts::TAU))))
}

/// A bar of a chart as the chart comes up: it grows to its height, each a moment after the one before (cmp-grow).
pub fn grow_in(scope: impl Into<SharedString>, i: usize, part: f32, bar: Div) -> impl IntoElement {
    bar.with_animation(ElementId::NamedInteger(scope.into(), i as u64), after(80. + i as f32 * 18., 600., ease_out_quint()), move |b, t| b.h(relative((part * t).max(0.02))))
}

/// A picture that has just loaded fades in (once: the key is the picture itself).
pub fn fade_in(picture: Img, of: &std::sync::Arc<Image>) -> impl IntoElement {
    let key = std::sync::Arc::as_ptr(of) as *const () as usize as u64;
    picture.with_animation(ElementId::Integer(key), Animation::new(std::time::Duration::from_millis(220)).with_easing(ease_out_quint()), |i, t| i.opacity(t))
}

/// How far in something that comes and goes is: None once it has gone, 0..1 on its way in or out, 1 while it stays.
/// GPUI Base's Presence keeps the time, so what leaves is still drawn for its last beat (the web page's ".out").
pub fn present(id: impl Into<ElementId>, here: bool, ms: u64, window: &mut Window, cx: &mut App) -> Option<f32> {
    let s = Presence::new(id.into(), here).transition(Move::new(std::time::Duration::from_millis(ms)).ease(ease_out_quint())).sample(window, cx);
    (s.should_render() && (here || s.progress > 0.001)).then_some(s.progress.clamp(0., 1.))
}

/// A menu: it fades in and settles down into place as it opens (the web page's "pop") and goes back the same way as
/// it closes. The page builds it open or not; it is drawn only while it is there or leaving.
#[derive(IntoElement)]
pub struct Pop {
    key: SharedString,
    open: bool,
    menu: AnyElement,
}

pub fn pop(key: impl Into<SharedString>, open: bool, menu: impl IntoElement) -> Pop {
    Pop { key: key.into(), open, menu: menu.into_any_element() }
}

impl RenderOnce for Pop {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let at = present(ElementId::Name(self.key), self.open, 140, window, cx);
        // It stands off the page on the web page's menu shadow (app.css .menu: 0 18px 50px), so it reads as lying on
        // top and not as part of what is under it.
        div().when_some(at, |d, t| d.opacity(t).mt(px((t - 1.) * 6.)).rounded(px(R_M)).shadow(menu_shadow(t)).child(self.menu))
    }
}

/// The shadow under a menu or anything else that lies over the page, `t` of the way in.
pub fn menu_shadow(t: f32) -> Vec<BoxShadow> {
    vec![
        BoxShadow { color: hsla(0., 0., 0., 0.5 * t), offset: point(px(0.), px(18.)), blur_radius: px(50.), spread_radius: px(0.), inset: false },
        BoxShadow { color: hsla(0., 0., 0., 0.35 * t), offset: point(px(0.), px(2.)), blur_radius: px(6.), spread_radius: px(0.), inset: false },
    ]
}

// ───────────────────────── dialogs ─────────────────────────
// The web page's dialog (app.css .modal): a head with the title and an × that closes it, then the body, then a foot
// with the buttons, the head and the foot cut off from the body by a hairline. GPUI Component's Dialog keeps the
// cover, the coming and going and the keys; these are what goes in it.

/// The width of most dialogs (app.css .modal: 520), and of the sign-in one (.modal.auth: 420).
pub const DIALOG_W: f32 = 520.;
pub const DIALOG_AUTH_W: f32 = 420.;

/// A dialog in the web page's frame: no padding of GPUI Component's own (the head, body and foot bring theirs), and
/// the head as its title. `close` is what the × does (the same as the dialog's Cancel or Close).
pub fn framed(dialog: Dialog, title: impl Into<SharedString>, close_tip: impl Into<SharedString>, close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Dialog {
    dialog.close_button(false).p_0().gap_0().title(dialog_head(title, close_tip, close))
}

/// The sign-in dialog's frame (app.css .modal.auth): its title in the middle and no hairline under it, the × at the
/// top right.
pub fn framed_center(dialog: Dialog, title: impl Into<SharedString>, close_tip: impl Into<SharedString>, close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Dialog {
    dialog.close_button(false).p_0().gap_0().title(
        div()
            .w_full()
            .relative()
            .pt(px(24.))
            .px(px(48.))
            .flex()
            .justify_center()
            .child(div().text_size(px(T_GROUP)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).text_center().child(title.into()))
            .child(div().absolute().top(px(10.)).right(px(10.)).child(dialog_x(close_tip, close))),
    )
}

/// What the app refused, in a red box (app.css .form-error), under the boxes it is about.
pub fn form_error(text: impl Into<SharedString>) -> Div {
    div().w_full().px(px(12.)).py(px(9.)).rounded(px(R_M)).bg(ca(BAD, 0.12)).border_1().border_color(ca(BAD, 0.35)).text_size(px(T_M)).line_height(px(19.)).text_color(mix(c(BAD), c(WHITE), 0.35)).child(text.into())
}

/// The × of a dialog.
pub fn dialog_x(tip: impl Into<SharedString>, close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> IconButton {
    IconButton::new("dialog-x", "x").size(32.).tip(tip).on_click(close)
}

/// A dialog's head: its title (app.css .modal-head h3: 18 px, 650) and the ×, over a hairline.
pub fn dialog_head(title: impl Into<SharedString>, close_tip: impl Into<SharedString>, close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Div {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(12.))
        .pl(px(24.))
        .pr(px(14.))
        .pt(px(14.))
        .pb(px(11.))
        .border_b_1()
        .border_color(c(LINE))
        .child(div().min_w_0().flex_1().text_size(px(T_GROUP)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title.into()))
        .child(dialog_x(close_tip, close))
}

/// A dialog's body (.modal-body: 24 px at the sides). GPUI Component already leaves 8 px under the head.
/// The body is never taller than the window leaves it: the dialog keeps DIALOG_GUTTER clear above and below (off the
/// title bar), its head and foot stay put, and only the body scrolls, its bar always shown while there is more.
pub fn dialog_body(content: impl IntoElement) -> DialogBody {
    DialogBody { content: content.into_any_element() }
}

/// What a dialog keeps clear of the window's top and bottom edges.
pub const DIALOG_GUTTER: f32 = 56.;
/// A dialog's head and foot together, with GPUI Component's gaps between them and the body (dialog_head ≈ 58,
/// dialog_foot ≈ 69, two gaps of 8 and the hairline frame).
const DIALOG_CHROME: f32 = 146.;

#[derive(IntoElement)]
pub struct DialogBody {
    content: AnyElement,
}

impl RenderOnce for DialogBody {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let room = (f32::from(window.viewport_size().height) - 2. * DIALOG_GUTTER - DIALOG_CHROME).max(120.);
        let h = window.with_element_namespace("dialog-body", |w| w.use_keyed_state("scroll", cx, |_, _| ScrollHandle::new())).read(cx).clone();
        // (GPUI draws a clipping box that lies wholly outside the body's clip unclipped: a picture in a box with
        // overflow_hidden, scrolled below the body, showed over the foot. Pictures in a body keep their own corners.)
        div()
            .relative()
            .w_full()
            .child(div().id("dialog-body").w_full().max_h(px(room)).overflow_y_scroll().track_scroll(&h).px(px(24.)).pt(px(8.)).pb(px(18.)).child(self.content))
            .child(div().absolute().top_0().bottom_0().right(px(4.)).w(px(10.)).child(gpui_kit::component::scroll::Scrollbar::vertical(&h).id("dialog-body-bar").mode(gpui_kit::component::scroll::ScrollbarMode::Scrolling).viewport_from_layout()))
    }
}

/// A dialog's foot (.modal-foot): its buttons at the right, over a hairline. Put the buttons (or a row of them) in.
pub fn dialog_foot() -> Div {
    div().w_full().flex().items_center().justify_end().gap(px(10.)).px(px(24.)).pt(px(14.)).pb(px(18.)).border_t_1().border_color(c(LINE))
}

/// Someone's face in a list (an account in a file, in the launch order): their picture, or their initials on a
/// raised square while there is none (the web page's .avatar .initials).
pub fn face(name: &str, picture: Option<std::sync::Arc<Image>>, size: f32) -> Div {
    let initials: String = name.split_whitespace().filter_map(|w| w.chars().next()).take(2).flat_map(char::to_uppercase).collect();
    let initials = if initials.is_empty() { "?".to_string() } else { initials };
    let r = (size * 0.3).round();
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(r))
        .bg(c(RAISED_2))
        .border_1()
        .border_color(ca(WHITE, 0.08))
        .text_size(px((size * 0.36).round()))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(TEXT_2))
        .map(|d| match picture {
            Some(p) => d.child(img(p).size_full().rounded(px(r))),
            None => d.child(initials),
        })
}

/// A drag handle's dots at the start of a row you can put in order (the web page's grip).
pub fn grip() -> Svg {
    icon("grip", 15., c(MUTED))
}

/// A list that scrolls inside a dialog or a card, at most `max_h` tall, with its scrollbar always there so a list
/// longer than its box can be seen to go on (no fade: the page has no gradients). `handle` keeps where it is.
pub fn scroll_list(id: impl Into<ElementId>, handle: &ScrollHandle, max_h: f32, content: impl IntoElement) -> Div {
    div()
        .relative()
        .w_full()
        .rounded(px(R_M))
        .border_1()
        .border_color(c(LINE))
        .bg(ca(WHITE, 0.02))
        .child(div().id(id).max_h(px(max_h)).overflow_y_scroll().track_scroll(handle).pl(px(10.)).pr(px(16.)).child(content))
        .child(gpui_kit::base::Scrollbar::vertical(handle).mode(gpui_kit::base::ScrollbarMode::Scrolling))
}

/// The same notice said again: it gives a short sideways shake instead of sliding in anew (the web page's "nudge").
pub fn nudge(n: u32, notice: impl IntoElement) -> impl IntoElement {
    div().child(notice).with_animation(ElementId::NamedInteger("nudge".into(), n as u64), Animation::new(std::time::Duration::from_millis(300)), |d, t| {
        d.left(px(-6. * (t * std::f32::consts::TAU).sin() * (1. - t)))
    })
}

/// A notice as it appears: it drops into place from above, a little past it and back (the web page's toast, which
/// rose from the foot; here it sits in the title bar, over no button of the page).
pub fn slide_in(key: impl Into<SharedString>, notice: impl IntoElement + Styled + 'static) -> impl IntoElement {
    notice.with_spring(ElementId::Name(key.into()), SpringAnimation::new(BOUNCE_SOFT).to(true).from(false), |n, p| n.opacity(p.0.clamp(0., 1.)).mt(px((p.0 - 1.) * 12.)))
}

/// Tiles still on their way: the shapes they will have, breathing.
pub fn waiting_tiles(n: usize, w: f32, gap: f32) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .gap(px(gap))
        .children((0..n).map(|_| div().w(px(w)).h(px(w + 58.)).rounded(px(R_L)).bg(ca(WHITE, 0.045))))
        .with_animation("waiting-tiles", Animation::new(std::time::Duration::from_millis(1300)).repeat().with_max_fps(30.).with_easing(pulsating_between(0.35, 1.)), |d, t| d.opacity(t))
}

/// Rows still on their way (a list of people, of lines): the same, as bars.
pub fn waiting_rows(n: usize, w: f32, h: f32, gap: f32) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .gap(px(gap))
        .children((0..n).map(|_| div().w(px(w)).h(px(h)).rounded(px(R_L)).bg(ca(WHITE, 0.045))))
        .with_animation("waiting-rows", Animation::new(std::time::Duration::from_millis(1300)).repeat().with_max_fps(30.).with_easing(pulsating_between(0.35, 1.)), |d, t| d.opacity(t))
}

/// Words as a search compares them: lower case, Vietnamese accents taken off, đ as d (ui/src/text.ts's fold).
pub fn folded(text: &str) -> String {
    const GROUPS: [(&str, char); 7] = [
        ("àáảãạăằắẳẵặâầấẩẫậ", 'a'),
        ("èéẻẽẹêềếểễệ", 'e'),
        ("ìíỉĩị", 'i'),
        ("òóỏõọôồốổỗộơờớởỡợ", 'o'),
        ("ùúủũụưừứửữự", 'u'),
        ("ỳýỷỹỵ", 'y'),
        ("đ", 'd'),
    ];
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch.is_ascii() { ch } else { GROUPS.iter().find(|(from, _)| from.contains(ch)).map_or(ch, |(_, to)| *to) })
        .collect()
}
