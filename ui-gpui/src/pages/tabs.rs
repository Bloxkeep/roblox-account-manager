// Account tabs, as a browser has them, over Games (the owner's call, 2026-11-10: several accounts open at once, not one
// account switched for another). A tab is an account with everything of Roblox's read for it (friends, avatar,
// profile, inbox) and the part it was left on; picking a tab shows it at once, as it was. "+" opens another account.
//
// The pages go on reading `root.friends`, `root.avatar`… : those hold the tab shown. Leaving a tab puts them away in
// it; coming to one takes them out. An answer still on its way for a tab put away finds another account there and is
// dropped (each page checks whose answer it is), and the tab asks again when it is shown.
use gpui_kit::{prelude::*, *};
use gpui_kit::component::input::{InputEvent, InputState};
use serde_json::Value;
use crate::val::s;

use crate::{
    kit::*,
    pages::{
        accounts::{avatar, label},
        avatar as avatar_page, friends,
        games::View,
        inbox, profile,
    },
    shell::Root,
    theme::*,
};

#[derive(Default)]
pub struct Tab {
    /// The account, by its id in the app (None: the first tab before the app has said which its pick, the main one, is).
    pub account: Option<String>,
    view: View,
    friends: friends::Data,
    avatar: avatar_page::Data,
    profile: profile::Kept,
    inbox: inbox::Data,
}

pub struct Data {
    pub list: Vec<Tab>,
    pub at: usize,
    scroll: ScrollHandle,
    /// "+"'s list of accounts is open.
    pub menu: bool,
    /// The tab held by the mouse, while it is dragged along the strip.
    dragging: Option<Drag>,
    /// The last tab that stood aside for the one dragged: its place now, the way it went (-1 left, 1 right) and which
    /// move it was (so its slide starts again for the next).
    slid: Option<(usize, f32, usize)>,
    account_query: Option<Entity<InputState>>,
    pub vng: super::vng::Data,
    /// Each account Roblox was told is here (`pulse`), by its Roblox user: its session's id for this run and when it
    /// was last told.
    pulses: std::collections::HashMap<i64, (String, std::time::Instant)>,
}

/// A tab held down: where it was and where the mouse was when pressed, its place now, the mouse's x now, and whether it
/// has moved far enough to be a drag (a press that stays put is a click).
#[derive(Clone, Copy)]
struct Drag {
    from: usize,
    at: usize,
    x0: f32,
    x: f32,
    moved: bool,
    moves: usize,
}

/// One tab's place along the strip (a tab's width and the gap after it), while there are several.
const STEP: f32 = 154.;

/// How far the held tab is drawn from its place: the mouse's way since the press, less the places it has taken,
/// kept inside the strip.
fn held_offset(d: &Drag, n: usize) -> f32 {
    let raw = (d.x - d.x0).clamp(-(d.from as f32) * STEP, (n.saturating_sub(1) - d.from.min(n.saturating_sub(1))) as f32 * STEP);
    raw - (d.at as f32 - d.from as f32) * STEP
}

/// The mouse moved with a tab held: past half a tab's width, it takes that place, as a browser's tabs do; the tab
/// passed slides over to the place it left.
fn drag_to(r: &mut Root, x: f32) {
    let n = r.tabs.list.len();
    let Some(mut d) = r.tabs.dragging else { return };
    d.x = x;
    if !d.moved && (x - d.x0).abs() >= 4. {
        d.moved = true;
    }
    if d.moved && n > 1 {
        let raw = (x - d.x0).clamp(-(d.from as f32) * STEP, (n - 1 - d.from) as f32 * STEP);
        let want = (d.from as f32 + (raw / STEP).round()).clamp(0., (n - 1) as f32) as usize;
        while d.at != want {
            let next = if want > d.at { d.at + 1 } else { d.at - 1 };
            move_tab(r, d.at, next);
            d.moves += 1;
            // (The one passed now stands where the held one was: it slides in from the side it came from.)
            r.tabs.slid = Some((d.at, if next > d.at { 1. } else { -1. }, d.moves));
            d.at = next;
        }
    }
    r.tabs.dragging = Some(d);
}

impl Default for Data {
    fn default() -> Self {
        Data { list: vec![Tab::default()], at: 0, scroll: ScrollHandle::new(), menu: false, account_query: None, vng: Default::default(), pulses: Default::default(), dragging: None, slid: None }
    }
}

impl Data {
    pub fn club_left(&mut self, id: i64, user: i64) {
        for tab in &mut self.list {
            if let (of, Some(Ok(p))) = &mut tab.profile {
                if *of == user {
                    p.groups.retain(|g| g.id != id);
                }
            }
        }
    }
}

/// Opening a Roblox tab is also useful for VNG: game discovery and launching use the saved account.
/// The signed-in roblox.com pages keep their separate eligibility rules in friends::able_ref.
pub(crate) fn can_open_account(a: &Value) -> bool {
    !s(a, "id").is_empty()
        && matches!(s(a, "realm"), "" | "vng")
        && a.get("cookieValid").and_then(Value::as_bool) != Some(false)
        && a.get("userId").and_then(Value::as_i64).unwrap_or(0) > 0
}

pub fn available(root: &Root) -> Vec<&Value> {
    root.accounts().iter().filter(|a| can_open_account(a)).collect()
}

/// How often Roblox is told an account is here: its website's own interval.
const PULSE_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

/// While an account's tab is shown and Bloxkeep is the window in use, Roblox is told the account is on its site, as
/// roblox.com tells it (roblox::pulse): the account shows online to those its privacy lets see it, as it would with
/// the website open (the owner, 2026-11-18: an open tab "cũng tính là đang online roblox"). Only roblox.com accounts
/// (a VNG sign-in stays in the core); a refusal is in error.log (roblox.rs's writes), and the first one that went
/// through is said there once.
pub fn pulse(r: &mut Root, cx: &mut Context<Root>) {
    if !looping() {
        return;
    }
    let Some((_, user, cookie)) = friends::signed(r).filter(|(_, _, cookie)| !cookie.is_empty()) else { return };
    let now = std::time::Instant::now();
    let told = r.tabs.pulses.entry(user).or_insert_with(|| (session_id(), now - PULSE_EVERY));
    if now.duration_since(told.1) < PULSE_EVERY {
        return;
    }
    told.1 = now;
    let session = told.0.clone();
    let rx = crate::roblox::run(move || crate::roblox::pulse(&cookie, &session));
    cx.background_executor()
        .spawn(async move {
            if let Ok(Ok(())) = rx.await {
                crate::net::note_worked("Roblox tab: told Roblox an account is online (as its website does, every 30 s while its tab is shown).");
            }
        })
        .detach();
}

/// A session's id as Roblox's site has them (a UUID), made anew each run.
fn session_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || std::collections::hash_map::RandomState::new().build_hasher().finish();
    let h = format!("{:016x}{:016x}", part(), part());
    format!("{}-{}-4{}-a{}-{}", &h[0..8], &h[8..12], &h[13..16], &h[17..20], &h[20..32])
}

/// The tab shown has no account yet and there are several to open it as: Roblox shows the choice of them (`picker`)
/// instead of opening the app's pick by itself. One account alone opens at once; so does a game's page, which needs
/// one to show at all. A hidden shot keeps the app's pick unless BLOXKEEP_SHOT_PICKACC asks for the choice.
pub fn picking(root: &Root) -> bool {
    if cfg!(feature = "shot") && std::env::var("BLOXKEEP_SHOT_PICKACC").is_err() {
        return false;
    }
    root.friends.account.is_none() && root.games.open.is_none() && friends::signed(root).is_none() && available(root).len() > 1
}

/// From how many accounts the choice gets its search box and is laid from the top (fewer stand centred).
const PICK_MANY: usize = 10;

/// Who Roblox is opened as, as a browser's or a film site's "who is this?" has it: a face each, its name under it;
/// the main accounts first. With many (a hundred, the owner's question) a search box over them finds one by its name
/// or its Roblox name, and the grid runs from the top and scrolls.
pub fn picker(root: &Root, cx: &mut Context<Root>) -> impl IntoElement {
    const FACE: f32 = 84.;
    let mut able = available(root);
    able.sort_by_key(|a| !a.get("isMain").and_then(Value::as_bool).unwrap_or(false));
    let many = able.len() > PICK_MANY;
    let query = if many { folded(root.pick_q.read(cx).value().trim()) } else { String::new() };
    let tiles: Vec<_> = able
        .into_iter()
        .filter(|a| query.is_empty() || folded(&label(a)).contains(query.as_str()) || folded(s(a, "username")).contains(query.as_str()))
        .enumerate()
        .map(|(i, a)| {
            let id = s(a, "id").to_string();
            let user = s(a, "username");
            hot(("rbx-pick", i))
                .w(px(152.))
                .px(px(12.))
                .pt(px(16.))
                .pb(px(14.))
                .rounded(px(R_L))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .cursor_pointer()
                .child(avatar(a, FACE, PANEL, cx))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(2.))
                        .child(div().max_w_full().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(label(a)))
                        .when(!user.is_empty() && user != label(a), |d| d.child(div().max_w_full().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(format!("@{user}"))))
                        .when(s(a, "realm") == "vng", |d| d.child(div().pt(px(4.)).child(vng_badge()))),
                )
                .on_click(cx.listener(move |r, _, window, cx| {
                    // (The search starts empty the next time the choice shows.)
                    r.pick_q.update(cx, |q, cx| q.set_value("", window, cx));
                    open(r, id.clone());
                    cx.notify();
                }))
        })
        .collect();
    let none = tiles.is_empty();
    let inner = div()
        .w_full()
        .min_h_full()
        .px(px(24.))
        .py(px(40.))
        .flex()
        .flex_col()
        .items_center()
        // (Centred, a grid taller than the page would run off its top.)
        .when(!many, |d| d.justify_center())
        .gap(px(if many { 22. } else { 28. }))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .child(div().text_size(px(T_TITLE)).font_weight(FontWeight::BOLD).child(root.tr("tabs.pick")))
                .child(div().text_size(px(T_M)).text_color(c(TEXT_2)).child(root.tr("tabs.pickSub"))),
        )
        .when(many, |d| d.child(div().w(px(340.)).max_w_full().child(field(&root.pick_q, Some("search")))))
        .when(none, |d| d.child(div().py(px(24.)).text_color(c(MUTED)).child(root.tr("tabs.noMatches"))))
        .child(div().w_full().max_w(px(1000.)).flex().flex_wrap().justify_center().gap(px(8.)).children(tiles));
    // (Its bar at the right edge, as every page that scrolls.)
    crate::scrolled::body("rbx-pick-scroll", inner)
}

pub fn is_vng(root: &Root) -> bool {
    root.friends.account.as_deref().is_some_and(|id| root.accounts().iter().any(|a| s(a, "id") == id && s(a, "realm") == "vng"))
}

/// Play in a VNG tab starts with that account checked, even if Accounts has other selections.
pub fn play_selected(root: &Root, id: &str) -> bool {
    if is_vng(root) { root.friends.account.as_deref() == Some(id) } else { root.selected.contains(id) }
}

fn vng_badge() -> Div {
    div().flex_none().px(px(5.)).py(px(2.)).rounded(px(4.)).bg(ca(ONLINE, 0.12)).text_color(c(ONLINE)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).child("VNG")
}

/// The account of the tab shown: the one the app answered for, else the one it was opened for.
fn shown_account(r: &Root) -> Option<String> {
    friends::signed(r).map(|s| s.0).or(r.friends.account.clone())
}

/// What is shown goes into its tab.
fn put_away(r: &mut Root) {
    let account = shown_account(r);
    r.friends.rest();
    r.avatar.rest();
    r.inbox.rest();
    r.rset = Default::default();
    let at = r.tabs.at;
    let Some(t) = r.tabs.list.get_mut(at) else { return };
    t.account = account;
    t.view = r.games.view;
    t.friends = std::mem::take(&mut r.friends);
    t.avatar = std::mem::take(&mut r.avatar);
    t.profile = r.profile.take_profile();
    t.inbox = std::mem::take(&mut r.inbox);
}

/// The tab at `tabs.at` comes out to be shown.
fn take_out(r: &mut Root) {
    let at = r.tabs.at;
    let Some(t) = r.tabs.list.get_mut(at) else { return };
    r.games.view = t.view;
    r.friends = std::mem::take(&mut t.friends);
    r.avatar = std::mem::take(&mut t.avatar);
    let kept = std::mem::take(&mut t.profile);
    r.inbox = std::mem::take(&mut t.inbox);
    r.profile.put_profile(kept);
}

/// The tab at `from` moved to `to` (dragged along the strip); the tab shown stays the one shown.
fn move_tab(r: &mut Root, from: usize, to: usize) {
    let tabs = &mut r.tabs;
    if from == to || from >= tabs.list.len() || to >= tabs.list.len() {
        return;
    }
    let tab = tabs.list.remove(from);
    tabs.list.insert(to, tab);
    tabs.at = if tabs.at == from {
        to
    } else if from < tabs.at && to >= tabs.at {
        tabs.at - 1
    } else if from > tabs.at && to <= tabs.at {
        tabs.at + 1
    } else {
        tabs.at
    };
}

pub fn switch(r: &mut Root, to: usize) {
    if to != r.tabs.at && to < r.tabs.list.len() {
        r.tabs.vng.clear();
        put_away(r);
        r.tabs.at = to;
        r.tabs.scroll.scroll_to_item(to);
        take_out(r);
    }
}

/// A default tab gets a stable identity when the app chooses its account. If another tab already
/// owns that account, keep its view and discard the unbound tab.
pub fn bind_default(r: &mut Root, id: &str) -> bool {
    let at = r.tabs.at;
    if let Some(existing) = r.tabs.list.iter().enumerate()
        .position(|(i, tab)| i != at && tab.account.as_deref() == Some(id))
    {
        switch(r, existing);
        close(r, at);
        return false;
    }
    r.friends.account = Some(id.to_string());
    r.tabs.list[at].account = Some(id.to_string());
    true
}

/// A tab for that account: the one it has already, else a new one at the end, on the part being looked at now.
pub fn open(r: &mut Root, id: String) {
    r.tabs.menu = false;
    let shown = shown_account(r);
    if shown.as_deref() == Some(id.as_str()) {
        return;
    }
    if let Some(i) = r.tabs.list.iter().position(|t| t.account.as_deref() == Some(id.as_str())) {
        return switch(r, i);
    }
    let view = if r.accounts().iter().any(|a| s(a, "id") == id && s(a, "realm") == "vng") { View::Games } else { r.games.view };
    r.tabs.vng.clear();
    let reuse = shown.is_none() && r.tabs.list[r.tabs.at].account.is_none();
    put_away(r);
    let tab = Tab { account: Some(id.clone()), view, friends: friends::Data::of(id), ..Default::default() };
    if reuse {
        r.tabs.list[r.tabs.at] = tab;
    } else {
        r.tabs.list.push(tab);
        r.tabs.at = r.tabs.list.len() - 1;
        r.tabs.scroll.scroll_to_item(r.tabs.at);
    }
    take_out(r);
}

/// Another vault opened, or accounts deleted: the tabs of accounts no longer there go (with what was read for them);
/// the tab shown stays, on the app's pick again if its account went.
pub fn forget_gone(r: &mut Root) {
    r.tabs.vng.clear();
    let ids: std::collections::HashSet<String> = r.accounts().iter().map(|a| s(a, "id").to_string()).collect();
    let gone = |t: &Tab| t.account.as_ref().is_some_and(|id| !ids.contains(id));
    let at = r.tabs.at;
    let shown_gone = r.friends.account.as_ref().is_some_and(|id| !ids.contains(id));
    if shown_gone {
        r.friends.account = None;
    }
    if let Some(t) = r.tabs.list.get_mut(at) {
        if shown_gone || gone(t) {
            t.account = None;
        }
    }
    // (The shown one is kept: its place is how many kept tabs come before it.)
    let (mut i, mut before) = (0, 0);
    r.tabs.list.retain(|t| {
        let keep = i == at || !gone(t);
        if keep && i < at {
            before += 1;
        }
        i += 1;
        keep
    });
    r.tabs.at = before.min(r.tabs.list.len().saturating_sub(1));
}

/// A tab closes; the last one stays. Closing the one shown shows its neighbour.
pub fn close(r: &mut Root, i: usize) {
    if r.tabs.list.len() < 2 || i >= r.tabs.list.len() {
        return;
    }
    if i == r.tabs.at {
        let to = if i + 1 < r.tabs.list.len() { i + 1 } else { i - 1 };
        switch(r, to);
    }
    r.tabs.list.remove(i);
    if r.tabs.at > i {
        r.tabs.at -= 1;
    }
    r.tabs.scroll.scroll_to_item(r.tabs.at);
}

/// The strip: a tab an account (its face and name; a cross while there is more than one), then "+".
pub fn strip(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let able = available(root);
    let (_, active) = friends::looking(root);
    let tabs = &root.tabs;
    let find = |id: &Option<String>| id.as_ref().and_then(|id| able.iter().find(|a| s(a, "id") == id)).map(|a| (*a).clone());
    let many = tabs.list.len() > 1;
    let mut open_ids = std::collections::HashSet::new();
    const TAB_WIDTH: f32 = 150.;
    let scroll = tabs.scroll.clone();
    let crowded = many && tabs.list.len() as f32 * (TAB_WIDTH + 4.) + 180. > width;
    let mut row = div()
        .id("rbx-tab-strip")
        .flex_shrink(1.)
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(4.))
        .overflow_x_scroll()
        .track_scroll(&scroll);
    for (i, t) in tabs.list.iter().enumerate() {
        let on = i == tabs.at;
        let acc = if on { active.clone() } else { find(&t.account) };
        if let Some(a) = &acc {
            open_ids.insert(s(a, "id").to_string());
        }
        // No account to see Roblox as (none signed in, or it isn't known yet): Roblox itself, signed out, with a globe
        // in the face's place, rather than a bare "…".
        let name = acc.as_ref().map(label).unwrap_or_else(|| "Roblox".into());
        // (Roblox's own status for it, as others see it: the Bloxkeep-only label it had, no one else saw.)
        let status = acc.as_ref().map(super::rsettings::roblox_presence);
        let tip: SharedString = match status {
            Some((words, _)) => format!("{name} · {}", root.tr(words)).into(),
            None => name.clone().into(),
        };
        // Held and moved over another tab, it takes that tab's place, as a browser's tabs do (the owner,
        // 2026-11-18: "kéo qua kéo lại được như trình duyệt"). The tabs are one width while there are several, so
        // the one held is under the mouse again after each move.
        // Held, it follows the mouse along the strip and takes another tab's place past half its width, as a browser's
        // tabs do (the owner, 2026-11-19: the old way jumped from place to place and the page faded in each time). A
        // press shows it at once, as a browser's does, so the one dragged is always the one shown.
        let key = t.account.clone().unwrap_or_else(|| format!("#{i}"));
        let held = tabs.dragging.filter(|d| d.moved && d.at == i);
        let offset = held.map(|d| held_offset(&d, tabs.list.len())).unwrap_or(0.);
        let slide = tabs.dragging.and(tabs.slid).filter(|(at, _, _)| *at == i);
        let base = div().flex_none().relative()
                .when(held.is_some(), |d| d.left(px(offset)))
                .on_mouse_down(MouseButton::Left, cx.listener(move |r, e: &MouseDownEvent, _, cx| {
                    let x = f32::from(e.position.x);
                    switch(r, i);
                    r.tabs.slid = None;
                    r.tabs.dragging = Some(Drag { from: i, at: i, x0: x, x, moved: false, moves: 0 });
                    cx.notify();
                }));
        // (Its hover and press are its account's, not its place's: a place's went with it to the tab that came there.)
        let el = base.child(
            hot(SharedString::from(format!("rbx-tab-{key}")))
                .h(px(34.))
                .pl(px(8.))
                .pr(px(if many { 5. } else { 12. }))
                .flex()
                // (One tab keeps its whole name: shrunk, the owner's showed "N…", 2026-11-12.)
                .flex_none()
                .when(many, |d| d.w(px(TAB_WIDTH)))
                .items_center()
                .gap(px(8.))
                .rounded(px(R_M))
                .text_size(px(T_M))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .map(|d| if on { d.tones(c(PANEL), c(PANEL)).text_color(c(TEXT)) } else { d.tones(ca(WHITE, 0.), ca(WHITE, 0.06)).text_color(c(TEXT_2)).hover(|d| d.text_color(c(TEXT))) })
                .map(|d| match acc.as_ref() {
                    // (One dot, the face's own: Roblox's status. A second at its top stood for the Bloxkeep-only
                    // label, gone 2026-11-18, and was left showing the same thing twice.)
                    Some(a) => d.child(avatar(a, 22., if on { PANEL } else { GROUND }, cx)),
                    None => d.child(div().size(px(22.)).flex_none().rounded_full().bg(ca(WHITE, 0.06)).flex().items_center().justify_center().child(icon("globe", 13., c(MUTED)))),
                })
                .child(div().max_w(px(150.)).min_w_0().truncate().child(name))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .when(acc.as_ref().is_some_and(|a| s(a, "realm") == "vng"), |d| d.child(vng_badge()))
                .when(many, |d| {
                    // (Pressed, the cross neither shows its tab nor starts a drag.)
                    d.child(div().flex_none().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(IconButton::new(SharedString::from(format!("rbx-tab-x-{key}")), "x").size(24.).tip(root.tr("fr.close")).on_click(cx.listener(move |r, _, _, cx| {
                        // The cross only: the tab under it isn't picked by the same press.
                        cx.stop_propagation();
                        close(r, i);
                        cx.notify();
                    }))))
                })
                .on_click(cx.listener(move |r, _, _, cx| {
                    switch(r, i);
                    cx.notify();
                })));
        row = row.child(match slide {
            Some((_, dir, n)) => el.with_animation(SharedString::from(format!("rbx-tab-slide-{key}-{n}")), Animation::new(std::time::Duration::from_millis(150)).with_easing(ease_out_quint()), move |d, t| d.left(px(dir * STEP * (1. - t)))).into_any_element(),
            None => el.into_any_element(),
        });
    }

    // While a tab is held: the mouse followed over the whole window (off the strip too, as a browser's tab does), and
    // let go anywhere, the tab stays where it is.
    if tabs.dragging.is_some() {
        let this = cx.entity();
        row = row.child(canvas(|_, _, _| {}, move |_, _, window, _| {
            let moved = this.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                moved.update(cx, |r, cx| {
                    if e.pressed_button != Some(MouseButton::Left) {
                        r.tabs.dragging = None;
                        r.tabs.slid = None;
                    } else {
                        drag_to(r, f32::from(e.position.x));
                    }
                    cx.notify();
                });
            });
            let up = this.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble {
                    up.update(cx, |r, cx| {
                        r.tabs.dragging = None;
                        r.tabs.slid = None;
                        cx.notify();
                    });
                }
            });
        }).absolute().size_0());
    }

    // "+": the accounts not open yet, under it (a second click closes it).
    let rest: Vec<&Value> = able.into_iter().filter(|a| !open_ids.contains(s(a, "id"))).collect();
    let add = IconButton::new("rbx-tab-add", "plus").size(30.).disabled(rest.is_empty());
    let add = if tabs.menu { add } else { add.tip(root.tr("tabs.open")) };
    let trigger = std::rc::Rc::new(std::cell::Cell::new(None::<Bounds<Pixels>>));
    let at = trigger.clone();
    let mut plus = div().relative().flex_none().child(add.on_click(cx.listener(|r, _, window, cx| {
        r.tabs.menu = !r.tabs.menu;
        r.tabs.account_query = r.tabs.menu.then(|| {
            let hint = r.tr("ui.accSearch");
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(hint));
            cx.subscribe(&input, |_, _, _: &InputEvent, cx| cx.notify()).detach();
            input
        });
        cx.notify();
    }))).child(canvas(move |bounds, _, _| at.set(Some(bounds)), |_, _, _, _| {}).absolute().inset_0());
    // (Built while open or fading out only, not every frame: an avatar a row.)
    if !rest.is_empty() && crate::pages::accounts::pop_alive("rbx-tab-pop", tabs.menu) {
        let mut menu = div()
            .id("rbx-tab-menu")
            .occlude()
            .w(px(260.))
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(move |r, e: &MouseDownEvent, _, cx| {
                if r.tabs.menu && !trigger.get().is_some_and(|b| b.contains(&e.position)) {
                    r.tabs.menu = false;
                    r.tabs.account_query = None;
                    cx.notify();
                }
            }));
        if rest.len() > 12 {
            menu = menu.children(tabs.account_query.as_ref().map(|input| div().p(px(4.)).child(field(input, Some("search")))));
        }
        let query = tabs.account_query.as_ref().map(|input| folded(input.read(cx).value().trim())).unwrap_or_default();
        let matches: Vec<_> = rest.into_iter().filter(|a| query.is_empty() || folded(&label(a)).contains(query.as_str())
            || folded(s(a, "username")).contains(query.as_str())).cloned().collect();
        let this = cx.weak_entity();
        if matches.is_empty() {
            menu = menu.child(div().p(px(8.)).text_color(c(MUTED)).child(root.tr("tabs.noMatches")));
        } else {
            let height = (matches.len() as f32 * 36.).min(306.);
            let rows = uniform_list(SharedString::from(format!("rbx-tab-accounts-{query}")), matches.len(), move |range, _, cx| {
                range.map(|i| {
                    let acc = &matches[i];
                    let id = s(acc, "id").to_string();
                    let this = this.clone();
                    menu_row(("rbx-tab-acc", i))
                    .w_full()
                    .h(px(36.))
                    .px(px(8.))
                    .child(avatar(acc, 22., RAISED, cx))
                    .child(div().min_w_0().flex_1().truncate().child(label(acc)))
                    .when(s(acc, "realm") == "vng", |d| d.child(vng_badge()))
                    .on_click(move |_, _, cx| {
                        let _ = this.update(cx, |r, cx| {
                            r.tabs.account_query = None;
                            open(r, id.clone());
                            cx.notify();
                        });
                    })
                }).collect::<Vec<_>>()
            }).h(px(height)).w_full();
            menu = menu.child(div().h(px(height)).overflow_hidden().child(rows));
        }
        plus = plus.child(popup(PopAt::BelowLeft(36.), "rbx-tab-pop", tabs.menu, menu));
    }
    let arrow = |id, glyph, label, direction| {
        let scroll = scroll.clone();
        IconButton::new(id, glyph).size(26.).tip(root.tr(label)).on_click(move |_, window, _| {
            let jump = (scroll.bounds().size.width - px(TAB_WIDTH)).max(px(TAB_WIDTH));
            let x = (scroll.offset().x + jump * direction).clamp(-scroll.max_offset().x, px(0.));
            scroll.set_offset(point(x, px(0.)));
            window.refresh();
        })
    };
    div()
        .flex()
        .flex_shrink(1.)
        .min_w_0()
        .items_center()
        .gap(px(4.))
        .when(crowded, |d| d.child(arrow("rbx-tabs-prev", "chevronLeft", "tabs.scrollBack", 1.0)))
        .child(row)
        .when(crowded, |d| d.child(arrow("rbx-tabs-next", "chevronRight", "tabs.scrollNext", -1.0)))
        .child(plus)
}

/// BLOXKEEP_SHOT_TABS=2: a hidden shot opens that many more accounts' tabs, then shows the first again.
#[cfg(feature = "shot")]
pub fn shot_open(r: &mut Root, more: usize) {
    if r.tabs.list.len() > 1 {
        return;
    }
    let ids: Vec<String> = friends::able(r).iter().skip(1).take(more).map(|a| s(a, "id").to_string()).collect();
    for id in ids {
        open(r, id);
    }
    switch(r, 0);
}
