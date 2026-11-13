// Friends, as the Roblox app has them, inside Games (its header's Games | Friends switch): the friends of the account
// whose tab is shown (pages/tabs.rs), with
// where each is now (in a game, online, offline), what changed among them since this PC first read them (roblox.rs), Join for a friend whose server Roblox shows to that account, the
// requests waiting for it (accept, decline) and Add friend (Roblox's people search, then a request). The lists are asked
// from here (roblox.rs) with that account's own sign-in, lent by the app (robloxSignIn { id }): Roblox gives them only to
// a signed-in request. What changes something goes through the app: friendAct (a request, its answer; the app shows
// Roblox's check when it holds one), joinServer (the usual launch), openProfile.
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{
    images,
    kit::*,
    roblox::{self, Friend},
    shell::Root,
    theme::*,
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Playing,
    Online,
    Offline,
}

/// Where a move asked of the app stands, by the Roblox user it is about.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Busy,
    Sent,
    Refused,
}

#[derive(Default)]
pub struct Data {
    /// The account looked through, by its id in the app (None: the app's pick, the main account).
    pub account: Option<String>,
    /// What the app lent for it: that account's id, its Roblox id and its sign-in (None: not answered yet; Some(None):
    /// no account is signed in).
    sign: Option<Option<(String, i64, String)>>,
    asked_sign: bool,
    /// The last asking for the sign-in: which one (an older one's answer is dropped), when, and how many accounts the
    /// app had then. "No account" is asked again after a while, or at once when the accounts change.
    sign_seq: u64,
    sign_at: Option<Instant>,
    sign_count: usize,
    pub list: Option<Result<Vec<Friend>, String>>,
    /// The requests waiting for that account (left empty when Roblox doesn't give them: the friends still show).
    pub requests: Vec<Friend>,
    /// What changed among the friends, as this PC has kept it (roblox.rs), newest first.
    pub shifts: Vec<roblox::Shift>,
    loading: bool,
    at: Option<Instant>,
    pub filter: Filter,
    pub menu: bool,
    /// Add friend: open, and the name asked with Roblox's answer (None while it runs).
    pub adding: bool,
    pub found: Option<(String, Option<Result<Vec<Friend>, String>>)>,
    pub acts: HashMap<i64, Act>,
    /// The friend whose card's menu is open, and the one whose Unfriend waits for its second click.
    pub card_menu: Option<i64>,
    /// The card whose menu was open last: still built, so it can close as it opened (kit::pop).
    pub card_menu_was: Option<i64>,
    pub sure: Option<i64>,
}

/// Where friends are is asked again when the page is looked at this long after the last answer.
const FRESH: Duration = Duration::from_secs(60);
/// "No account is signed in" is asked again at most this often (at once when the app's accounts change).
const NOBODY_AGAIN: Duration = Duration::from_secs(5);
const CARD_W: f32 = 300.;

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// The accounts whose friends can be asked: signed in, on roblox.com.
pub fn able(root: &Root) -> Vec<Value> {
    root.accounts()
        .iter()
        .filter(|a| a.get("cookieValid").and_then(Value::as_bool) != Some(false) && s(a, "realm").is_empty() && a.get("userId").and_then(Value::as_i64).unwrap_or(0) > 0)
        .cloned()
        .collect()
}

/// The account looked through, when the app has lent its sign-in: its id in the app, its Roblox id, the sign-in.
pub fn signed(r: &Root) -> Option<(String, i64, String)> {
    r.friends.sign.clone().flatten()
}

/// No account is signed in (the app answered so).
pub fn nobody(r: &Root) -> bool {
    matches!(r.friends.sign, Some(None))
}

/// The accounts that can be looked through, and the one that is: the one the app answered for, else the one picked.
pub fn looking(root: &Root) -> (Vec<Value>, Option<Value>) {
    let able = able(root);
    let at = root.friends.sign.as_ref().and_then(|s| s.as_ref()).map(|s| s.0.clone()).or(root.friends.account.clone());
    let viewer = at.as_ref().and_then(|id| able.iter().find(|a| s(a, "id") == id)).cloned();
    (able, viewer)
}

/// The sign-in first, then the friends; again when the answer is old.
pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    want_sign(r, link, cx);
    let old = r.friends.at.is_none_or(|t| t.elapsed() > FRESH);
    if old && !r.friends.loading {
        load(r, cx);
    }
}

/// The sign-in of the account looked through, asked of the app once (Friends and Avatar share it).
pub fn want_sign(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    #[cfg(feature = "shot")]
    if r.friends.sign.is_none() && (std::env::var("BLOXKEEP_SHOT_FRIENDS").is_ok() || std::env::var("BLOXKEEP_SHOT_AVATAR").is_ok() || std::env::var("BLOXKEEP_SHOT_PROFILE").is_ok() || std::env::var("BLOXKEEP_SHOT_INBOX").is_ok()) {
        // A hidden shot: the mock's first account, with the friends from the file (roblox.rs) or the avatar of the
        // Roblox user BLOXKEEP_SHOT_AVATAR names (an avatar is public).
        let first = r.friends.account.clone().or(able(r).first().map(|a| s(a, "id").to_string())).unwrap_or_default();
        let user = std::env::var("BLOXKEEP_SHOT_AVATAR").or(std::env::var("BLOXKEEP_SHOT_PROFILE")).ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        r.friends.asked_sign = true;
        r.friends.sign = Some(Some((first, user, String::new())));
    }
    // "No account" isn't kept for good: one may have been added or signed in since. (The answer stays shown while it is
    // asked again.)
    if r.friends.asked_sign && matches!(r.friends.sign, Some(None)) {
        let count = r.accounts().len();
        if count != r.friends.sign_count || r.friends.sign_at.is_none_or(|t| t.elapsed() >= NOBODY_AGAIN) {
            r.friends.asked_sign = false;
        }
    }
    if !r.friends.asked_sign {
        r.friends.asked_sign = true;
        r.friends.sign_seq = r.friends.sign_seq.wrapping_add(1);
        r.friends.sign_at = Some(Instant::now());
        r.friends.sign_count = r.accounts().len();
        let seq = r.friends.sign_seq;
        let asked = r.friends.account.clone();
        let rx = link.call("robloxSignIn", json!({ "id": asked }));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |r, cx| {
                // Another tab is shown by now: this one asks again when it comes back (Data::rest). A later asking
                // (or a sign-in forgotten since) has the say; a sign-in held already stays.
                if r.friends.account != asked || r.friends.sign_seq != seq || matches!(r.friends.sign, Some(Some(_))) {
                    return;
                }
                r.friends.sign = Some(got.and_then(|v| {
                    let cookie = s(&v, "cookie").to_string();
                    let user = v.get("userId").and_then(Value::as_i64).unwrap_or(0);
                    (!cookie.is_empty() && user > 0).then(|| (s(&v, "id").to_string(), user, cookie))
                }));
                cx.notify();
            });
        })
        .detach();
    }
}

fn load(r: &mut Root, cx: &mut Context<Root>) {
    let Some(Some((id, user, cookie))) = r.friends.sign.clone() else { return };
    r.friends.loading = true;
    let rx = roblox::run(move || {
        // (The requests are asked while the friends are.)
        let (mut got, asks) = roblox::both(|| roblox::friends(&cookie, user), || roblox::requests(&cookie).unwrap_or_default());
        // The list that came is held against the one known here: what changed is kept, which Roblox never tells; and
        // each friend gets when this PC last saw them online and the game it sees them in most.
        let shifts = got.as_mut().map(|l| roblox::friend_shifts(user, l)).unwrap_or_default();
        (got, asks, shifts)
    });
    cx.spawn(async move |this, cx| {
        let (got, asks, shifts) = rx.await.unwrap_or_else(|_| (Err(String::new()), Vec::new(), Vec::new()));
        let _ = this.update(cx, |r, cx| {
            // An answer for the account looked through before is dropped.
            if r.friends.sign.as_ref().and_then(|s| s.as_ref()).is_some_and(|s| s.0 == id) {
                if got.is_ok() {
                    r.friends.shifts = shifts;
                }
                r.friends.list = Some(got);
                r.friends.requests = asks;
                r.friends.loading = false;
                r.friends.at = Some(Instant::now());
                cx.notify();
            }
        });
    })
    .detach();
}

impl Data {
    /// A tab's, for the account it is opened for (pages/tabs.rs).
    pub fn of(account: String) -> Data {
        Data { account: Some(account), ..Default::default() }
    }

    /// Put away with its tab: nothing is on its way any more (an answer that comes is dropped), so what is missing
    /// is asked again when the tab is shown; its menus close.
    pub fn rest(&mut self) {
        self.loading = false;
        if self.sign.is_none() {
            self.asked_sign = false;
        }
        self.menu = false;
        self.card_menu = None;
        self.sure = None;
        // A move or a search still under way is dropped with its answer: its button and the box come back.
        self.acts.retain(|_, a| *a != Act::Busy);
        if matches!(self.found, Some((_, None))) {
            self.found = None;
        }
    }

    /// The sign-in held is let go (the open vault or the app's accounts changed): it is asked again, and what was read
    /// with it goes, as for another account.
    pub fn forget_sign_in(&mut self) {
        self.rest();
        self.sign = None;
        self.asked_sign = false;
        // (An answer still on its way for the old sign-in is dropped.)
        self.sign_seq = self.sign_seq.wrapping_add(1);
        self.sign_at = None;
        self.list = None;
        self.requests.clear();
        self.shifts.clear();
        self.at = None;
        self.acts.clear();
        self.found = None;
        self.card_menu_was = None;
    }
}

fn again(r: &mut Root, cx: &mut Context<Root>) {
    if !r.friends.loading {
        if r.friends.list.as_ref().is_some_and(|l| l.is_err()) {
            r.friends.list = None;
        }
        load(r, cx);
        cx.notify();
    }
}

/// Roblox's people search for the name in Add friend's box (Enter).
pub fn find(r: &mut Root, cx: &mut Context<Root>) {
    let q = r.friend_search.read(cx).value().trim().to_string();
    if q.is_empty() {
        r.friends.found = None;
        cx.notify();
        return;
    }
    r.friends.found = Some((q.clone(), None));
    cx.notify();
    let ask = q.clone();
    let rx = roblox::run(move || roblox::people(&ask));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.friends.found.as_ref().is_some_and(|(at, _)| *at == q) {
                r.friends.found = Some((q, Some(got)));
                cx.notify();
            }
        });
    })
    .detach();
}

/// A move for the account looked through, asked of the app: "request", "accept", "decline" or "unfriend" that Roblox
/// user. Done, the lists are asked again (an accepted request is a friend now).
fn act(r: &mut Root, user: i64, what: &'static str, cx: &mut Context<Root>) {
    let (Some(link), Some(Some((id, _, _)))) = (r.link.clone(), r.friends.sign.clone()) else { return };
    if r.friends.acts.get(&user) == Some(&Act::Busy) {
        return;
    }
    r.friends.acts.insert(user, Act::Busy);
    cx.notify();
    let rx = link.call("friendAct", json!({ "id": id, "userId": user, "act": what }));
    cx.spawn(async move |this, cx| {
        let got = rx.await.ok().and_then(Result::ok);
        let ok = got.as_ref().and_then(|v| v.get("ok")).and_then(Value::as_bool).unwrap_or(false);
        // A check closed unsolved isn't Roblox's refusal: the button comes back as it was.
        let held = got.as_ref().and_then(|v| v.get("held")).and_then(Value::as_bool).unwrap_or(false);
        let _ = this.update(cx, |r, cx| {
            if !r.friends.sign.as_ref().and_then(|s| s.as_ref()).is_some_and(|s| s.0 == id) {
                return;
            }
            if ok {
                r.friends.requests.retain(|f| f.id != user);
                if what == "request" {
                    r.friends.acts.insert(user, Act::Sent);
                } else {
                    r.friends.acts.remove(&user);
                }
                if what == "accept" {
                    again(r, cx);
                }
                if what == "unfriend" {
                    if let Some(Ok(l)) = &mut r.friends.list {
                        l.retain(|f| f.id != user);
                    }
                }
            } else if held {
                r.friends.acts.remove(&user);
            } else {
                r.friends.acts.insert(user, Act::Refused);
            }
            cx.notify();
        });
    })
    .detach();
}

fn passes(f: &Friend, by: Filter) -> bool {
    match by {
        Filter::All => true,
        Filter::Playing => f.kind == 2,
        Filter::Online => f.kind != 0,
        Filter::Offline => f.kind == 0,
    }
}

pub fn friends(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let fr = &root.friends;
    let inner = width - 2. * (PAD + 4.);
    let (cols, card_w) = grid_fit(inner, CARD_W, GRID_GAP);
    let (able, viewer) = looking(root);
    let list: &[Friend] = match &fr.list {
        Some(Ok(l)) => l,
        _ => &[],
    };

    let shell = div().id("friends").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let body: AnyElement = if able.is_empty() || matches!(fr.sign, Some(None)) {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("fr.noAccount")))
            .child(Button::new("fr-add-acc", root.tr("acc.add")).icon("plus").primary().on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
            .into_any_element()
    } else {
        match &fr.list {
            None => note(root.tr("fr.loading")).into_any_element(),
            Some(Err(why)) => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(14.))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("fr.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])))
                .child(Button::new("fr-retry", root.tr("fr.retry")).icon("refresh").on_click(cx.listener(|r, _, _, cx| again(r, cx))))
                .into_any_element(),
            Some(Ok(l)) => {
                let join_as = viewer.as_ref().map(|a| s(a, "id").to_string());
                let mut rows = div().flex().flex_col().gap(px(SECTION_GAP)).px(px(PAD + 4.)).pt(px(4.)).pb(px(24.));
                if fr.adding {
                    rows = rows.child(add(root, l, inner, cx));
                }
                // A group a line, the page's width, its head over its cards on the page's fixed grid (critique
                // 2026-11-13, P2: groups flowing side by side ran Requests, a game and Online into one ragged row and
                // the reading order was lost).
                let _ = cols;
                let mut blocks: Vec<AnyElement> = Vec::new();
                let block = |head: AnyElement, cards: Vec<AnyElement>| {
                    div()
                        .w_full()
                        .flex_none()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        // (A head its own height now the groups stack: a game's picture, or a line of words.)
                        .child(div().w_full().flex().items_center().child(head))
                        .child(grid(GRID_GAP).children(cards))
                        .into_any_element()
                };
                if !fr.requests.is_empty() && fr.filter == Filter::All {
                    let cards: Vec<AnyElement> = fr.requests.iter().map(|f| card(root, f, card_w, Kind::Request, cx).into_any_element()).collect();
                    blocks.push(block(text_head(root.tr("fr.requests"), fr.requests.len(), None).into_any_element(), cards));
                }
                let mut any = false;
                // Friends in a game stand by game, a group each (as Accounts lays out its windows), the fullest first:
                // where everyone is shows at a glance, which Roblox's one row of faces doesn't.
                let playing: Vec<&Friend> = l.iter().filter(|f| f.kind == 2 && passes(f, fr.filter)).collect();
                let mut lanes: Vec<(String, String, Vec<&Friend>)> = Vec::new();
                for f in playing {
                    match lanes.iter_mut().find(|(name, _, _)| *name == f.game) {
                        Some(lane) => lane.2.push(f),
                        // (The game's wide picture; its square icon until that is known.)
                        None => lanes.push((f.game.clone(), if f.game_cover.is_empty() { f.game_icon.clone() } else { f.game_cover.clone() }, vec![f])),
                    }
                }
                lanes.sort_by(|a, b| a.0.is_empty().cmp(&b.0.is_empty()).then(b.2.len().cmp(&a.2.len())));
                for (name, pic, of) in lanes {
                    any = true;
                    const PIC_W: f32 = 78.;
                    let icon_pic = (!pic.is_empty()).then(|| images::pic_at(&pic, PIC_W, GROUP_PIC, cx)).flatten();
                    // (The picture itself keys its fade-in.)
                    let icon_key = (!pic.is_empty()).then(|| images::pic(&pic, cx)).flatten();
                    let n = of.len();
                    let cards: Vec<AnyElement> = of.into_iter().map(|f| card(root, f, card_w, Kind::Friend(join_as.clone(), true), cx).into_any_element()).collect();
                    // The game's picture, wide: the group is seen for the game it is before its words are read. No
                    // picture (a game Roblox doesn't name: private, or not known yet): a game mark on a block.
                    let picture = div()
                        .relative()
                        .w(px(PIC_W))
                        .h(px(GROUP_PIC))
                        .rounded(px(R_M))
                        .bg(c(RAISED_2))
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .justify_center()
                        .map(|d| match (icon_pic, icon_key) {
                            (Some(p), Some(k)) => d.child(fade_in(img(p).absolute().inset_0().size_full().rounded(px(R_M)).object_fit(ObjectFit::Cover), &k)),
                            (Some(p), None) => d.child(img(p).absolute().inset_0().size_full().rounded(px(R_M)).object_fit(ObjectFit::Cover)),
                            (None, _) => d.child(icon(if pic.is_empty() { "lock" } else { "gamepad" }, 18., c(MUTED))),
                        });
                    let title = if name.is_empty() { root.tr("fr.someGame") } else { name };
                    let head = group_head(picture, title, facts(vec![(root.i18n.tv("fr.playingHere", &[("n", &n.to_string())]), Some(c(OK)))]));
                    blocks.push(block(head.into_any_element(), cards));
                }
                for (g, key) in [(1, "chip.online"), (0, "chip.offline")] {
                    // Studio sits with Online: there, but not in a game to join.
                    let mut of: Vec<&Friend> = l.iter().filter(|f| passes(f, fr.filter)).filter(|f| if g == 1 { f.kind == 1 || f.kind == 3 } else { f.kind == g }).collect();
                    // Offline: those seen online latest come first.
                    if g == 0 {
                        of.sort_by(|a, b| b.seen.cmp(&a.seen));
                    }
                    if of.is_empty() {
                        continue;
                    }
                    any = true;
                    let n = of.len();
                    let cards = of.into_iter().map(|f| card(root, f, card_w, Kind::Friend(join_as.clone(), false), cx).into_any_element()).collect();
                    blocks.push(block(text_head(root.tr(key), n, None).into_any_element(), cards));
                }
                // What changed among the friends, the latest dozen: who is new, who is no longer a friend, who took
                // another name. Kept on this PC: Roblox tells none of it.
                if !fr.shifts.is_empty() && fr.filter == Filter::All {
                    let cards: Vec<AnyElement> = fr
                        .shifts
                        .iter()
                        .take(12)
                        .map(|sh| {
                            let day = crate::time::day_time(sh.at);
                            let (mark, words) = match sh.kind.as_str() {
                                "new" => (OK, root.i18n.tv("fr.s.new", &[("d", &day)])),
                                "gone" => (BAD, root.i18n.tv("fr.s.gone", &[("d", &day)])),
                                _ => (MUTED, root.i18n.tv("fr.s.renamed", &[("was", &sh.was), ("d", &day)])),
                            };
                            let who = Friend { id: sh.id, name: sh.name.clone(), display: if sh.display.is_empty() { sh.name.clone() } else { sh.display.clone() }, face: sh.face.clone(), ..Default::default() };
                            card(root, &who, card_w, Kind::Note(mark, words), cx).into_any_element()
                        })
                        .collect();
                    let head = text_head(root.tr("fr.shifts"), fr.shifts.len(), Some(root.tr("fr.shiftsNote")));
                    blocks.push(block(head.into_any_element(), cards));
                }
                if !blocks.is_empty() {
                    rows = rows.child(div().w_full().flex().flex_col().gap(px(SECTION_GAP)).children(blocks));
                }
                if !any {
                    rows = rows.child(div().pt(px(12.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr(if l.is_empty() { "fr.none" } else { "fr.noneHere" })));
                }
                crate::scrolled::body("fr-body", rows).into_any_element()
            }
        }
    };
    shell.child(header(root, list, viewer.as_ref(), cx)).child(body)
}

fn note(text: String) -> Div {
    div().flex_1().flex().items_center().justify_center().text_size(px(T_M)).text_color(c(MUTED)).child(text)
}

/// Add friend, open over the lists: the name box, then the people Roblox found for it.
fn add(root: &Root, friends: &[Friend], inner: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let fr = &root.friends;
    let top = div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr("fr.add")))
        .child(div().w(px(300.)).min_w(px(160.)).flex_shrink(1.).child(field(&root.friend_search, Some("search"))))
        .child(div().flex_1())
        .child(IconButton::new("fr-add-close", "x").size(30.).tip(root.tr("fr.close")).on_click(cx.listener(|r, _, _, cx| {
            r.friends.adding = false;
            cx.notify();
        })));
    let mut panel = surface().p(px(18.)).flex().flex_col().gap(px(14.)).child(top);
    match &fr.found {
        None => {}
        Some((_, None)) => panel = panel.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("games.searching"))),
        Some((_, Some(Err(why)))) => {
            panel = panel.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("fr.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])))
        }
        Some((_, Some(Ok(l)))) if l.is_empty() => panel = panel.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("fr.nobody"))),
        Some((_, Some(Ok(l)))) => {
            // The cards share the panel's inside (its padding and hairline off), as the lists below share the page.
            let inside = inner - 38.;
            let (_, w) = grid_fit(inside, CARD_W, GRID_GAP);
            let cards: Vec<AnyElement> = l.iter().map(|f| card(root, f, w, Kind::Found(friends.iter().any(|x| x.id == f.id)), cx).into_any_element()).collect();
            panel = panel.child(grid(GRID_GAP).children(cards));
        }
    }
    panel
}

/// The title and count, the Games | Friends switch, then Add friend, whose friends these are and Refresh; the filters
/// under them.
fn header(root: &Root, list: &[Friend], viewer: Option<&Value>, cx: &mut Context<Root>) -> impl IntoElement {
    let fr = &root.friends;
    let playing = list.iter().filter(|f| f.kind == 2).count();
    let online = list.iter().filter(|f| f.kind != 0).count();
    let chips = [
        (Filter::All, root.tr("chip.all"), list.len()),
        (Filter::Playing, root.tr("chip.ingame"), playing),
        (Filter::Online, root.tr("chip.online"), online),
        (Filter::Offline, root.tr("chip.offline"), list.len() - online),
    ];


    div()
        .flex_none()
        .px(px(PAD + 4.))
        .pt(px(18.))
        .pb(px(18.))
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(div().text_size(px(T_TITLE)).font_weight(FontWeight::BOLD).child("Roblox"))
                .child(div().ml(px(6.)).child(crate::pages::games::views(root, cx)))
                .child(div().flex_1())
                .when(matches!(fr.list, Some(Ok(_))), |d| {
                    d.child(Button::new("fr-add", root.tr("fr.add")).icon("userPlus").on_click(cx.listener(|r, _, window, cx| {
                        r.friends.adding = !r.friends.adding;
                        if r.friends.adding {
                            let h = r.friend_search.read(cx).focus_handle(cx);
                            window.focus(&h, cx);
                        }
                        cx.notify();
                    })))
                })
                .when(viewer.is_some(), |d| {
                    d.child(IconButton::new("fr-refresh", "refresh").size(34.).busy(fr.loading).tip(root.tr("fr.refresh")).on_click(cx.listener(|r, _, _, cx| again(r, cx))))
                }),
        )
        // The filters as the Accounts page's: no track, a dot of where they are, the count; the one shown a yellow block.
        .when(!list.is_empty(), |d| {
            d.child(div().flex().flex_wrap().items_center().gap(px(4.)).children(chips.into_iter().enumerate().map(|(i, (f, text, count))| {
                let on = fr.filter == f;
                let dot = match f {
                    Filter::All => None,
                    Filter::Playing => Some(OK),
                    Filter::Online => Some(ONLINE),
                    Filter::Offline => Some(MUTED),
                };
                chip(("fr-chip", i), on)
                    .h(px(TOOL_H))
                    .when_some(dot, |d, col| d.child(div().size(px(7.)).flex_none().rounded_full().bg(c(col))))
                    .child(text)
                    .child(chip_count(count, on))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        r.friends.filter = f;
                        cx.notify();
                    }))
            })))
        })
}

/// What a card is for, which decides its buttons: a friend (Join as that account, when the server shows), a request
/// waiting (accept, decline), someone found by Add friend (already a friend or not).
enum Kind {
    /// (Join as that account; and whether the card stands in its game's lane, where the game needn't be said again.)
    Friend(Option<String>, bool),
    Request,
    Found(bool),
    /// A change kept of this person: a dot of that colour and the words for it, in place of where they are.
    Note(u32, String),
}

/// A person: face with a dot for where they are, name, then the game or their Roblox name; the buttons for what the
/// card is, and the profile.
fn card(root: &Root, f: &Friend, w: f32, kind: Kind, cx: &mut Context<Root>) -> Div {
    const FACE: f32 = 46.;
    let face = (!f.face.is_empty()).then(|| images::pic_sq(&f.face, FACE - 8., cx)).flatten();
    let in_lane = matches!(kind, Kind::Friend(_, true));
    let game_pic = (!f.game_icon.is_empty() && !in_lane).then(|| images::pic_sq(&f.game_icon, 14., cx)).flatten();
    let is_friend = matches!(kind, Kind::Friend(..));
    // One person can show twice (found by Add friend, and in the list): each card's buttons get ids of their own.
    let tag = match kind {
        Kind::Friend(..) => 0usize,
        Kind::Request => 1,
        Kind::Found(_) => 2,
        Kind::Note(..) => 3,
    };
    let noted = if let Kind::Note(mark, words) = &kind { Some((*mark, words.clone())) } else { None };
    let dot = match f.kind {
        2 => OK,
        1 | 3 => ONLINE,
        _ => DIM,
    };
    let line = match f.kind {
        2 if in_lane => format!("@{}", f.name),
        2 if !f.game.is_empty() => f.game.clone(),
        2 => root.tr("chip.ingame"),
        3 => root.tr("fr.studio"),
        1 => root.tr("chip.online"),
        // Offline: when this PC last saw them online and the game it sees them in most (kept here; Roblox shows neither).
        _ if f.seen > 0 => {
            let on = root.i18n.tv("fr.lastOn", &[("ago", &crate::time::ago(crate::time::now_secs() - f.seen, &root.i18n))]);
            if f.usual.is_empty() { on } else { format!("{on} · {}", root.i18n.tv("fr.usual", &[("game", &f.usual)])) }
        }
        _ => format!("@{}", f.name),
    };
    let (mark, line) = match noted {
        Some((m, w)) => (Some(m), w),
        None => (None, line),
    };
    let user = f.id;
    let key = (user as usize).wrapping_mul(4) + tag;
    let state = root.friends.acts.get(&user).copied();
    let busy = state == Some(Act::Busy);
    let said = |key: &str| div().flex_none().px(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr(key));
    // A request's Accept and Decline go under its name, so the name and @name keep the card's width.
    let request_buttons = matches!(kind, Kind::Request) && state != Some(Act::Refused);
    let first: String = f.display.trim().chars().next().or_else(|| f.name.chars().next()).map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
    let hue = (f.id.unsigned_abs() % 360) as f32 / 360.;
    let ring = match f.kind {
        _ if !is_friend => ca(WHITE, 0.12),
        2 => c(OK),
        1 | 3 => c(ONLINE),
        _ => ca(WHITE, 0.12),
    };
    // Offline: the card sits on the page ground, faded.
    let dim = is_friend && f.kind == 0 && root.friends.card_menu != Some(user);
    let base = div()
        .w(px(w))
        .min_h(px(70.))
        .py(px(10.))
        .pl(px(12.))
        .pr(px(8.))
        .flex()
        .items_center()
        .gap(px(12.))
        .rounded(px(R_L))
        .bg(c(if dim { PANEL } else { RAISED }))
        .border_1()
        .border_color(c(LINE))
        // (A card with its menu open stays whole: the menu would fade with it.)
        .when(dim, |d| d.opacity(0.7))
        // The face in a ring of where they are (green in a game, blue online), the web page's .avatar; their first
        // letter on a colour of their own while Roblox gives no picture.
        // (A click on the face opens their profile, on every card: the request cards' lone person icon at the right
        // read as an unlabelled mark, review 3c.2.)
        .child(
            div()
                .id(("fr-face", key))
                .relative()
                .size(px(FACE))
                .flex_none()
                .rounded_full()
                .p(px(2.))
                .bg(ring)
                .cursor_pointer()
                .hover(|d| d.opacity(0.85))
                .tooltip({
                    let t: SharedString = root.tr("ui.profile").into();
                    move |_, cx| cx.new(|_| Tip(t.clone())).into()
                })
                .on_click(cx.listener(move |r, _, _, cx| crate::pages::profile::open_person(r, user, cx)))
                .child(
                    div()
                        .size_full()
                        .rounded_full()
                        .border_2()
                        .border_color(c(PANEL))
                        .bg(c(RAISED_2))
                        .flex()
                        .items_center()
                        .justify_center()
                        .map(|x| match face {
                            Some(p) => x.child(img(p).size_full().rounded_full().object_fit(ObjectFit::Cover)),
                            None => x.bg(hsla(hue, 0.38, 0.40, 1.)).text_color(c(WHITE)).text_size(px(17.)).font_weight(FontWeight::BOLD).child(first.clone()),
                        }),
                )
                .when(is_friend, |x| x.child(div().absolute().right(px(-1.)).bottom(px(-1.)).size(px(12.)).rounded_full().border_2().border_color(c(PANEL)).bg(c(dot)))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_size(px(T_L)).font_weight(FontWeight::MEDIUM).truncate().child(f.display.clone()))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(T_S))
                        .text_color(c(MUTED))
                        .when_some(mark, |d, col| d.child(div().size(px(7.)).flex_none().rounded_full().bg(c(col))))
                        .when_some(game_pic, |d, p| d.child(img(p).size(px(14.)).flex_none().rounded(px(3.))))
                        .child(div().min_w_0().truncate().child(line)),
                )
                .when(request_buttons, |d| {
                    d.child(
                        div()
                            .pt(px(6.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(Button::new(("fr-accept", key), root.tr("fr.accept")).small().disabled(busy).on_click(cx.listener(move |r, _, _, cx| act(r, user, "accept", cx))))
                            .child(IconButton::new(("fr-decline", key), "x").size(30.).disabled(busy).tip(root.tr("fr.decline")).on_click(cx.listener(move |r, _, _, cx| act(r, user, "decline", cx)))),
                    )
                }),
        );
    let with = match kind {
        Kind::Friend(join_as, _) => {
            let joinable = f.kind == 2 && f.place > 0 && !f.job.is_empty();
            let (place, job, name) = (f.place, f.job.clone(), f.name.clone());
            base.when_some(join_as.filter(|_| joinable), |d, acc| {
                d.child(Button::new(("fr-join", key), root.tr("fr.join")).small().on_click(cx.listener(move |r, _, _, cx| {
                    r.launch("joinServer", json!({ "ids": [acc], "placeId": place, "jobId": job, "followUserId": user, "followName": name }), false, cx)
                })))
            })
        }
        Kind::Request => match state {
            Some(Act::Refused) => base.child(said("fr.refused")),
            _ => base,
        },
        Kind::Note(..) => base,
        Kind::Found(true) => base.child(said("nav.friends")),
        Kind::Found(false) => match state {
            Some(Act::Sent) => base.child(said("fr.sent")),
            Some(Act::Refused) => base.child(said("fr.refused")),
            _ => base.child(Button::new(("fr-request", key), root.tr("fr.request")).small().disabled(busy).on_click(cx.listener(move |r, _, _, cx| act(r, user, "request", cx)))),
        },
    };
    if !is_friend {
        return with;
    }
    // A friend's card: a menu with the profile and Unfriend (a second click does it).
    let mut more = div().relative().flex_none().child(IconButton::new(("fr-more", key), "more").size(30.).on_click(cx.listener(move |r, _, _, cx| {
        r.friends.card_menu = if r.friends.card_menu == Some(user) { None } else { Some(user) };
        r.friends.card_menu_was = Some(user);
        r.friends.sure = None;
        cx.notify();
    })));
    if root.friends.card_menu == Some(user) || root.friends.card_menu_was == Some(user) {
        let sure = root.friends.sure == Some(user);
        let row = |id: &'static str| hot((id, key)).h(px(32.)).px(px(10.)).rounded(px(R_S)).flex().items_center().gap(px(8.)).text_size(px(T_M)).cursor_pointer();
        let menu = div()
            .id(("fr-card-menu", key))
            .occlude()
            .w(px(210.))
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|r, _, _, cx| {
                r.friends.card_menu = None;
                r.friends.sure = None;
                cx.notify();
            }))
            .child(row("fr-m-profile").text_color(c(TEXT_2)).child(icon("user", 14., c(MUTED))).child(root.tr("ui.profile")).on_click(cx.listener(move |r, _, _, cx| {
                r.friends.card_menu = None;
                crate::pages::profile::open_person(r, user, cx);
            })))
            .child(row("fr-m-unfriend").text_color(c(if sure { BAD } else { TEXT_2 })).child(icon("x", 14., c(if sure { BAD } else { MUTED }))).child(root.tr(if sure { "fr.sure" } else { "fr.unfriend" })).on_click(
                cx.listener(move |r, _, _, cx| {
                    if r.friends.sure == Some(user) {
                        r.friends.card_menu = None;
                        r.friends.sure = None;
                        act(r, user, "unfriend", cx);
                    } else {
                        r.friends.sure = Some(user);
                    }
                    cx.notify();
                }),
            ));
        more = more.child(div().absolute().top(px(34.)).right_0().child(deferred(anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(pop(format!("fr-card-pop-{user}"), root.friends.card_menu == Some(user), menu))).with_priority(1)));
    }
    with.child(more)
}

/// A group's head with no picture (Requests, Online, Offline): its title and count set as a game's head sets its name,
/// and a line under it (a note, or kept empty), so heads side by side stand on one baseline (review 3c.1).
fn text_head(title: String, n: usize, note: Option<String>) -> Div {
    let has_note = note.is_some();
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .text_size(px(T_GROUP))
                .line_height(px(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(div().flex_none().text_color(c(TEXT)).child(title))
                .child(div().flex_none().text_color(c(MUTED)).font_weight(FontWeight::NORMAL).font_features(tnum()).child(n.to_string())),
        )
        .child(facts(vec![(note.unwrap_or_else(|| "·".to_string()), None)]).when(!has_note, |d| d.opacity(0.)))
}
