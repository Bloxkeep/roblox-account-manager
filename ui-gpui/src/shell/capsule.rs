// The status bar's pills opening up, as Dynamic Island's do (Apple HIG, Live Activities): a click on a pill's empty
// part (its words and figures, not its buttons) grows it from where it stands into a card with more; a click outside,
// Esc or the pill again closes it. One open at a time; the card lies over the page, it doesn't push it up. The owner
// approved the mock-up (artifacts/mock/con-nhong.html) on 2026-11-19: "ok quá đẳng cấp rồi, áp vào app".
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use super::*;

/// Which pill is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pill {
    Left,
    Music,
    Right,
}

impl Pill {
    pub fn key(self) -> &'static str {
        match self {
            Pill::Left => "bar-left",
            Pill::Music => "bar-music",
            Pill::Right => "bar-right",
        }
    }
}

/// The open card's size: each pill's own (the windows card grows with its windows, up to MAX_ROWS).
const LEFT_W: f32 = 520.;
const MUSIC_W: f32 = 420.;
const MUSIC_H: f32 = 236.;
const RIGHT_W: f32 = 380.;
const RIGHT_H: f32 = 250.;
const MAX_ROWS: usize = 4;
const ROW_H: f32 = 58.;
const CARD_R: f32 = 22.;
/// A sparkline's samples: one a metrics tick (1.5 s), about the last minute.
const SPARK_N: usize = 40;

thread_local! {
    /// Where each pill was drawn last frame, in the window.
    static PILL_AT: RefCell<HashMap<&'static str, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    /// The card on screen: the open one, or the one closing.
    static SHOWN: Cell<Option<Pill>> = const { Cell::new(None) };
    /// The pill whose card stands over it, grown past it: that pill is not drawn. Its 14 px corner showed past the
    /// card's rounder 22 px one (owner, 2026-11-19: "góc bên trái có thụt vào 1 tí").
    static COVER: Cell<Option<&'static str>> = const { Cell::new(None) };
    /// Each open window's RAM over the last minute, for its sparkline (account id → when last taken, the samples).
    static RAM_HISTORY: RefCell<HashMap<String, (Instant, VecDeque<f32>)>> = RefCell::new(HashMap::new());
    static TICK_ASKED: Cell<bool> = const { Cell::new(false) };
    static ROLLS: RefCell<HashMap<&'static str, Roll>> = RefCell::new(HashMap::new());
    #[cfg(feature = "shot")]
    static SHOT_OPENED: Cell<bool> = const { Cell::new(false) };
}

/// Notes where a pill is (drawn inside it): the card grows out of that place.
/// Whether pill `id`'s card is open over it (so the pill is not drawn).
pub(super) fn covers(id: &'static str) -> bool {
    COVER.with(|c| c.get()) == Some(id)
}

pub(super) fn note_at(id: &'static str) -> impl IntoElement {
    canvas(
        move |b, window, _| {
            let moved = PILL_AT.with(|m| m.borrow_mut().insert(id, b).is_none_or(|old| old != b));
            if moved && SHOWN.with(|s| s.get()).is_some() {
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// Keeps each open window's RAM for its sparkline (called every frame the bar is drawn; a sample a metrics tick).
pub(super) fn note_ram(metrics: &Value) {
    let Some(list) = metrics.get("accounts").and_then(Value::as_array) else { return };
    let now = Instant::now();
    RAM_HISTORY.with(|h| {
        let mut h = h.borrow_mut();
        h.retain(|id, _| list.iter().any(|a| a.get("id").and_then(Value::as_str) == Some(id.as_str())));
        for a in list {
            let (Some(id), Some(mb)) = (a.get("id").and_then(Value::as_str), a.get("ramMb").and_then(Value::as_f64)) else { continue };
            let e = h.entry(id.to_string()).or_insert_with(|| (now - Duration::from_secs(5), VecDeque::new()));
            if now.duration_since(e.0) >= Duration::from_millis(1400) {
                e.0 = now;
                e.1.push_back(mb as f32);
                while e.1.len() > SPARK_N {
                    e.1.pop_front();
                }
            }
        }
    });
}

// ── Rolling figures ───────────────────────────────────────────────────────────────────────────────────────────────
/// A figure changing: each digit that changes slides away and the new one comes up in its place, as Apple's
/// numericText does in Live Activities, not jumping.
struct Roll {
    old: String,
    new: String,
    at: Instant,
    up: bool,
}

const ROLL_DUR: f32 = 0.42;

/// The figures' own colours, here and on the bar's tags: CPU, RAM, GPU, then a fourth.
pub(super) const TONES: [u32; 4] = [0x4db8ff, 0xb388ff, 0x3ddc84, 0xffb547];

/// `text` drawn with its changed digits rolling (`key` is the figure's own; one row high, `h`).
pub(super) fn roll(key: &'static str, text: String, h: f32, window: &mut Window) -> Div {
    let now = Instant::now();
    let (old, t, up) = ROLLS.with(|m| {
        let mut m = m.borrow_mut();
        let r = m.entry(key).or_insert_with(|| Roll { old: text.clone(), new: text.clone(), at: now - Duration::from_secs(5), up: true });
        if r.new != text {
            let num = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>().parse::<i64>().unwrap_or(0);
            r.up = num(&text) >= num(&r.new);
            r.old = std::mem::replace(&mut r.new, text.clone());
            r.at = now;
        }
        (r.old.clone(), now.duration_since(r.at).as_secs_f32(), r.up)
    });
    let moving = t < ROLL_DUR * 1.6 && old.chars().count() == text.chars().count() && old != text;
    // One box per character, rolling or still: one run of text is laid out a few pixels narrower (its kerning and
    // sub-pixel places), so going back to it when the roll ended moved everything after the number again (owner's
    // OBS clip, 2026-11-19: 2–3 px at 0.67 s after each change).
    // Rolling or still, every character is laid out on one h line, so nothing about the row changes during a roll.
    let mut row = div().flex().flex_none().line_height(px(h));
    if !moving {
        for n in text.chars() {
            row = row.child(div().flex_none().child(n.to_string()));
        }
        return row;
    }
    window.request_animation_frame();
    let p = apple_spring(t, ROLL_DUR, 0.18);
    let dir = if up { 1. } else { -1. };
    for (o, n) in old.chars().zip(text.chars()) {
        if o == n {
            row = row.child(div().flex_none().child(n.to_string()));
            continue;
        }
        // The new character stays in place unseen, so the box keeps the still text's size and baseline (the figure row
        // lines the number up with its "%" by baseline: a box whose text moved gave another one, and the whole number
        // sat 4–5 px low until the roll ended); the two that roll are drawn over it.
        row = row.child(
            div()
                .relative()
                .flex_none()
                .overflow_hidden()
                .child(div().opacity(0.).child(n.to_string()))
                .child(div().absolute().left_0().top(px((1. - p) * h * dir)).opacity(p.clamp(0., 1.)).child(n.to_string()))
                .child(div().absolute().left_0().top(px(-p * h * dir)).opacity((1. - p).clamp(0., 1.)).child(o.to_string())),
        );
    }
    row
}

impl Root {
    /// A click on a pill's empty part: opens its card, or closes it when it is the open one.
    pub(super) fn pill_toggle(&mut self, p: Pill, cx: &mut Context<Self>) {
        self.pill = if self.pill == Some(p) { None } else { Some(p) };
        cx.notify();
    }

    /// The open pill's card over everything, with the room around it that closes it on a click.
    pub(super) fn pill_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        #[cfg(feature = "shot")]
        if !SHOT_OPENED.with(|s| s.replace(true)) {
            self.pill = match std::env::var("BLOXKEEP_SHOT_PILL").ok().as_deref() {
                Some("left") => Some(Pill::Left),
                Some("music") => Some(Pill::Music),
                Some("right") => Some(Pill::Right),
                _ => self.pill,
            };
        }
        COVER.with(|c| c.set(None));
        // (The song's pill gone with its song: its card closes.)
        if self.pill == Some(Pill::Music) && self.music.is_none() {
            self.pill = None;
        }
        let want = self.pill;
        if want.is_some() {
            SHOWN.with(|s| s.set(want));
        }
        let shown = SHOWN.with(|s| s.get())?;
        let opening = want == Some(shown);
        // Apple's springs: open .5 s with a little bounce; closing, the insides fade first (90 ms), then it shrinks
        // back into the pill without one.
        let p = bar_tween("pill-open", if opening { 1. } else { 0. }, 0., if opening { 0.5 } else { 0.36 }, if opening { 0.3 } else { 0.05 }, if opening { 0. } else { 0.09 }, window);
        let a = bar_tween("pill-words", if opening { 1. } else { 0. }, 0., if opening { 0.22 } else { 0.09 }, 0., if opening { 0.12 } else { 0. }, window);
        if !opening && p <= 0.003 {
            SHOWN.with(|s| s.set(None));
            return None;
        }
        // (Only once it has grown past the pill: at the start and the end of the spring the card is the pill's size
        // and shape, so the swap doesn't show.)
        if p > 0.02 {
            COVER.with(|c| c.set(Some(shown.key())));
        }
        let Some(b) = PILL_AT.with(|m| m.borrow().get(shown.key()).copied()) else {
            window.request_animation_frame();
            return None;
        };
        let (content, w_full, h_full, ground, music_tone) = match shown {
            Pill::Left => {
                let (el, h) = self.left_card(window, cx);
                (el, LEFT_W, h, None, None)
            }
            Pill::Music => {
                let (el, ground, tone) = self.music_card(cx)?;
                (el, MUSIC_W, MUSIC_H, Some(ground), tone)
            }
            Pill::Right => (self.right_card(cx), RIGHT_W, RIGHT_H, None, None),
        };
        let (bx, by, bw, bh) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height));
        let w = bw + (w_full - bw) * p;
        let h = bh + (h_full - bh) * p;
        let view_w = f32::from(window.viewport_size().width);
        let x = match shown {
            Pill::Left => bx,
            Pill::Right => bx + bw - w,
            Pill::Music => bx + bw / 2. - w / 2.,
        }
        .clamp(8., (view_w - w - 8.).max(8.));
        let y = by + bh - h;
        let r = bh / 2. + (CARD_R - bh / 2.) * p.clamp(0., 1.);
        let shade = music_tone.map_or(hsla(0., 0., 0., 0.5), |t| ca(t, 0.32));
        let card = div()
            .id("pill-card")
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(w.max(0.)))
            .h(px(h.max(0.)))
            .rounded(px(r))
            .overflow_hidden()
            .bg(ground.unwrap_or(c(RAISED_2)))
            .shadow(vec![
                BoxShadow { color: hsla(0., 0., 0., 0.35), offset: point(px(0.), px(1.)), blur_radius: px(2.), spread_radius: px(0.), inset: false },
                BoxShadow { color: shade, offset: point(px(0.), px(18.)), blur_radius: px(44.), spread_radius: px(-10.), inset: false },
            ])
            // (Its own clicks stay in it: the room around it closes it.)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .w(px(w_full))
                    .h(px(h_full))
                    .map(|d| match shown {
                        Pill::Left => d.left_0(),
                        Pill::Right => d.right_0(),
                        Pill::Music => d.left(px((w - w_full) / 2.)),
                    })
                    .opacity(a.clamp(0., 1.))
                    .child(content),
            )
            // (The same lit edge as the pill it grew from.)
            .child(div().absolute().inset_0().rounded(px(r)).border_1().border_color(ca(WHITE, if crate::theme::light() { 0.0 } else { 0.06 })))
            .when(!crate::theme::light(), |d| d.child(div().absolute().inset_0().rounded(px(r)).border_t_1().border_color(ca(WHITE, 0.14))));
        Some(
            div()
                .absolute()
                .inset_0()
                .when(opening, |d| {
                    d.child(
                        div()
                            .id("pill-away")
                            .absolute()
                            .inset_0()
                            .occlude()
                            .on_mouse_down(MouseButton::Left, cx.listener(|r, e: &MouseDownEvent, _, cx| {
                                // On another pill: its card at once, this one gone (owner, 2026-11-19: the first click
                                // only closed the open card, a second one was needed to open the other).
                                let other = [Pill::Left, Pill::Music, Pill::Right].into_iter().find(|p| {
                                    Some(*p) != r.pill
                                        && (*p != Pill::Music || r.music.is_some())
                                        && PILL_AT.with(|m| m.borrow().get(p.key()).is_some_and(|b| b.contains(&e.position)))
                                });
                                match other {
                                    Some(p) => {
                                        // (It grows from its own pill, from nothing, as when opened from none.)
                                        super::BAR_TWEENS.with(|m| {
                                            let mut m = m.borrow_mut();
                                            m.remove("pill-open");
                                            m.remove("pill-words");
                                        });
                                        r.pill = Some(p);
                                    }
                                    None => r.pill = None,
                                }
                                cx.notify();
                            }))
                            .on_mouse_down(MouseButton::Right, cx.listener(|r, _, _, cx| {
                                r.pill = None;
                                cx.notify();
                            })),
                    )
                })
                .child(card)
                .into_any_element(),
        )
    }

    // ── The windows card ──────────────────────────────────────────────────────────────────────────────────────────
    /// Roblox's windows and the PC's figures: a ring for each figure, then each window with its game, how long it has
    /// been open, its RAM over the last minute and now; Statistics and Close all at its foot.
    fn left_card(&self, window: &mut Window, cx: &mut Context<Self>) -> (AnyElement, f32) {
        let m = &self.metrics;
        let windows = m.get("robloxCount").and_then(Value::as_i64).unwrap_or(0);
        let ram = m.get("usedPct").and_then(Value::as_f64).unwrap_or(0.);
        let cpu = m.get("pcCpuPct").and_then(Value::as_f64).unwrap_or(0.);
        let gpu = m.get("gpuPct").and_then(Value::as_f64).filter(|_| windows > 0);
        let open: Vec<Value> = self.accounts().iter().filter(|a| pages::accounts::is_local(a)).cloned().collect();
        let since = open.iter().filter_map(|a| a.pointer("/session/since").and_then(Value::as_str).and_then(crate::time::iso_secs)).min();
        let ram_of = |id: &str| m.get("accounts").and_then(Value::as_array).and_then(|l| l.iter().find(|x| x.get("id").and_then(Value::as_str) == Some(id))).and_then(|x| x.get("ramMb")?.as_f64());
        let tones = TONES;
        let gauge = |name: &str, pct: f64, tone: u32| {
            let col = if pct >= LOAD_FULL { c(BAD) } else if pct >= LOAD_BUSY { c(WARN) } else { c(tone) };
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(3.))
                .child(div().relative().size(px(46.)).child(ring(pct as f32, 46., 4., col)).child(div().absolute().inset_0().flex().items_center().justify_center().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(format!("{}", pct.round() as i64))))
                .child(div().text_size(px(9.5)).font_weight(FontWeight::SEMIBOLD).text_color(ca(TEXT, 0.4)).child(name.to_string()))
        };
        let title = if windows > 0 { self.i18n.tn("pill.windows", windows, &[]) } else { self.tr("pill.noWindows") };
        let sub = since.map(|t| self.i18n.tv("pill.openFor", &[("t", &crate::time::clock(crate::time::now_secs() - t))]));
        let head = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(div().size(px(40.)).flex_none().rounded(px(12.)).bg(ca(WHITE, 0.07)).flex().items_center().justify_center().child(icon("roblox", 18., c(TEXT))))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).truncate().child(title))
                    .when_some(sub, |d, s| d.child(div().mt(px(2.)).text_size(px(T_S)).text_color(ca(TEXT, 0.6)).font_features(tnum()).child(s))),
            )
            .child(div().flex().gap(px(14.)).child(gauge("CPU", cpu, tones[0])).child(gauge("RAM", ram, tones[1])).when_some(gpu, |d, g| d.child(gauge("GPU", g, tones[2]))));
        let shown: Vec<&Value> = open.iter().take(MAX_ROWS).collect();
        let mut list = div().mt(px(14.)).flex().flex_col().gap(px(6.));
        if shown.is_empty() {
            list = list.child(div().h(px(40.)).flex().items_center().px(px(4.)).text_color(ca(TEXT, 0.55)).child(self.tr("bar.none")));
        }
        for (i, a) in shown.iter().enumerate() {
            let id = pages::accounts::label(a);
            let acc_id = a.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
            let game = a.pointer("/session/game/name").and_then(Value::as_str).filter(|g| !g.is_empty()).map(crate::pages::home::clean);
            let up = a.pointer("/session/since").and_then(Value::as_str).and_then(crate::time::iso_secs).map(|t| crate::time::clock(crate::time::now_secs() - t));
            let sub = [game, up].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            let icon_url = a.pointer("/session/game/iconUrl").and_then(Value::as_str).filter(|u| !u.is_empty()).map(str::to_string);
            let thumb = icon_url.and_then(|u| images::pic(&u, cx));
            let samples = RAM_HISTORY.with(|h| h.borrow().get(&acc_id).map(|(_, s)| s.iter().copied().collect::<Vec<f32>>()).unwrap_or_default());
            let tone = c(tones[i % tones.len()]);
            let ram_words = ram_of(&acc_id).map(|mb| pages::accounts::mb_text(mb)).unwrap_or_default();
            list = list.child(
                div()
                    .id(SharedString::from(format!("pill-win-{i}")))
                    .h(px(ROW_H))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .rounded(px(14.))
                    .bg(ca(WHITE, 0.04))
                    .hover(|d| d.bg(ca(WHITE, 0.07)))
                    .cursor_pointer()
                    .on_click(cx.listener(|r, _, _, cx| {
                        r.pill = None;
                        crate::pages::accounts::set_win_view(r, true);
                        r.go(Page::Accounts);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .relative()
                            .size(px(42.))
                            .flex_none()
                            .child(match thumb {
                                Some(p) => img(p).size(px(42.)).rounded(px(11.)).object_fit(ObjectFit::Cover).into_any_element(),
                                None => div().size(px(42.)).rounded(px(11.)).bg(ca(WHITE, 0.07)).flex().items_center().justify_center().child(icon("gamepad", 18., ca(TEXT, 0.6))).into_any_element(),
                            })
                            .child(div().absolute().right(px(-5.)).bottom(px(-5.)).child(pages::accounts::stacked(a, 22., RAISED_2, cx))),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).truncate().child(id))
                            .child(div().mt(px(1.)).text_size(px(11.5)).text_color(ca(TEXT, 0.55)).truncate().font_features(tnum()).child(sub)),
                    )
                    .child(spark(samples, 96., 28., tone))
                    .child(
                        div()
                            .w(px(66.))
                            .flex_none()
                            .flex()
                            .flex_col()
                            .items_end()
                            .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).font_features(tnum()).child(ram_words))
                            .child(div().text_size(px(9.5)).font_weight(FontWeight::SEMIBOLD).text_color(ca(TEXT, 0.4)).child("RAM")),
                    ),
            );
        }
        let rows = shown.len();
        let list_h = if rows == 0 { 40. } else { rows as f32 * ROW_H + (rows - 1) as f32 * 6. };
        // (The head is the rings' height: 46 + 3 + their word's line.)
        let h = 16. + 62. + 14. + list_h + 14. + 32. + 18.;
        let foot = div()
            .mt(px(14.))
            .flex()
            .gap(px(8.))
            .child(no_pick(Button::new("pill-stats", self.tr("nav.stats")).icon("gauge").small().pill().on_click(cx.listener(|r, _, _, cx| {
                r.pill = None;
                r.go(Page::Stats);
                cx.notify();
            }))))
            .child(no_pick(Button::new("pill-clean-ram", self.tr("ram.clean")).icon("shield").small().pill().disabled(crate::clean_ram::running()).tip(self.tr("ram.cleanDesc")).on_click(cx.listener(|r, _, w, cx| {
                // (The pop-up goes, as Statistics does: left open, it stayed dimmed behind the panel.)
                r.pill = None;
                crate::clean_ram::open(r, w, cx);
            }))))
            .when(windows > 0, |d| {
                d.child(no_pick(Button::new("pill-close-all", self.tr("sys.closeAll")).icon("power").small().pill().on_click(cx.listener(|r, _, window, cx| {
                    r.pill = None;
                    r.close_all(false, window, cx);
                }))))
            });
        let _ = window;
        (div().size_full().px(px(18.)).py(px(16.)).flex().flex_col().text_size(px(T_M)).child(head).child(list).child(foot).into_any_element(), h)
    }

    // ── The song card ─────────────────────────────────────────────────────────────────────────────────────────────
    /// The song large: its cover, title and artist, where it is, and the keys; on its cover's own colour, dark.
    fn music_card(&self, cx: &mut Context<Self>) -> Option<(AnyElement, Hsla, Option<u32>)> {
        let s = self.music.clone()?;
        let cover = s.cover.as_deref().and_then(|p| images::pic(p, cx));
        let tone = s.cover.as_deref().and_then(|p| images::tone(p, cx));
        let ground = tone.map_or(hsla(0., 0., 0.13, 1.), |t| {
            let h = c(t);
            hsla(h.h, h.s.min(0.45), 0.16, 1.)
        });
        let white = |a: f32| hsla(0., 0., 1., a);
        // While it plays, where it is moves on: drawn again as the song's next second starts, while the card is open.
        // (Once a second from whenever it was drawn, the count turned over anywhere in the second, so one second
        // showed for longer and the next for less: the owner, 2026-11-24.) A little after the turn, so the count it
        // reads is past it even with Windows' timers a tick late.
        if s.playing && !TICK_ASKED.with(|t| t.replace(true)) {
            let wait = s.at().map_or(1000, |(at, _)| 1000 - at % 1000 + 20);
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(wait)).await;
                TICK_ASKED.with(|t| t.set(false));
                this.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
        }
        let mmss = |ms: u64| format!("{}:{:02}", ms / 60_000, ms / 1000 % 60);
        let at = s.at().filter(|(_, len)| *len > 0);
        let inner = MUSIC_W - 40.;
        let top = div()
            .flex()
            .items_center()
            .gap(px(16.))
            .child(match cover {
                Some(p) => img(p).size(px(84.)).flex_none().rounded(px(14.)).object_fit(ObjectFit::Cover).into_any_element(),
                None => div().size(px(84.)).flex_none().rounded(px(14.)).bg(white(0.08)).flex().items_center().justify_center().child(icon(s.player.icon(), 30., c(s.player.colour()))).into_any_element(),
            })
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(white(1.)).truncate().child(s.title.clone()))
                    .when(!s.artist.trim().is_empty(), |d| d.child(div().mt(px(2.)).text_color(white(0.66)).truncate().child(s.artist.clone())))
                    .child(div().mt(px(10.)).child(playing_bars("pill-eq", s.playing, tone.map_or(c(OK), |t| lift(c(t)))))),
            )
            .child(div().self_start().flex_none().child(icon(s.player.icon(), 20., c(s.player.colour()))));
        let progress = at.map(|(pos, len)| {
            let part = (pos as f32 / len as f32).clamp(0., 1.);
            div()
                .mt(px(18.))
                .child(div().relative().w(px(inner)).h(px(5.)).rounded(px(3.)).bg(white(0.2)).child(div().absolute().left_0().top_0().h_full().w(px(inner * part)).rounded(px(3.)).bg(white(1.))))
                .child(div().mt(px(6.)).flex().justify_between().text_size(px(11.)).text_color(white(0.6)).font_features(tnum()).child(mmss(pos)).child(format!("-{}", mmss(len.saturating_sub(pos)))))
        });
        use crate::media::Act;
        let key = |id: &'static str, name: &'static str, act: Act, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(44.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .hover(|d| d.bg(hsla(0., 0., 1., 0.1)))
                .cursor_pointer()
                .child(icon(name, 22., hsla(0., 0., 1., 1.)))
                .on_click(cx.listener(move |r, _, _, cx| music_go(r, act, cx)))
        };
        let keys = div()
            .mt(px(if at.is_some() { 6. } else { 24. }))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(30.))
            .child(key("pill-back", "skipBack", Act::Back, cx))
            .child(
                div()
                    .id("pill-play")
                    .size(px(54.))
                    .rounded_full()
                    .bg(white(0.94))
                    .hover(|d| d.bg(hsla(0., 0., 1., 1.)))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(if s.playing { "pause" } else { "play" }, 22., hsla(0., 0., 0.08, 1.)))
                    .on_click(cx.listener(|r, _, _, cx| music_go(r, Act::Toggle, cx))),
            )
            .child(key("pill-next", "skipNext", Act::Next, cx));
        Some((div().size_full().px(px(20.)).py(px(18.)).flex().flex_col().text_size(px(T_M)).child(top).children(progress).child(keys).into_any_element(), ground, tone))
    }

    // ── The versions card ─────────────────────────────────────────────────────────────────────────────────────────
    /// Four tiles: Roblox's version, Multi-Roblox, the mini window and Bloxkeep's version; the two that turn on are
    /// yellow while on, as Control Center's tiles.
    fn right_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let on = |p: &str| self.state.pointer(p).and_then(Value::as_bool).unwrap_or(false);
        let multi = on("/multi/enabled");
        let armed = on("/multi/armed");
        let closed = multi && on("/robloxUpdates/closed");
        let check = self.state.pointer("/robloxUpdates/watch/check").and_then(Value::as_str).unwrap_or_default();
        let watched = multi && armed && !closed && ["changed", "renamed", "gone"].contains(&check) && !on("/robloxUpdates/watch/worked");
        let to_updates = closed || watched;
        let multi_warn = to_updates || (multi && !armed);
        let mini_open = on("/miniOpen");
        let version = self.state.pointer("/update/current").and_then(Value::as_str).unwrap_or_default().to_string();
        let ready = self.state.pointer("/update/status").and_then(Value::as_str) == Some("ready") && !on("/update/quiet");
        let no_roblox = self.state.get("robloxInstalled").and_then(Value::as_bool) == Some(false);
        let rbx = (!no_roblox).then(|| self.roblox_version_words()).flatten();
        let tile_w = (RIGHT_W - 32. - 10.) / 2.;
        let tile = |id: &'static str, lit: bool| {
            div()
                .id(id)
                .w(px(tile_w))
                .h(px(104.))
                .flex_none()
                .rounded(px(20.))
                .px(px(14.))
                .py(px(13.))
                .flex()
                .flex_col()
                .cursor_pointer()
                .bg(if lit { c(FACE) } else { ca(WHITE, 0.06) })
                .hover(move |d| d.bg(if lit { c(FACE_HI) } else { ca(WHITE, 0.09) }))
                .text_color(if lit { c(INK) } else { c(TEXT) })
        };
        let mark = |name: &'static str, lit: bool| div().size(px(30.)).rounded_full().bg(if lit { hsla(0., 0., 0., 0.12) } else { ca(WHITE, 0.1) }).flex().items_center().justify_center().child(icon(name, 15., if lit { c(INK) } else { c(TEXT) }));
        let sub = |s: String, lit: bool, warn: bool| div().mt(px(1.)).text_size(px(11.5)).truncate().text_color(if warn { c(WARN) } else if lit { ca(INK, 0.7) } else { ca(TEXT, 0.6) }).child(s);
        let big = |s: String| div().mt_auto().text_size(px(20.)).line_height(px(22.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).truncate().child(s);
        let name = |s: String| div().mt_auto().font_weight(FontWeight::SEMIBOLD).truncate().child(s);

        // Roblox's version: up to date, or what is newer (orange); leads to Settings › Roblox version.
        let roblox = {
            let (words, old) = rbx.as_ref().map_or((if no_roblox { "—".to_string() } else { "…".to_string() }, false), |(v, old, _)| (v.clone(), *old));
            let up_to_date = rbx.is_some() && !old;
            tile("pill-rbx", false)
                .child(mark("roblox", false))
                .child(
                    div()
                        .mt_auto()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(div().text_size(px(20.)).line_height(px(22.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(if old { c(WARN) } else { c(TEXT) }).child(words))
                        .when(up_to_date, |d| d.child(div().px(px(7.)).py(px(2.)).rounded_full().bg(ca(OK, 0.16)).text_color(c(OK)).text_size(px(10.5)).font_weight(FontWeight::SEMIBOLD).child(self.tr("pill.latest")))),
                )
                .child(sub("Roblox".into(), false, false))
                .on_click(cx.listener(|r, _, _, cx| {
                    r.pill = None;
                    r.settings.part = pages::settings::Part::Roblox;
                    r.go(Page::Settings);
                    cx.notify();
                }))
        };
        // Multi-Roblox: on, off, or needing a look (orange; then it leads to Settings › Roblox updates).
        let multi_tile = {
            let words = if multi_warn { self.tr("pill.check") } else if multi { self.tr("pill.on") } else { self.tr("pill.off") };
            tile("pill-multi", multi && !multi_warn)
                .child(mark("layers", multi && !multi_warn))
                .child(name(self.tr("multi.title")))
                .child(sub(words, multi && !multi_warn, multi_warn))
                .on_click(cx.listener(move |r, _, window, cx| {
                    if to_updates {
                        r.pill = None;
                        r.settings.part = pages::settings::Part::Roblox;
                        r.go(Page::Settings);
                    } else if !multi {
                        // Turning it on asks first (Roblox's rules), as the bar's switch does.
                        r.pill = None;
                        r.confirm(r.tr("set.multiConfirmTitle"), r.tr("set.multiConfirm"), r.tr("set.multiOk"), false, Some("multi"), window, cx, |r, _| {
                            r.send("setSettings", json!({ "multiInstance": true }))
                        });
                    } else {
                        r.send("setSettings", json!({ "multiInstance": false }));
                    }
                    cx.notify();
                }))
        };
        let mini = tile("pill-mini", mini_open)
            .child(mark("screens", mini_open))
            .child(name(self.tr("sys.mini")))
            .child(sub(self.tr(if mini_open { "pill.on" } else { "pill.off" }), mini_open, false))
            .on_click(cx.listener(|r, _, _, _| r.send("toggleMini", json!({}))));
        // Bloxkeep's version: What's new; an update in and waiting, Update (it restarts the app, asked first).
        let app = tile("pill-app", false)
            // (Bloxkeep's own logo, tile and all, as logo.svg has it: the small upright blocks read as another face, the owner,
            // 2026-11-24 "không giống logo bloxkeep".)
            .child(div().flex_none().size(px(30.)).child(crate::crisp::mark(30.)))
            .child(big(if version.is_empty() { "Bloxkeep".into() } else { version }))
            .child(sub(if ready { self.tr("bar.update") } else { format!("Bloxkeep · {}", self.tr("pill.news")) }, false, false).when(ready, |d| d.text_color(c(FACE))))
            .on_click(cx.listener(move |r, _, _, cx| {
                r.pill = None;
                if ready {
                    r.ask("applyUpdate", json!({}), cx, |_, _| {});
                } else {
                    crate::pages::extras::ask_whats_new(r, cx);
                }
                cx.notify();
            }));
        div().size_full().p(px(16.)).flex().flex_wrap().gap(px(10.)).text_size(px(T_M)).child(roblox).child(multi_tile).child(mini).child(app).into_any_element()
    }
}

/// A colour light enough to read on the song's dark ground (the cover's own colour, lifted).
fn lift(h: Hsla) -> Hsla {
    hsla(h.h, h.s.max(0.55), h.l.max(0.62), 1.)
}

/// A small line of the last minute's samples, `w` × `h`, its own scale (lowest to highest seen).
fn spark(samples: Vec<f32>, w: f32, h: f32, tone: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            if samples.len() < 2 {
                return;
            }
            let (lo, hi) = samples.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
            let span = (hi - lo).max(hi * 0.05).max(1.);
            let (x0, y0) = (f32::from(b.origin.x), f32::from(b.origin.y));
            let step = w / (SPARK_N - 1) as f32;
            let start = w - step * (samples.len() - 1) as f32;
            let at = |i: usize, v: f32| point(px(x0 + start + step * i as f32), px(y0 + 2. + (h - 4.) * (1. - (v - lo) / span)));
            let mut pb = PathBuilder::stroke(px(1.6));
            pb.move_to(at(0, samples[0]));
            for (i, v) in samples.iter().enumerate().skip(1) {
                pb.line_to(at(i, *v));
            }
            if let Ok(path) = pb.build() {
                window.paint_path(path, tone);
            }
        },
    )
    .w(px(w))
    .h(px(h))
    .flex_none()
}
