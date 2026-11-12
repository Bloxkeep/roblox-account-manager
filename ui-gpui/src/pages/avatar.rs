// Avatar, as the Roblox app has it, inside Games (its header's switch): the avatar of the account Friends looks
// through (the same pick, pages/friends.rs): its full picture, what it wears (take one off), its saved outfits
// (wear one) and its inventory by shelf (put one thing on); and, what Roblox has not, its history: every look this PC
// has seen it in is kept (roblox.rs), and any of them can be put on again. Reading it needs no sign-in (Roblox shows avatars to anyone); changing it is asked from here (roblox.rs)
// with that account's own sign-in, lent by the app. Roblox draws the new picture a moment after a change, so the
// avatar is read again then.
use gpui_kit::{prelude::*, *};

use crate::{
    images,
    kit::*,
    pages::{friends, games::thousands},
    roblox::{self, Avatar, Owned, Price, Worth},
    shell::Root,
    theme::*,
};

#[derive(Default)]
pub struct Data {
    /// The Roblox user this avatar is of, and the avatar (None while the first one loads).
    of: i64,
    pub got: Option<Result<Avatar, String>>,
    loading: bool,
    /// The item or outfit a change is under way for, and why Roblox turned the last one down.
    busy: Option<i64>,
    pub refused: Option<String>,
    /// The inventory: the shelf shown, what it holds so far (None while its first page loads), the mark for its next
    /// page, and the shelf a page is being read for.
    pub shelf: usize,
    owned: Option<Result<Vec<Owned>, String>>,
    next: String,
    reading: Option<usize>,
    /// What the worn things are worth (None until Roblox has said), the things it was asked for, and whether an
    /// asking is under way.
    worth: Option<Worth>,
    worth_of: Vec<i64>,
    pricing: bool,
    /// The avatar with depth (mesh.rs): Roblox's model of it once asked for, the way it is turned, the picture drawn
    /// of it, whether one is being drawn (and another is wanted after it), and a drag under way (where it began, and
    /// the turn then).
    asked_solid: bool,
    solid: Option<std::sync::Arc<crate::mesh::Model>>,
    yaw: f32,
    frame: Option<std::sync::Arc<RenderImage>>,
    drawing: bool,
    stale: bool,
    drag: Option<(f32, f32)>,
}

impl Data {
    /// Put away with its tab (pages/tabs.rs): nothing is on its way any more, so what is missing is asked again.
    pub fn rest(&mut self) {
        self.loading = false;
        self.busy = None;
        self.reading = None;
        self.pricing = false;
        // (A picture being drawn is dropped too: another is drawn when the tab is back.)
        self.drawing = false;
        self.stale = false;
    }
}

/// What can be done to the avatar from here.
#[derive(Clone)]
enum Change {
    Off(i64),
    Outfit(i64),
    On(i64, String),
    Rig(bool),
    Colour(String),
    Scale(usize, f64),
    /// Back to a look of the history, by its place in it.
    Look(usize),
    /// An emote off the wheel: the emote, and its place there.
    EmoteOff(i64, i64),
}

const TILE: f32 = 150.;
const TGAP: f32 = 14.;

pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    friends::want_sign(r, link, cx);
    let Some((_, user, _)) = friends::signed(r) else { return };
    if (r.avatar.of != user || r.avatar.got.is_none()) && !r.avatar.loading {
        if r.avatar.of != user {
            r.avatar = Data { of: user, ..Default::default() };
        }
        load(r, cx);
    }
    if r.avatar.of == user && r.avatar.owned.is_none() && r.avatar.reading.is_none() {
        shelf_page(r, cx);
    }
    if r.avatar.of == user && matches!(r.avatar.got, Some(Ok(_))) && !r.avatar.asked_solid {
        solid(r, cx);
    }
    price(r, cx);
}

/// The side of the square the avatar is drawn into, in dots (the box is 312 wide, 468 dots at 150 %).
const STAGE: usize = 560;
/// How far round the avatar stands when it first shows; it turns to face you from there.
const TURN_IN: f32 = -0.9;

/// Roblox's model of the avatar, asked once it shows and again after a change. With it the picture can be turned
/// (a drag on it); without it (no sign-in, or Roblox gives none) the flat picture stays.
fn solid(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    r.avatar.asked_solid = true;
    let shot = cfg!(feature = "shot") && std::env::var("BLOXKEEP_SHOT_MODEL").is_ok();
    if cookie.is_empty() && !shot {
        return;
    }
    let rx = roblox::run(move || {
        let s = roblox::solid(&cookie, user).ok()?;
        crate::mesh::Model::parse(&s.obj, &s.mtl, &s.pictures, s.eye, s.min, s.max).map(std::sync::Arc::new)
    });
    cx.spawn(async move |this, cx| {
        let Some(model) = rx.await.ok().flatten() else { return };
        // BLOXKEEP_SHOT_YAW=1.2: a hidden shot's avatar stands turned that far, at once.
        let fixed: Option<f32> = if cfg!(feature = "shot") { std::env::var("BLOXKEEP_SHOT_YAW").ok().and_then(|y| y.parse().ok()) } else { None };
        // A model that replaces one (after a change) stands as the old one did; the first turns in to face you.
        let first = this
            .update(cx, |r, cx| {
                if r.avatar.of != user {
                    return false;
                }
                let first = r.avatar.solid.is_none();
                r.avatar.solid = Some(model);
                if let Some(y) = fixed {
                    r.avatar.yaw = y;
                } else if first {
                    r.avatar.yaw = TURN_IN;
                }
                draw(r, cx);
                first && fixed.is_none()
            })
            .unwrap_or(false);
        if !first {
            return;
        }
        for i in 1..=26 {
            cx.background_executor().timer(std::time::Duration::from_millis(22)).await;
            let k = i as f32 / 26.;
            let on = this
                .update(cx, |r, cx| {
                    if r.avatar.of != user || r.avatar.drag.is_some() {
                        return false;
                    }
                    r.avatar.yaw = TURN_IN * (1. - k).powi(3);
                    draw(r, cx);
                    true
                })
                .unwrap_or(false);
            if !on {
                break;
            }
        }
    })
    .detach();
}

/// Draws the model as it is turned now, off the window's thread. One picture at a time: a turn asked for while one is
/// being drawn is drawn after it.
fn draw(r: &mut Root, cx: &mut Context<Root>) {
    let Some(model) = r.avatar.solid.clone() else { return };
    if r.avatar.drawing {
        r.avatar.stale = true;
        return;
    }
    r.avatar.drawing = true;
    let (yaw, user) = (r.avatar.yaw, r.avatar.of);
    let rx = roblox::run(move || model.draw(yaw, STAGE));
    cx.spawn(async move |this, cx| {
        let dots = rx.await.ok();
        let _ = this.update(cx, |r, cx| {
            // (Another account's avatar by now: its Data was put away with its tab, which let go of `drawing`, or is
            // a new one.)
            if r.avatar.of != user {
                return;
            }
            r.avatar.drawing = false;
            if let Some(buffer) = dots.and_then(|dots| image::RgbaImage::from_raw(STAGE as u32, STAGE as u32, dots)) {
                let new = std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
                // (The picture it replaces is taken off the graphics card: a drag draws many.)
                if let Some(old) = r.avatar.frame.replace(new) {
                    cx.drop_image(old, None);
                }
            }
            if r.avatar.stale {
                r.avatar.stale = false;
                draw(r, cx);
            }
            cx.notify();
        });
    })
    .detach();
}

/// What the worn things are worth, read once the avatar shows and again when what is worn changes (roblox::worth). An
/// asking that fails isn't made again for the same things until the avatar is refreshed.
fn price(r: &mut Root, cx: &mut Context<Root>) {
    let Some(Ok(a)) = &r.avatar.got else { return };
    let mut ids: Vec<i64> = a.items.iter().map(|i| i.id).collect();
    ids.sort_unstable();
    if r.avatar.pricing || ids.is_empty() || r.avatar.worth_of == ids {
        return;
    }
    let user = r.avatar.of;
    r.avatar.pricing = true;
    let asked = ids.clone();
    let rx = roblox::run(move || roblox::worth(&ids));
    cx.spawn(async move |this, cx| {
        let got = rx.await.ok().and_then(Result::ok);
        let _ = this.update(cx, |r, cx| {
            // Another account's tab is shown by now: asked again when this one comes back.
            if r.avatar.of != user || !r.avatar.pricing {
                return;
            }
            r.avatar.pricing = false;
            r.avatar.worth_of = asked;
            if let Some(w) = got {
                r.avatar.worth.get_or_insert_with(Default::default).extend(w);
            }
            cx.notify();
        });
    })
    .detach();
}

/// The shelf's next page (its first, when it holds nothing yet).
fn shelf_page(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let shelf = r.avatar.shelf;
    let kinds = roblox::SHELVES[shelf].1;
    let cursor = if r.avatar.owned.is_some() { r.avatar.next.clone() } else { String::new() };
    r.avatar.reading = Some(shelf);
    let rx = roblox::run(move || roblox::inventory(&cookie, user, kinds, &cursor));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            // A page for a shelf or an account no longer shown is dropped.
            if r.avatar.of != user || r.avatar.reading != Some(shelf) {
                return;
            }
            r.avatar.reading = None;
            if r.avatar.shelf != shelf {
                return;
            }
            match (got, &mut r.avatar.owned) {
                (Ok((page, next)), Some(Ok(have))) => {
                    have.extend(page);
                    r.avatar.next = next;
                }
                (Ok((page, next)), _) => {
                    r.avatar.owned = Some(Ok(page));
                    r.avatar.next = next;
                }
                (Err(e), Some(Ok(_))) => r.avatar.refused = Some(if e.is_empty() { "Roblox".into() } else { e }),
                (Err(e), _) => r.avatar.owned = Some(Err(e)),
            }
            cx.notify();
        });
    })
    .detach();
}

fn pick_shelf(r: &mut Root, shelf: usize, cx: &mut Context<Root>) {
    if r.avatar.shelf != shelf {
        r.avatar.shelf = shelf;
        r.avatar.owned = None;
        r.avatar.next.clear();
        r.avatar.reading = None;
        shelf_page(r, cx);
        cx.notify();
    }
}

fn load(r: &mut Root, cx: &mut Context<Root>) {
    let user = r.avatar.of;
    r.avatar.loading = true;
    let rx = roblox::run(move || roblox::avatar(user));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.avatar.of == user {
                // A read that fails after a change keeps what was shown.
                if got.is_ok() || !matches!(r.avatar.got, Some(Ok(_))) {
                    r.avatar.got = Some(got);
                }
                r.avatar.loading = false;
                r.avatar.busy = None;
                price(r, cx);
                cx.notify();
            }
        });
    })
    .detach();
}

fn again(r: &mut Root, cx: &mut Context<Root>) {
    if !r.avatar.loading {
        if r.avatar.got.as_ref().is_some_and(|g| g.is_err()) {
            r.avatar.got = None;
        }
        r.avatar.worth_of.clear();
        load(r, cx);
        cx.notify();
    }
}

/// A change for the account looked through: one thing off, a saved outfit on, or one owned thing on. Then the avatar
/// is read again, a moment later so Roblox has started on the new picture.
fn change(r: &mut Root, what: Change, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    if r.avatar.busy.is_some() || user != r.avatar.of {
        return;
    }
    // (The body's changes have no thing of their own to fade: a mark below zero says one is under way.)
    r.avatar.busy = Some(match &what {
        Change::Off(id) | Change::Outfit(id) | Change::On(id, _) | Change::EmoteOff(id, _) => *id,
        Change::Rig(_) | Change::Colour(_) | Change::Scale(..) => -1,
        Change::Look(i) => -2 - *i as i64,
    });
    let look = match (&what, &r.avatar.got) {
        (Change::Look(i), Some(Ok(a))) => a.looks.get(*i).cloned(),
        _ => None,
    };
    r.avatar.refused = None;
    cx.notify();
    let rx = roblox::run(move || {
        let done = match &what {
            Change::Off(id) => roblox::take_off(&cookie, user, *id),
            Change::Outfit(id) => roblox::wear_outfit(&cookie, *id),
            Change::On(id, kind) => roblox::put_on(&cookie, user, *id, kind),
            Change::Rig(r15) => roblox::set_rig(&cookie, *r15),
            Change::Colour(hex) => roblox::set_colour(&cookie, hex),
            Change::Scale(which, value) => roblox::set_scale(&cookie, user, *which, *value),
            Change::Look(_) => look.as_ref().ok_or(String::new()).and_then(|l| roblox::wear_look(&cookie, l)),
            Change::EmoteOff(_, slot) => roblox::clear_emote(&cookie, *slot),
        };
        if done.is_ok() {
            std::thread::sleep(std::time::Duration::from_millis(1200));
        }
        done
    });
    cx.spawn(async move |this, cx| {
        let done = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.avatar.of != user {
                return;
            }
            match done {
                Ok(()) => {
                    r.avatar.asked_solid = false;
                    load(r, cx)
                }
                Err(why) => {
                    r.avatar.busy = None;
                    r.avatar.refused = Some(if why.is_empty() { "Roblox".into() } else { why });
                }
            }
            cx.notify();
        });
    })
    .detach();
}

fn note(text: String) -> Div {
    div().flex_1().flex().items_center().justify_center().text_size(px(T_M)).text_color(c(MUTED)).child(text)
}

pub fn picture(url: &str, cx: &mut App) -> Option<std::sync::Arc<Image>> {
    (!url.is_empty()).then(|| images::pic(url, cx)).flatten()
}

pub fn avatar(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let av = &root.avatar;
    let (able, viewer) = friends::looking(root);
    let shell = div().id("avatar").size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();

    let header = div()
        .flex_none()
        .px(px(PAD + 4.))
        .pt(px(18.))
        .pb(px(18.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(div().text_size(px(T_TITLE)).font_weight(FontWeight::BOLD).child("Roblox"))
        .child(div().ml(px(6.)).child(crate::pages::games::views(root, cx)))
        .child(div().flex_1())
        .when(viewer.is_some(), |d| {
            d.child(IconButton::new("av-refresh", "refresh").size(34.).busy(av.loading).tip(root.tr("fr.refresh")).on_click(cx.listener(|r, _, _, cx| again(r, cx))))
        });

    let body: AnyElement = if able.is_empty() || friends::nobody(root) {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("fr.noAccount")))
            .child(Button::new("av-add-acc", root.tr("acc.add")).icon("plus").primary().on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
            .into_any_element()
    } else {
        match &av.got {
            None => note(root.tr("av.loading")).into_any_element(),
            Some(Err(why)) => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(14.))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("av.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])))
                .child(Button::new("av-retry", root.tr("fr.retry")).icon("refresh").on_click(cx.listener(|r, _, _, cx| again(r, cx))))
                .into_any_element(),
            Some(Ok(a)) => filled(root, a, width, cx).into_any_element(),
        }
    };
    shell.child(header).child(body)
}

/// The picture on the left (above, in a narrow window), what it wears and the saved outfits beside it.
fn filled(root: &Root, a: &Avatar, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let av = &root.avatar;
    let inner = width - 2. * (PAD + 4.);
    let wide = inner >= 760.;
    let side = if wide { 340. } else { inner };
    let lists_w = if wide { inner - side - 24. } else { inner };
    let cols = ((lists_w + TGAP) / (TILE + TGAP)).floor().max(1.);
    let tile_w = ((lists_w - TGAP * (cols - 1.)) / cols).floor();
    let busy = av.busy;

    let portrait = surface()
        .w(px(side))
        .flex_none()
        .p(px(14.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .id("av-stage")
                .w_full()
                .h(px(if wide { side - 28. } else { 300. }))
                .rounded(px(R_M))
                .bg(c(INSET))
                .flex()
                .items_center()
                .justify_center()
                .map(|d| match av.frame.clone() {
                    // The avatar with depth: a drag across it turns it.
                    Some(frame) => d
                        .cursor_grab()
                        .child(img(frame).h_full().object_fit(ObjectFit::Contain))
                        .on_mouse_down(MouseButton::Left, cx.listener(|r, e: &MouseDownEvent, _, _| r.avatar.drag = Some((f32::from(e.position.x), r.avatar.yaw))))
                        .on_mouse_move(cx.listener(|r, e: &MouseMoveEvent, _, cx| {
                            if e.pressed_button != Some(MouseButton::Left) {
                                r.avatar.drag = None;
                            } else if let Some((from, was)) = r.avatar.drag {
                                r.avatar.yaw = was + (f32::from(e.position.x) - from) * 0.012;
                                draw(r, cx);
                            }
                        }))
                        .on_mouse_up(MouseButton::Left, cx.listener(|r, _, _, _| r.avatar.drag = None))
                        .on_mouse_up_out(MouseButton::Left, cx.listener(|r, _, _, _| r.avatar.drag = None)),
                    None => d.when_some(picture(&a.pic, cx), |d, p| d.child(fade_in(img(p.clone()).h_full().object_fit(ObjectFit::Contain), &p))),
                }),
        )
        .children(worth(root, a))
        .child(body(root, a, cx));

    let section = |title: String, count: Option<usize>, tiles: Vec<AnyElement>, none: &str| {
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(section_head(title, count))
            .map(|d| if tiles.is_empty() { d.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr(none))) } else { d.child(div().flex().flex_wrap().gap(px(TGAP)).children(tiles)) })
    };

    let worn: Vec<AnyElement> = a
        .items
        .iter()
        .map(|it| {
            let id = it.id;
            tile_of(tile_w, &it.pic, it.name.clone(), busy == Some(id), cx)
                .child(price_row(root, av.worth.as_ref().and_then(|w| w.get(&id)).copied()))
                .child(
                    div().absolute().top(px(6.)).right(px(6.)).child(
                        IconButton::new(("av-off", id as usize), "x")
                            .size(26.)
                            .filled()
                            .disabled(busy.is_some())
                            .tip(root.tr("av.takeOff"))
                            .on_click(cx.listener(move |r, _, _, cx| change(r, Change::Off(id), cx))),
                    ),
                )
                .into_any_element()
        })
        .collect();
    // The wheel: each emote with its place on it, and a way off.
    let wheel: Vec<AnyElement> = a
        .emotes
        .iter()
        .map(|e| {
            let (id, slot) = (e.id, e.slot);
            tile_of(tile_w, &e.pic, e.name.clone(), busy == Some(id), cx)
                .child(div().absolute().top(px(8.)).left(px(8.)).size(px(22.)).flex().items_center().justify_center().rounded_full().bg(c(RAISED_2)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(slot.to_string()))
                .child(
                    div().absolute().top(px(6.)).right(px(6.)).child(
                        IconButton::new(("av-emote-off", slot as usize), "x")
                            .size(26.)
                            .filled()
                            .disabled(busy.is_some())
                            .tip(root.tr("av.takeOff"))
                            .on_click(cx.listener(move |r, _, _, cx| change(r, Change::EmoteOff(id, slot), cx))),
                    ),
                )
                .into_any_element()
        })
        .collect();
    let outfits: Vec<AnyElement> = a
        .outfits
        .iter()
        .map(|o| {
            let id = o.id;
            tile_of(tile_w, &o.pic, o.name.clone(), busy == Some(id), cx)
                .child(div().pb(px(4.)).flex().child(
                    Button::new(("av-wear", (id as u64 % 1_000_000_007) as usize), root.tr("av.wear")).small().disabled(busy.is_some()).on_click(cx.listener(move |r, _, _, cx| change(r, Change::Outfit(id), cx))),
                ))
                .into_any_element()
        })
        .collect();

    let lists = div()
        .w(px(lists_w))
        .flex()
        .flex_col()
        .gap(px(SECTION_GAP))
        .when_some(av.refused.clone(), |d, why| d.child(div().text_size(px(T_M)).text_color(c(WARN)).child(root.i18n.tv("av.refused", &[("why", &why)]))))
        .child(section(root.tr("av.wearing"), Some(a.items.len()), worn, "av.nothing"))
        .when(!a.emotes.is_empty(), |d| d.child(section(format!("Emote  {}/{}", a.emotes.len(), roblox::EMOTES), None, wheel, "av.nothing")))
        .when(a.looks.len() > 1, |d| d.child(history(root, a, tile_w, cx)))
        .child(section(root.tr("av.outfits"), Some(a.outfits.len()), outfits, "av.noOutfits"))
        .child(shelves(root, a, tile_w, cx));

    crate::scrolled::body("av-body", 
        div().px(px(PAD + 4.)).pt(px(4.)).pb(px(24.)).flex().items_start().gap(px(24.)).when(!wide, |d| d.flex_col()).child(portrait).child(lists),
    )
}

/// What the avatar is worth, which Roblox doesn't add up: the Robux of everything worn (roblox::worth), a bundle
/// counted once however many of its parts are worn, and how many things that counts and leaves out (no price to
/// count). Nothing until Roblox has said, or when nothing worn has a price.
fn worth(root: &Root, a: &Avatar) -> Option<Div> {
    let w = root.avatar.worth.as_ref()?;
    let vi = root.i18n.vietnamese;
    let known: Vec<Option<Price>> = a.items.iter().filter_map(|i| w.get(&i.id).copied()).collect();
    let counted = known.iter().flatten().count();
    if counted == 0 {
        return None;
    }
    let left = known.len() - counted;
    let mut packs: Vec<i64> = Vec::new();
    let mut sum = 0;
    for p in known.iter().flatten() {
        if p.pack == 0 || !packs.contains(&p.pack) {
            sum += p.robux;
        }
        if p.pack != 0 {
            packs.push(p.pack);
        }
    }
    let note = if left == 0 { root.i18n.tv("av.worthAll", &[("n", &counted.to_string())]) } else { root.i18n.tv("av.worthSome", &[("n", &counted.to_string()), ("m", &left.to_string())]) };
    Some(
        div()
            .px(px(2.))
            .flex()
            .items_end()
            .justify_between()
            .gap(px(10.))
            .child(
                div().flex().flex_col().gap(px(2.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("av.worth"))).child(
                    div().flex().items_center().gap(px(7.)).child(icon("robux", 18., c(TEXT))).child(div().text_size(px(T_TITLE)).font_weight(FontWeight::BOLD).font_features(tnum()).child(thousands(sum, vi))),
                ),
            )
            .child(div().pb(px(4.)).text_size(px(T_S)).text_color(c(MUTED)).child(note)),
    )
}

/// Under a worn thing's name: what it is worth, or that it is free or has no price; a Limited says so, and a part of
/// a bundle that the price is the bundle's. Empty (the same height) until Roblox has said.
fn price_row(root: &Root, known: Option<Option<Price>>) -> Div {
    let vi = root.i18n.vietnamese;
    let row = div().px(px(2.)).pb(px(4.)).h(px(22.)).flex().items_center().gap(px(5.)).text_size(px(T_S)).font_features(tnum());
    match known {
        None => row,
        Some(None) => row.text_color(c(MUTED)).child(root.tr("av.offSale")),
        Some(Some(p)) if p.robux == 0 => row.text_color(c(MUTED)).child(root.tr("av.free")),
        Some(Some(p)) => row
            .text_color(c(TEXT))
            .child(icon("robux", 12., c(TEXT)))
            .child(div().font_weight(FontWeight::MEDIUM).child(thousands(p.robux, vi)))
            .when(p.limited, |d| d.child(div().text_color(c(WARN)).child("Limited")))
            .when(p.pack != 0, |d| d.child(div().text_color(c(MUTED)).child(root.tr("av.pack")))),
    }
}

/// The avatar's history, which Roblox doesn't keep: every look this PC has seen the avatar in, newest first, each with
/// when it was first seen and Back to put it on again (the one it is in now says so instead).
fn history(root: &Root, a: &Avatar, tile_w: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let busy = root.avatar.busy;
    let tiles: Vec<AnyElement> = a
        .looks
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let pic = picture(&l.pic, cx);
            tile(tile_w, pic, crate::time::day_time(l.at), busy == Some(-2 - i as i64))
                .child(div().pb(px(4.)).h(px(34.)).flex().items_center().map(|d| {
                    if i == a.now {
                        d.child(div().px(px(2.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("av.now")))
                    } else {
                        d.child(Button::new(("av-look", i), root.tr("av.back")).small().icon("history").disabled(busy.is_some()).on_click(cx.listener(move |r, _, _, cx| change(r, Change::Look(i), cx))))
                    }
                }))
                .into_any_element()
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            section_head(root.tr("av.history"), Some(a.looks.len())).child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("av.historyNote"))),
        )
        .child(div().flex().flex_wrap().gap(px(TGAP)).children(tiles))
}

/// The body, under the picture: R6 or R15, the skin's colour (Roblox's own tones, one all over), and each size as a
/// row of steps from the least to the most Roblox allows.
fn body(root: &Root, a: &Avatar, cx: &mut Context<Root>) -> impl IntoElement {
    let busy = root.avatar.busy.is_some();
    let label = |key: &str| div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr(key));
    let rig = div().flex().items_center().gap(px(4.)).children([(false, "R6"), (true, "R15")].into_iter().map(|(r15, word)| {
        let on = (a.rig == "R15") == r15;
        chip(SharedString::from(format!("av-rig-{word}")), on).h(px(TOOL_H)).px(px(14.)).cursor_pointer().child(word).on_click(cx.listener(move |r, _, _, cx| {
            if matches!(&r.avatar.got, Some(Ok(a)) if (a.rig == "R15") != r15) {
                change(r, Change::Rig(r15), cx);
            }
        }))
    }));
    let mut col = div().flex().flex_col().gap(px(14.)).when(busy, |d| d.opacity(0.6)).child(div().flex().child(rig));
    if !a.palette.is_empty() {
        // The colour worn leads when it isn't one of Roblox's skin tones (a colour set elsewhere), so what is worn
        // always shows picked.
        let mut tones: Vec<String> = a.palette.clone();
        if !a.colour.is_empty() && !tones.iter().any(|h| h.eq_ignore_ascii_case(&a.colour)) {
            tones.insert(0, a.colour.clone());
        }
        col = col.child(div().flex().flex_col().gap(px(8.)).child(label("av.skin")).child(div().flex().flex_wrap().gap(px(4.)).children(tones.into_iter().enumerate().map(|(i, hex)| {
            let on = hex.eq_ignore_ascii_case(&a.colour);
            let rgb = u32::from_str_radix(&hex, 16).unwrap_or(0x808080);
            let pick = hex.clone();
            // Picked: the yellow ring round it, a gap between (as everything chosen in the app); the others a
            // hairline, so a dark one still shows on the card.
            focusable(div().id(("av-skin", i)))
                .size(px(28.))
                .p(px(2.))
                .rounded_full()
                .border_2()
                .border_color(if on { c(FACE) } else { ca(WHITE, 0.) })
                .cursor_pointer()
                .child(div().size_full().rounded_full().bg(c(rgb)).border_1().border_color(ca(WHITE, 0.12)))
                .on_click(cx.listener(move |r, _, _, cx| {
                    if !on {
                        change(r, Change::Colour(pick.clone()), cx);
                    }
                }))
        }))));
    }
    const STEPS: usize = 8;
    for (which, (lo, hi)) in a.ranges.iter().copied().enumerate() {
        if hi <= lo {
            continue;
        }
        let at = (((a.scales[which] - lo) / (hi - lo)).clamp(0., 1.) * (STEPS - 1) as f64).round() as usize;
        // Its value in words at the end of the row, as Roblox's editor shows it (a size in per cent of the usual one).
        let shown = format!("{}%", (a.scales[which] * 100.).round() as i64);
        col = col.child(
            div().flex().items_center().gap(px(12.)).child(div().w(px(84.)).flex_none().child(label(&format!("av.scale.{which}")))).child(div().flex_1().flex().gap(px(3.)).children((0..STEPS).map(|i| {
                let value = lo + (hi - lo) * i as f64 / (STEPS - 1) as f64;
                div()
                    .id(("av-scale", which * 16 + i))
                    .flex_1()
                    .h(px(10.))
                    .rounded(px(3.))
                    .cursor_pointer()
                    .bg(if i <= at { ca(WHITE, 0.55) } else { ca(WHITE, 0.1) })
                    .hover(|d| d.bg(ca(WHITE, 0.8)))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        if i != at {
                            change(r, Change::Scale(which, value), cx);
                        }
                    }))
            }))).child(div().w(px(40.)).flex_none().text_right().text_size(px(T_S)).font_features(tnum()).text_color(c(TEXT_2)).child(shown)),
        );
    }
    col
}

/// The inventory: a shelf picked on the track, its things as tiles (Wear, or a word when it is on already), and More
/// while Roblox has another page.
fn shelves(root: &Root, a: &Avatar, tile_w: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let av = &root.avatar;
    let busy = av.busy;
    // The shelves as the page's filters: no track, the one shown a yellow block.
    let track = div().flex().flex_wrap().items_center().gap(px(4.)).children(roblox::SHELVES.iter().enumerate().map(|(i, (key, _))| {
        chip(("av-shelf", i), av.shelf == i).h(px(TOOL_H)).px(px(12.)).cursor_pointer().child(root.tr(&format!("av.shelf.{key}"))).on_click(cx.listener(move |r, _, _, cx| pick_shelf(r, i, cx)))
    }));
    let said = |text: String| div().text_size(px(T_M)).text_color(c(MUTED)).child(text);
    let body: AnyElement = match &av.owned {
        None => waiting_tiles(10, tile_w, TGAP).into_any_element(),
        Some(Err(why)) => said(root.i18n.tv("av.shelfFailed", &[("why", if why.is_empty() { "Roblox" } else { why })])).into_any_element(),
        Some(Ok(l)) if l.is_empty() => said(root.tr("av.shelfEmpty")).into_any_element(),
        Some(Ok(l)) => {
            let tiles: Vec<AnyElement> = l
                .iter()
                .enumerate()
                .map(|(n, o)| {
                    let (id, kind) = (o.id, o.kind.clone());
                    let on = a.items.iter().any(|i| i.id == id) || a.emotes.iter().any(|e| e.id == id);
                    tile_of(tile_w, &o.pic, o.name.clone(), busy == Some(id), cx)
                        .child(div().pb(px(4.)).h(px(34.)).flex().items_center().map(|d| {
                            if on {
                                d.child(div().px(px(2.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("av.wearing")))
                            } else {
                                d.child(Button::new(("av-on", n), root.tr("av.wear")).small().disabled(busy.is_some()).on_click(cx.listener(move |r, _, _, cx| change(r, Change::On(id, kind.clone()), cx))))
                            }
                        }))
                        .into_any_element()
                })
                .collect();
            div()
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(div().flex().flex_wrap().gap(px(TGAP)).children(tiles))
                .when(!av.next.is_empty(), |d| {
                    d.child(div().flex().child(Button::new("av-more", root.tr("av.more")).disabled(av.reading.is_some()).on_click(cx.listener(|r, _, _, cx| {
                        if r.avatar.reading.is_none() {
                            shelf_page(r, cx);
                            cx.notify();
                        }
                    }))))
                })
                .into_any_element()
        }
    };
    div().flex().flex_col().gap(px(12.)).child(section_head(root.tr("av.inventory"), None)).child(track).child(body)
}

/// A worn thing or an outfit: its picture on a block, its name under it (two lines
/// at most, so a row's prices and buttons line up).
pub fn tile(w: f32, pic: Option<std::sync::Arc<Image>>, name: String, busy: bool) -> Div {
    frame(w, name, busy, |d| d.when_some(pic, |d, p| d.child(fade_in(img(p.clone()).size_full().object_fit(ObjectFit::Contain), &p))))
}

/// [`tile`] with its picture read at the exact size it is shown (crisp, not a larger picture scaled down).
pub fn tile_of(w: f32, url: &str, name: String, busy: bool, cx: &mut App) -> Div {
    tile_said(w, url, name, busy, None, cx)
}

/// [`tile_of`], and when its picture couldn't be had, `lost` said quietly in its place (the house line: a small
/// picture mark and the words) rather than an empty block.
pub fn tile_said(w: f32, url: &str, name: String, busy: bool, lost: Option<SharedString>, cx: &mut App) -> Div {
    tile_zoomed(w, url, name, busy, lost, 1., cx)
}

/// `tile_said` with the picture drawn `zoom` times the tile, centred and cut at its edges: a whole figure (a bundle)
/// stands small in the middle of Roblox's picture, a 40 px figure in a 210 px tile beside full-size hats (review 3f.1).
pub fn tile_zoomed(w: f32, url: &str, name: String, busy: bool, lost: Option<SharedString>, zoom: f32, cx: &mut App) -> Div {
    let side = ((w - 16.) * zoom).max(8.);
    let pic = (!url.is_empty()).then(|| images::pic_pair(url, side, side, cx)).flatten();
    let failed = !url.is_empty() && images::failed(url, cx);
    frame(w, name, busy, move |d| match (pic, lost) {
        (Some((src, key)), _) => d.child(fade_in(img(src).flex_none().size(px(side)).object_fit(ObjectFit::Contain), &key)),
        (None, Some(text)) if failed => d.flex_col().gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(icon("image", 13., c(MUTED))).child(div().text_center().child(text)),
        (None, _) => d.child(icon("image", 22., c(DIM))),
    })
}

fn frame(w: f32, name: String, busy: bool, fill: impl FnOnce(Div) -> Div) -> Div {
    div()
        .relative()
        .w(px(w))
        .flex()
        .flex_col()
        .when(busy, |d| d.opacity(0.5))
        .child(fill(div().w(px(w)).h(px(w)).flex_none().rounded(px(R_L)).overflow_hidden().bg(c(RAISED)).p(px(8.)).flex().items_center().justify_center()))
        // (Two lines and no more: the clip is the text's own box, so a third never shows in the padding under it. A
        // one-line name takes one line: what goes under it follows close, and the room left in a row whose tiles
        // stand as tall as its tallest comes before it, so a row's prices and buttons still line up.)
        .child(div().px(px(2.)).pt(px(10.)).pb(px(4.)).child(div().max_h(px(36.)).text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).line_height(px(18.)).overflow_hidden().child(name)))
        .child(div().flex_1())
}
