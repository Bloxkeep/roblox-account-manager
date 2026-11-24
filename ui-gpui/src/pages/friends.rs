// Friends, as the Roblox app has them, inside Games (its header's Games | Friends switch): the friends of the account
// whose tab is shown (pages/tabs.rs), with
// where each is now (in a game, online, offline), what changed among them since this PC first read them (roblox.rs), Join for a friend whose server Roblox shows to that account, the
// requests waiting for it (accept, decline) and Add friend (Roblox's people search, then a request). The lists are asked
// from here (roblox.rs) with that account's own sign-in, lent by the app (robloxSignIn { id }): Roblox gives them only to
// a signed-in request. What changes something goes through the app: friendAct (a request, its answer; the app shows
// Roblox's check when it holds one), joinServer (the usual launch), openProfile.
use crate::threads::Hold;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};
use crate::val::s;
use crate::ask::reply;

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
    /// A right click on a face in Games' row of faces: whose menu and where the mouse was, and whether it is open
    /// (false: on its way out, still built so it closes as it opened).
    pub face_menu: Option<(i64, Point<Pixels>)>,
    pub face_menu_open: bool,
    /// Only the friends with this label (with the status filter).
    pub label_filter: Option<u32>,
    /// The labels' box (under the Labels button): open, the colour a new label gets, and the friend a new one is for
    /// (opened from that friend's menu).
    pub tags_open: bool,
    pub tag_colour: usize,
    pub tag_for: Option<i64>,
    /// A friend's menu shows the labels to tick.
    pub menu_tags: bool,
}

/// A label of the user's own for friends: a name and a colour (LABEL_COLOURS). Kept on this PC (prefs.rs), the same for
/// every account: a friend is the same person whichever account looks.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Label {
    pub id: u32,
    pub name: String,
    pub colour: u32,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Labels {
    pub list: Vec<Label>,
    /// The labels each friend has, by their Roblox id.
    #[serde(default)]
    pub of: HashMap<i64, Vec<u32>>,
}

pub const LABEL_COLOURS: [u32; 8] = [0xff6257, 0xffab3d, 0xf6c63a, 0x3ddc84, 0x2ec4b6, 0x5aa2ff, 0xa78bfa, 0xf472b6];

static LABELS: LazyLock<Mutex<Labels>> = LazyLock::new(|| Mutex::new(crate::prefs::get("friendLabels").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()));

pub fn labels() -> Labels {
    LABELS.hold().clone()
}

/// A friend's labels, in the list's order.
pub fn labels_of(user: i64) -> Vec<Label> {
    let l = LABELS.hold();
    let Some(ids) = l.of.get(&user) else { return Vec::new() };
    l.list.iter().filter(|x| ids.contains(&x.id)).cloned().collect()
}

fn change_labels(f: impl FnOnce(&mut Labels)) {
    let mut l = LABELS.hold();
    f(&mut l);
    // (A friend left with none is forgotten.)
    l.of.retain(|_, v| !v.is_empty());
    crate::prefs::set("friendLabels", serde_json::to_value(&*l).unwrap_or_default());
}

fn toggle_label(user: i64, id: u32) {
    change_labels(|l| {
        let v = l.of.entry(user).or_default();
        if let Some(i) = v.iter().position(|x| *x == id) {
            v.remove(i);
        } else {
            v.push(id);
        }
    });
}

/// A new label from the box: its name (one already there isn't made twice), the colour picked, and given to the friend
/// it was opened for.
pub fn add_label(r: &mut Root, cx: &mut Context<Root>) {
    let name: String = r.label_box.read(cx).value().trim().chars().take(24).collect();
    if name.is_empty() {
        return;
    }
    let colour = LABEL_COLOURS[r.friends.tag_colour % LABEL_COLOURS.len()];
    let user = r.friends.tag_for;
    let mut made = false;
    change_labels(|l| {
        let id = match l.list.iter().find(|x| x.name.eq_ignore_ascii_case(&name)) {
            Some(x) => x.id,
            None => {
                let id = l.list.iter().map(|x| x.id).max().unwrap_or(0) + 1;
                l.list.push(Label { id, name: name.clone(), colour });
                made = true;
                id
            }
        };
        if let Some(u) = user {
            let v = l.of.entry(u).or_default();
            if !v.contains(&id) {
                v.push(id);
            }
        }
    });
    // The next one gets the next colour, so labels made one after another differ.
    if made {
        r.friends.tag_colour = (r.friends.tag_colour + 1) % LABEL_COLOURS.len();
    }
    r.friends.tag_for = None;
    empty_field(&r.label_box, cx);
    cx.notify();
}

/// The server the account looked through is in (to invite a friend to), when it is in one.
fn my_server(root: &Root) -> Option<(i64, String)> {
    looking_ref(root).1.and_then(|a| crate::pages::accounts::menu::server_of(a))
}

/// A friend invited to the server the account is in: its link copied and put in their chat's box (Inbox), to send with
/// Enter, so it goes as the user's own words and Roblox's filter shows on it.
fn invite(r: &mut Root, user: i64, window: &mut Window, cx: &mut Context<Root>) {
    let Some((place, job)) = my_server(r) else { return };
    let link = format!("https://www.roblox.com/games/start?placeId={place}&gameInstanceId={job}");
    cx.write_to_clipboard(ClipboardItem::new_string(link.clone()));
    r.chat_box.update(cx, |s, cx| s.set_value(link, window, cx));
    r.toast = None;
    crate::pages::inbox::chat_with(r, user, cx);
    if r.toast.is_none() {
        r.toast = Some((r.tr("fr.inviteReady"), "ok".into()));
    }
}

/// A friend's menu's labels part: "Labels ›" and, open, each label to tick for them and "New label…" (the labels' box,
/// for them).
fn tag_rows(root: &Root, user: i64, from_face: bool, close: fn(&mut Root), cx: &mut Context<Root>) -> Vec<AnyElement> {
    let base = |id: ElementId| hot(id).h(px(32.)).px(px(10.)).rounded(px(R_S)).flex().items_center().gap(px(8.)).text_size(px(T_M)).text_color(c(TEXT_2)).cursor_pointer();
    let k = from_face as usize;
    let open = root.friends.menu_tags;
    let mut out = vec![
        base(("fr-m-tags", k).into())
            .child(icon("tag", 14., c(MUTED)))
            .child(div().flex_1().child(root.tr("tag.labels")))
            .child(icon(if open { "chevronUp" } else { "chevronDown" }, 12., c(MUTED)))
            .on_click(cx.listener(|r, _, _, cx| {
                r.friends.menu_tags = !r.friends.menu_tags;
                cx.notify();
            }))
            .into_any_element(),
    ];
    if !open {
        return out;
    }
    let mine: Vec<u32> = labels_of(user).iter().map(|l| l.id).collect();
    for (i, l) in labels().list.into_iter().enumerate() {
        let on = mine.contains(&l.id);
        let id = l.id;
        out.push(
            base(("fr-m-tag", i * 2 + k).into())
                .pl(px(18.))
                .child(div().size(px(9.)).flex_none().rounded_full().bg(c(l.colour)))
                .child(div().flex_1().min_w_0().truncate().child(l.name))
                .when(on, |d| d.child(icon("check", 14., c(FACE))))
                .on_click(cx.listener(move |_, _, _, cx| {
                    toggle_label(user, id);
                    cx.notify();
                }))
                .into_any_element(),
        );
    }
    out.push(
        base(("fr-m-newtag", k).into())
            .pl(px(18.))
            .text_color(c(MUTED))
            .child(icon("plus", 13., c(MUTED)))
            .child(root.tr("tag.new"))
            .on_click(cx.listener(move |r, _, window, cx| {
                close(r);
                // The labels' box is on Friends, under its Labels button: opened from a friend's face on the Roblox
                // home (the same menu), nothing showed (the owner, 2026-11-15: "new label không hoạt động"). That
                // page comes first, then its box, open for this friend.
                r.games.open = None;
                r.games.view = crate::pages::games::View::Friends;
                r.friends.tags_open = true;
                r.friends.tag_for = Some(user);
                let h = r.label_box.read(cx).focus_handle(cx);
                window.focus(&h, cx);
                cx.notify();
            }))
            .into_any_element(),
    );
    out
}

/// The labels' box, under the Labels button: each label (its colour turns on a click, the bin deletes it from every
/// friend) and a new one's name, colour and Add.
fn tags_box(root: &Root, cx: &mut Context<Root>) -> impl IntoElement {
    let lb = labels();
    let count = |id: u32| lb.of.values().filter(|v| v.contains(&id)).count();
    let for_name = root.friends.tag_for.and_then(|u| match &root.friends.list {
        Some(Ok(l)) => l.iter().find(|f| f.id == u).map(|f| f.display.clone()),
        _ => None,
    });
    let picked = root.friends.tag_colour % LABEL_COLOURS.len();
    let typed = !root.label_box.read(cx).value().trim().is_empty();
    div()
        .id("fr-tags-box")
        .occlude()
        .w(px(300.))
        .p(px(14.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .shadow_lg()
        .flex()
        .flex_col()
        .gap(px(10.))
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            r.friends.tags_open = false;
            r.friends.tag_for = None;
            cx.notify();
        }))
        .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(root.tr("tag.yours")))
        .map(|d| {
            if lb.list.is_empty() {
                return d.child(div().text_size(px(T_S)).line_height(px(18.)).text_color(c(MUTED)).child(root.tr("tag.none")));
            }
            d.child(div().flex().flex_col().gap(px(2.)).children(lb.list.iter().enumerate().map(|(i, l)| {
                let id = l.id;
                div()
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .id(("fr-tag-col", i))
                            .size(px(16.))
                            .flex_none()
                            .rounded_full()
                            .bg(c(l.colour))
                            .cursor_pointer()
                            .hover(|d| d.opacity(0.8))
                            .tooltip({
                                let t: SharedString = root.tr("tag.recolour").into();
                                move |_, cx| cx.new(|_| Tip(t.clone())).into()
                            })
                            .on_click(cx.listener(move |_, _, _, cx| {
                                change_labels(|lb| {
                                    if let Some(x) = lb.list.iter_mut().find(|x| x.id == id) {
                                        let at = LABEL_COLOURS.iter().position(|c| *c == x.colour).unwrap_or(0);
                                        x.colour = LABEL_COLOURS[(at + 1) % LABEL_COLOURS.len()];
                                    }
                                });
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).child(l.name.clone()))
                    .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(count(id).to_string()))
                    .child(IconButton::new(("fr-tag-del", i), "trash").size(28.).tip(root.tr("tag.delete")).on_click(cx.listener(move |r, _, _, cx| {
                        change_labels(|lb| {
                            lb.list.retain(|x| x.id != id);
                            for v in lb.of.values_mut() {
                                v.retain(|x| *x != id);
                            }
                        });
                        if r.friends.label_filter == Some(id) {
                            r.friends.label_filter = None;
                        }
                        cx.notify();
                    })))
            })))
        })
        .child(div().h(px(1.)).bg(c(LINE)))
        .when_some(for_name, |d, n| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tv("tag.forWho", &[("name", &n)]))))
        .child(field(&root.label_box, Some("tag")).h(px(36.)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .children(LABEL_COLOURS.iter().enumerate().map(|(i, col)| {
                    div()
                        .id(("fr-tag-pick", i))
                        .size(px(24.))
                        .p(px(3.))
                        .rounded_full()
                        .border_2()
                        .border_color(if i == picked { c(FACE) } else { gpui_kit::transparent_black() })
                        .cursor_pointer()
                        .child(div().size_full().rounded_full().bg(c(*col)))
                        .on_click(cx.listener(move |r, _, _, cx| {
                            r.friends.tag_colour = i;
                            cx.notify();
                        }))
                }))
                .child(div().flex_1())
                .child(Button::new("fr-tag-add", root.tr("tag.add")).small().primary().disabled(!typed).on_click(cx.listener(|r, _, _, cx| add_label(r, cx)))),
        )
}

/// Where friends are is asked again when the page is looked at this long after the last answer.
const FRESH: Duration = Duration::from_secs(60);
/// "No account is signed in" is asked again at most this often (at once when the app's accounts change).
const NOBODY_AGAIN: Duration = Duration::from_secs(5);
const CARD_W: f32 = 300.;
/// A friend's card's height (its words are one line each: the card never grows).
const FRIEND_H: f32 = 70.;

/// The accounts whose friends can be asked: signed in, on roblox.com (a shot's copy of them).
#[cfg(feature = "shot")]
pub fn able(root: &Root) -> Vec<Value> {
    able_ref(root).into_iter().cloned().collect()
}

/// `able`, borrowed from the app's state (what a page drawn every frame asks for: no copy of every account).
pub fn able_ref(root: &Root) -> Vec<&Value> {
    root.accounts()
        .iter()
        .filter(|a| a.get("cookieValid").and_then(Value::as_bool) != Some(false) && s(a, "realm").is_empty() && a.get("userId").and_then(Value::as_i64).unwrap_or(0) > 0)
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

/// What a Roblox tab shows before there is a signed-in account: the tab's own mark, what it needs it for, and Add account.
pub fn need_account(root: &Root, id: &'static str, key: &str, mark: &'static str, cx: &mut Context<Root>) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(16.))
        .pb(px(40.))
        .child(div().size(px(56.)).rounded_full().border_1().border_color(c(LINE)).flex().items_center().justify_center().child(icon(mark, 24., c(MUTED))))
        .child(div().max_w(px(420.)).text_center().text_size(px(T_M)).line_height(px(21.)).text_color(c(TEXT_2)).child(root.tr(key)))
        .child(Button::new(id, root.tr("acc.add")).icon("plus").primary().on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
        .into_any_element()
}

/// The accounts that can be looked through, and the one that is: the one the app answered for, else the one picked.
pub fn looking(root: &Root) -> (Vec<Value>, Option<Value>) {
    let (able, viewer) = looking_ref(root);
    (able.into_iter().cloned().collect(), viewer.cloned())
}

/// `looking`, borrowed from the app's state.
pub fn looking_ref(root: &Root) -> (Vec<&Value>, Option<&Value>) {
    let able = crate::pages::tabs::available(root);
    let at = root.friends.sign.as_ref().and_then(|s| s.as_ref()).map(|s| s.0.as_str()).or(root.friends.account.as_deref());
    let viewer = at.and_then(|id| able.iter().find(|a| s(a, "id") == id)).copied();
    (able, viewer)
}

/// The sign-in first, then the friends; again when the answer is old.
pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    want_sign(r, link, cx);
    want_list(r, cx);
}

/// The friends' list, read when it isn't fresh (the Friends tab, Inbox's group picker).
pub fn want_list(r: &mut Root, cx: &mut Context<Root>) {
    let old = r.friends.at.is_none_or(|t| t.elapsed() > FRESH);
    if old && !r.friends.loading {
        load(r, cx);
    }
}

/// The sign-in of the account looked through, asked of the app once (Friends and Avatar share it).
pub fn want_sign(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    // A VNG-only vault still gets its account tab. A VNG account's sign-in is lent for its own tab only, and its calls
    // go to VNG's hosts (roblox.rs, routed).
    if r.friends.account.is_none() && able_ref(r).is_empty()
        && let Some(id) = crate::pages::tabs::available(r).first().map(|a| s(a, "id").to_string())
        && !crate::pages::tabs::picking(r)
    {
        crate::pages::tabs::bind_default(r, &id);
    }
    // (Several accounts and none picked for this tab: the choice is shown, tabs::picker; nobody's sign-in is asked.)
    if crate::pages::tabs::picking(r) {
        return;
    }
    #[cfg(feature = "shot")]
    if r.friends.sign.is_none() && (std::env::var("BLOXKEEP_SHOT_FRIENDS").is_ok() || std::env::var("BLOXKEEP_SHOT_AVATAR").is_ok() || std::env::var("BLOXKEEP_SHOT_PROFILE").is_ok() || std::env::var("BLOXKEEP_SHOT_INBOX").is_ok() || std::env::var("BLOXKEEP_SHOT_CART").is_ok()) {
        // A hidden shot: the mock's first account, with the friends from the file (roblox.rs) or the avatar of the
        // Roblox user BLOXKEEP_SHOT_AVATAR names (an avatar is public).
        let first = r.friends.account.clone().or(able_ref(r).first().map(|a| s(a, "id").to_string())).unwrap_or_default();
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
                let sign = got.and_then(|v| {
                    let cookie = s(&v, "cookie").to_string();
                    let id = s(&v, "id").to_string();
                    let user = v.get("userId").and_then(Value::as_i64).unwrap_or(0);
                    (!cookie.is_empty() && user > 0 && !id.is_empty()
                        && asked.as_ref().is_none_or(|expected| *expected == id)).then_some((id, user, cookie))
                });
                if let Some((id, _, _)) = &sign
                    && asked.is_none()
                    && !crate::pages::tabs::bind_default(r, id)
                {
                    cx.notify();
                    return;
                }
                r.friends.sign = Some(sign);
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
                // A friend in a game now who wasn't (or was in another) at the last reading: the capsule opens for them.
                if let (Some(Ok(old)), Ok(new)) = (&r.friends.list, &got) {
                    let went = new.iter().find(|f| f.kind == 2 && !f.game.is_empty() && old.iter().any(|o| o.id == f.id && (o.kind != 2 || o.place != f.place)));
                    if let Some(f) = went.cloned() {
                        let act = (f.place > 0 && !f.job.is_empty()).then(|| crate::pages::games::LiveAct::Join { acc: id.clone(), place: f.place, job: f.job.clone(), user: f.id, name: f.name.clone() });
                        let l = crate::pages::games::Live {
                            key: format!("friend:{}:{}", f.id, f.place),
                            pic: f.face.clone(),
                            round: true,
                            icon: "users",
                            tone: OK,
                            title: if f.display.is_empty() { f.name.clone() } else { f.display.clone() },
                            sub: r.i18n.tv("live.inGame", &[("game", &f.game)]),
                            act,
                            busy: false,
                            at: Instant::now(),
                        };
                        crate::pages::games::live_show(r, l, cx);
                    }
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
        self.face_menu_open = false;
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
        self.face_menu = None;
    }
}

/// A right click on a friend's face (Games' row of faces): its menu opens where the mouse is.
pub fn open_face_menu(r: &mut Root, user: i64, at: Point<Pixels>) {
    r.friends.face_menu = Some((user, at));
    r.friends.face_menu_open = true;
    r.friends.sure = None;
    r.friends.menu_tags = false;
}

/// The menu of a friend's face: what the Friends page does for a friend, where the right click was. Join (as the
/// account whose friends these are) and their game only while Roblox shows where they play; Unfriend takes a second
/// click, as on the card's menu.
pub fn face_menu(root: &Root, cx: &mut Context<Root>) -> Option<AnyElement> {
    let (user, at) = root.friends.face_menu?;
    let Some(Ok(list)) = &root.friends.list else { return None };
    let f = list.iter().find(|f| f.id == user)?;
    let open = root.friends.face_menu_open;
    let join_as = looking(root).1.and_then(|a| a.get("id").and_then(Value::as_str).map(str::to_string));
    let joinable = f.kind == 2 && f.place > 0 && !f.job.is_empty();
    let in_game = f.kind == 2 && f.place > 0;
    let sure = root.friends.sure == Some(user);
    fn close(r: &mut Root) {
        r.friends.face_menu_open = false;
        r.friends.sure = None;
        r.friends.menu_tags = false;
    }
    let row = |id: &'static str, glyph: &'static str, text: String| {
        hot(id).h(px(32.)).px(px(10.)).rounded(px(R_S)).flex().items_center().gap(px(8.)).text_size(px(T_M)).text_color(c(TEXT_2)).cursor_pointer().child(icon(glyph, 14., c(MUTED))).child(text)
    };
    let (place, job, name, game) = (f.place, f.job.clone(), f.name.clone(), f.game.clone());
    let mut menu = div()
        .id("ff-menu")
        .occlude()
        .w(px(220.))
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .shadow_lg()
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(move |r, _, _, cx| {
            // (Only the menu this one was built for: a right click on another face has just opened its own.)
            if r.friends.face_menu_open && r.friends.face_menu == Some((user, at)) {
                close(r);
                cx.notify();
            }
        }));
    if let Some(acc) = join_as.filter(|_| joinable) {
        let (job, name) = (job.clone(), name.clone());
        menu = menu.child(row("ff-join", "play", root.tr("fr.join")).on_click(cx.listener(move |r, _, _, cx| {
            close(r);
            r.launch("joinServer", json!({ "ids": [acc], "placeId": place, "jobId": job, "followUserId": user, "followName": name }), false, cx)
        })));
    }
    if in_game {
        menu = menu.child(row("ff-game", "gamepad", root.tr("fr.seeGame")).on_click(cx.listener(move |r, _, _, cx| {
            close(r);
            crate::pages::games::open_place(r, place, &game);
            cx.notify();
        })));
    }
    if my_server(root).is_some() {
        menu = menu.child(row("ff-invite", "link", root.tr("fr.invite")).on_click(cx.listener(move |r, _, window, cx| {
            close(r);
            invite(r, user, window, cx);
            cx.notify();
        })));
    }
    let copied = root.tr("t.copiedUser");
    menu = menu
        .child(row("ff-profile", "user", root.tr("ui.profile")).on_click(cx.listener(move |r, _, _, cx| {
            close(r);
            crate::pages::profile::open_person(r, user, cx);
        })))
        .child(row("ff-chat", "message", root.tr("fr.chat")).on_click(cx.listener(move |r, _, _, cx| {
            close(r);
            crate::pages::inbox::chat_with(r, user, cx);
        })))
        .child(row("ff-copy", "copy", root.tr("m.copyUser")).on_click(cx.listener(move |r, _, _, cx| {
            close(r);
            cx.write_to_clipboard(ClipboardItem::new_string(name.clone()));
            r.toast = Some((copied.clone(), "ok".into()));
            cx.notify();
        })))
        .children(tag_rows(root, user, true, close, cx))
        .child(div().h(px(1.)).mx(px(6.)).my(px(4.)).bg(c(LINE)))
        .child(
            hot("ff-unfriend")
                .h(px(32.))
                .px(px(10.))
                .rounded(px(R_S))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(T_M))
                .cursor_pointer()
                .text_color(c(if sure { BAD } else { TEXT_2 }))
                .child(icon("x", 14., c(if sure { BAD } else { MUTED })))
                .child(root.tr(if sure { "fr.sure" } else { "fr.unfriend" }))
                .on_click(cx.listener(move |r, _, _, cx| {
                    if r.friends.sure == Some(user) {
                        close(r);
                        act(r, user, "unfriend", cx);
                    } else {
                        r.friends.sure = Some(user);
                    }
                    cx.notify();
                })),
        );
    Some(at_pointer(at, pop(format!("ff-pop-{user}"), open, menu)))
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
    reply(rx, cx, move |r, got, cx| {
        if r.friends.found.as_ref().is_some_and(|(at, _)| *at == q) {
            r.friends.found = Some((q, Some(got)));
            cx.notify();
        }
    });
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
                    r.again(cx, |r| (r.friends.loading, &mut r.friends.list), load);
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

/// Passes the status filter and the label filter.
fn shown(f: &Friend, fr: &Data, lb: &Labels) -> bool {
    passes(f, fr.filter) && fr.label_filter.is_none_or(|id| lb.of.get(&f.id).is_some_and(|v| v.contains(&id)))
}

pub fn friends(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let fr = &root.friends;
    let inner = width - 2. * (PAGE_X);
    let (cols, card_w) = grid_fit(inner, CARD_W, GRID_GAP);
    // BLOXKEEP_SHOT_SCROLL=<px>: a shot of the page scrolled down that far (asked again until the page is long enough).
    #[cfg(feature = "shot")]
    if let Ok(y) = std::env::var("BLOXKEEP_SHOT_SCROLL").map(|v| v.parse::<f32>().unwrap_or(0.)) {
        let h = crate::scrolled::handle("fr-body");
        if (h.offset().y + px(y)).abs() > px(1.) {
            h.set_offset(point(px(0.), px(-y)));
        }
    }
    let (able, viewer) = looking_ref(root);
    let list: &[Friend] = match &fr.list {
        Some(Ok(l)) => l,
        _ => &[],
    };
    let lb = labels();

    let shell = div().id("friends").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let body: AnyElement = if able.is_empty() || matches!(fr.sign, Some(None)) {
        need_account(root, "fr-add-acc", "fr.noAccount", "users", cx)
    } else {
        match &fr.list {
            None => empty_note(root.tr("fr.loading")).into_any_element(),
            Some(Err(why)) => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(14.))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("fr.failed", &[("why", crate::net::why_words(&root.i18n, why).as_str())])))
                .child(Button::new("fr-retry", root.tr("fr.retry")).icon("refresh").on_click(cx.listener(|r, _, _, cx| r.again(cx, |r| (r.friends.loading, &mut r.friends.list), load))))
                .into_any_element(),
            Some(Ok(l)) => {
                let join_as = viewer.as_ref().map(|a| s(a, "id").to_string());
                let mut rows = div().flex().flex_col().gap(px(SECTION_GAP)).px(px(PAGE_X)).pt(px(4.)).pb(px(24.));
                if fr.adding {
                    rows = rows.child(add(root, l, inner, cx));
                }
                // A group a line, the page's width, its head over its cards on the page's fixed grid (critique
                // 2026-11-13, P2: groups flowing side by side ran Requests, a game and Online into one ragged row and
                // the reading order was lost).
                let mut blocks: Vec<AnyElement> = Vec::new();
                let block_with = |head: AnyElement, grid: Div| {
                    div()
                        .w_full()
                        .flex_none()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(div().w_full().flex().items_center().child(head))
                        .child(grid)
                        .into_any_element()
                };
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
                if !fr.requests.is_empty() && fr.filter == Filter::All && fr.label_filter.is_none() {
                    let cards: Vec<AnyElement> = fr.requests.iter().map(|f| card(root, f, card_w, Kind::Request, cx).into_any_element()).collect();
                    blocks.push(block(text_head(root.tr("fr.requests"), fr.requests.len(), None).into_any_element(), cards));
                }
                let mut any = false;
                // Friends in a game stand by game, a group each (as Accounts lays out its windows), the fullest first:
                // where everyone is shows at a glance, which Roblox's one row of faces doesn't.
                let playing: Vec<&Friend> = l.iter().filter(|f| f.kind == 2 && shown(f, fr, &lb)).collect();
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
                    let mut of: Vec<&Friend> = l.iter().filter(|f| shown(f, fr, &lb)).filter(|f| if g == 1 { f.kind == 1 || f.kind == 3 } else { f.kind == g }).collect();
                    // Offline: those seen online latest come first.
                    if g == 0 {
                        of.sort_by(|a, b| b.seen.cmp(&a.seen));
                    }
                    if of.is_empty() {
                        continue;
                    }
                    any = true;
                    let n = of.len();
                    // Hundreds of friends: only the rows near the view are made (crate::near), each card one height.
                    let id = if g == 1 { "fr-online" } else { "fr-offline" };
                    let grid = crate::near::grid_in(Some(&crate::scrolled::handle("fr-body")), n >= crate::near::FROM, id, n, cols, FRIEND_H, GRID_GAP, None, |i| {
                        card(root, of[i], card_w, Kind::Friend(join_as.clone(), false), cx).h(px(FRIEND_H)).into_any_element()
                    });
                    blocks.push(block_with(text_head(root.tr(key), n, None).into_any_element(), grid));
                }
                // What changed among the friends, the latest dozen: who is new, who is no longer a friend, who took
                // another name. Kept on this PC: Roblox tells none of it.
                if !fr.shifts.is_empty() && fr.filter == Filter::All && fr.label_filter.is_none() {
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
                    // (In the middle of the page, as every page's "nothing here": tucked at the top left it read as
                    // left over, the owner's word for Accounts' own, 2026-11-17.)
                    rows = rows.child(empty_note(root.tr(if l.is_empty() { "fr.none" } else { "fr.noneHere" })).w_full().py(px(96.)));
                }
                crate::scrolled::body("fr-body", rows).into_any_element()
            }
        }
    };
    shell.child(header(root, list, viewer, cx)).child(body)
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
            panel = panel.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("fr.failed", &[("why", crate::net::why_words(&root.i18n, why).as_str())])))
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
        .px(px(PAGE_X))
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
                .child(crate::pages::games::brand(root))
                .relative().child(crate::pages::games::island(root, cx))
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
                    d.child(IconButton::new("fr-refresh", "refresh").size(34.).busy(fr.loading).tip(root.tr("fr.refresh")).on_click(cx.listener(|r, _, _, cx| r.again(cx, |r| (r.friends.loading, &mut r.friends.list), load))))
                }),
        )
        // The filters as the Accounts page's: no track, a dot of where they are, the count; the one shown a yellow block.
        // Then the user's own labels, the same way, and the Labels button (its box under it).
        .when(!list.is_empty(), |d| {
            let lb = labels();
            let tag_chips: Vec<AnyElement> = lb
                .list
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let id = l.id;
                    let on = fr.label_filter == Some(id);
                    let n = list.iter().filter(|f| lb.of.get(&f.id).is_some_and(|v| v.contains(&id))).count();
                    chip(("fr-tag-chip", i), on)
                        .h(px(TOOL_H))
                        .child(div().size(px(8.)).flex_none().rounded_full().bg(c(l.colour)))
                        .child(l.name.clone())
                        .child(chip_count(n, on))
                        .on_click(cx.listener(move |r, _, _, cx| {
                            r.friends.label_filter = if r.friends.label_filter == Some(id) { None } else { Some(id) };
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect();
            let has_tags = !tag_chips.is_empty();
            let tags_button = div()
                .relative()
                .child(
                    chip("fr-tags", fr.tags_open)
                        .h(px(TOOL_H))
                        .child(icon("tag", 13., c(if fr.tags_open { INK } else { MUTED })))
                        .child(root.tr(if has_tags { "tag.labels" } else { "tag.addFirst" }))
                        .on_click(cx.listener(|r, _, window, cx| {
                            r.friends.tags_open = !r.friends.tags_open;
                            r.friends.tag_for = None;
                            if r.friends.tags_open {
                                let h = r.label_box.read(cx).focus_handle(cx);
                                window.focus(&h, cx);
                            }
                            cx.notify();
                        })),
                )
                .when(fr.tags_open, |d| d.child(float(PopAt::BelowLeft(TOOL_H + 6.), 2, tags_box(root, cx))));
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
                    .into_any_element()
            })
            .chain(has_tags.then(|| div().w(px(1.)).h(px(18.)).mx(px(6.)).bg(c(LINE)).into_any_element()))
            .chain(tag_chips)
            .chain(std::iter::once(div().ml(px(4.)).child(tags_button).into_any_element()))))
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
        _ if matches!(kind, Kind::Request) => format!("@{}", f.name),
        2 if in_lane => format!("@{}", f.name),
        2 if !f.game.is_empty() && f.since > 0 => format!("{} · {}", f.game, crate::time::span(crate::time::now_secs() - f.since)),
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
        .min_h(px(FRIEND_H))
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
                .child({
                    // (Labels as their colour dots after the name, their names on hover: a pill took the name's room on
                    // a card this narrow; the chips over the list say which colour is which.)
                    let tags = if is_friend { labels_of(user) } else { Vec::new() };
                    let names: SharedString = tags.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(", ").into();
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .min_w_0()
                        .child(div().min_w_0().flex_shrink(1.).text_size(px(T_L)).font_weight(FontWeight::MEDIUM).truncate().child(f.display.clone()))
                        .when(!tags.is_empty(), |d| {
                            d.child(
                                div()
                                    .id(("fr-dots", user as u64 as usize))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .gap(px(3.))
                                    .children(tags.iter().take(4).map(|l| div().size(px(8.)).rounded_full().bg(c(l.colour))))
                                    .tooltip(move |_, cx| cx.new(|_| Tip(names.clone())).into()),
                            )
                        })
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(T_S))
                        .text_color(c(MUTED))
                        .when_some(mark, |d, col| d.child(div().size(px(7.)).flex_none().rounded_full().bg(c(col))))
                        .when_some(game_pic, |d, p| d.child(img(p).size(px(14.)).flex_none().rounded(px(3.))))
                        .child(div().min_w_0().truncate().child(line))
                        // A request: how long ago it was sent, after the @name and never cut.
                        .when(matches!(kind, Kind::Request) && f.sent > 0, |d| {
                            d.child(div().flex_none().child(format!("· {}", crate::time::ago_far(crate::time::now_secs() - f.sent, &root.i18n))))
                        }),
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
    // A friend's card: Chat (their chat in the Inbox), and a menu with the profile and Unfriend (a second click does it).
    let with = with.child(IconButton::new(("fr-chat", key), "message").size(30.).tip(root.tr("fr.chat")).on_click(cx.listener(move |r, _, _, cx| crate::pages::inbox::chat_with(r, user, cx))));
    let mut more = div().relative().flex_none().child(IconButton::new(("fr-more", key), "more").size(30.).on_click(cx.listener(move |r, _, _, cx| {
        r.friends.card_menu = if r.friends.card_menu == Some(user) { None } else { Some(user) };
        r.friends.card_menu_was = Some(user);
        r.friends.sure = None;
        r.friends.menu_tags = false;
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
                r.friends.menu_tags = false;
                cx.notify();
            }))
            .when(my_server(root).is_some(), |d| {
                d.child(row("fr-m-invite").text_color(c(TEXT_2)).child(icon("link", 14., c(MUTED))).child(root.tr("fr.invite")).on_click(cx.listener(move |r, _, window, cx| {
                    r.friends.card_menu = None;
                    invite(r, user, window, cx);
                    cx.notify();
                })))
            })
            .child(row("fr-m-profile").text_color(c(TEXT_2)).child(icon("user", 14., c(MUTED))).child(root.tr("ui.profile")).on_click(cx.listener(move |r, _, _, cx| {
                r.friends.card_menu = None;
                crate::pages::profile::open_person(r, user, cx);
            })))
            .children(tag_rows(root, user, false, |r| {
                r.friends.card_menu = None;
                r.friends.menu_tags = false;
            }, cx))
            .child(div().h(px(1.)).mx(px(6.)).my(px(4.)).bg(c(LINE)))
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
        more = more.child(popup(PopAt::BelowRight(34.), format!("fr-card-pop-{user}"), root.friends.card_menu == Some(user), menu));
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
