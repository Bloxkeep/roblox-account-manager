// Accounts, with the Roblox windows (the old Game monitor) where the accounts are: what needs a look on top, then a
// lane for each game played here with a tile a window (its live picture laid there by the app; Roblox windows not
// opened by Bloxkeep too), then the accounts not open. Any tile or card is a tick box;
// the bar at the foot plays, rejoins or closes those picked. The commands are the app's (launch, closeAccounts,
// hideWindows, showWindows, monitorAct, cancelRejoin, resolveGame, openProfile, openAppeal), as the web page sends them.
use crate::threads::Hold;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{images, kit::*, shell::Root, theme::*};

// The list layout, the account menu (its state and the page's other state Root doesn't hold), and the start for new
// users (no accounts yet, or none opened yet).
pub(crate) mod groups;
mod list;
mod load;
pub(crate) mod menu;
mod start;

/// The filters over the list, as the web page's chips.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Playing,
    Online,
    Offline,
    /// Signed out: they need signing in again.
    Invalid,
    Banned,
    /// Open on this PC (the web page's "On this PC").
    Local,
}

fn conn(a: &Value) -> &str {
    a.pointer("/session/connection").and_then(Value::as_str).unwrap_or_default()
}
thread_local! {
    /// Where each account's card, tile or row was last drawn (window pixels), for `reveal` to scroll to, with the
    /// Accounts frame it was drawn in: only the latest frame's count (a card a filter or the windows view took away
    /// kept its old place, and the arrow keys landed on it).
    static SPOTS: std::cell::RefCell<std::collections::HashMap<String, (u64, Bounds<Pixels>)>> = Default::default();
    /// Counts the Accounts page's frames (one per `accounts` call).
    static FRAME: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The account being revealed (or that the arrow keys are on), as its place among `shown`: a long list draws it
/// wherever it is, so it can be scrolled to.
pub fn revealed_in(root: &Root, shown: &[&Value]) -> Option<usize> {
    let id = root.revealed.as_ref().map(|(id, _)| id.as_str()).or(root.acc_cursor.as_deref())?;
    shown.iter().position(|a| s(a, "id") == id)
}

/// Where this account was drawn in the latest Accounts frame.
fn spot_now(id: &str) -> Option<Bounds<Pixels>> {
    let now = FRAME.with(std::cell::Cell::get);
    SPOTS.with(|s| s.borrow().get(id).filter(|(f, _)| *f == now).map(|(_, b)| *b))
}

/// How long a revealed account's ring stays.
const FLASH: std::time::Duration = std::time::Duration::from_millis(1600);

/// Laid over an account's card, tile or row: notes where it is drawn, and rings it while it is being revealed (yellow)
/// or while the arrow keys are on it (a thin light ring).
pub fn spot(root: &Root, id: &str, radius: f32) -> impl IntoElement {
    let key = id.to_string();
    let lit = root.revealed.as_ref().is_some_and(|(r, at)| r == id && at.elapsed() < FLASH);
    let cursor = root.acc_cursor.as_deref() == Some(id);
    div()
        .absolute()
        .inset_0()
        .rounded(px(radius))
        .when(cursor && !lit, |d| d.border_2().border_color(ca(WHITE, 0.55)))
        .when(lit, |d| d.border_2().border_color(c(FACE)))
        .child(
            canvas(
                move |b, _, _| {
                    let now = FRAME.with(std::cell::Cell::get);
                    SPOTS.with(|s| s.borrow_mut().insert(key, (now, b)))
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
}

/// The windows view on or off, kept for the next run.
pub fn set_win_view(r: &mut Root, on: bool) {
    r.acc_win = on;
    crate::prefs::set("accWindows", Value::Bool(on));
}

/// A key among the cards.
#[derive(Clone, Copy, PartialEq)]
pub enum CardKey {
    Left,
    Right,
    Up,
    Down,
    First,
    Last,
    /// Space: the card the keys are on picked or let go.
    Pick,
}

/// Scrolls the Accounts body so a noted card is in view (in its middle when it was out of it).
fn scroll_to_spot(spot: Bounds<Pixels>) {
    let h = crate::scrolled::handle("acc-body");
    let (view, off) = (h.bounds(), h.offset());
    if spot.top() < view.top() || spot.bottom() > view.bottom() {
        let y = spot.top() - view.top() - off.y;
        let want = -(y - (view.size.height - spot.size.height).max(px(0.)) / 2.);
        let max = h.max_offset().y;
        h.set_offset(point(off.x, want.min(px(0.)).max(-max)));
    }
}

/// The arrows move among the cards as they are laid out on screen (the nearest one that way), Home and End to the
/// first and the last, Space picks; the first key lands on the first card.
pub fn card_key(r: &mut Root, k: CardKey, cx: &mut Context<Root>) -> bool {
    let shown: Vec<String> = shown_accounts(r, cx).iter().map(|a| s(a, "id").to_string()).collect();
    let mut cards: Vec<(String, Bounds<Pixels>)> = shown.iter().filter_map(|id| spot_now(id).map(|b| (id.clone(), b))).collect();
    if cards.is_empty() {
        return false;
    }
    // Reading order: by line, then left to right.
    cards.sort_by(|a, b| (f32::from(a.1.top()).round() as i64, f32::from(a.1.left()) as i64).cmp(&(f32::from(b.1.top()).round() as i64, f32::from(b.1.left()) as i64)));
    let at = r.acc_cursor.as_ref().and_then(|id| cards.iter().position(|(c, _)| c == id));
    if k == CardKey::Pick {
        let Some(i) = at else { return false };
        let id = cards[i].0.clone();
        toggle(r, &id, false, cx);
        r.acc_cursor = Some(id);
        cx.notify();
        return true;
    }
    // Home and End in a long list: the first or last account may not be drawn yet (crate::near). The keys' account
    // is always drawn (revealed_in), so it is put there and scrolled to once it has been.
    let end = match k {
        CardKey::First => shown.first(),
        CardKey::Last => shown.last(),
        _ => None,
    };
    if let Some(id) = end.filter(|id| !cards.iter().any(|(c, _)| c == *id)).cloned() {
        r.acc_cursor = Some(id.clone());
        cx.spawn(async move |_, cx| {
            for _ in 0..20 {
                cx.background_executor().timer(std::time::Duration::from_millis(40)).await;
                if let Some(b) = spot_now(&id) {
                    scroll_to_spot(b);
                    let _ = cx.update(|cx| cx.refresh_windows());
                    break;
                }
            }
        })
        .detach();
        cx.notify();
        return true;
    }
    let to = match (at, k) {
        (None, _) | (_, CardKey::First) => 0,
        (_, CardKey::Last) => cards.len() - 1,
        (Some(i), CardKey::Left) => i.saturating_sub(1),
        (Some(i), CardKey::Right) => (i + 1).min(cards.len() - 1),
        (Some(i), _) => {
            let here = cards[i].1;
            let (cx0, cy0) = (f32::from(here.left() + here.right()) / 2., f32::from(here.top() + here.bottom()) / 2.);
            let down = k == CardKey::Down;
            cards
                .iter()
                .enumerate()
                .filter(|(_, (_, b))| {
                    let cy = f32::from(b.top() + b.bottom()) / 2.;
                    if down { cy > cy0 + 4. } else { cy < cy0 - 4. }
                })
                .min_by(|(_, (_, a)), (_, (_, b))| {
                    let d = |b: &Bounds<Pixels>| ((f32::from(b.top() + b.bottom()) / 2. - cy0).abs(), (f32::from(b.left() + b.right()) / 2. - cx0).abs());
                    let (ay, ax) = d(a);
                    let (by, bx) = d(b);
                    // The nearest line first, then the nearest across it.
                    (ay.round() as i64, ax as i64).cmp(&(by.round() as i64, bx as i64))
                })
                .map(|(j, _)| j)
                .unwrap_or(i)
        }
    };
    let (id, b) = cards[to].clone();
    r.acc_cursor = Some(id);
    scroll_to_spot(b);
    cx.notify();
    true
}

/// Brings an account into view on Accounts and rings it a moment, the selection left as it was; a filter, a group or a
/// search that hides it is put aside first (app.js's revealAccount).
pub fn reveal(r: &mut Root, id: &str, window: &mut Window, cx: &mut Context<Root>) {
    r.go(crate::pages::Page::Accounts);
    if !shown_accounts(r, cx).iter().any(|a| s(a, "id") == id) {
        r.acc_filter = Filter::All;
        r.acc_group = None;
        r.acc_search.update(cx, |st, cx| st.set_value("", window, cx));
    }
    SPOTS.with(|s| s.borrow_mut().clear());
    let at = std::time::Instant::now();
    r.revealed = Some((id.to_string(), at));
    let id = id.to_string();
    cx.spawn(async move |this, cx| {
        // Once the page has been drawn and its cards noted (a few frames at most). Not there in the windows view (an
        // account with no window has no tile in it): the cards come back and it is looked for there.
        for i in 0..20 {
            cx.background_executor().timer(std::time::Duration::from_millis(40)).await;
            let Some(spot) = spot_now(&id) else {
                if i == 9 {
                    let _ = this.update(cx, |r, cx| {
                        if r.acc_win {
                            set_win_view(r, false);
                            cx.notify();
                        }
                    });
                }
                continue;
            };
            let h = crate::scrolled::handle("acc-body");
            let (view, off) = (h.bounds(), h.offset());
            if spot.top() < view.top() || spot.bottom() > view.bottom() {
                // Its place in the scrolled body, then the offset that stands it in the middle of the view.
                let y = spot.top() - view.top() - off.y;
                let want = -(y - (view.size.height - spot.size.height).max(px(0.)) / 2.);
                let max = h.max_offset().y;
                h.set_offset(point(off.x, want.min(px(0.)).max(-max)));
            }
            let _ = this.update(cx, |_, cx| cx.notify());
            break;
        }
        cx.background_executor().timer(FLASH).await;
        let _ = this.update(cx, |r, cx| {
            if r.revealed.as_ref().is_some_and(|(_, t)| *t == at) {
                r.revealed = None;
            }
            cx.notify();
        });
    })
    .detach();
    cx.notify();
}

/// How long a window just closed here says "Closing…" at most (while Roblox's status still says online).
pub const CLOSING: std::time::Duration = std::time::Duration::from_secs(60);

/// Its window here closed under a minute ago and Roblox still says it is online (Accounts.tsx's closing): the card
/// says "Closing…" and the game it was in, not "Playing elsewhere".
pub fn closing<'a>(root: &'a Root, a: &Value) -> Option<&'a str> {
    if is_local(a) || launching(a) || a.get("rejoin").is_some_and(|v| !v.is_null()) || !is_online(a) {
        return None;
    }
    root.closed_at.get(s(a, "id")).filter(|(at, _)| at.elapsed() < CLOSING).map(|(_, game)| game.as_str())
}

pub fn is_local(a: &Value) -> bool {
    s(a, "state") == "running"
}
/// Its window open but not in a game: back on Roblox's menu after leaving one, or signed out in it. Its session still
/// names the last game it played, which is not where it is (owner 2026-11-13: Home said "Playing Thai Anomalies" for
/// hours after they left it).
pub fn in_menu(a: &Value) -> bool {
    is_local(a) && matches!(conn(a), "menu" | "signedout")
}
fn launching(a: &Value) -> bool {
    s(a, "state") == "launching"
}
pub fn is_kicked(a: &Value) -> bool {
    is_local(a) && (conn(a) == "disconnected" || a.pointer("/session/bannedByGame").and_then(Value::as_bool).unwrap_or(false))
}
pub(crate) fn presence(a: &Value) -> &str {
    a.pointer("/presence/type").and_then(Value::as_str).unwrap_or_default()
}
pub(crate) fn in_game(a: &Value) -> bool {
    (is_local(a) && !matches!(conn(a), "disconnected" | "menu" | "signedout")) || presence(a) == "ingame"
}
pub(crate) fn is_online(a: &Value) -> bool {
    is_local(a) || matches!(presence(a), "online" | "ingame" | "studio")
}
fn signed_out(a: &Value) -> bool {
    a.get("cookieValid").and_then(Value::as_bool) == Some(false)
}
fn banned(a: &Value) -> bool {
    a.get("banned").and_then(Value::as_bool).unwrap_or(false)
}
pub fn label(a: &Value) -> String {
    ["label", "alias", "displayName", "username"].iter().map(|k| s(a, k)).find(|n| !n.is_empty()).unwrap_or_default().to_string()
}
// No numbered tags (owner 2026-11-14: "bỏ số đi"): the face's dot and the state line say how an account is.

/// An account card's and an open window's tile corner (owner 2026-11-14: rounder cards).
const R_CARD: f32 = 18.;

/// Free to join a game: not open, signed in, not banned (as the Accounts bar's Play).
pub fn can_play(a: &Value) -> bool {
    !is_local(a) && can_join_game(a)
}

/// The game page can also switch a Roblox client that is already open to its chosen game.
pub fn can_join_game(a: &Value) -> bool {
    !launching(a) && !signed_out(a) && !banned(a)
}
pub fn id_of(a: &Value) -> String {
    s(a, "id").to_string()
}

/// What it is really called while names are hidden (shell.rs keeps it under "real"), else the field.
fn real<'a>(a: &'a Value, k: &str) -> &'a str {
    a.pointer(&format!("/real/{k}")).and_then(Value::as_str).unwrap_or_else(|| s(a, k))
}

/// The search as app.js's visible(): without accents ("hoang" finds "Hoàng"), "@name" for the username, a user ID
/// whole (or its start from 6 digits on), real names while they are hidden, and the game it plays.
pub fn found(a: &Value, query: &str) -> bool {
    let q = folded(query.trim().trim_start_matches('@'));
    if q.is_empty() {
        return true;
    }
    let id = a.get("userId").map(|v| match v {
        Value::String(t) => t.clone(),
        other => other.to_string(),
    });
    if q.chars().all(|c| c.is_ascii_digit()) && id.as_deref().is_some_and(|id| id == q || (q.len() >= 6 && id.starts_with(&q))) {
        return true;
    }
    [real(a, "alias"), real(a, "username"), real(a, "displayName"), s(a, "label"), s(a, "group"), s(a, "note")]
        .into_iter()
        .chain([a.pointer("/presence/game").and_then(Value::as_str).unwrap_or_default(), a.pointer("/session/game/name").and_then(Value::as_str).unwrap_or_default()])
        .any(|v| folded(v).contains(&q))
}

fn passes(a: &Value, f: Filter, query: &str) -> bool {
    found(a, query) && match f {
        Filter::All => true,
        Filter::Playing => in_game(a),
        Filter::Online => is_online(a),
        Filter::Offline => !is_online(a),
        Filter::Invalid => signed_out(a),
        Filter::Banned => banned(a),
        Filter::Local => is_local(a),
    }
}

/// Text already folded (no accents, no case), compared as the web page's collator did (numeric: "Farm 2" before
/// "Farm 10").
struct Natural(String);
impl PartialEq for Natural {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}
impl Eq for Natural {}
impl Ord for Natural {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        natural(&self.0, &other.0)
    }
}
impl PartialOrd for Natural {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn natural(x: &str, y: &str) -> std::cmp::Ordering {
    let (mut a, mut b) = (x.chars().peekable(), y.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(c) = a.peek().copied().filter(char::is_ascii_digit) {
                    n.push(c);
                    a.next();
                }
                let mut m = String::new();
                while let Some(d) = b.peek().copied().filter(char::is_ascii_digit) {
                    m.push(d);
                    b.next();
                }
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let o = n.len().cmp(&m.len()).then_with(|| n.cmp(m));
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                if c != d {
                    return c.cmp(&d);
                }
                a.next();
                b.next();
            }
        }
    }
}

/// The order picked (the app's "sort" setting, as the web page kept it): by name, by what they are doing, by when
/// they were last opened, or in the launch order; names break ties.
fn sort(root: &Root, list: &mut [Value]) {
    let how = root.state.pointer("/settings/sort").and_then(Value::as_str).unwrap_or("name").to_string();
    let order: Vec<String> = root.state.pointer("/settings/launchOrder").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let place = |a: &Value| order.iter().position(|i| i == s(a, "id")).unwrap_or(usize::MAX);
    // As app.js's statusRank: open here, opening, playing elsewhere, online or in Studio, offline, then the ones signed
    // out or banned last.
    let rank = |a: &Value| {
        if is_local(a) {
            0
        } else if launching(a) {
            1
        } else if presence(a) == "ingame" {
            2
        } else if matches!(presence(a), "online" | "studio") {
            3
        } else if signed_out(a) || banned(a) {
            5
        } else {
            4
        }
    };
    // Names by their real text even while hidden (hiding doesn't reshuffle the list), then the username.
    fn name_of(a: &Value) -> &str {
        ["label", "alias", "displayName", "username"].iter().map(|k| real(a, k)).find(|n| !n.is_empty()).unwrap_or_default()
    }
    // Each account's key made once (its names folded once), not on every compare.
    list.sort_by_cached_key(|a| {
        let (first, last) = match how.as_str() {
            "status" => (rank(a) as u64, std::cmp::Reverse(String::new())),
            "recent" => (0u64, std::cmp::Reverse(s(a, "lastUsed").to_string())),
            "order" => (place(a) as u64, std::cmp::Reverse(String::new())),
            _ => (0, std::cmp::Reverse(String::new())),
        };
        (first, last, Natural(folded(name_of(a))), Natural(folded(real(a, "username"))))
    });
}

/// What a list was made from: the state and windows as sent, the search, filter, group and order.
#[derive(Clone, PartialEq)]
struct ShownKey {
    state: u64,
    windows: u64,
    query: String,
    filter: Option<Filter>,
    group: Option<String>,
    sort: String,
    /// Ordered by group first (the list layout: a part a group).
    grouped: bool,
}

thread_local! {
    /// The cards shown and every account in order (the windows view), made again only when what they come from
    /// changes: with hundreds of accounts, cloning and sorting them every frame cost a lot.
    static SHOWN: std::cell::RefCell<Option<(ShownKey, std::rc::Rc<Vec<Value>>)>> = const { std::cell::RefCell::new(None) };
    static EVERY: std::cell::RefCell<Option<(ShownKey, std::rc::Rc<Vec<Value>>)>> = const { std::cell::RefCell::new(None) };
}

fn shown_key(root: &Root, query: String, filter: Option<Filter>, group: Option<String>) -> ShownKey {
    ShownKey {
        state: root.state_gen,
        windows: root.windows_gen,
        query,
        filter,
        group,
        sort: root.state.pointer("/settings/sort").and_then(Value::as_str).unwrap_or("name").to_string(),
        grouped: false,
    }
}

fn cached(
    slot: &'static std::thread::LocalKey<std::cell::RefCell<Option<(ShownKey, std::rc::Rc<Vec<Value>>)>>>,
    key: ShownKey,
    make: impl FnOnce() -> Vec<Value>,
) -> std::rc::Rc<Vec<Value>> {
    if let Some(list) = slot.with(|c| c.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, l)| l.clone())) {
        return list;
    }
    let list = std::rc::Rc::new(make());
    slot.with(|c| *c.borrow_mut() = Some((key, list.clone())));
    list
}

/// Every account sorted, other PCs' windows after: the windows view shows them all, whatever the filter says.
fn every_sorted(root: &Root) -> std::rc::Rc<Vec<Value>> {
    cached(&EVERY, shown_key(root, String::new(), None, None), || {
        let mut every = root.accounts().to_vec();
        sort(root, &mut every);
        every.extend(others(root));
        every
    })
}

/// Why an account dropped, from Roblox's disconnect code (roblox_codes.rs: every code Roblox has).
fn kick_words(root: &Root, code: i64) -> String {
    match crate::roblox_codes::why(code as i32) {
        // Roblox's own box says no more than "an error" for these, and a number it doesn't have says nothing: the code.
        Some(c) if c.key != "error" => root.tr(&format!("kick.{}", c.key)),
        _ => root.tr("kick.code").replace("{code}", &code.to_string()),
    }
}

/// The colour of an account's dot: dropped or opening orange, playing green, online blue, else grey.
fn dot_of(a: &Value) -> u32 {
    if is_kicked(a) || launching(a) {
        WARN
    } else if in_game(a) {
        OK
    } else if is_online(a) {
        ONLINE
    } else {
        DIM
    }
}

pub(crate) fn game_name(a: &Value) -> Option<String> {
    a.pointer("/session/game/name")
        .and_then(Value::as_str)
        .filter(|g| !g.is_empty())
        .or_else(|| (presence(a) == "ingame").then(|| a.pointer("/presence/game").and_then(Value::as_str)).flatten())
        .map(crate::pages::home::clean)
}
pub(crate) fn game_place(a: &Value) -> Option<i64> {
    a.pointer("/session/game/placeId").and_then(Value::as_i64).or_else(|| a.pointer("/presence/placeId").and_then(Value::as_i64)).filter(|p| *p > 0)
}
pub(crate) fn game_icon(a: &Value) -> Option<String> {
    a.pointer("/session/game/iconUrl")
        .and_then(Value::as_str)
        .or_else(|| a.pointer("/presence/iconUrl").and_then(Value::as_str))
        .filter(|u| !u.is_empty())
        .map(str::to_string)
}

fn phase(a: &Value) -> &str {
    a.pointer("/session/phase").and_then(Value::as_str).unwrap_or_default()
}
/// Held by Roblox's bot check on the way in: the window stays open on its Security page.
fn held(a: &Value) -> bool {
    is_local(a) && phase(a) == "challenged"
}
/// Auto-rejoin waiting its turn (longer after several drops in a row), or a preset's restart about to open it again.
fn rejoin_wait(a: &Value) -> Option<&Value> {
    a.get("rejoin").filter(|r| r.is_object() && !is_local(a))
}
/// Auto-rejoin for the account's client: a preset that opened it may turn it on or off for that client.
fn rejoin_on(a: &Value) -> bool {
    a.pointer("/session/autoRejoin").and_then(Value::as_bool).or_else(|| a.get("autoRejoin").and_then(Value::as_bool)).unwrap_or(false)
}
/// A window on an older Roblox than the newest.
fn old_build(a: &Value) -> bool {
    is_local(a) && a.pointer("/session/oldBuild").and_then(Value::as_bool).unwrap_or(false)
}
/// The FPS limit Bloxkeep set when this window opened.
fn fps_limit(a: &Value) -> Option<i64> {
    a.pointer("/session/fpsLimit").and_then(Value::as_i64).filter(|f| *f > 0 && is_local(a))
}
fn pid_of(a: &Value) -> Option<i64> {
    a.pointer("/session/pid").and_then(Value::as_i64).filter(|p| *p > 0)
}

/// A countdown in whole minutes, rounded up, then "in a moment" for the last one (a rejoin waiting its turn).
fn until(root: &Root, iso: &str, tpl: &str, soon: &str) -> String {
    let left = crate::time::iso_secs(iso).map(|t| t - crate::time::now_secs()).unwrap_or(0);
    if left < 60 { root.tr(soon) } else { root.i18n.tv(tpl, &[("n", &((left + 59) / 60).to_string())]) }
}

/// What Bloxkeep does next with a window here (the app's session.next), in a few words: "rejoining in 8 s", "waits
/// for the other device to leave", "asking Roblox".
fn next_words(root: &Root, a: &Value) -> Option<String> {
    let next = a.pointer("/session/next").filter(|n| n.is_object())?;
    Some(match s(next, "kind") {
        "rejoin" => {
            let left = crate::time::iso_secs(s(next, "at")).map(|t| t - crate::time::now_secs()).unwrap_or(0);
            if left > 0 { root.i18n.tv("next.rejoinIn", &[("t", &root.i18n.tv("next.sec", &[("n", &left.to_string())]))]) } else { root.tr("next.rejoinNow") }
        }
        "waitDevice" => root.tr("next.waitDevice"),
        "checking" => root.tr("next.checking"),
        _ => return None,
    })
}

/// Whether a card counts something down now (the page is then drawn again each second).
pub fn counting(a: &Value) -> bool {
    a.get("rejoin").is_some_and(Value::is_object) || a.pointer("/session/next").is_some_and(Value::is_object)
}

/// What it does now, in a few words, its colour, and whether the game's name goes after it. A window here says how its
/// join goes (the web page's Activity): teleporting, waiting for a server, a full server and its place in line, held
/// by Roblox's bot check, didn't get in, dropped and why.
pub fn state_of(root: &Root, a: &Value) -> (String, u32, bool) {
    let now = crate::time::now_secs();
    if launching(a) {
        return (root.tr("act.opening"), WARN, false);
    }
    if let Some(rj) = rejoin_wait(a) {
        // (Under a minute: the seconds, counted down; the page is drawn again each second while a card counts.)
        let left = crate::time::iso_secs(s(rj, "at")).map(|t| t - now).unwrap_or(0);
        let mut text = if rj.get("restart").and_then(Value::as_bool).unwrap_or(false) {
            root.tr("rj.rowRestart")
        } else if (1..60).contains(&left) {
            root.i18n.tv("rj.rowSec", &[("n", &left.to_string())])
        } else {
            until(root, s(rj, "at"), "rj.rowMin", "rj.rowSoon")
        };
        if let Some(g) = rj.pointer("/game/name").and_then(Value::as_str).filter(|g| !g.is_empty()) {
            text = format!("{text} · {}", crate::pages::home::clean(g));
        }
        return (text, WARN, false);
    }
    if is_local(a) {
        // The game's own words when it gave any ("Banned by the game: cheating").
        let said = |why: String| match a.pointer("/session/kickMessage").and_then(Value::as_str).filter(|m| !m.is_empty()) {
            Some(m) => format!("{why}: {m}"),
            None => why,
        };
        if a.pointer("/session/bannedByGame").and_then(Value::as_bool).unwrap_or(false) {
            return (said(root.tr("act.bannedByGame")), BAD, true);
        }
        if conn(a) == "disconnected" {
            let why = a.pointer("/session/reason").and_then(Value::as_i64).map(|code| kick_words(root, code)).unwrap_or_else(|| root.tr("row.kickedUnknown"));
            // …and what comes next: "Lost connection · rejoining in 8 s".
            return match next_words(root, a) {
                Some(next) => (format!("{} · {next}", said(why)), WARN, false),
                None => (said(why), WARN, true),
            };
        }
        return match (conn(a), phase(a)) {
            // Left before any server took it (a join error's Leave, or a wait given up).
            ("menu", "joinFailed") => (root.tr("act.joinFailed"), WARN, true),
            ("menu", _) => (root.tr("act.menuHere"), TEXT_2, false),
            ("signedout", _) => (root.tr("act.signedOutHere"), BAD, false),
            ("joining", "challenged") => (root.tr("act.botCheck"), WARN, true),
            ("joining", "full") => {
                let line = a.pointer("/session/queuePosition").and_then(Value::as_i64).filter(|n| *n > 0);
                let text = match line {
                    Some(n) => format!("{} · {}", root.tr("row.full"), root.i18n.tv("act.inLine", &[("n", &n.to_string())])),
                    None => root.tr("act.serverFull"),
                };
                (text, WARN, true)
            }
            (_, "teleporting") => match next_words(root, a) {
                // (Under way too long: Roblox is being asked whether the account is still in a game.)
                Some(next) => (format!("{} · {next}", root.tr("act.teleporting")), WARN, false),
                None => (root.tr("act.teleporting"), OK, true),
            },
            ("joining", "waiting") => (root.tr("act.waitServer"), OK, true),
            ("ingame", _) => (root.tr("act.playingHere"), OK, true),
            _ => (root.tr("act.joiningHere"), OK, true),
        };
    }
    if let Some(game) = closing(root, a) {
        let words = if game.is_empty() { root.tr("card.closing") } else { format!("{} · {}", root.tr("card.closing"), crate::pages::home::clean(game)) };
        (words, MUTED, false)
    } else if presence(a) == "ingame" {
        (root.tr("row.elsewhere"), TEXT_2, true)
    } else if presence(a) == "online" {
        (root.tr("act.online"), TEXT_2, false)
    } else if presence(a) == "studio" {
        (root.tr("act.studio"), TEXT_2, false)
    } else if banned(a) {
        (root.tr("card.banned"), MUTED, false)
    } else if signed_out(a) {
        (root.tr("card.expired"), BAD, false)
    } else if let Some(t) = a.pointer("/presence/lastOnline").and_then(Value::as_str).and_then(crate::time::iso_secs) {
        // Roblox's own "last online" when it says (Accounts.tsx's card), else when it was last opened here.
        (root.i18n.tv("act.lastOnline", &[("t", &crate::time::ago(now - t, &root.i18n))]), MUTED, false)
    } else {
        let last = a.get("lastUsed").and_then(Value::as_str).and_then(crate::time::iso_secs);
        (
            match last {
                Some(t) => root.i18n.tv("card.opened", &[("t", &crate::time::ago(now - t, &root.i18n))]),
                None => root.tr("card.never"),
            },
            MUTED,
            false,
        )
    }
}

/// What it does now with the game after it, as one line (a card's, a list row's); a window here whose game Roblox
/// hasn't named yet says it is loading it.
fn line_of(root: &Root, a: &Value) -> (String, u32) {
    let (state, tone, with_game) = state_of(root, a);
    // Where it is when no game says it (Accounts.tsx's Activity, its second line): a window still opening, the menu,
    // signed out in a window here, online on the site or app, a game hidden by privacy, Studio, a status not known yet.
    let detail = |k: &str| format!("{state} · {}", root.tr(k));
    let text = match (with_game, game_name(a)) {
        (true, Some(g)) => format!("{state} · {g}"),
        // Dropped, banned by the game, held by the bot check, a join that failed: the window is still there.
        (true, None) if is_local(a) && (matches!(conn(a), "disconnected") || matches!(phase(a), "challenged" | "joinFailed") || a.pointer("/session/bannedByGame").and_then(Value::as_bool).unwrap_or(false)) => detail("act.stillOpen"),
        (true, None) if is_local(a) => detail("act.gameLoading"),
        (true, None) if presence(a) == "ingame" => detail("act.hidden"),
        _ if launching(a) => match game_name(a) {
            Some(g) => format!("{state} · {g}"),
            None => detail("act.waitWindow"),
        },
        _ if rejoin_wait(a).is_some() || closing(root, a).is_some() => state,
        _ if is_local(a) && conn(a) == "menu" => detail("act.menuWhere"),
        _ if is_local(a) && conn(a) == "signedout" => detail("act.signedOutWhere"),
        _ if is_local(a) => state,
        _ if presence(a) == "online" => detail("act.onlineWhere"),
        _ if presence(a) == "studio" => match a.pointer("/presence/game").and_then(Value::as_str).filter(|g| !g.is_empty()) {
            Some(g) => format!("{state} · {}", crate::pages::home::clean(g)),
            None => detail("act.studioWhat"),
        },
        // Roblox hasn't said yet (the first look after opening the app): not "never opened".
        _ if a.get("presence").is_none_or(Value::is_null) && !banned(a) && !signed_out(a) => format!("{} · {}", root.tr("act.unknown"), root.tr("act.updating")),
        _ => state,
    };
    (text, tone)
}

/// The Robux on an account ("…" while the first count is on its way).
fn robux_of(a: &Value) -> Option<String> {
    match a.get("robux").and_then(Value::as_i64) {
        Some(n) => Some(crate::kit::thousands(n)),
        None => a.get("robuxWait").and_then(Value::as_bool).filter(|w| *w).map(|_| "…".to_string()),
    }
}
fn robux_el(root: &Root, id: &str, text: String) -> Stateful<Div> {
    let tip: SharedString = root.tr("card.robuxTip").into();
    div()
        .id(SharedString::from(format!("robux-{id}")))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .text_size(px(T_S))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(TEXT_2))
        .font_features(tnum())
        .child(icon("robux", 13., c(MUTED)))
        .child(text)
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
}

/// A small word on a card or row (Main, VNG, 60 FPS, a group, Old build), in a colour.
fn tag(text: impl Into<SharedString>, col: u32) -> Div {
    div()
        .h(px(20.))
        .px(px(7.))
        .flex()
        .flex_none()
        .items_center()
        .rounded(px(6.))
        .bg(if col == TEXT_2 { ca(WHITE, 0.08) } else { ca(col, 0.14) })
        .text_color(c(col))
        .text_size(px(T_XS))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .child(text.into())
}

/// "Old build" on a window on an older Roblox: a click goes to where Roblox is updated (Settings › Roblox).
fn old_tag(root: &Root, a: &Value, cx: &mut Context<Root>) -> Stateful<Div> {
    let tip: SharedString = root.i18n.tv("card.oldTip", &[("v", a.pointer("/session/robloxVersion").and_then(Value::as_str).unwrap_or_default())]).into();
    div()
        .id(SharedString::from(format!("old-{}", s(a, "id"))))
        .flex_none()
        .cursor_pointer()
        .child(tag(root.tr("card.old"), WARN))
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(|r, _, _, cx| {
            r.settings.part = crate::pages::settings::Part::Roblox;
            r.go(crate::pages::Page::Settings);
            cx.notify();
        }))
}

/// The process id, for tools that pick a Roblox window by it; a click copies it.
fn pid_el(root: &Root, a: &Value, cx: &mut Context<Root>) -> Option<Stateful<Div>> {
    let pid = pid_of(a)?;
    let id = id_of(a);
    let tip: SharedString = root.tr("card.pidTip").into();
    let copy_id = id.clone();
    Some(
        div()
            .id(SharedString::from(format!("pid-{id}")))
            .flex_none()
            .text_color(c(MUTED))
            .font_features(tnum())
            .cursor_pointer()
            .hover(|d| d.text_color(c(TEXT)))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .map(|d| {
                if foreign(a) {
                    // A window Bloxkeep didn't open: copied by its window (MonitorView.tsx's Copy PID).
                    let hwnd = s(a, "hwnd").to_string();
                    d.on_click(cx.listener(move |r, _, _, cx| {
                        r.send("monitorAct", json!({ "id": hwnd, "what": "copyPid" }));
                        r.toast = Some((r.tr("t.copiedPid"), "ok".into()));
                        cx.notify();
                    }))
                } else {
                    d.on_click(cx.listener(move |r, _, _, cx| copy(r, copy_id.clone(), "pid", cx)))
                }
            })
            // Bloxkeep hid its window (Accounts.tsx's "Hidden" beside the PID).
            .child(if hidden(a) { format!("{} · PID {pid}", root.tr("card.hidden")) } else { format!("PID {pid}") }),
    )
}

/// An account's main button (the web page's card and row button): Verify while Roblox holds it on a bot check,
/// Rejoin when dropped, Close while open, Cancel rejoin while one waits, Appeal when banned, Sign in when signed out,
/// else Play.
fn action(root: &Root, a: &Value, at: &str, cx: &mut Context<Root>) -> Option<Button> {
    if foreign(a) {
        return None;
    }
    let id = id_of(a);
    let key = SharedString::from(format!("{at}-act-{id}"));
    let b = if held(a) {
        Button::new(key, root.tr("card.botCheck")).icon("shield").tint().on_click(cx.listener(move |r, _, _, _| r.send("solveBotCheck", json!({ "id": id }))))
    } else if is_kicked(a) {
        let me = vec![a.clone()];
        Button::new(key, root.tr("card.rejoinNow")).icon("repeat").tint().tip(root.tr("card.rejoinNowTip")).on_click(cx.listener(move |r, _, _, cx| r.rejoin(&me, cx)))
    } else if is_local(a) || launching(a) {
        Button::new(key, root.tr("card.close")).icon("power").on_click(cx.listener(move |r, _, _, cx| close(r, vec![id.clone()], false, cx)))
    } else if rejoin_wait(a).is_some() {
        Button::new(key, root.tr("rj.cancel")).icon("x").tip(root.tr("rj.cancelTip")).on_click(cx.listener(move |r, _, _, _| r.send("cancelRejoin", json!({ "id": id }))))
    } else if banned(a) {
        // Play skips it, so its button is Roblox's appeal page, signed in as it.
        Button::new(key, root.tr("card.appeal")).icon("external").tip(root.tr("card.appealTip")).on_click(cx.listener(move |r, _, _, _| r.send("openAppeal", json!({ "id": id }))))
    } else if signed_out(a) {
        let vng = s(a, "realm") == "vng";
        let name = label(a);
        Button::new(key, root.tr("card.relogin")).icon("logIn").tint().tip(root.tr("card.reloginTip")).on_click(cx.listener(move |r, _, window, cx| crate::pages::quick::open_for(r, vng, name.clone(), window, cx)))
    } else {
        Button::new(key, root.tr("card.play")).icon("play").on_click(cx.listener(move |r, _, _, cx| r.play(std::slice::from_ref(&id), cx)))
    };
    Some(b.small())
}

use crate::kit::no_pick;
use crate::val::s;
use crate::ask::answer;

/// An account's ⋮: its menu, under it.
fn more_button(root: &Root, a: &Value, at: &str, size: f32, cx: &mut Context<Root>) -> Div {
    let id = id_of(a);
    let open = menu::open_here(&id, true, cx);
    let key = SharedString::from(format!("{at}-more-{id}"));
    no_pick(IconButton::new(key, "more").size(size).on(open).tip(root.tr("card.moreTip")).on_click(cx.listener(move |_, _, _, cx| {
        menu::open_button(id.clone(), cx);
        cx.notify();
    })))
    .relative()
    .children(menu::place(root, a, true, cx))
}

/// A right click on a card, tile or row: its menu, where the mouse is.
fn on_right(a: &Value, cx: &mut Context<Root>) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
    let id = id_of(a);
    cx.listener(move |_, e: &MouseDownEvent, _, cx| {
        menu::open_at(id.clone(), e.position, cx);
        cx.notify();
    })
}

fn up_of(a: &Value) -> Option<String> {
    (is_local(a) && !is_kicked(a))
        .then(|| a.pointer("/session/since").and_then(Value::as_str).and_then(crate::time::iso_secs).map(|t| crate::time::clock(crate::time::now_secs() - t)))
        .flatten()
}
/// The ring round an account's face (the web page's .avatar background): green playing, blue online, red signed out
/// or banned, amber cut off or opening, else a quiet line.
fn ring_of(a: &Value) -> Hsla {
    if is_kicked(a) || launching(a) {
        c(WARN)
    } else if in_game(a) {
        c(OK)
    } else if is_online(a) {
        c(ONLINE)
    } else if signed_out(a) {
        ca(BAD, 0.6)
    } else if banned(a) {
        ca(WHITE, 0.2)
    } else {
        ca(WHITE, 0.12)
    }
}

/// An account's face as the web page drew it (.avatar): the picture inside a coloured ring of its state, a thin edge
/// of the surface between them, and the state's dot at the corner; its first letter on a colour of its own while it
/// has no picture. `ring` is the surface it sits on.
pub fn avatar(a: &Value, size: f32, ring: u32, cx: &mut App) -> Div {
    let d = (size * 0.26).clamp(9., 14.);
    face(a, size, c(ring), cx).child(div().absolute().right(px(-2.)).bottom(px(-2.)).size(px(d)).rounded(px(d * 0.35)).border_2().border_color(c(ring)).bg(c(dot_of(a))))
}

/// An account's face in a row of faces, each over the one before: no state mark (caught under the next face's edge it
/// read as a grey chip between them; the owner, 2026-11-18: "cấn cấn"), and its edge solid, so the face before doesn't
/// show through it where the surface `ring` is see-through (a picture theme).
pub fn stacked(a: &Value, size: f32, ring: u32, cx: &mut App) -> Div {
    face(a, size, crate::theme::over(c(ring), Hsla { a: 1., ..c(GROUND) }), cx)
}

/// The face itself: the picture (or the first letter) inside the ring of its state, an edge of `ring` between them.
fn face(a: &Value, size: f32, ring: Hsla, cx: &mut App) -> Div {
    let pad = if size >= 34. { 2. } else { 1.5 };
    let edge = if size >= 34. { 2. } else { 1.5 };
    let inner = size - 2. * pad - 2. * edge;
    let url = s(a, "avatarUrl");
    let face = images::pic_sq(url, inner, cx);
    let r = (size * 0.3).round();
    let letter = (url.is_empty()).then(|| {
        let name = label(a);
        let first: String = name.trim().chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
        // A hue of its own (FNV-1a over the id), as the web page's --h.
        let h = s(a, "id").bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
        (first, hsla((h % 360) as f32 / 360., 0.38, 0.40, 1.))
    });
    div()
        .relative()
        .size(px(size))
        .flex_none()
        .rounded(px(r))
        .p(px(pad))
        .bg(ring_of(a))
        .child(
            div()
                .size_full()
                .rounded(px((r - pad).max(2.)))
                .border(px(edge))
                .border_color(ring)
                .bg(c(RAISED_2))
                .flex()
                .items_center()
                .justify_center()
                .map(|x| match (face, letter) {
                    (Some(f), _) => x.child(img(f).size_full().object_fit(ObjectFit::Cover).rounded(px((r - pad - edge).max(2.)))),
                    (None, Some((first, bg))) => {
                        x.bg(bg).text_color(c(WHITE)).text_size(px((inner * 0.45).round())).font_weight(FontWeight::BOLD).child(first)
                    }
                    (None, None) => x,
                }),
        )
}

/// One grid for the whole page: window tiles and cards share its columns (each at least this wide, the row ending
/// where the page does), so every section lines up: four across at 1707 px, three at 1180.
const COL_MIN: f32 = 330.;
/// Monitor tiles can become compact, with room for the account details and actions below the preview.
const MON_COL_MIN: f32 = 320.;
/// With many windows open (a farm, past MON_MANY), tiles as narrow as this and pared down (`tile`'s `compact`): six
/// in a row at a common window's width instead of four (the owner, 2026-11-17: 30 windows in eight rows).
const MON_COL_COMPACT: f32 = 228.;
const MON_MANY: usize = 12;
const MON_COL_MAX: f32 = 560.;
const MON_PIC_INSET: f32 = 14.;
/// The page's sides (the header's and the body's), on the 8 px rhythm.
const SIDE_PAD: f32 = PAGE_X;
/// How far a block's base shows under it.
const RISE: f32 = 4.;

/// A Roblox window not opened by Bloxkeep: shown with the accounts' windows, as the old Game monitor did.
fn foreign(a: &Value) -> bool {
    a.get("foreign").and_then(Value::as_bool).unwrap_or(false)
}

/// The window the app last sent for this account (by its process), or the foreign one itself.
fn window_of<'a>(root: &'a Root, a: &Value) -> Option<&'a Value> {
    if foreign(a) {
        let h = s(a, "hwnd");
        return root.windows.iter().find(|w| s(w, "id") == h);
    }
    let pid = a.pointer("/session/pid").and_then(Value::as_i64).filter(|p| *p > 0)?;
    root.windows.iter().find(|w| w.get("pid").and_then(Value::as_i64) == Some(pid))
}

/// The windows Bloxkeep didn't open, as accounts of their own (named as the app names them, "Roblox"), so they sit in
/// their game's lane; "No game" is the Roblox menu.
fn others(root: &Root) -> Vec<Value> {
    let no_game = root.tr("mon.noGame");
    root.windows
        .iter()
        .filter(|w| w.get("ours").and_then(Value::as_bool) == Some(false))
        .map(|w| {
            let game = s(w, "game");
            let status = if game == no_game || game.is_empty() { "menu" } else { s(w, "status") };
            json!({
                "id": format!("win-{}", s(w, "id")), "hwnd": s(w, "id"), "label": s(w, "name"), "state": "running", "foreign": true,
                "session": { "pid": w.get("pid"), "connection": status, "game": { "name": game, "iconUrl": w.get("iconUrl") } },
            })
        })
        .collect()
}

fn ram_mb(root: &Root, a: &Value) -> Option<f64> {
    if let Some(mb) = window_of(root, a).and_then(|w| w.get("ramMb")).and_then(Value::as_f64).filter(|m| *m > 0.) {
        return Some(mb);
    }
    let id = s(a, "id");
    is_local(a)
        .then(|| root.metrics.get("accounts").and_then(Value::as_array).and_then(|l| l.iter().find(|m| s(m, "id") == id)).and_then(|m| m.get("ramMb")?.as_f64()))
        .flatten()
}
/// RAM in MB or GB, with the language's decimal mark ("1,7 GB").
pub(crate) fn mb_text(mb: f64) -> String {
    let t = if mb >= 1024. { format!("{:.1} GB", mb / 1024.) } else { format!("{mb:.0} MB") };
    crate::lang::cur().decimal(&t)
}
fn hidden(a: &Value) -> bool {
    a.pointer("/session/hidden").and_then(Value::as_bool).unwrap_or(false)
}
fn cover_of(root: &Root, place: Option<i64>) -> Option<String> {
    place.and_then(|p| root.looks.get(&p)).and_then(|l| l.get("cover")).and_then(Value::as_str).filter(|u| !u.is_empty()).map(str::to_string)
}

/// Picks or drops one account (a card or a tile is a tick box: the bar below acts on all those picked). With Shift,
/// every one shown from the last clicked to this one is picked (app.js's onCardClick).
fn toggle(r: &mut Root, id: &str, shift: bool, cx: &App) {
    // (A click ends the arrow keys' ring.)
    r.acc_cursor = None;
    if shift && let Some(anchor) = r.acc_anchor.clone() {
        // (Accounts only: another program's window is picked by its own click, as Select all leaves them out.)
        let shown = shown_accounts(r, cx);
        let mut own: Vec<&Value> = shown.iter().filter(|a| !foreign(a)).collect();
        // In the order on screen: the cards come in sections (needs a look, open here, the rest), so a range by the
        // sorted list picked accounts that weren't between the two clicks.
        let listed = cx.try_global::<menu::Ui>().and_then(|u| u.list).unwrap_or(r.state.pointer("/settings/layout").and_then(Value::as_str) != Some("cards"));
        if !r.acc_win && !listed {
            let not_open = |a: &Value| !is_local(a) && !launching(a);
            own.sort_by_key(|a| if not_open(a) { if signed_out(a) || banned(a) { 0 } else { 2 } } else { 1 });
        }
        let ids: Vec<String> = own.into_iter().map(id_of).collect();
        if let (Some(i), Some(j)) = (ids.iter().position(|x| *x == anchor), ids.iter().position(|x| x == id)) {
            let (i, j) = (i.min(j), i.max(j));
            r.selected.extend(ids[i..=j].iter().cloned());
            return;
        }
    }
    if !r.selected.remove(id) {
        r.selected.insert(id.to_string());
    }
    r.acc_anchor = Some(id.to_string());
}

/// The Roblox windows of one game: the old Game monitor's lane, now beside the accounts they belong to.
struct Lane {
    name: String,
    place: Option<i64>,
    icon: Option<String>,
    items: Vec<Value>,
    kicked: usize,
    ram: f64,
}

/// The open accounts by game: a game with one cut off first, then the most windows; the Roblox menu ("No game") last.
fn lanes(root: &Root, shown: &[Value]) -> Vec<Lane> {
    let no_game = root.tr("mon.noGame");
    let loading = root.tr("act.gameLoading");
    let mut out: Vec<Lane> = Vec::new();
    for a in shown.iter().filter(|a| is_local(a) || launching(a)) {
        let menu = is_local(a) && conn(a) == "menu";
        let place = if menu { None } else { game_place(a) };
        // (A window on its way into a game Roblox hasn't named yet: the name is on its way too.)
        let unnamed = if is_local(a) && !foreign(a) && !matches!(conn(a), "disconnected" | "signedout") { &loading } else { &no_game };
        let name = if menu { no_game.clone() } else { game_name(a).unwrap_or_else(|| unnamed.clone()) };
        // A window known only by its game's name (not opened by Bloxkeep) joins the lane of that name.
        let i = match out.iter().position(|l| l.name == name && (l.place == place || l.place.is_none() || place.is_none())) {
            Some(i) => {
                out[i].place = out[i].place.or(place);
                i
            }
            None => {
                out.push(Lane { name, place, icon: None, items: Vec::new(), kicked: 0, ram: 0. });
                out.len() - 1
            }
        };
        let l = &mut out[i];
        l.items.push(a.clone());
        l.kicked += is_kicked(a) as usize;
        l.ram += ram_mb(root, a).unwrap_or(0.);
        if l.icon.is_none() && !menu {
            l.icon = game_icon(a);
        }
    }
    out.sort_by(|a, b| {
        (b.kicked > 0)
            .cmp(&(a.kicked > 0))
            .then(a.place.is_none().cmp(&b.place.is_none()))
            .then(b.items.len().cmp(&a.items.len()))
            .then(a.name.cmp(&b.name))
    });
    out
}

/// One place for everything an account is doing. On top what needs a look (signed out, banned); then a lane for each
/// game the accounts play here, each window a tile (its picture, face, name, time, RAM); then the accounts not open.
/// A tile or a card is a tick box: the bar at the foot acts on those picked (play, rejoin, close).
/// The cards the page shows: the search, the filter and the group applied, sorted, other PCs' after.
fn shown_accounts(root: &Root, cx: &App) -> std::rc::Rc<Vec<Value>> {
    let query = root.acc_search.read(cx).value().to_string();
    // The list layout is a part a group: the accounts go by group first, those in none last, each group in the order
    // picked. So the arrow keys and a range picked with Shift go the way the page reads.
    let grouped = cx.try_global::<menu::Ui>().and_then(|u| u.list).unwrap_or_else(|| root.state.pointer("/settings/layout").and_then(Value::as_str) != Some("cards"));
    let key = ShownKey { grouped, ..shown_key(root, query.clone(), Some(root.acc_filter), root.acc_group.clone()) };
    cached(&SHOWN, key, || {
        let all = root.accounts();
        // (A group picked that no account is in any more counts as none.)
        let group = root.acc_group.clone().filter(|g| all.iter().any(|a| s(a, "group") == g));
        let mut shown: Vec<Value> = all.iter().filter(|a| passes(a, root.acc_filter, &query)).filter(|a| group.as_deref().is_none_or(|g| s(a, "group") == g)).cloned().collect();
        sort(root, &mut shown);
        if grouped {
            // (Keeps the order within a group: this sort is stable.)
            shown.sort_by_cached_key(|a| {
                let g = s(a, "group");
                (g.is_empty(), Natural(folded(g)), g.to_string())
            });
        }
        shown.extend(others(root).into_iter().filter(|a| passes(a, root.acc_filter, &query)));
        shown
    })
}

/// A key on the Accounts page (none typed into a box, no dialog open), as the old page had them.
#[derive(Clone, Copy)]
pub enum Key {
    /// Ctrl+A: every card shown picked.
    All,
    /// Delete: the picked ones deleted, asked first.
    Delete,
    /// F2: the one picked edited.
    Edit,
    /// Enter: the picked ones played.
    Play,
    /// Escape: none picked.
    Clear,
}

/// Does what the key says; false when it says nothing here (the key then goes on as usual).
pub fn key(r: &mut Root, k: Key, window: &mut Window, cx: &mut Context<Root>) -> bool {
    let picked: Vec<Value> = r.accounts().iter().filter(|a| r.selected.contains(s(a, "id"))).cloned().collect();
    let own: Vec<String> = picked.iter().filter(|a| !foreign(a)).map(id_of).collect();
    match k {
        Key::All => {
            // (As the header's Select all: accounts only, not another program's windows.)
            let shown: Vec<Value> = shown_accounts(r, cx).iter().filter(|a| !foreign(a)).cloned().collect();
            if shown.is_empty() {
                return false;
            }
            let farm: Vec<String> = shown.iter().filter(|a| !a.get("isMain").and_then(Value::as_bool).unwrap_or(false)).map(id_of).collect();
            let all: std::collections::HashSet<String> = if farm.is_empty() { shown.iter().map(id_of).collect() } else { farm.into_iter().collect() };
            // (Again with all of them picked: none.)
            r.selected = if r.selected == all { Default::default() } else { all };
        }
        Key::Clear => {
            if menu::is_open(cx) {
                menu::shut(cx);
            } else if r.acc_menu {
                r.acc_menu = false;
            } else if !r.selected.is_empty() {
                r.selected.clear();
            } else if cx.try_global::<menu::Ui>().is_some_and(|u| u.place_wanted.is_some()) {
                cx.global_mut::<menu::Ui>().place_wanted = None;
            } else {
                return false;
            }
        }
        Key::Delete => {
            if own.is_empty() {
                return false;
            }
            let names = picked.iter().filter(|a| !foreign(a)).map(label).collect();
            ask_delete(r, own, names, window, cx);
        }
        Key::Edit => {
            if own.len() != 1 || picked.len() != 1 {
                return false;
            }
            crate::pages::account_form::open(r, Some(own[0].clone()), window, cx);
        }
        Key::Play => {
            if own.is_empty() {
                return false;
            }
            r.play(&own, cx);
        }
    }
    cx.notify();
    true
}

/// Asks before deleting these accounts (their names, five at most, in the question), then deletes them.
fn ask_delete(r: &mut Root, ids: Vec<String>, names: Vec<String>, window: &mut Window, cx: &mut Context<Root>) {
    let n = ids.len() as i64;
    let list = format!("{}{}", names.iter().take(5).cloned().collect::<Vec<_>>().join(", "), if names.len() > 5 { ", …" } else { "" });
    let (title, text) = (r.i18n.tn("t.deleteTitle", n, &[("n", &n.to_string())]), r.i18n.tn("t.deleteQ", n, &[("n", &n.to_string()), ("list", &list)]));
    r.confirm(title, text, r.tr("t.deleteOk"), true, None, window, cx, move |r, cx| {
        let gone = ids.clone();
        act(r, "deleteAccounts", json!({ "ids": ids }), cx, move |r, v| {
            for id in &gone {
                r.selected.remove(id);
            }
            let n = v.as_i64().unwrap_or(0);
            r.toast = Some((r.i18n.tn("t.deleted", n, &[("n", &n.to_string())]), "ok".into()));
        });
    });
}

/// The layout shown: cards or a list, as picked here or as the app's settings keep it ("layout", as app.js sent it).
/// A list until cards are picked (the setting is "" until first chosen): the owner, 2026-11-13, "muốn cái list là mặc
/// định chứ không phải card".
fn as_list(root: &Root, cx: &mut App) -> bool {
    let kept = root.state.pointer("/settings/layout").and_then(Value::as_str) != Some("cards");
    let picked = cx.try_global::<menu::Ui>().and_then(|u| u.list);
    // (Once the settings say the same, they lead again: another vault brings its own.)
    if picked == Some(kept) {
        cx.default_global::<menu::Ui>().list = None;
    }
    picked.unwrap_or(kept)
}

pub fn accounts(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    FRAME.with(|f| f.set(f.get() + 1));
    // Older frames' places go (windows not opened by Bloxkeep get a new id each time, so the map only grew).
    let now = FRAME.with(std::cell::Cell::get);
    SPOTS.with(|s| s.borrow_mut().retain(|_, (f, _)| *f + 1 >= now));
    #[cfg(feature = "shot")]
    menu::shot_hooks(root, cx);
    #[cfg(feature = "shot")]
    shot_place(cx);
    let all = root.accounts();
    let shown = shown_accounts(root, cx);
    let picked: Vec<Value> = all.iter().filter(|a| root.selected.contains(s(a, "id"))).cloned().collect();
    let inner = width - 2. * SIDE_PAD;
    let rows = as_list(root, cx);

    let mut body = div().flex().flex_col().gap(px(SECTION_GAP)).px(px(SIDE_PAD)).pt(px(8.)).pb(px(32.));
    // Getting started (add an account, pick a game, select and Play), until Bloxkeep has opened Roblox for one.
    if let Some(steps) = start::steps(root, true) {
        body = body.child(steps);
    }
    if root.acc_win {
        #[cfg(feature = "core")]
        if crate::core::remote_watch::available() {
            body = body.child(super::remote_watch::controls(root, cx));
        }
        // The Roblox windows' live pictures, a block a game, with the buttons for each game's windows (the old Game
        // monitor, its own view of the page since 2026-11-13: the owner's pick over pictures above the cards).
        // Every window, whatever the filter says: with "Playing" picked, a window on Roblox's home screen (or one not
        // opened by Bloxkeep) went, and the view said "Open a game…" under a tab that counted 1, with "No matching
        // accounts" under that (the owner's screenshot, 2026-11-13).
        let every = every_sorted(root);
        body = match windows(root, &every, inner, cx) {
            // (What a picture does, as the old monitor's head said.)
            Some(w) => body
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(div().flex_1().min_w_0().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("mon.hint")))
                        .child(load::strip(root, cx)),
                )
                .child(w),
            None => body.child(
                div()
                    .py(px(56.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(12.))
                    .child(div().size(px(44.)).rounded(px(R_M)).bg(c(RAISED_2)).flex().items_center().justify_center().child(icon("screens", 20., c(MUTED))))
                    .child(div().text_size(px(T_L)).text_color(c(TEXT_2)).child(root.tr("mon.empty"))),
            ),
        };
    } else if rows {
        let own: Vec<&Value> = shown.iter().filter(|a| !foreign(a)).collect();
        if !own.is_empty() {
            // Wide enough: the account clicked last in full beside the list (a list and its detail, as launchers lay
            // out a list of things to pick), else the first one picked, else the first one.
            if width >= 1300. {
                let focus = root
                    .acc_anchor
                    .as_deref()
                    .and_then(|id| own.iter().copied().find(|a| s(a, "id") == id))
                    .or_else(|| own.iter().copied().find(|a| root.selected.contains(s(a, "id"))))
                    .unwrap_or(own[0]);
                body = body.child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(16.))
                        .child(div().flex_1().min_w_0().child(list::rows(root, &own, inner - 356., cx)))
                        .child(list::detail(root, focus, cx)),
                );
            } else {
                body = body.child(list::rows(root, &own, inner, cx));
            }
        }
    } else {
        body = cards(root, body, &shown, inner, cx);
    }
    if shown.is_empty() && !all.is_empty() && !root.acc_win {
        body = body.child(
            // In the middle of the page (the owner, 2026-11-17: tucked at the top left it read as left over).
            div()
                .w_full()
                .pt(px(120.))
                .pb(px(40.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .child(div().size(px(52.)).mb(px(8.)).rounded_full().bg(c(RAISED)).border_1().border_color(c(LINE)).flex().items_center().justify_center().child(icon("search", 20., c(MUTED))))
                .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr("nores.title")))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("nores.text")))
                // Clear search with a search typed, else Show all; either puts the search, filter and group aside
                // (Accounts.tsx's clear-search).
                .child(div().pt(px(12.)).flex().child(
                    Button::new("nores-clear", root.tr(if root.acc_search.read(cx).value().trim().is_empty() { "nores.all" } else { "nores.clear" })).on_click(cx.listener(
                        |r, _, window, cx| {
                            r.acc_filter = Filter::All;
                            r.acc_group = None;
                            let search = r.acc_search.clone();
                            search.update(cx, |st, cx| {
                                st.set_value("", window, cx);
                                st.focus(window, cx);
                            });
                            cx.notify();
                        },
                    )),
                )),
        );
    }

    // A low window (the smallest is 500 px high): the header goes with the list, a scroll away. Held in place it took
    // most of the page, and with the selected accounts' bar up no line of the list was left to see (the shots of
    // every size, 2026-11-18).
    let head = header(root, all, width, rows, cx).into_any_element();
    let (fixed_head, body) = if root.window_h.get() < 620. && !all.is_empty() { (None, div().flex().flex_col().child(head).child(body)) } else { (Some(head), body) };
    div()
        .size_full()
        .rounded(px(R_PANEL))
        .bg(c(PANEL))
        .overflow_hidden()
        .relative()
        .flex()
        .flex_col()
        .children(fixed_head)
        .child(if all.is_empty() {
            start::empty(root, width, cx).into_any_element()
        } else {
            // The live pictures show only inside the scrolling part (never over the header or the bar).
            div()
                .flex_1()
                .min_h_0()
                .relative()
                .child(root.live.area())
                .child(crate::scrolled::body("acc-body", body))
                .into_any_element()
        })
        .when(!picked.is_empty() || cx.try_global::<menu::Ui>().is_some_and(|u| u.place_wanted == Some(root.nav)), |d| d.child(bar(root, &picked, width, cx)))
}

/// A pop-up menu is built while it is open and while it fades out after (kit::pop's 140 ms): not, as before, every
/// frame from its first opening on, shut or not.
pub fn pop_alive(key: &str, open: bool) -> bool {
    thread_local! {
        /// By menu: open at the last look, and when it was seen shut after being open.
        static SEEN: std::cell::RefCell<std::collections::HashMap<String, (bool, Option<std::time::Instant>)>> = Default::default();
    }
    SEEN.with(|m| {
        let mut m = m.borrow_mut();
        let e = m.entry(key.to_string()).or_insert((false, None));
        if open {
            *e = (true, None);
            return true;
        }
        if e.0 {
            *e = (false, Some(std::time::Instant::now()));
        }
        e.1.is_some_and(|at| at.elapsed() < std::time::Duration::from_millis(250))
    })
}

/// Ctrl+L or "/" (app.js ~3222): the bar's game box wanted, even with none picked (the bar shows for it).
pub fn want_place(nav: u64, cx: &mut App) {
    cx.default_global::<menu::Ui>().place_wanted = Some(nav);
}

/// Esc in the game box: the bar shown for it alone goes.
pub fn unwant_place(cx: &mut App) {
    cx.default_global::<menu::Ui>().place_wanted = None;
}

/// A shot's bar with a game in its box (BLOXKEEP_SHOT_PLACE=<place id>) and, with BLOXKEEP_SHOT_JOB=1, the Job ID box
/// open with a server in it: set once, just after this frame (a box needs the window).
#[cfg(feature = "shot")]
fn shot_place(cx: &mut Context<Root>) {
    let Ok(place) = std::env::var("BLOXKEEP_SHOT_PLACE") else { return };
    if cx.try_global::<menu::Ui>().is_some_and(|u| u.shot_place_done) {
        return;
    }
    let Some(h) = cx.windows().first().copied() else { return };
    cx.default_global::<menu::Ui>().shot_place_done = true;
    let this = cx.weak_entity();
    cx.spawn(async move |_, cx| {
        let _ = cx.update_window(h, |_, window, cx| {
            let _ = this.update(cx, |r, cx| {
                clear_job(window, cx);
                r.place.update(cx, |st, cx| st.set_value(place.clone(), window, cx));
                r.resolve_place(cx);
                if std::env::var("BLOXKEEP_SHOT_JOB").is_ok() {
                    let e = job_entity(r, window, cx);
                    e.update(cx, |st, cx| st.set_value("1f0a3c2e-77d1-4b8e-9c5a-2d6e8f9a0b1c", window, cx));
                    cx.default_global::<menu::Ui>().job_open = true;
                }
                cx.notify();
            });
        });
    })
    .detach();
}

/// The Roblox windows open now, a block per game with their live pictures (the old Game monitor), for the cards and the
/// list alike: the games flow on the page's grid, each taking as many columns as it has windows (all of them at most,
/// wrapping inside its own block past that), its head over its own tiles. None while no window is open.
fn windows(root: &Root, shown: &[Value], inner: f32, cx: &mut Context<Root>) -> Option<Div> {
    let mut lanes = lanes(root, shown);
    if lanes.is_empty() {
        return None;
    }
    let multi = lanes.len() > 1;
    // One game alone (its head's Only this game), while it is still open.
    if let Some(solo) = root.mon_solo.as_deref().filter(|g| lanes.iter().any(|l| l.name == *g)) {
        lanes.retain(|l| l.name == solo);
    }
    // Every tile reserves room for the fullest account's details, so neighboring tiles end on one line (pared-down
    // tiles show none of them).
    let many = lanes.iter().map(|l| l.items.len()).sum::<usize>() > MON_MANY;
    let tile_n = if many { 0 } else { lanes.iter().flat_map(|l| l.items.iter()).map(|a| tile_lines(root, a)).max().unwrap_or(0) };
    let height = root.live.area_height().unwrap_or_else(|| (root.window_h.get() - 210.).max(0.));
    let tile_room = (height - 112.).max(0.);
    Some(
        div().w_full().flex().flex_wrap().items_start().gap_x(px(GRID_GAP)).gap_y(px(SECTION_GAP)).children(
            lanes
                .into_iter()
                .enumerate()
                .map(|(i, l)| {
                    let (cols, col_w) = monitor_grid(l.items.len(), inner, tile_room, tile_n, many);
                    lane(root, i, l, col_w, cols, tile_n, multi, many, cx).into_any_element()
                })
                .collect::<Vec<_>>(),
        ),
    )
}

/// Pick the largest 16:9 previews that fit the lane. If all layouts need to scroll, use the fewest rows.
fn monitor_grid(count: usize, width: f32, height: f32, detail_lines: usize, many: bool) -> (usize, f32) {
    let narrowest = if many { MON_COL_COMPACT } else { MON_COL_MIN };
    let (max_cols, _) = grid_fit(width, narrowest, GRID_GAP);
    let max_cols = max_cols.min(count).max(1);
    let minimum = width.min(narrowest);
    let chrome = if many { 110. } else { 175. + 30. * detail_lines as f32 };
    // When no layout fits the screen (30 windows of a farm), the most columns, the tiles across the whole width: they
    // stayed at their smallest and left a band empty at the right (the owner, 2026-11-17).
    let across_all = ((width - (max_cols - 1) as f32 * GRID_GAP) / max_cols as f32).min(MON_COL_MAX).max(minimum).floor();
    let mut best = (max_cols, across_all);
    let mut best_fits = false;
    for cols in 1..=max_cols {
        let rows = count.max(1).div_ceil(cols) as f32;
        let room = (height - (rows - 1.) * GRID_GAP) / rows;
        let across = (width - (cols - 1) as f32 * GRID_GAP) / cols as f32;
        let by_height = (room - chrome) * 16. / 9. + MON_PIC_INSET;
        let tile = across.min(MON_COL_MAX).min(by_height.max(minimum)).floor();
        let fits = tile <= by_height;
        let fewer_rows = rows < count.max(1).div_ceil(best.0) as f32;
        if fits && (!best_fits || tile > best.1 || (tile == best.1 && fewer_rows)) {
            best = (cols, tile);
            best_fits = true;
        }
    }
    best
}

/// The cards layout: what needs a look on top, a lane for each game played here with a tile a window, then the
/// accounts not open.
fn cards(root: &Root, mut body: Div, shown: &[Value], inner: f32, cx: &mut Context<Root>) -> Div {
    let not_open = |a: &&Value| !is_local(a) && !launching(a);
    let check: Vec<&Value> = shown.iter().filter(not_open).filter(|a| signed_out(a) || banned(a)).collect();
    let idle: Vec<&Value> = shown.iter().filter(not_open).filter(|a| !signed_out(a) && !banned(a)).collect();
    let (per, col_w) = grid_fit(inner, COL_MIN, GRID_GAP);
    // What needs a hand: one calm panel, a full-width row each with its fix lined up at the right.
    if !check.is_empty() {
        let n = check.len();
        let mut panel = alert_panel(WARN, root.tr("ui.grp.check"), n);
        for a in check {
            panel = panel.child(alert_line(WARN)).child(look_row(root, a, cx));
        }
        body = body.child(panel);
    }
    // Open here (or opening): cards as the others, their windows' pictures being on the Windows view.
    let open: Vec<&Value> = shown.iter().filter(|a| !foreign(a) && (is_local(a) || launching(a))).collect();
    if !open.is_empty() {
        let n = open.len();
        let lines = open.iter().map(|a| card_lines(root, a)).max().unwrap_or(0);
        let must = revealed_in(root, &open);
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(section_head(root.tr("acc.grp.open"), Some(n)))
                .child(crate::near::grid("acc-open", n, per, card_h(lines), GRID_GAP, must, |i| card(root, open[i], col_w, lines, cx).into_any_element()).pb(px(RISE))),
        );
    }
    if !idle.is_empty() {
        let n = idle.len();
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(section_head(root.tr("acc.grp.notHere"), Some(n)))
                // (Every card as tall as the fullest one: the grid's rows stay even.)
                .child({
                    let lines = idle.iter().map(|a| card_lines(root, a)).max().unwrap_or(0);
                    let must = revealed_in(root, &idle);
                    crate::near::grid("acc-idle", n, per, card_h(lines), GRID_GAP, must, |i| card(root, idle[i], col_w, lines, cx).into_any_element()).pb(px(RISE))
                }),
        );
    }
    body
}

/// What is wrong with an account that needs a look, in plain words after its state: a sign-in to do again, a ban.
fn look_hint(root: &Root, a: &Value) -> Option<String> {
    if rejoin_wait(a).is_some() {
        None
    } else if banned(a) {
        Some(root.tr("card.bannedTip"))
    } else if signed_out(a) {
        Some(root.tr("act.signInAgain"))
    } else {
        None
    }
}

/// A row of the panel of what needs a look: the face, the name, what is wrong in plain words, then its fix (Sign in,
/// Appeal, Cancel rejoin) in one column at the right, and the ⋮. A click picks it, as a card's.
fn look_row(root: &Root, a: &Value, cx: &mut Context<Root>) -> impl IntoElement {
    let id = id_of(a);
    let on = root.selected.contains(&id);
    let (line, tone) = line_of(root, a);
    let hint = look_hint(root, a);
    let fix = action(root, a, "look", cx);
    let user = s(a, "username").to_string();
    let pick = id.clone();
    // Its group, when Bloxkeep last opened it, its note: what the card had before it needed a look (review 2.1).
    let tags = tags_of(root, a);
    let opened = opened_of(root, a);
    let note = note_of(a);
    // (Three lines: taller than an alert row's two, or the third ran into the next row.)
    alert_row(SharedString::from(format!("look-{id}")))
        .h(px(88.))
        .relative()
        .child(spot(root, &id, R_M))
        .map(|d| if on { d.tones(tinted(PANEL, FACE, 0.10), tinted(PANEL, FACE, 0.13)) } else { d })
        .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
            toggle(r, &pick, e.modifiers().shift, cx);
            cx.notify();
        }))
        .on_mouse_down(MouseButton::Right, on_right(a, cx))
        .children(menu::place(root, a, false, cx))
        .child(avatar(a, 40., PANEL, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(8.))
                        // (The name keeps its room, the @username gives way first: a long one cut the name to a letter.)
                        .child(div().flex_shrink_0().max_w(relative(1.)).truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(label(a)))
                        .when(!user.is_empty(), |d| d.child(div().min_w_0().flex_shrink(1.).truncate().text_size(px(T_S)).text_color(c(MUTED)).child(format!("@{user}")))),
                )
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(T_M))
                        .child(div().size(px(7.)).flex_none().rounded_full().bg(c(tone)))
                        .child(div().flex_none().text_color(c(tone)).font_weight(FontWeight::MEDIUM).child(line))
                        .when_some(hint, |d, h| d.child(div().min_w_0().truncate().text_color(c(MUTED)).child(h))),
                )
                .child(
                    div()
                        .pt(px(2.))
                        .flex()
                        .min_w_0()
                        .items_center()
                        .gap(px(10.))
                        .children(tags)
                        .child(last_line(root, a, opened).flex_none())
                        .when_some(note, |d, n| d.child(small_line("message", n).min_w_0())),
                ),
        )
        .when(on, |d| d.child(tick_mark(22.)))
        .child(alert_fix(div().children(fix.map(no_pick))))
        .child(main_button(root, a, "look", 28., cx))
        .child(edit_button(root, a, "look", 28., cx))
        .child(more_button(root, a, "look", 28., cx))
        // Picked: the yellow ring every picked card has (the tick alone was easy to miss on a long row).
        .when(on, |d| d.child(div().absolute().inset_0().rounded(px(R_M)).border_2().border_color(c(FACE))))
}

/// The title and count with a quiet summary under them (who plays, their RAM, what needs a look), the search and Add on
/// that line; the filters under them, and at their right one row of quiet tools, all of one height.
fn header(root: &Root, all: &[Value], width: f32, rows: bool, cx: &mut Context<Root>) -> impl IntoElement {
    // Short of room (a window about 1180 wide): the tools keep the value chosen and drop their names.
    let compact = width < 1300.;
    // Cards or a list (the web page's layout switch): kept by the app with the sort, so it is the same next time.
    // Cards, a list, or the Roblox windows (the Game monitor): the windows one named, with how many are open.
    let n_win = root.windows.len();
    let mut views: Vec<Hot> = [("cards", "grid", false), ("list", "rows", true)]
        .into_iter()
        .map(|(key, mark, list)| {
            let tip: SharedString = root.tr(&format!("layout.{key}")).into();
            view_choice(SharedString::from(format!("acc-layout-{key}")), mark, !root.acc_win && rows == list).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()).on_click(cx.listener(move |r, _, _, cx| {
                cx.default_global::<menu::Ui>().list = Some(list);
                r.send("setSettings", json!({ "layout": key }));
                set_win_view(r, false);
                cx.notify();
            }))
        })
        .collect();
    views.push(
        segment("acc-layout-windows", root.acc_win)
            .h(px(TOOL_H - 6.))
            .px(px(10.))
            .gap(px(6.))
            .rounded(px(R_S - 2.))
            .text_size(px(T_M))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if root.acc_win { crate::theme::on_text() } else { c(TEXT_2) })
            .child(icon("screens", 15., if root.acc_win { crate::theme::on_text() } else { c(TEXT_2) }))
            .when(!compact, |d| d.child(root.tr("mon.title")))
            .when(n_win > 0, |d| d.child(div().text_color(if root.acc_win { crate::theme::on_text() } else { c(MUTED) }).child(n_win.to_string())))
            .on_click(cx.listener(|r, _, _, cx| {
                set_win_view(r, true);
                cx.notify();
            })),
    );
    let layout = view_switch(views);
    let playing = all.iter().filter(|a| in_game(a)).count();
    // The chips count the group picked and the search typed, as the list shows them ("All 500" over three rows read
    // wrong); in a group, empty ones go.
    let picked_group = root.acc_group.clone().filter(|g| all.iter().any(|a| s(a, "group") == g));
    let typed = root.acc_search.read(cx).value().to_string();
    let grouped: Vec<&Value> = all.iter().filter(|a| picked_group.as_deref().is_none_or(|g| s(a, "group") == g) && found(a, &typed)).collect();
    let g_online = grouped.iter().filter(|a| is_online(a)).count();
    let mut chips = vec![(Filter::All, root.tr("chip.all"), grouped.len())];
    for (f, key, n) in [
        (Filter::Playing, "chip.ingame", grouped.iter().filter(|a| in_game(a)).count()),
        (Filter::Online, "chip.online", g_online),
        (Filter::Offline, "chip.offline", grouped.len() - g_online),
    ] {
        if picked_group.is_none() || n > 0 || root.acc_filter == f {
            chips.push((f, root.tr(key), n));
        }
    }
    // (Shown only while there are some: open on this PC, needing a sign-in, and banned.)
    for (f, key, n) in [
        (Filter::Local, "chip.local", grouped.iter().filter(|a| is_local(a)).count()),
        (Filter::Invalid, "chip.invalid", grouped.iter().filter(|a| signed_out(a)).count()),
        (Filter::Banned, "chip.banned", grouped.iter().filter(|a| banned(a)).count()),
    ] {
        if n > 0 || root.acc_filter == f {
            chips.push((f, root.tr(key), n));
        }
    }
    // The page at a glance, in muted words: who plays (or that Roblox isn't open), their windows' RAM, who dropped,
    // how many need a look.
    let strays = others(root);
    let ram: f64 = all.iter().chain(strays.iter()).filter(|a| is_local(a)).filter_map(|a| ram_mb(root, a)).sum();
    let dropped = all.iter().filter(|a| is_kicked(a)).count();
    let look = all.iter().filter(|a| !is_local(a) && !launching(a) && (signed_out(a) || banned(a))).count();
    // (A window open with nobody in a game, or one Bloxkeep didn't open: "Roblox not open" stood over a bar that
    // said "1 Roblox window", the owner's screenshot, 2026-11-13.)
    let open = all.iter().filter(|a| is_local(a)).count() + strays.len();
    let first = if playing > 0 {
        root.i18n.tn("acc.sum.playing", playing as i64, &[])
    } else if open > 0 {
        root.i18n.tn("ui.robloxWindows", open as i64, &[("n", &open.to_string())])
    } else {
        root.tr("acc.sum.idle")
    };
    let mut summary = vec![(first, None)];
    if ram > 0. {
        summary.push((mb_text(ram), None));
    }
    // About how many windows this PC runs at once, measured from the person's own windows (core/capacity.rs).
    if let Some(n) = root.metrics.pointer("/fit/max").and_then(Value::as_i64) {
        summary.push((root.i18n.tv("cap.fits", &[("n", &n.to_string())]), None));
    }
    if dropped > 0 {
        summary.push((root.i18n.tv("ui.droppedN", &[("n", &dropped.to_string())]), None));
    }
    if look > 0 {
        summary.push((root.i18n.tn("acc.sum.look", look as i64, &[]), None));
    }
    // The group shown, when the accounts have any (picked in the Group tool beside Sort: pages/accounts/groups.rs).
    let names = groups::names(all);
    let now = root.acc_group.clone().filter(|g| names.contains(g));
    // The order: kept by the app, so it is the same next time; under it, the order Play opens the picked accounts in.
    let how = root.state.pointer("/settings/sort").and_then(Value::as_str).unwrap_or("name").to_string();
    let mut items: Vec<DropItem> = ["name", "status", "recent", "order"]
        .into_iter()
        .map(|key| {
            let f: DropAct = std::rc::Rc::new(move |r, _, cx| {
                // Shown at once: the app only remembers this one and sends no new state for it (set_settings' "quick"
                // keys: the web page showed it itself), so the list kept its old order until some other change came
                // (the owner, 2026-11-13: "bấm 1 hồi nó mới nhảy qua cái mình chọn").
                if let Some(now) = r.state.pointer_mut("/settings/sort") {
                    *now = json!(key);
                }
                r.send("setSettings", json!({ "sort": key }));
                cx.notify();
            });
            (root.tr(&format!("sort.{key}")), how == key, f)
        })
        .collect();
    items.push((LINE_ITEM.to_string(), false, std::rc::Rc::new(|_, _, _| {})));
    items.push((root.tr("lo.edit"), false, std::rc::Rc::new(|r, window, cx| open_order(r, window, cx))));
    let shown = root.tr(&format!("sort.{how}"));
    let sorts = drop_list(root, "acc-sort", "sort", Some(root.tr("sort.label")), shown, items, cx);
    // Select all: the ones shown; the main accounts (the ones played on) are left out, unless they are all there is.
    let query = root.acc_search.read(cx).value().to_string();
    let seen: Vec<&Value> = all.iter().filter(|a| passes(a, root.acc_filter, &query)).filter(|a| now.as_deref().is_none_or(|g| s(a, "group") == g)).collect();
    let farm: Vec<String> = seen.iter().filter(|a| !a.get("isMain").and_then(Value::as_bool).unwrap_or(false)).map(|a| id_of(a)).collect();
    let pickable: Vec<String> = if farm.is_empty() { seen.iter().map(|a| id_of(a)).collect() } else { farm };
    let none = pickable.is_empty();
    // (Named at every width: short of room the tools take a line of their own under the filters, review 2.5.)
    // (With all of them picked it unpicks them, and says so.)
    let all_picked = !none && pickable.len() == root.selected.len() && pickable.iter().all(|id| root.selected.contains(id));
    let all_words: SharedString = crate::pages::lock::plain(root.tr(if all_picked { "acc.selectNone" } else { "acc.selectAll" })).into();
    let pick_all = tool("acc-pick-all", Some(if all_picked { "x" } else { "check" }), false).child(all_words).map(|d| {
        if none {
            d.opacity(0.45).cursor_default()
        } else {
            d.on_click(cx.listener(|r, _, window, cx| {
                key(r, Key::All, window, cx);
                cx.notify();
            }))
        }
    });
    let tools = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .child(layout)
        .child(tool_rule())
        .child(sorts)
        // Which group shows, and the group the selected accounts go in (pages/accounts/groups.rs).
        .child(groups::button(root, all, cx))
        // Windows: Open Roblox only, hide or show them all, and Arrange's layouts as pictures (pages/snap.rs).
        .child(crate::pages::snap::button(root, false, cx))
        .child(tool_rule())
        .child(pick_all);
    div()
        .flex_none()
        .px(px(SIDE_PAD))
        .pt(px(22.))
        .pb(px(16.))
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .flex()
                                .items_baseline()
                                .gap(px(10.))
                                .child(div().text_size(px(T_TITLE)).line_height(px(38.)).font_weight(FontWeight::BOLD).child(root.tr("acc.title")))
                                .child(div().text_size(px(T_TITLE)).line_height(px(38.)).text_color(c(MUTED)).font_features(tnum()).child(all.len().to_string())),
                        )
                        // (Not in a low window while the selected accounts' bar is up: its three lines were the
                        // list's last room, 860×500.)
                        .when(!all.is_empty() && (root.window_h.get() >= 620. || root.selected.is_empty()), |d| d.child(facts(summary))),
                )
                .child(
                    div()
                        .w(px(if width < 900. { 220. } else { 280. }))
                        .min_w(px(140.))
                        .flex_shrink(1.)
                        .child(field(&root.acc_search, Some("search"))),
                )
                // Online status asked again now (the app's refreshPresence; its icon turns until it answers).
                .child({
                    let busy = cx.try_global::<menu::Ui>().is_some_and(|u| u.refreshing);
                    IconButton::new("acc-refresh", "refresh").size(38.).busy(busy).tip(root.tr("acc.refreshTip")).on_click(cx.listener(|r, _, _, cx| refresh(r, cx)))
                })
                .child(tools_menu(root, all, cx))
                // Add account, and the other ways to add one beside it: one yellow button split by a hairline, as the C#
                // page's (owner 2026-11-13: the arrow was a separate dark square).
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .rounded(px(R_M + 1.))
                        .bg(c(FACE))
                        .child(Button::new("acc-add", root.tr("acc.add")).icon("userPlus").primary().on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
                        .child(div().w(px(1.)).h(px(22.)).bg(ca(INK, 0.22)))
                        .child(add_menu(root, cx)),
                ),
        )
        // No Roblox on this PC: nothing can open until it is installed (the web page's sidebar card); Get Roblox opens
        // Roblox's download page.
        .when(root.state.get("robloxInstalled").and_then(Value::as_bool) == Some(false), |d| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(8.))
                    .rounded(px(R_M))
                    .bg(tinted(PANEL, BAD, 0.07))
                    .border_1()
                    .border_color(ca(BAD, 0.3))
                    .child(icon("alert", 16., c(BAD)))
                    .child(div().flex_1().min_w_0().text_size(px(T_M)).text_color(c(TEXT_2)).child(root.tr("side.noRoblox")))
                    .child(Button::new("acc-get-roblox", root.tr("acc.getRoblox")).icon("download").small().on_click(|_, _, cx| cx.open_url("https://www.roblox.com/download"))),
            )
        })
        // (No accounts yet: nothing to filter, sort or lay out; the start below says what to do.)
        .when(!all.is_empty(), |d| {
            d.child(
                // One row: the filters take what the tools leave; short of room (about 1180 wide), the tools on a line
                // of their own under them, with their names (icons alone read as "All", "Name" and a picture).
                div()
                    .flex()
                    .when(compact, |d| d.flex_col().items_start().gap(px(10.)))
                    .when(!compact, |d| d.items_center().gap(px(12.)))
                    .child(
                        div().when(compact, |d| d.w_full()).when(!compact, |d| d.flex_1()).min_w_0().flex().flex_wrap().items_center().gap(px(if compact { 2. } else { 4. })).children(chips.into_iter().map(|(f, text, count)| {
                            let on = root.acc_filter == f;
                            let dot = match f {
                                Filter::All => None,
                                Filter::Playing => Some(OK),
                                Filter::Online => Some(ONLINE),
                                Filter::Offline => Some(MUTED),
                                Filter::Invalid => Some(BAD),
                                Filter::Banned => Some(MUTED),
                                Filter::Local => Some(OK),
                            };
                            chip(SharedString::from(format!("chip-{text}")), on)
                                .h(px(TOOL_H))
                                .when(compact, |d| d.px(px(8.)))
                                .when_some(dot, |d, col| d.child(div().size(px(7.)).flex_none().rounded_full().bg(c(col))))
                                .child(text)
                                .child(chip_count(count, on))
                                .on_click(cx.listener(move |r, _, _, cx| {
                                    r.acc_filter = f;
                                    cx.notify();
                                }))
                        })),
                    )
                    .child(tools),
            )
        })
        // Multi-Roblox off: opening one more account closes the window before it (the web page said so in its
        // sidebar card, "Off: one Roblox window at a time", and in Multi-Roblox's box); said here, where Play is.
        .when(root.state.pointer("/multi/enabled").and_then(Value::as_bool) == Some(false) && all.len() > 1, |d| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(8.))
                    .rounded(px(R_M))
                    .bg(block_ground())
                    .border_1()
                    .border_color(ca(WARN, 0.3))
                    .child(icon("alert", 16., c(WARN)))
                    .child(div().flex_1().min_w_0().text_size(px(T_M)).text_color(c(TEXT_2)).child(root.tr("mh.off")))
                    .child(Button::new("acc-multi-on", root.tr("mh.enable")).icon("layers").small().on_click(cx.listener(|r, _, window, cx| r.multi_help(window, cx)))),
            )
        })
}

/// Refresh: the accounts' online status asked again (app.js's refresh-presence); only a click that did nothing is said.
fn refresh(r: &mut Root, cx: &mut Context<Root>) {
    if cx.try_global::<menu::Ui>().is_some_and(|u| u.refreshing) {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    cx.default_global::<menu::Ui>().refreshing = true;
    let rx = link.call("refreshPresence", json!({}));
    answer(rx, cx, move |r, answer, cx| {
        cx.default_global::<menu::Ui>().refreshing = false;
        match answer {
            Ok(Ok(v)) if v.get("skipped").and_then(Value::as_bool) == Some(true) => r.toast = Some((r.tr("t.refreshBusy"), "info".into())),
            Ok(Err(e)) => r.toast = Some((e.to_string(), "error".into())),
            _ => {}
        }
        cx.notify();
    });
    cx.notify();
}

/// A line of an icon menu: its icon, words, whether it is a danger (red), and what picking it does; None: a hairline.
/// (None for the act: the line greyed, as a disabled item of the web page's menus.)
type MenuLine = Option<(&'static str, String, bool, Option<DropAct>)>;

/// An icon button with a short menu under it (the header's ⋮, Add's other ways), on the same open/closed state as the
/// toolbar's lists, so one of them is open at a time.
fn icon_menu(root: &Root, key: &'static str, button: IconButton, width: f32, lines: Vec<MenuLine>, cx: &mut Context<Root>) -> Div {
    let open = root.acc_drop == Some(key);
    let mut wrap = div().relative().flex_none().child(button.selected(open).on_click(cx.listener(move |r, _, _, cx| {
        let just = r.acc_drop_closed.is_some_and(|(k, at)| k == key && at.elapsed().as_millis() < 300);
        r.acc_drop_closed = None;
        r.acc_drop = if just || r.acc_drop == Some(key) { None } else { Some(key) };
        cx.notify();
    })));
    if pop_alive(key, open) {
        let mut menu = div()
            .id(SharedString::from(format!("{key}-menu")))
            .occlude()
            .w(px(width))
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(move |r, _, _, cx| {
                if r.acc_drop == Some(key) {
                    r.acc_drop = None;
                    r.acc_drop_closed = Some((key, std::time::Instant::now()));
                    cx.notify();
                }
            }));
        for (i, l) in lines.into_iter().enumerate() {
            let Some((mark, text, danger, f)) = l else {
                menu = menu.child(div().h(px(1.)).my(px(4.)).mx(px(6.)).bg(c(LINE)));
                continue;
            };
            let tone = if danger { BAD } else { TEXT };
            let row = menu_row((key, 2000 + i)).child(icon(mark, 14., c(if danger { BAD } else { TEXT_2 }))).child(div().min_w_0().flex_1().truncate().child(text));
            menu = menu.child(match f {
                Some(f) => row.text_color(c(tone)).on_click(cx.listener(move |r, _, window, cx| {
                    r.acc_drop = None;
                    f(r, window, cx);
                    cx.notify();
                })),
                None => row.opacity(0.45).cursor_default(),
            });
        }
        wrap = wrap.child(popup(PopAt::BelowRight(42.), SharedString::from(format!("{key}-pop")), open, menu));
    }
    wrap
}

/// The header's ⋮ (app.js's tools-menu): check every sign-in, the launch order, close or end every Roblox, the guide.
fn tools_menu(root: &Root, all: &[Value], cx: &mut Context<Root>) -> Div {
    let some = !all.is_empty();
    let act = |f: fn(&mut Root, &mut Window, &mut Context<Root>)| -> Option<DropAct> { Some(std::rc::Rc::new(f)) };
    let mut lines: Vec<MenuLine> = Vec::new();
    if some {
        // (Greyed while a check runs, as app.js's S.app.checking.)
        let checking = root.state.get("checking").and_then(Value::as_bool).unwrap_or(false);
        lines.push(Some(("refresh", root.tr("m.checkAll"), false, if checking { None } else { act(|r, _, cx| check_accounts(r, Vec::new(), cx)) })));
    }
    if all.len() > 1 {
        lines.push(Some(("sort", root.tr("m.launchOrder"), false, act(|r, window, cx| open_order(r, window, cx)))));
    }
    if !lines.is_empty() {
        lines.push(None);
    }
    lines.push(Some(("power", root.tr("m.closeAll"), true, act(|r, window, cx| r.close_all(false, window, cx)))));
    lines.push(Some(("x", root.tr("m.killAll"), true, act(|r, window, cx| r.close_all(true, window, cx)))));
    lines.push(None);
    lines.push(Some(("book", root.tr("help.title"), false, act(|r, _, _| r.send("openGuide", json!({}))))));
    icon_menu(root, "acc-tools", IconButton::new("acc-tools-btn", "more").size(38.).tip(root.tr("acc.moreTip")), 280., lines, cx)
}

/// Beside Add account (app.js's add-menu): the other ways to add one, from another Bloxkeep file, and roblox.com.
fn add_menu(root: &Root, cx: &mut Context<Root>) -> Div {
    let act = |f: fn(&mut Root, &mut Window, &mut Context<Root>)| -> Option<DropAct> { Some(std::rc::Rc::new(f)) };
    let lines: Vec<MenuLine> = vec![
        Some(("qr", root.tr("m.addCode"), false, act(|r, window, cx| crate::pages::quick::open(r, false, window, cx)))),
        Some(("globe", root.tr("m.addBrowser"), false, act(|r, _, cx| r.add_browser(cx)))),
        Some(("cookie", root.tr("m.addCookie"), false, act(|r, window, cx| crate::pages::account_form::open(r, None, window, cx)))),
        None,
        Some(("folder", root.tr("m.importVault"), false, act(|r, window, cx| crate::pages::import_vault::open(r, window, cx)))),
        None,
        Some(("external", root.tr("m.openWeb"), false, act(|r, _, _| r.send("openWebsiteBlank", json!({}))))),
    ];
    icon_menu(root, "acc-addmore", IconButton::new("acc-addmore-btn", "chevronDown").size(38.).primary().tip(root.tr("acc.addMoreTip")), 320., lines, cx)
}

/// What a short list's row does when picked.
pub(crate) type DropAct = std::rc::Rc<dyn Fn(&mut Root, &mut Window, &mut Context<Root>)>;
/// A row of a short list: its words, whether it is the one chosen, what picking it does. Words of `LINE_ITEM` are a
/// hairline between groups of rows.
pub(crate) type DropItem = (String, bool, DropAct);
const LINE_ITEM: &str = "-";
/// A list item that is a small head over the items after it (its words after this mark).
const HEAD_ITEM: &str = "\u{1}";

/// A quiet tool that opens a short list under it ("Group  All ▾", "Sort  Name ▾"): `name` before the value chosen
/// (none when room is short). The row keeps one bright thing, the chosen filter.
pub(crate) fn drop_list(root: &Root, key: &'static str, mark: &'static str, name: Option<String>, value: String, items: Vec<DropItem>, cx: &mut Context<Root>) -> Div {
    let open = root.acc_drop == Some(key);
    let mut wrap = div().relative().flex_none().child(tool(key, Some(mark), open).child(tool_words(name, value)).child(tool_chev()).on_click(cx.listener(move |r, _, _, cx| {
        // A click outside the open list closed it just now: this click on its own button only closes it.
        let just = r.acc_drop_closed.is_some_and(|(k, at)| k == key && at.elapsed().as_millis() < 300);
        r.acc_drop_closed = None;
        r.acc_drop = if just || r.acc_drop == Some(key) { None } else { Some(key) };
        cx.notify();
    })));
    if pop_alive(key, open) {
        let mut menu = div()
            .id(SharedString::from(format!("{key}-menu")))
            .occlude()
            .min_w(px(200.))
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(move |r, _, _, cx| {
                if r.acc_drop == Some(key) {
                    r.acc_drop = None;
                    r.acc_drop_closed = Some((key, std::time::Instant::now()));
                    cx.notify();
                }
            }));
        for (i, (text, on, f)) in items.into_iter().enumerate() {
            if text == LINE_ITEM {
                menu = menu.child(div().h(px(1.)).my(px(4.)).mx(px(6.)).bg(c(LINE)));
                continue;
            }
            if let Some(head) = text.strip_prefix(HEAD_ITEM) {
                menu = menu.child(div().px(px(10.)).pt(px(6.)).pb(px(2.)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(head.to_string()));
                continue;
            }
            menu = menu.child(
                menu_row((key, 1000 + i))
                    .text_color(if on { c(TEXT) } else { c(TEXT_2) })
                    .child(div().min_w_0().flex_1().whitespace_nowrap().child(text))
                    .child(div().w(px(14.)).flex_none().when(on, |d| d.child(icon("check", 14., c(TEXT)))))
                    .on_click(cx.listener(move |r, _, window, cx| {
                        r.acc_drop = None;
                        f(r, window, cx);
                        cx.notify();
                    })),
            );
        }
        wrap = wrap.child(popup(PopAt::BelowRight(TOOL_H + 4.), SharedString::from(format!("{key}-pop")), open, menu));
    }
    wrap
}

/// A window in trouble: dropped, held by Roblox's bot check, signed out in its window. It stands on a red base, its
/// state in words on it.
fn trouble(a: &Value) -> bool {
    !foreign(a) && (is_kicked(a) || held(a) || (is_local(a) && conn(a) == "signedout"))
}

/// One game's windows as a block on the page's grid, `span` columns wide: its head over its own tiles (the game's icon
/// on the game's own colour, its name, how many windows and their RAM, and the buttons for all of them at its right
/// edge), then its windows, each tile standing on the game's colour.
#[allow(clippy::too_many_arguments)]
fn lane(root: &Root, i: usize, l: Lane, col_w: f32, span: usize, lines: usize, multi: bool, compact: bool, cx: &mut Context<Root>) -> impl IntoElement {
    // As wide as its columns; one column wide, its buttons are icons (their words the tips).
    let w = span as f32 * col_w + (span as f32 - 1.) * GRID_GAP;
    let tight = span < 2;
    let n = l.items.len();
    // Bloxkeep's accounts by id; the other windows by their handle (closed one by one, as the Game monitor did).
    let ids: Vec<String> = l.items.iter().filter(|a| !foreign(a)).map(id_of).collect();
    let strays: Vec<String> = l.items.iter().filter(|a| foreign(a)).map(|a| s(a, "hwnd").to_string()).collect();
    let dropped: Vec<Value> = l.items.iter().filter(|a| is_kicked(a) && !foreign(a)).cloned().collect();
    let any_hidden = l.items.iter().any(hidden);
    let gicon = l.icon.as_deref().and_then(|u| images::pic_sq(u, GROUP_PIC, cx));
    // The game's own colour, from its icon: the base its icon and its windows' tiles stand on.
    let game_base = l.icon.as_deref().and_then(|u| images::tone(u, cx)).map(|t| base_of(c(t))).unwrap_or_else(base_dark);
    let mut meta = vec![(root.i18n.tn("mon.windows", n as i64, &[]), None)];
    if l.ram > 0. {
        meta.push((mb_text(l.ram), None));
    }
    if l.kicked > 0 {
        meta.push((root.i18n.tn("mon.nBad", l.kicked as i64, &[]), Some(c(WARN))));
    }
    // How its windows are doing (the old monitor's lane head): still joining, playing.
    let how = |a: &Value| if foreign(a) { window_of(root, a).map(|w| s(w, "status").to_string()).unwrap_or_default() } else { conn(a).to_string() };
    let joining = l.items.iter().filter(|a| how(a) == "joining").count();
    let playing = l.items.iter().filter(|a| how(a) == "ingame").count();
    if joining > 0 {
        meta.push((root.i18n.tn("mon.nJoin", joining as i64, &[]), Some(c(WARN))));
    }
    if playing > 0 && playing < n {
        meta.push((root.i18n.tn("mon.nOk", playing as i64, &[]), Some(c(OK))));
    }
    // A letter for each server two or more of its windows share (MonitorView.tsx's lanes()).
    let mut seen: Vec<(String, usize)> = Vec::new();
    for a in &l.items {
        if let Some(j) = job_of(root, a) {
            match seen.iter_mut().find(|(k, _)| *k == j) {
                Some(e) => e.1 += 1,
                None => seen.push((j, 1)),
            }
        }
    }
    let letters: Vec<(String, char)> = seen.into_iter().filter(|(_, n)| *n > 1).enumerate().map(|(k, (j, _))| (j, (b'A' + (k % 26) as u8) as char)).collect();
    let close_ids = ids.clone();
    let win_ids = ids.clone();
    let win_btn = (!win_ids.is_empty()).then(|| {
        let label = root.tr(if any_hidden { "card.showWindow" } else { "ui.hideWin" });
        let mark = if any_hidden { "eye" } else { "eyeOff" };
        let act = cx.listener(move |r: &mut Root, _: &ClickEvent, _, _| r.send(if any_hidden { "showWindows" } else { "hideWindows" }, json!({ "ids": win_ids })));
        // (An icon on every lane, its words the tip: lanes side by side look the same whatever their width.)
        IconButton::new(("lane-win", i), mark).size(30.).tip(label).on_click(act).into_any_element()
    });
    let close_btn = {
        let label = format!("{} · {n}", root.tr("card.close"));
        // Two windows or more: asked first, as Close all is (its "Don't ask again" covers both), so one stray click
        // doesn't end a farm (critique 2026-11-13, P1).
        let game = crate::pages::home::clean(&l.name);
        let act = cx.listener(move |r: &mut Root, _: &ClickEvent, window, cx| {
            let (ids, hs) = (close_ids.clone(), strays.clone());
            let go = move |r: &mut Root, _: &mut Context<Root>| {
                if !ids.is_empty() {
                    r.send("closeAccounts", json!({ "ids": ids, "force": false }));
                }
                for h in &hs {
                    r.send("monitorAct", json!({ "id": h, "what": "close" }));
                }
            };
            if n < 2 {
                return go(r, cx);
            }
            let title = if game.is_empty() { r.tr("t.closeAllTitle") } else { format!("{} {game}?", r.tr("card.close")) };
            let text = r.i18n.tv("t.closeAllN", &[("n", &n.to_string())]);
            r.confirm(title, text, r.tr("card.close"), true, Some("closeAll"), window, cx, go);
        });
        // (Labelled: it is the lane's main button. A one-column lane short of room keeps its icon, its words the tip,
        // so the game's name isn't cut to "Brookhaven…", review 2.5.)
        if tight && col_w < 400. {
            IconButton::new(("lane-close", i), "power").size(30.).tip(label).on_click(act).into_any_element()
        } else {
            Button::new(("lane-close", i), label).icon("power").small().on_click(act).into_any_element()
        }
    };
    let picture = on_base(
        game_base,
        3.,
        R_M + 1.,
        div().size(px(GROUP_PIC)).rounded(px(R_M)).overflow_hidden().bg(c(RAISED_2)).flex().items_center().justify_center().map(|d| match gicon {
            Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
            None => d.child(icon("gamepad", 20., c(MUTED))),
        }),
    );
    // The old Game monitor's buttons for a game's windows (MonitorView.tsx): to the front, to the back, minimised or
    // open again, this game alone, folded to its head.
    let key = l.name.clone();
    let folded = root.mon_fold.contains(&key);
    let solo = root.mon_solo.as_deref() == Some(key.as_str());
    let wins: Vec<(String, bool)> = l
        .items
        .iter()
        .filter_map(|a| window_of(root, a))
        .map(|w| (s(w, "id").to_string(), w.get("minimized").and_then(Value::as_bool).unwrap_or(false)))
        .filter(|(h, _)| !h.is_empty())
        .collect();
    let any_up = wins.iter().any(|(_, m)| !m);
    let tool = |k: &'static str, mark: &'static str, tip: String| IconButton::new((k, i), mark).size(30.).tip(tip);
    let mut win_tools: Vec<AnyElement> = Vec::new();
    // The same buttons folded or not, whenever a window of the game is open: with one of two windows still joining
    // (no window yet), they showed only once the game was folded (the owner, 2026-11-17).
    if !wins.is_empty() {
        let hs: Vec<String> = wins.iter().map(|(h, _)| h.clone()).collect();
        let (front, back) = (hs.clone(), hs);
        win_tools.push(tool("lane-front", "winFront", root.tr("mon.frontAll")).on_click(cx.listener(move |r, _, _, _| r.send("monitorLane", json!({ "ids": front, "what": "front" })))).into_any_element());
        win_tools.push(tool("lane-back", "winBack", root.tr("mon.backAll")).on_click(cx.listener(move |r, _, _, _| r.send("monitorLane", json!({ "ids": back, "what": "back" })))).into_any_element());
        let flip: Vec<String> = wins.iter().filter(|(_, m)| *m != any_up).map(|(h, _)| h.clone()).collect();
        let what = if any_up { "minimize" } else { "restore" };
        win_tools.push(
            tool("lane-min", if any_up { "winMin" } else { "winRestore" }, root.tr(if any_up { "mon.minAll" } else { "mon.restoreAll" }))
                .on_click(cx.listener(move |r, _, _, _| {
                    for h in &flip {
                        r.send("monitorAct", json!({ "id": h, "what": what }));
                    }
                }))
                .into_any_element(),
        );
    }
    if multi || solo {
        let k = key.clone();
        win_tools.push(
            if solo {
                Button::new(("lane-solo", i), root.tr("mon.allGames")).icon("chevronLeft").small().quiet().on_click(cx.listener(move |r, _, _, cx| {
                    r.mon_solo = None;
                    cx.notify();
                })).into_any_element()
            } else {
                tool("lane-solo", "focus", root.tr("mon.solo")).on_click(cx.listener(move |r, _, _, cx| {
                    r.mon_solo = Some(k.clone());
                    cx.notify();
                })).into_any_element()
            },
        );
    }
    let fold = {
        let k = key.clone();
        tool("lane-fold", if folded { "chevronRight" } else { "chevronDown" }, root.tr(if folded { "mon.unfold" } else { "mon.fold" })).on_click(cx.listener(move |r, _, _, cx| {
            if !r.mon_fold.remove(&k) {
                r.mon_fold.insert(k.clone());
            }
            cx.notify();
        }))
    };
    let head = group_head(picture, l.name.clone(), facts(meta)).child(
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(if tight { 2. } else { 8. }))
            // One cut off has its own Rejoin on its tile; this is for several at once.
            .when(dropped.len() > 1, |d| {
                let label = root.i18n.tv("ui.rejoinN", &[("n", &dropped.len().to_string())]);
                d.child(Button::new(("lane-rejoin", i), label).icon("refresh").tint().small().on_click(cx.listener(move |r, _, _, cx| r.rejoin(&dropped, cx))))
            })
            .child(div().flex().items_center().gap(px(2.)).children(win_tools).children(win_btn))
            .when(n > 0, |d| d.child(close_btn))
            .child(fold),
    );
    if folded {
        return div().w(px(w)).flex_none().flex().flex_col().child(head);
    }
    let tiles: Vec<AnyElement> = l
        .items
        .iter()
        .map(|a| {
            let base = if root.selected.contains(&id_of(a)) {
                c(FACE_BASE)
            } else if trouble(a) {
                bad_base()
            } else {
                game_base
            };
            let srv = job_of(root, a).and_then(|j| letters.iter().find(|(k, _)| *k == j).map(|(_, ch)| *ch));
            on_base(base, RISE, R_CARD + 1., tile(root, a, col_w, lines, srv, compact, cx)).w(px(col_w)).into_any_element()
        })
        .collect();
    div().w(px(w)).flex_none().flex().flex_col().gap(px(16.)).child(head).child(grid(GRID_GAP).pb(px(RISE)).children(tiles))
}

thread_local! {
    /// The pictures of hidden and minimized windows drawn last, by window: the core's serial, the picture, when asked.
    static STILLS: std::cell::RefCell<std::collections::HashMap<isize, (u64, std::sync::Arc<RenderImage>, std::time::Instant)>> = Default::default();
}

/// Window `h`'s latest picture while it is hidden or minimized (asking keeps it pictured); None until the first.
#[cfg(feature = "core")]
fn still_of(h: &str, cx: &mut Context<Root>) -> Option<std::sync::Arc<RenderImage>> {
    let h: isize = h.parse().ok()?;
    let now = std::time::Instant::now();
    let fresh = crate::core::hidden_stills::want(h);
    STILLS.with(|m| {
        let mut m = m.borrow_mut();
        // (Windows no tile asked for lately: their pictures off the graphics card.)
        m.retain(|k, (_, p, at)| {
            let keep = *k == h || now.duration_since(*at).as_secs() < 10;
            if !keep {
                cx.drop_image(p.clone(), None);
            }
            keep
        });
        let fresh = fresh?;
        let made = m.get(&h).filter(|(serial, p, _)| *serial == fresh.serial && !images::made_lost(p)).map(|(_, p, _)| p.clone());
        let pic = match made {
            Some(p) => p,
            None => {
                let (w, h2) = (fresh.picture.width as u32, fresh.picture.height as u32);
                // (Picture's pixels are 0xAARRGGBB: as bytes blue, green, red, alpha, what GPUI keeps.)
                let bytes: Vec<u8> = fresh.picture.pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
                let buffer = image::RgbaImage::from_raw(w, h2, bytes)?;
                let p = images::made("window still", RenderImage::new(vec![image::Frame::new(buffer)]));
                if let Some((_, old, _)) = m.insert(h, (fresh.serial, p.clone(), now)) {
                    cx.drop_image(old, None);
                }
                p
            }
        };
        if let Some(e) = m.get_mut(&h) {
            e.2 = now;
        }
        Some(pic)
    })
}

/// A shot: the picture in BLOXKEEP_SHOT_STILL for every hidden or minimized window.
#[cfg(not(feature = "core"))]
fn still_of(_h: &str, _cx: &mut Context<Root>) -> Option<std::sync::Arc<RenderImage>> {
    thread_local! {
        static SHOT: Option<std::sync::Arc<RenderImage>> = std::env::var("BLOXKEEP_SHOT_STILL").ok().and_then(|f| {
            let mut rgba = image::open(f).ok()?.into_rgba8();
            // (GPUI keeps blue, green, red, alpha.)
            for p in rgba.pixels_mut() {
                p.0.swap(0, 2);
            }
            Some(std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
        });
    }
    SHOT.with(Clone::clone)
}

/// The lines of facts a tile has under its state: its chips and Robux, its note.
fn tile_lines(root: &Root, a: &Value) -> usize {
    if foreign(a) {
        return 0;
    }
    let chips = !tags_of(root, a).is_empty() || old_build(a) || fps_limit(a).is_some() || robux_of(a).is_some();
    chips as usize + note_of(a).is_some() as usize
}

/// `lines`: the most lines of facts a tile on the page has (`tile_lines`), so all of them are one height.
/// The server a window is in (Roblox's job id), from its log for an account, from the window list for another window.
fn job_of(root: &Root, a: &Value) -> Option<String> {
    let j = if foreign(a) { window_of(root, a).and_then(|w| w.get("jobId")).and_then(Value::as_str) } else { a.pointer("/session/jobId").and_then(Value::as_str) };
    j.filter(|j| !j.is_empty()).map(str::to_string)
}

/// Server colours for the letters, as app.css's .mon-srv s0–s3.
const SRV_TONES: [u32; 4] = [0x60a5fa, 0xf472b6, 0x34d399, 0xfbbf24];

/// One Roblox window. Its picture first, across the top at 16:9: the live one the app lays there (Windows draws it
/// over this window, so nothing of ours goes on it), for a hidden or minimised window its picture taken every
/// half minute, else the game's own while there's none (opening, or not sent yet). Under it the face, name and time on one line, then a line of facts: the state's dot and
/// words at the left, RAM and PID at the right; its chips and note, today's strip, then its main button (Close, or the
/// way back in: Rejoin, Verify), Edit and its menu at the foot. A click brings the window to the
/// front, as the old Game monitor did; the round tick (or Ctrl+click) picks the account for the bar.
/// `compact`: one of many windows (MON_MANY): the picture, the name, the state, the RAM and Close with the menu; the
/// handle, playing time, PID, chips, note, day strip, Main and Edit go (Edit stays in the menu).
fn tile(root: &Root, a: &Value, w: f32, lines: usize, srv: Option<char>, compact: bool, cx: &mut Context<Root>) -> impl IntoElement {
    const INSET_PAD: f32 = 6.;
    let lines = if compact { 0 } else { lines };
    want_strips(root, cx);
    let id = id_of(a);
    let other = foreign(a);
    let on = root.selected.contains(&id);
    let bad = trouble(a);
    let (state, tone, _) = state_of(root, a);
    let win = window_of(root, a);
    let hwnd = win.map(|w| s(w, "id").to_string()).filter(|h| !h.is_empty());
    let minimized = win.and_then(|w| w.get("minimized")).and_then(Value::as_bool).unwrap_or(false);
    let live = hwnd.clone().filter(|_| !minimized && !hidden(a));
    // A hidden or minimized window: its picture taken every half minute (hidden_stills.rs), instead of the game's only.
    let still = (minimized || hidden(a)).then(|| still_of(hwnd.as_deref().unwrap_or_default(), cx)).flatten();
    // (The tile's hairline is inside its width.)
    let pic_w = w - MON_PIC_INSET;
    let h = (pic_w * 9. / 16.).floor();
    // No game picture for a window back on the menu: its session still names the game it left.
    let cover = cover_of(root, if in_menu(a) { None } else { game_place(a) }).and_then(|u| images::pic_at(&u, pic_w, h, cx));
    // Its main button at the foot, as a not-open card has Play there: Close, or the way back in (Rejoin, Verify) when
    // dropped or held on Roblox's bot check; then Edit and its menu (review 2.2: a playing tile had no Edit).
    let main = action(root, a, "tile", cx);
    let ram = ram_mb(root, a).map(|m| mb_text(m));
    let pid = (!compact).then(|| pid_el(root, a, cx)).flatten();
    // A pared-down tile keeps its PID too, in the room beside Close (farms tell windows apart by it).
    let pid_foot = compact.then(|| pid_el(root, a, cx)).flatten();
    let both = ram.is_some() && pid.is_some();
    let old = old_build(a).then(|| old_tag(root, a, cx));
    let fps = fps_limit(a);
    // Who it is and what it holds (a window not opened by Bloxkeep is no account: none of these).
    let under = (!other && !compact).then(|| handle_of(a)).flatten();
    let tags = if other { Vec::new() } else { tags_of(root, a) };
    let robux = (!other).then(|| robux_of(a).map(|r| robux_el(root, &id, r))).flatten();
    let note = (!other).then(|| note_of(a)).flatten();
    // Over the game's picture while there is no live one: why not.
    let why = if hidden(a) {
        Some(("eyeOff", root.tr("ui.winHidden")))
    } else if minimized {
        Some(("screens", root.tr("mon.minimized")))
    } else {
        None
    };
    let pick = id.clone();
    let tick_id = id.clone();
    let front = hwnd.clone();
    // Minimise it or open it again, beside Edit (the old monitor had it on the picture, where Windows draws now).
    let min_btn = hwnd.clone().map(|h| {
        let what = if minimized { "restore" } else { "minimize" };
        no_pick(
            IconButton::new(SharedString::from(format!("tile-min-{id}")), if minimized { "winRestore" } else { "winMin" })
                .size(30.)
                .tip(root.tr(if minimized { "mon.restore" } else { "mon.minimize" }))
                .on_click(cx.listener(move |r, _, _, _| r.send("monitorAct", json!({ "id": h, "what": what })))),
        )
    });
    let group = SharedString::from(format!("tile-{id}"));
    hot_block(group.clone(), R_CARD)
        .lift(3.)
        .group(group.clone())
        .child(spot(root, &id, R_CARD))
        .w(px(w))
        .p(px(INSET_PAD))
        .flex()
        .flex_col()
        .rounded(px(R_CARD))
        .cursor_pointer()
        .map(|d| {
            if on {
                // (The plain ground and the yellow ring: yellow mixed into the ground read olive, the owner's rule
                // of 2026-11-15 against tinted grounds.)
                d.tones(c(RAISED), c(RAISED_2)).edge(c(FACE), c(FACE))
            } else if bad {
                d.tones(tinted(RAISED, BAD, 0.09), tinted(RAISED_2, BAD, 0.11)).edge(ca(BAD, 0.38), ca(BAD, 0.52))
            } else if crate::theme::glass_blocks() {
                // (A glass tile keeps the glass's own light edge.)
                d.tones(c(RAISED), c(RAISED_2))
            } else {
                d.tones(c(RAISED), c(RAISED_2)).edge(ca(WHITE, 0.), ca(WHITE, 0.17))
            }
        })
        .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
            if (e.modifiers().control || e.modifiers().shift) && !other {
                toggle(r, &pick, e.modifiers().shift, cx);
                cx.notify();
            } else if let Some(h) = &front {
                r.send("monitorAct", json!({ "id": h, "what": "show" }));
            } else if !other {
                toggle(r, &pick, false, cx);
                cx.notify();
            }
        }))
        .when(!other, |d| d.on_mouse_down(MouseButton::Right, on_right(a, cx)).children(menu::place(root, a, false, cx)))
        .child(
            div()
                .relative()
                .w_full()
                .h(px(h))
                .rounded(px(R_CARD - INSET_PAD + 1.))
                .overflow_hidden()
                .bg(ca(WHITE, 0.05))
                .when_some(cover, |d, p| d.child(div().absolute().inset_0().child(img(p).size_full().object_fit(ObjectFit::Cover))))
                .when_some(still.clone(), |d, p| d.child(div().absolute().inset_0().child(img(ImageSource::Render(p)).size_full().object_fit(ObjectFit::Cover))))
                .when(still.is_none() && (why.is_some() || launching(a) || conn(a) == "menu"), |d| d.child(div().absolute().inset_0().bg(ca(GROUND, 0.6))))
                // (Over its picture, why it isn't live: a small label in the corner.)
                .when_some(why.clone().filter(|_| still.is_some()), |d, (glyph, text)| {
                    d.child(
                        div()
                            .absolute()
                            .left(px(8.))
                            .bottom(px(8.))
                            .max_w(px(pic_w - 16.))
                            .px(px(8.))
                            .py(px(3.))
                            .rounded(px(R_M))
                            .bg(ca(GROUND, 0.78))
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .text_size(px(T_S))
                            .text_color(c(TEXT_2))
                            .child(icon(glyph, 13., c(TEXT_2)))
                            .child(div().truncate().child(text)),
                    )
                })
                .when_some(why.filter(|_| still.is_none()), |d, (glyph, text)| {
                    d.child(
                        div()
                            .absolute()
                            .inset_0()
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6.))
                            .text_size(px(T_M))
                            .text_color(c(TEXT_2))
                            .child(icon(glyph, 15., c(TEXT_2)))
                            .child(div().truncate().child(text)),
                    )
                })
                .when_some(live, |d, h| d.child(root.live.picture(h))),
        )
        .child(
            div()
                .px(px(if compact { 4. } else { 8. }))
                .pt(px(if compact { 10. } else { 13. }))
                .pb(px(if compact { 2. } else { 6. }))
                .flex()
                .flex_col()
                .gap(px(if compact { 8. } else { 10. }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(if compact { 8. } else { 10. }))
                        .child(avatar(a, if compact { 28. } else { 36. }, RAISED, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(div().min_w_0().text_size(px(if compact { T_M } else { T_L })).line_height(px(20.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).truncate().child(label(a))),
                                )
                                .when_some(under, |d, u| d.child(div().min_w_0().truncate().text_size(px(T_S)).line_height(px(16.)).text_color(c(MUTED)).child(u))),
                        )
                        .when_some(up_of(a).filter(|_| !compact), |d, t| {
                            let tip: SharedString = root.tr("card.upTip").into();
                            d.child(div().id(SharedString::from(format!("up-{}", id_of(a)))).flex_none().text_size(px(T_S)).text_color(c(TEXT_2)).px(px(7.)).py(px(4.)).rounded(px(R_S)).bg(c(INSET)).font_features(tnum()).child(t).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
                        })
                        // The round tick: picks the account for the bar below (a window not opened by Bloxkeep has none).
                        .when(!other, |d| {
                            d.child(
                                div()
                                    .id(SharedString::from(format!("tick-{id}")))
                                    .size(px(20.))
                                    .flex_none()
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .border_1()
                                    .map(|d| {
                                        if on {
                                            d.bg(c(FACE)).border_color(c(FACE)).child(icon("check", 13., c(INK)))
                                        } else {
                                            d.border_color(ca(WHITE, 0.18)).group_hover(group.clone(), |s| s.border_color(ca(WHITE, 0.45)))
                                        }
                                    })
                                    .tooltip({
                                        let t: SharedString = root.tr("ui.pickOne").into();
                                        move |_, cx| cx.new(|_| Tip(t.clone())).into()
                                    })
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
                                        toggle(r, &tick_id, e.modifiers().shift, cx);
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                // The facts: the state at the left, RAM and PID at the right in tabular figures.
                .child(
                    div()
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(T_M))
                        .child(div().min_w_0().flex().items_center().gap(px(6.)).h(px(25.)).px(px(8.)).rounded(px(R_S)).bg(c(INSET))
                            .child(div().size(px(6.)).flex_none().rounded_full().bg(c(tone)))
                            .child(div().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).text_color(if tone == TEXT_2 { c(TEXT) } else { c(tone) }).child(state)))
                        .child(div().flex_1())
                        // Windows with the same letter are in the same server.
                        .when_some(srv, |d, ch| {
                            let tip: SharedString = root.tr("mon.serverTip").into();
                            let tone = SRV_TONES[((ch as u8 - b'A') % 4) as usize];
                            d.child(
                                div()
                                    .id(SharedString::from(format!("srv-{id}")))
                                    .flex_none()
                                    .child(tag(root.i18n.tv("mon.server", &[("s", &ch.to_string())]), tone))
                                    .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
                            )
                        })
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .gap(px(6.))
                                .text_color(c(TEXT_2))
                                .font_features(tnum())
                                .when_some(ram, |d, r| {
                                    // "RAM in use", with the FPS limit it was opened with when there is one (Accounts.tsx).
                                    let tip: SharedString = match fps {
                                        Some(f) => format!("{} · {f} FPS. {}", root.tr("card.ramTip"), root.tr("card.fpsTip")),
                                        None => root.tr("card.ramTip"),
                                    }
                                    .into();
                                    d.child(div().id(SharedString::from(format!("tile-ram-{id}"))).child(r).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
                                })
                                .when(both, |d| d.child(sep()))
                                .children(pid),
                        ),
                )
                // Main and its group, an older Roblox than the newest, the FPS limit it was opened with, and its Robux
                // at the right: a line of their own, so the state and the RAM keep their room.
                // (Held at the height of the fullest tile's lines, so tiles in a row end together.)
                .when(lines > 0, |d| {
                    let tip: SharedString = root.tr("card.fpsTip").into();
                    let chips = !tags.is_empty() || old.is_some() || fps.is_some() || robux.is_some();
                    d.child(
                        div()
                            .h(px(lines as f32 * 20. + (lines as f32 - 1.) * 10.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .when(chips, |d| {
                                d.child(
                                    div()
                                        .h(px(20.))
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .children(tags)
                                        .children(old)
                                        .when_some(fps, |d, f| {
                                            d.child(div().id(SharedString::from(format!("fps-{id}"))).flex_none().child(tag(format!("{f} FPS"), TEXT_2)).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
                                        })
                                        .child(div().flex_1())
                                        .children(robux),
                                )
                            })
                            .when_some(note, |d, n| d.child(small_line("message", n).h(px(20.)))),
                    )
                })
                // Today, as a strip (a window not opened by Bloxkeep: its room, empty).
                .when(!compact, |d| d.child(if other { div().h(px(12.)).into_any_element() } else { day_strip(root, &id).into_any_element() }))
                // Its main button, Edit, its menu (also a right click); a window not opened by Bloxkeep keeps the room.
                .child(
                    div()
                        .h(px(36.)).pt(px(6.)).border_t_1().border_color(c(LINE))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .children(main.map(no_pick))
                        .child(div().flex_1().min_w_0().overflow_hidden().pl(px(6.)).text_size(px(T_S)).children(pid_foot))
                        .children(min_btn)
                        .when(!other && !compact, |d| d.child(main_button(root, a, "tile", 30., cx)).child(edit_button(root, a, "tile", 30., cx)))
                        .when(!other, |d| d.child(more_button(root, a, "tile", 30., cx)))
                        // A window Bloxkeep didn't open has no account menu: its Close here (MonitorView.tsx's tile menu).
                        .when_some(hwnd.clone().filter(|_| other), |d, h| {
                            d.child(no_pick(
                                IconButton::new(SharedString::from(format!("tile-close-{id}")), "power")
                                    .size(30.)
                                    .tip(root.tr("mon.close"))
                                    .on_click(cx.listener(move |r, _, _, _| r.send("monitorAct", json!({ "id": h, "what": "close" })))),
                            ))
                        }),
                ),
        )
}

/// A command for the picked accounts; the app's refusal is said as a notice.
fn act(r: &mut Root, cmd: &'static str, args: Value, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Value) + 'static) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call(cmd, args);
    answer(rx, cx, move |r, answer, cx| {
        match answer {
            Ok(Ok(v)) => done(r, v),
            Ok(Err(e)) => r.toast = Some((e.to_string(), "error".into())),
            Err(_) => {}
        }
        cx.notify();
    });
}

/// Launch order (app.js's launchOrderModal): every account, moved up or down into the order Play opens them in; Done
/// hands the order to the app ("setLaunchOrder"). The web page's list could also be dragged.
pub fn open_order(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    use gpui_kit::component::WindowExt as _;
    use gpui_kit::component::dialog::DialogEntrance;
    let order: Vec<String> = r.state.pointer("/settings/launchOrder").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let mut ids: Vec<String> = r.accounts().iter().map(id_of).collect();
    if ids.is_empty() {
        r.toast = Some((r.tr("rc.noAccounts"), "info".into()));
        cx.notify();
        return;
    }
    // (Accounts the order doesn't name yet go last, as they are.)
    ids.sort_by_key(|id| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
    r.order_edit = ids;
    let this = cx.weak_entity();
    window.open_dialog(cx, move |dialog, _, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        // Names and pictures first: a picture's load needs the app, which the read of Root holds.
        let (people, tips, words): (Vec<(String, String)>, [SharedString; 4], [SharedString; 4]) = {
            let r = root.read(cx);
            let people = r
                .order_edit
                .iter()
                .map(|id| r.accounts().iter().find(|a| s(a, "id") == id).map(|a| (label(a), s(a, "avatarUrl").to_string())).unwrap_or_default())
                .collect();
            (
                people,
                [r.tr("lo.up").into(), r.tr("lo.down").into(), r.tr("common.cancel").into(), r.tr("common.done").into()],
                [r.tr("lo.title").into(), r.tr("lo.text").into(), r.tr("common.close").into(), SharedString::default()],
            )
        };
        let count = people.len();
        let mut rows = div().flex().flex_col();
        for (i, (name, url)) in people.iter().enumerate() {
            let step = |to: usize, key: &'static str, icon_name: &'static str, off: bool| {
                let this = this.clone();
                IconButton::new((key, i), icon_name).size(26.).disabled(off).tip(if to < i { tips[0].clone() } else { tips[1].clone() }).on_click(move |_, w, cx| {
                    let _ = this.update(cx, |r, _| {
                        if i < r.order_edit.len() && to < r.order_edit.len() {
                            r.order_edit.swap(i, to);
                        }
                    });
                    w.refresh();
                })
            };
            let pic = crate::images::pic_sq(url, 28., cx);
            let face = match pic {
                Some(src) => div().size(px(28.)).flex_none().rounded(px(8.)).overflow_hidden().bg(c(RAISED_2)).child(img(src).size_full()),
                None => crate::kit::face(name, None, 28.),
            };
            rows = rows.child(
                div()
                    .h(px(46.))
                    .px(px(10.))
                    .when(i > 0, |d| d.border_t_1().border_color(c(LINE)))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(crate::kit::grip())
                    .child(face)
                    .child(div().min_w_0().flex_1().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(name.clone()))
                    .child(step(i.saturating_sub(1), "lo-up", "chevronUp", i == 0))
                    .child(step((i + 1).min(count - 1), "lo-down", "chevronDown", i + 1 >= count)),
            );
        }
        let done = this.clone();
        let list = crate::kit::scroll_list("lo-list", &crate::scrolled::handle("lo-list"), 420., rows);
        crate::kit::framed(dialog, words[0].clone(), words[2].clone(), |_, w, cx| w.close_dialog(cx))
            .w(px(crate::kit::DIALOG_W))
            .centered()
            .overlay_closable(false)
            .entrance(DialogEntrance::FadeSlide)
            .child(crate::kit::dialog_body(div().flex().flex_col().gap(px(12.)).child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(words[1].clone())).child(list)))
            .footer(
                crate::kit::dialog_foot()
                    .child(Button::new("lo-cancel", tips[2].clone()).on_click(|_, w, cx| w.close_dialog(cx)))
                    .child(Button::new("lo-done", tips[3].clone()).primary().on_click(move |_, w, cx| {
                        w.close_dialog(cx);
                        let _ = done.update(cx, |r, cx| {
                            let ids = r.order_edit.clone();
                            act(r, "setLaunchOrder", json!({ "ids": ids }), cx, |_, _| {});
                        });
                    })),
            )
    });
}

/// Every Roblox window laid out on the main screen, as Settings › Launching says, and how many were (app.js's arrange).
pub fn arrange(r: &mut Root, cx: &mut Context<Root>) {
    let monitor = r.state.get("monitors").and_then(Value::as_array).and_then(|l| l.iter().find(|m| m.get("primary").and_then(Value::as_bool) == Some(true))).and_then(|m| m.get("index")).and_then(Value::as_i64).unwrap_or(0);
    act(r, "arrangeWindows", json!({ "monitor": monitor }), cx, |r, v| {
        let n = v.as_i64().unwrap_or(0);
        r.toast = Some(if n > 0 { (r.i18n.tn("t.arranged", n, &[("n", &n.to_string())]), "ok".into()) } else { (r.tr("t.noWindows"), "info".into()) });
    });
}

/// Close these accounts' windows (or end them, forced), and say how many (app.js's closeIds).
fn close(r: &mut Root, ids: Vec<String>, force: bool, cx: &mut Context<Root>) {
    act(r, "closeAccounts", json!({ "ids": ids, "force": force }), cx, move |r, v| {
        let n = v.as_i64().unwrap_or(0);
        r.toast = Some(if n > 0 { (r.i18n.tn(if force { "t.killing" } else { "t.closing" }, n, &[("n", &n.to_string())]), "ok".into()) } else { (r.tr("t.noneRunning"), "info".into()) });
    });
}

/// Copies one thing of an account's (the app puts it on the clipboard) and says what; a 2FA code says how long it
/// still works (app.js's copy).
pub fn copy(r: &mut Root, id: String, field: &'static str, cx: &mut Context<Root>) {
    act(r, "copy", json!({ "id": id, "field": field }), cx, move |r, v| {
        if v.is_null() {
            return;
        }
        let text = if field == "totp" {
            r.i18n.tv("t.copiedCode", &[("s", &v.get("seconds").and_then(Value::as_i64).unwrap_or(0).to_string())])
        } else if v.get("secret").and_then(Value::as_bool).unwrap_or(false) {
            r.tr("t.copiedSecret")
        } else {
            r.tr(match field {
                "username" => "t.copiedUser",
                "pid" => "t.copiedPid",
                _ => "t.copied",
            })
        };
        r.toast = Some((text, "ok".into()));
    });
}

/// Asks Roblox whether these accounts are still signed in, and says how many are, how many need signing in again, how
/// many couldn't be asked; the ones Roblox holds back (a warning to accept) are said apart (app.js's checkAccounts).
fn check_accounts(r: &mut Root, ids: Vec<String>, cx: &mut Context<Root>) {
    act(r, "checkAccounts", json!({ "ids": ids }), cx, |r, v| {
        if !v.is_object() {
            return;
        }
        let n = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
        let (ok, bad, failed, refused) = (n("ok"), n("bad"), n("failed"), n("refused"));
        let mut text = r.i18n.tn(if failed > 0 { "t.checkedFail" } else { "t.checked" }, bad, &[("ok", &ok.to_string()), ("bad", &bad.to_string()), ("f", &failed.to_string())]);
        if refused > 0 {
            let key = if n("moderated") == refused { "t.checkedModerated" } else { "t.checkedRefused" };
            text = format!("{text} {}", r.i18n.tn(key, refused, &[("n", &refused.to_string())]));
        }
        r.toast = Some((text, if bad > 0 || refused > 0 { "warn" } else { "ok" }.into()));
    });
}

/// More for the picked accounts, as the web page's account menu: check their sign-ins, main account, hide or show
/// their windows, another server, roblox.com and Copy for one account, force close, and Delete (after a question).
fn more(root: &Root, picked: &[Value], cx: &mut Context<Root>) -> Div {
    let open = root.acc_menu;
    let ids: Vec<String> = picked.iter().filter(|a| !foreign(a)).map(id_of).collect();
    let local: Vec<String> = picked.iter().filter(|a| !foreign(a) && is_local(a)).map(id_of).collect();
    let one = (ids.len() == 1).then(|| picked.iter().find(|a| !foreign(a)).cloned()).flatten();
    let flag = |a: &Value, k: &str| a.get(k).and_then(Value::as_bool).unwrap_or(false);
    let all_main = !ids.is_empty() && picked.iter().filter(|a| !foreign(a)).all(|a| flag(a, "isMain"));
    let any_hidden = picked.iter().any(hidden);
    let row = |key: &'static str, icon_name: &'static str, label: String, danger: bool| {
        menu_row(key)
            .h(px(32.))
            .gap(px(10.))
            .text_color(c(if danger { BAD } else { TEXT }))
            .child(icon(icon_name, 14., c(if danger { BAD } else { TEXT_2 })))
            .child(div().flex_1().min_w_0().truncate().child(label))
    };
    let line = || div().h(px(1.)).my(px(4.)).mx(px(6.)).bg(c(LINE));
    let mut menu = div()
        .id("acc-more-menu")
        .occlude()
        .w(px(250.))
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED_2))
        .border_1()
        .border_color(ca(WHITE, 0.1))
        .shadow_lg()
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            if r.acc_menu {
                r.acc_menu = false;
                cx.notify();
            }
        }));
    {
        let ids = ids.clone();
        menu = menu.child(row("am-check", "refresh", root.tr("m.check"), false).on_click(cx.listener(move |r, _, _, cx| {
            r.acc_menu = false;
            check_accounts(r, ids.clone(), cx);
            cx.notify();
        })));
    }
    // Who sees them online and who may join them: their own Roblox privacy settings.
    {
        let ids = ids.clone();
        menu = menu.child(row("am-vis", "eye", root.tr("vis.menu"), false).on_click(cx.listener(move |r, _, window, cx| {
            r.acc_menu = false;
            crate::pages::visibility::open(r, ids.clone(), window, cx);
        })));
    }
    {
        let ids = ids.clone();
        menu = menu.child(
            row("am-main", if all_main { "starFill" } else { "star" }, root.tr("m.main"), false)
                .when(all_main, |d| d.child(icon("check", 14., c(TEXT))))
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.acc_menu = false;
                    r.send("setMain", json!({ "ids": ids, "on": !all_main }));
                    cx.notify();
                })),
        );
    }
    {
        // Roblox's own app for these accounts, without a game.
        let ids = ids.clone();
        menu = menu.child(row("am-app", "monitor", root.tr("dock.openAppTip"), false).on_click(cx.listener(move |r, _, _, cx| {
            r.acc_menu = false;
            r.launch("launch", json!({ "ids": ids, "appOnly": true, "placeInput": "", "jobId": "" }), false, cx);
            cx.notify();
        })));
    }
    // Every Roblox window of this PC hidden (their games run on) or shown again, whichever can apply; and all of
    // them laid out on the main screen as Settings › Launching says.
    {
        let all = root.accounts();
        let can_hide = all.iter().any(|a| is_local(a) && !hidden(a));
        let can_show = all.iter().any(hidden);
        menu = menu.child(line());
        if can_hide {
            menu = menu.child(row("am-hide-all", "eyeOff", root.tr("m.hideAll"), false).on_click(cx.listener(|r, _, _, cx| {
                r.acc_menu = false;
                r.send("hideWindows", json!({}));
                cx.notify();
            })));
        }
        if can_show {
            menu = menu.child(row("am-show-all", "eye", root.tr("m.showAll"), false).on_click(cx.listener(|r, _, _, cx| {
                r.acc_menu = false;
                r.send("showWindows", json!({}));
                cx.notify();
            })));
        }
        let monitor = root.state.get("monitors").and_then(Value::as_array).and_then(|l| l.iter().find(|m| m.get("primary").and_then(Value::as_bool) == Some(true))).and_then(|m| m.get("index")).and_then(Value::as_i64).unwrap_or(0);
        menu = menu.child(row("am-arrange", "grid", root.tr("m.arrange"), false).on_click(cx.listener(move |r, _, _, cx| {
            r.acc_menu = false;
            act(r, "arrangeWindows", json!({ "monitor": monitor }), cx, |r, v| {
                let n = v.as_i64().unwrap_or(0);
                r.toast = Some(if n > 0 { (r.i18n.tn("t.arranged", n, &[("n", &n.to_string())]), "ok".into()) } else { (r.tr("t.noWindows"), "info".into()) });
            });
            cx.notify();
        })));
    }
    if !local.is_empty() {
        let (ids, ids2) = (local.clone(), local.clone());
        menu = menu
            .child(line())
            .child(row("am-hide", if any_hidden { "eye" } else { "eyeOff" }, root.tr(if any_hidden { "m.showWindow" } else { "m.hideWindow" }), false).on_click(cx.listener(move |r, _, _, cx| {
                r.acc_menu = false;
                r.send(if any_hidden { "showWindows" } else { "hideWindows" }, json!({ "ids": ids }));
                cx.notify();
            })))
            .child(row("am-switch", "shuffle", root.tr("m.switchServer"), false).on_click(cx.listener(move |r, _, _, cx| {
                r.acc_menu = false;
                r.send("switchServer", json!({ "ids": ids2 }));
                cx.notify();
            })));
    }
    if let Some(a) = one {
        let id = id_of(&a);
        let vng = s(&a, "realm") == "vng";
        let (has_pw, has_code) = (flag(&a, "hasPassword"), flag(&a, "hasAuthenticator"));
        menu = menu.child(line());
        {
            let id = id.clone();
            menu = menu.child(row("am-edit", "edit", root.tr("m.edit"), false).on_click(cx.listener(move |r, _, window, cx| {
                r.acc_menu = false;
                crate::pages::account_form::open(r, Some(id.clone()), window, cx);
                cx.notify();
            })));
        }
        if !vng {
            let id = id.clone();
            // roblox.com signed in as this account, in a window of Bloxkeep's own.
            menu = menu.child(row("am-web", "globe", root.tr("m.website"), false).on_click(cx.listener(move |r, _, _, cx| {
                r.acc_menu = false;
                r.send("openWebsite", json!({ "id": id }));
                cx.notify();
            })));
        }
        for (key, label, field, has, none) in [("am-copy-user", "m.copyUser", "username", true, ""), ("am-copy-pw", "m.copyPw", "password", has_pw, "m.noPw"), ("am-copy-code", "m.copyCode", "totp", has_code, "m.noCode")] {
            let id = id.clone();
            menu = menu.child(row(key, "copy", root.tr(label), false).on_click(cx.listener(move |r, _, _, cx| {
                r.acc_menu = false;
                // Nothing saved to copy: picking it says why (a sign-in never keeps the password).
                if has {
                    copy(r, id.clone(), field, cx);
                } else {
                    r.toast = Some((r.tr(none), "info".into()));
                }
                cx.notify();
            })));
        }
    }
    menu = menu.child(line());
    if !local.is_empty() {
        let ids = local.clone();
        menu = menu.child(row("am-kill", "x", root.tr("m.kill"), true).on_click(cx.listener(move |r, _, _, cx| {
            r.acc_menu = false;
            close(r, ids.clone(), true, cx);
            cx.notify();
        })));
    }
    {
        let n = ids.len() as i64;
        let names: Vec<String> = picked.iter().filter(|a| !foreign(a)).map(label).collect();
        let label = if n > 1 { root.i18n.tv("m.deleteN", &[("n", &n.to_string())]) } else { root.tr("m.delete") };
        menu = menu.child(row("am-delete", "trash", label, true).on_click(cx.listener(move |r, _, window, cx| {
            r.acc_menu = false;
            ask_delete(r, ids.clone(), names.clone(), window, cx);
            cx.notify();
        })));
    }
    div()
        .relative()
        .flex_none()
        .child(IconButton::new("bar-more", "more").size(32.).selected(open).tip(root.tr("dock.moreTip")).on_click(cx.listener(|r, _, _, cx| {
            r.acc_menu = !r.acc_menu;
            cx.notify();
        })))
        .child(popup(PopAt::AboveRight(38.), "acc-more-pop", open, menu))
}

/// Today's stays of every account (todayStrips: since local midnight), asked once a minute while cards show, and when.
/// (Shared: each card reads it, and a clone of the handle costs nothing next to one of the whole answer.)
static STRIPS: std::sync::Mutex<Option<(std::sync::Arc<Value>, std::time::Instant)>> = std::sync::Mutex::new(None);
static STRIPS_ASKED: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

fn want_strips(root: &Root, cx: &mut Context<Root>) {
    let Some(link) = root.link.clone() else { return };
    let mut asked = STRIPS_ASKED.hold();
    if asked.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(60)) {
        return;
    }
    *asked = Some(std::time::Instant::now());
    let rx = link.call("todayStrips", json!({}));
    cx.spawn(async move |_, cx| {
        if let Some(d) = rx.await.ok().and_then(Result::ok) {
            *STRIPS.hold() = Some((std::sync::Arc::new(d), std::time::Instant::now()));
            let _ = cx.update(|cx| cx.refresh_windows());
        }
    })
    .detach();
}

/// Today on an account's card, in words (the old DayStrip's thin bar read as noise): how long it played since
/// midnight and how often it dropped, only when it played at all ("Played 2 h 10 min today · 1 drop").
fn day_line(root: &Root, id: &str) -> Option<String> {
    let (d, _) = STRIPS.hold().clone()?;
    let stays: Vec<(f64, bool)> = d
        .pointer(&format!("/rows/{id}"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let s = s.as_array()?;
            Some(((s.get(1)?.as_f64()? - s.first()?.as_f64()?).max(0.), s.get(2).and_then(Value::as_i64).unwrap_or(0) == 1))
        })
        .collect();
    let played: f64 = stays.iter().map(|s| s.0).sum();
    if played < 60. {
        return None;
    }
    let drops = stays.iter().filter(|s| s.1).count() as i64;
    let mut t = root.i18n.tv("acc.today", &[("t", &crate::pages::stats::played(root, played as i64))]);
    if drops > 0 {
        t = format!("{t} · {}", root.i18n.tn("acc.todayDrops", drops, &[]));
    }
    Some(t)
}

/// Today on a card or tile as a strip from midnight to midnight (the web page's DayStrip, review 2.1): filled green
/// where the account played, a red tick where a stay ended cut off, a light mark at now. Flat: one hairline track, no
/// glow. Its words ("Today: played 2 h · cut off once") are its tip. Every card and tile has one, so they line up.
fn day_strip(root: &Root, id: &str) -> Stateful<Div> {
    const DAY: f32 = 86400.;
    let data = STRIPS.hold().clone().map(|(d, _)| d);
    let mut stays: Vec<(f32, f32, bool)> = Vec::new();
    let mut now = None;
    if let Some(d) = data.as_ref() {
        stays = d
            .pointer(&format!("/rows/{id}"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|s| {
                let s = s.as_array()?;
                let (from, to) = (s.first()?.as_f64()? as f32, s.get(1)?.as_f64()? as f32);
                Some((from.clamp(0., DAY), to.clamp(0., DAY), s.get(2).and_then(Value::as_i64).unwrap_or(0) == 1))
            })
            .filter(|(a, b, _)| b > a)
            .collect();
        let at = |k: &str| d.get(k).and_then(Value::as_str).and_then(crate::time::iso_secs);
        now = at("now").zip(at("since")).map(|(n, s)| ((n - s) as f32).clamp(0., DAY));
    }
    let tip: SharedString = day_line(root, id).unwrap_or_else(|| root.tr("card.dayNone")).into();
    let frac = |s: f32| s / DAY;
    div()
        .id(SharedString::from(format!("day-{id}")))
        .relative()
        .w_full()
        .h(px(12.))
        .flex_none()
        // The track, 4 px, in the middle of a 12 px band (room for the ticks above and below it).
        .child(div().absolute().left_0().right_0().top(px(4.)).h(px(4.)).rounded_full().bg(ca(WHITE, 0.08)))
        .children(stays.into_iter().map(|(from, to, cut)| {
            div()
                .absolute()
                .top(px(4.))
                .h(px(4.))
                .left(relative(frac(from)))
                .w(relative(frac(to - from)))
                .min_w(px(2.))
                .rounded_full()
                .bg(c(OK))
                .when(cut, |d| d.child(div().absolute().right(px(-1.)).top(px(-2.)).w(px(2.)).h(px(8.)).rounded(px(1.)).bg(c(BAD))))
        }))
        // The day so far, a shade lighter than the rest of the track (was a tick standing out of it).
        .when_some(now, |d, n| d.child(div().absolute().left_0().top(px(4.)).h(px(4.)).w(relative(frac(n))).rounded_full().bg(ca(WHITE, 0.07))))
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
}

/// "Opened 9 days ago" (from Bloxkeep, the last time), or "Never opened".
fn opened_of(root: &Root, a: &Value) -> String {
    match a.get("lastUsed").and_then(Value::as_str).and_then(crate::time::iso_secs) {
        Some(t) => root.i18n.tv("card.opened", &[("t", &crate::time::ago(crate::time::now_secs() - t, &root.i18n))]),
        None => root.tr("card.never"),
    }
}

/// "@username · Display name" under a name (the display name only when the name shown isn't it already).
fn handle_of(a: &Value) -> Option<String> {
    let (user, display, shown) = (s(a, "username"), s(a, "displayName"), label(a));
    let mut parts = Vec::new();
    if !user.is_empty() {
        parts.push(format!("@{user}"));
    }
    if !display.is_empty() && display != shown && display != user {
        parts.push(display.to_string());
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The note the owner wrote on an account (Edit), when there is one.
fn note_of(a: &Value) -> Option<String> {
    Some(s(a, "note").trim()).filter(|n| !n.is_empty()).map(str::to_string)
}

/// A quiet line of facts under a card's state: an icon, then the words, cut at the card's edge.
fn small_line(mark: &'static str, text: String) -> Div {
    div()
        .h(px(18.))
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(T_S))
        .text_color(c(MUTED))
        .font_features(tnum())
        .child(icon(mark, 12., c(MUTED)))
        .child(div().min_w_0().truncate().child(text))
}

/// "Opened 9 days ago" as a quiet line, with "Last launched from this app" under the mouse (Accounts.tsx's title).
fn last_line(root: &Root, a: &Value, text: String) -> Stateful<Div> {
    let tip: SharedString = root.tr("card.lastTip").into();
    div().id(SharedString::from(format!("last-{}", id_of(a)))).min_w_0().child(small_line("history", text)).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
}

/// The small words a card or tile carries after the name: Main, and the group it is in.
fn tags_of(root: &Root, a: &Value) -> Vec<Div> {
    let mut out = Vec::new();
    // A tag with what it means under the mouse (Accounts.tsx's titles).
    let id = id_of(a);
    let explained = |key: &str, el: Div, tip: String| {
        let tip: SharedString = tip.into();
        div().flex_none().child(div().id(SharedString::from(format!("{key}-{id}"))).child(el).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
    };
    if a.get("isMain").and_then(Value::as_bool).unwrap_or(false) {
        out.push(explained("main-tag", tag(root.tr("card.main"), TEXT_2), root.tr("card.mainTip")));
    }
    // Roblox VNG's, as the list's row says (Accounts.tsx's card).
    if s(a, "realm") == "vng" {
        out.push(explained("vng-tag", tag("VNG".to_string(), ONLINE), root.tr("card.vngTip")));
    }
    // The Roblox its open window runs ("0.742"), unless it is the old build (that one has its own tag).
    if is_local(a) && !a.pointer("/session/oldBuild").and_then(Value::as_bool).unwrap_or(false) {
        if let Some(v) = a.pointer("/session/robloxVersion").and_then(Value::as_str).filter(|v| !v.is_empty()) {
            out.push(tag(v.split('.').take(2).collect::<Vec<_>>().join("."), MUTED));
        }
    }
    // (Its group gives way where the line is short, cut with "…": a long one, at its own width, squeezed the
    // account's name to nothing and pushed its Robux out past the card's edge: the shots with long names, 2026-11-18.)
    if let Some(g) = Some(s(a, "group").trim()).filter(|g| !g.is_empty()) {
        out.push(tag("", TEXT_2).flex_shrink(1.).min_w(px(36.)).max_w(px(140.)).child(div().min_w_0().truncate().child(g.to_string())));
    }
    // Auto-rejoin on: its mark beside the chips, as the web page's card has (review 2.1).
    if rejoin_on(a) {
        let tip: SharedString = root.tr("card.rejoinTip").into();
        out.push(div().flex_none().child(
            div()
                .id(SharedString::from(format!("rejoin-on-{}", id_of(a))))
                .size(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .child(icon("repeat", 13., c(ONLINE)))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
        ));
    }
    out
}

/// The star: this account is a main account, or no longer (its menu's "Main account" does the same). On the card
/// itself, since people didn't find where a main account is set (the owner, 2026-11-15: "người dùng khó biết gán main
/// account ở đâu"); lit while it is one.
fn main_button(root: &Root, a: &Value, at: &str, size: f32, cx: &mut Context<Root>) -> Div {
    let id = id_of(a);
    let on = a.get("isMain").and_then(Value::as_bool).unwrap_or(false);
    let key = SharedString::from(format!("{at}-main-{id}"));
    let tip: SharedString = root.tr(if on { "card.mainOn" } else { "card.mainSet" }).into();
    // Its outline alone while it isn't one (the star's inner part left clear), all of it in the face colour once it is:
    // drawn whole in grey, every card looked starred.
    no_pick(
        hot(key)
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(R_M + 1.))
            .cursor_pointer()
            .child(icon2("star", (size * 0.48).round().min(20.), c(if on { FACE } else { TEXT_2 }), if on { c(FACE) } else { ca(WHITE, 0.) }))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            .on_click(cx.listener(move |r, _, _, _| r.send("setMain", json!({ "ids": [id.clone()], "on": !on })))),
    )
}

/// Edit (the pencil): the account's form, as F2 and its menu's Edit open it.
fn edit_button(root: &Root, a: &Value, at: &str, size: f32, cx: &mut Context<Root>) -> Div {
    let id = id_of(a);
    let key = SharedString::from(format!("{at}-edit-{id}"));
    no_pick(IconButton::new(key, "edit").size(size).tip(root.tr("card.editTip")).on_click(cx.listener(move |r, _, window, cx| crate::pages::account_form::open(r, Some(id.clone()), window, cx))))
}

/// What a card says under its name: its state, when Bloxkeep last opened it (when the state doesn't say so already:
/// online, playing elsewhere), and its note. Today is the strip at its foot.
fn card_facts(root: &Root, a: &Value) -> (String, u32, Option<String>, Option<String>) {
    let (line, tone) = line_of(root, a);
    let opened = opened_of(root, a);
    let opened = (line != opened && line != root.tr("card.never")).then_some(opened);
    (line, tone, opened, note_of(a))
}

/// The lines a card has under its state: when it was opened, and its note. Every card in the grid takes as many as the
/// fullest one, so the rows stay one height.
fn card_lines(root: &Root, a: &Value) -> usize {
    let (_, _, opened, note) = card_facts(root, a);
    opened.is_some() as usize + note.is_some() as usize
}

/// An account not open here: its face, name, @username and Robux; Main and its group; what it does (online, playing
/// elsewhere, last opened…), how long it played today and its note; then its main button (Play, Sign in, Appeal, Cancel
/// rejoin), Edit and its ⋮ at the foot. A click picks it, a double click plays it; picked, warm with a yellow ring.
/// A card's height with `lines` extra lines of facts: its padding, the face's row, the state's line and the extra
/// lines, today's strip, the foot.
fn card_h(lines: usize) -> f32 {
    18. + 44. + 12. + 20. + lines as f32 * 20. + 12. + 12. + 14. + 1. + 12. + 30. + 14.
}

/// `lines`: the most lines of facts a card in its grid has, so all of them are one height.
fn card(root: &Root, a: &Value, w: f32, lines: usize, cx: &mut Context<Root>) -> impl IntoElement {
    want_strips(root, cx);
    let id = id_of(a);
    let on = root.selected.contains(&id);
    let (line, tone, opened, note) = card_facts(root, a);
    let robux = robux_of(a).map(|r| robux_el(root, &id, r));
    let tags = tags_of(root, a);
    let under = handle_of(a);
    let main = action(root, a, "card", cx);
    let pick = id.clone();
    let h = card_h(lines);
    // In a game: the card stands on a ground faintly of that game's colour, and its line starts with the game's icon
    // (accounts in the same game read as one group at a glance; every card was the same grey).
    let playing = game_icon(a);
    let (ground, ground_hot, line_col, line_hot) = toned(if rich() { playing.as_deref().and_then(|u| crate::images::tone(u, cx)) } else { None });
    let game_pic = if rich() { playing.as_deref().and_then(|u| crate::images::pic_sq(u, 18., cx)) } else { None };
    let body = hot_block(SharedString::from(format!("card-{id}")), R_CARD)
        .lift(3.)
        .child(spot(root, &id, R_CARD))
        .w(px(w))
        .h(px(h))
        .pt(px(18.))
        .pb(px(14.))
        .px(px(18.))
        .flex()
        .flex_col()
        .rounded(px(R_CARD))
        .cursor_pointer()
        .map(|d| {
            if on {
                d.tones(ground, ground_hot).edge(c(FACE), c(FACE))
            } else if crate::theme::glass_blocks() {
                // (A glass card keeps the glass's own light edge.)
                d.tones(ground, ground_hot)
            } else {
                d.tones(ground, ground_hot).edge(line_col, line_hot)
            }
        })
        .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
            // A double click plays this one alone (the first click already picked it).
            if e.click_count() >= 2 {
                r.selected.clear();
                r.selected.insert(pick.clone());
                r.play(std::slice::from_ref(&pick), cx);
            } else {
                toggle(r, &pick, e.modifiers().shift, cx);
            }
            cx.notify();
        }))
        .on_mouse_down(MouseButton::Right, on_right(a, cx))
        // The face, the name with its tags and Robux, @username under it.
        .child(
            div()
                .h(px(44.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(avatar(a, 44., RAISED, cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                // (The name keeps a few letters at least; its tags give way before it does.)
                                .child(div().min_w(px(64.)).flex_shrink(1.).text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().child(label(a)))
                                .children(tags)
                                .child(div().flex_1())
                                .children(robux.map(|r| div().flex_none().child(r))),
                        )
                        .when_some(under, |d, u| d.child(div().min_w_0().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(u))),
                )
                .when(on, |d| d.child(tick_mark(22.))),
        )
        // What it does, then today and the note.
        .child(
            div()
                .mt(px(12.))
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .h(px(20.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(T_M))
                        .child(div().size(px(7.)).flex_none().rounded_full().bg(c(dot_of(a))))
                        .when_some(game_pic, |d, p| d.child(img(p).size(px(18.)).flex_none().rounded(px(5.))))
                        .child(div().min_w_0().truncate().text_color(c(if tone == TEXT_2 || tone == OK || tone == MUTED { TEXT_2 } else { tone })).child(line)),
                )
                .when_some(opened, |d, t| d.child(last_line(root, a, t)))
                .when_some(note, |d, n| d.child(small_line("message", n)))
                // Today, at the foot of the facts (the room a card with fewer lines keeps is above it, not a gap).
                .child(div().flex_1())
                .child(day_strip(root, &id)),
        )
        // Its main button, Edit, its ⋮, under a hairline that sets the foot apart from the facts.
        .child(div().mt(px(14.)).h(px(1.)).flex_none().bg(ca(WHITE, 0.06)))
        .child(
            div()
                .mt(px(12.))
                .h(px(30.))
                .flex()
                .items_center()
                .gap(px(4.))
                .children(main.map(no_pick))
                .child(div().flex_1())
                .child(main_button(root, a, "card", 30., cx))
                .child(edit_button(root, a, "card", 30., cx))
                .child(more_button(root, a, "card", 30., cx)),
        )
        .children(menu::place(root, a, false, cx))
        .relative();
    on_base(if on { c(FACE_BASE) } else { base_dark() }, RISE, R_CARD + 1., body).w(px(w))
}

/// Play for the picked accounts that are free (Root::play reads the Job ID box, as every Play does).
fn play_picked(r: &mut Root, ids: &[String], cx: &mut Context<Root>) {
    r.play(ids, cx);
}

/// The picked accounts played (Enter in the game box or the Job ID box, as app.js's launch([...S.selected])).
pub fn play_selected(r: &mut Root, cx: &mut Context<Root>) {
    let own: Vec<String> = r.accounts().iter().filter(|a| r.selected.contains(s(a, "id")) && !foreign(a)).map(id_of).collect();
    r.play(&own, cx);
}

/// The Job ID box, made once (a box needs the window): Enter in it plays the picked accounts (app.js ~620).
fn job_entity(r: &Root, window: &mut Window, cx: &mut Context<Root>) -> Entity<gpui_kit::component::input::InputState> {
    if let Some(e) = cx.try_global::<menu::Ui>().and_then(|u| u.job.clone()) {
        return e;
    }
    let hint = r.tr("dock.job");
    let e = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).placeholder(hint));
    cx.subscribe(&e, |r: &mut Root, _, ev: &gpui_kit::component::input::InputEvent, cx| {
        if matches!(ev, gpui_kit::component::input::InputEvent::PressEnter { .. }) {
            play_selected(r, cx);
            cx.notify();
        }
    })
    .detach();
    cx.default_global::<menu::Ui>().job = Some(e.clone());
    e
}

/// What the bar's Job ID box holds while it is out (app.js's #job, read by every Play); empty when put away.
pub fn job_text(cx: &App) -> String {
    cx.try_global::<menu::Ui>().filter(|u| u.job_open).and_then(|u| u.job.clone()).map(|e| e.read(cx).value().trim().to_string()).unwrap_or_default()
}

/// The game in the bar's box changed: a Job ID left from another game no longer goes with it (app.js empties #job in
/// pickGame, pickSaved, the recent-game chips, usePrivateServer and putGame).
pub fn clear_job(window: &mut Window, cx: &mut App) {
    let ui = cx.default_global::<menu::Ui>();
    ui.job_stale = false;
    if let Some(e) = ui.job.clone() {
        e.update(cx, |st, cx| st.set_value("", window, cx));
    }
}

/// The Job ID box emptied where no window is at hand: at the next frame (Root::render calls settle_job).
pub fn clear_job_soon(cx: &mut App) {
    let ui = cx.default_global::<menu::Ui>();
    ui.job_stale = true;
}

pub fn settle_job(window: &mut Window, cx: &mut App) {
    if cx.try_global::<menu::Ui>().is_some_and(|u| u.job_stale) {
        clear_job(window, cx);
    }
}

/// The game box's game is known (app.js's resolveGame): a Job ID that went with another game is emptied, and this game
/// is the one the box's Job ID now goes with.
pub fn game_known(place: i64, cx: &mut App) {
    let (at, job) = cx.try_global::<menu::Ui>().map(|u| (u.job_place, u.job.clone())).unwrap_or_default();
    let held = job.is_some_and(|e| !e.read(cx).value().trim().is_empty());
    if held && at.is_some_and(|j| j != place) {
        clear_job_soon(cx);
    }
    cx.default_global::<menu::Ui>().job_place = Some(place);
}

/// The Job ID to send with Play: none when the game box now holds another game than the one it went with (a game
/// typed and played without Enter, before its lookup); that box is then emptied.
pub fn job_for(place_input: &str, cx: &mut App) -> String {
    let job = job_text(cx);
    if job.is_empty() {
        return job;
    }
    let at = cx.try_global::<menu::Ui>().and_then(|u| u.job_place);
    match (at, crate::target::parse(place_input, "")) {
        (Some(j), Ok(t)) if t.place > 0 && t.place != j => {
            clear_job_soon(cx);
            String::new()
        }
        _ => job,
    }
}

/// The server to join, by its Job ID (the web page's Server box): a button until it is wanted (a box needs the window
/// to be made), then the box with an x that puts it away (and empties it: any server again).
/// A server picked to play in (app.js's useServer): its game in the game box, its Job ID in the bar's server box; the
/// accounts are then picked and played as usual.
/// A game put in the Play box on Accounts, its Job ID box emptied (app.js's putGame).
pub fn put_game(r: &mut Root, place: i64, window: &mut Window, cx: &mut Context<Root>) {
    r.go(crate::pages::Page::Accounts);
    let url = format!("https://www.roblox.com/games/{place}");
    clear_job(window, cx);
    if r.place.read(cx).value().trim() != url {
        r.place.update(cx, |st, cx| st.set_value(url, window, cx));
        r.resolve_place(cx);
    }
    cx.notify();
}

pub fn use_server(r: &mut Root, place: i64, job: String, window: &mut Window, cx: &mut Context<Root>) {
    cx.default_global::<menu::Ui>().job_place = Some(place);
    let url = format!("https://www.roblox.com/games/{place}");
    if r.place.read(cx).value().trim() != url {
        r.place.update(cx, |st, cx| st.set_value(url, window, cx));
        r.resolve_place(cx);
    }
    let e = job_entity(r, window, cx);
    e.update(cx, |st, cx| st.set_value(job, window, cx));
    cx.default_global::<menu::Ui>().job_open = true;
    cx.notify();
}

fn job_box(root: &Root, compact: bool, cx: &mut Context<Root>) -> AnyElement {
    let ui = cx.try_global::<menu::Ui>();
    let shown = ui.filter(|u| u.job_open).and_then(|u| u.job.clone());
    match shown {
        Some(e) => div()
            .id("bar-job")
            .w(px(if compact { 170. } else { 230. }))
            .flex_none()
            .child(field(&e, Some("server")).child(IconButton::new("bar-job-x", "x").size(26.).tip(root.tr("common.close")).on_click(cx.listener(|_, _, window, cx| {
                let ui = cx.default_global::<menu::Ui>();
                ui.job_open = false;
                if let Some(e) = ui.job.clone() {
                    e.update(cx, |st, cx| st.set_value("", window, cx));
                }
                cx.notify();
            }))))
            .tooltip({
                let tip: SharedString = root.tr("dock.jobTip").into();
                move |_, cx| cx.new(|_| Tip(tip.clone())).into()
            })
            .into_any_element(),
        None => {
            let open = cx.listener(|r: &mut Root, _: &ClickEvent, window: &mut Window, cx: &mut Context<Root>| {
                let e = job_entity(r, window, cx);
                cx.default_global::<menu::Ui>().job_open = true;
                e.update(cx, |st, cx| st.focus(window, cx));
                cx.notify();
            });
            if compact {
                IconButton::new("bar-job-open", "server").size(34.).tip(root.tr("dock.jobTip")).on_click(open).into_any_element()
            } else {
                div().id("bar-job-wrap").flex_none().child(Button::new("bar-job-open", root.tr("dock.job")).icon("server").quiet().on_click(open)).tooltip({
                    let tip: SharedString = root.tr("dock.jobTip").into();
                    move |_, cx| cx.new(|_| Tip(tip.clone())).into()
                }).into_any_element()
            }
        }
    }
}

/// Browse servers: the game in the box, on its page's Servers tab (the friends' servers, the public ones and the
/// private ones); a server picked there joins with these accounts. There once the box holds a game Roblox knows.
/// Always in the bar (a button that came and went with the box read as missing, review 2.4): greyed, with why, until
/// the box holds a game.
fn browse_servers(root: &Root, compact: bool, cx: &mut Context<Root>) -> Option<AnyElement> {
    let known = root
        .game
        .as_ref()
        .filter(|g| g.get("ok").and_then(Value::as_bool) == Some(true))
        .and_then(|g| g.get("placeId").and_then(Value::as_i64).filter(|p| *p > 0).map(|p| (p, s(g, "name").to_string())));
    let off = known.is_none();
    let go = cx.listener(move |r: &mut Root, _: &ClickEvent, _: &mut Window, cx: &mut Context<Root>| {
        let Some((place, name)) = known.as_ref() else { return };
        crate::pages::games::open_place(r, *place, name);
        r.games.view = crate::pages::games::View::Games;
        r.games.tab = crate::pages::games::Tab::Servers;
        cx.notify();
    });
    let tip: SharedString = root.tr(if off { "dock.serversFirst" } else { "dock.servers" }).into();
    Some(if compact {
        IconButton::new("bar-servers", "screens").size(34.).disabled(off).tip(tip).on_click(go).into_any_element()
    } else {
        div()
            .id("bar-servers-wrap")
            .flex_none()
            .child(Button::new("bar-servers", root.tr("dock.servers")).icon("screens").quiet().disabled(off).on_click(go))
            .when(off, |d| d.tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
            .into_any_element()
    })
}

/// At the foot while some are picked: their faces and count, then what can be done for them: play (the game box,
/// games played lately, Play), rejoin those cut off, close those open; the profile when one is picked.
fn bar(root: &Root, picked: &[Value], width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    // Short of room (a window about 1180 wide): the bar's tools keep their icons, their words the tips.
    let compact = width < 1300.;
    let n = picked.len();
    let playable: Vec<String> = picked.iter().filter(|a| can_play(a)).map(id_of).collect();
    let dropped: Vec<Value> = picked.iter().filter(|a| is_kicked(a)).cloned().collect();
    let open: Vec<String> = picked.iter().filter(|a| is_local(a) || launching(a)).map(id_of).collect();
    let profile = (n == 1).then(|| picked[0].get("userId").and_then(Value::as_i64)).flatten().filter(|u| *u > 0);
    let game = root.game.as_ref().filter(|g| g.get("ok").and_then(Value::as_bool) == Some(true));
    let gpic = game.and_then(|g| g.get("iconUrl").and_then(Value::as_str)).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, 34., cx));
    let has_game = !root.place.read(cx).value().trim().is_empty();
    let current = game.and_then(|g| g.get("placeId").and_then(Value::as_i64));
    let recents: Vec<Value> = root
        .state
        .pointer("/settings/recentGames")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter(|r| r.get("placeId").and_then(Value::as_i64) != current).take(3).cloned().collect())
        .unwrap_or_default();
    let verb = root.tr(if has_game { "dock.play" } else { "dock.openHome" });
    let k = playable.len();

    let who = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(12.))
        .child(div().flex().children(picked.iter().take(4).enumerate().map(|(i, a)| {
            div().when(i > 0, |d| d.ml(px(-10.))).child(stacked(a, 28., RAISED, cx))
        })))
        .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.i18n.tv("ui.nPicked", &[("n", &n.to_string())])))
        .when_some(profile, |d, user_id| {
            d.child(IconButton::new("bar-profile", "external").size(32.).tip(root.tr("ui.profile")).on_click(cx.listener(move |r, _, _, _| {
                r.send("openProfile", json!({ "userId": user_id }))
            })))
        });

    // (Also while opening: the ones picked are then all opening, and Stop stands in Play's place.)
    let opening = root.state.get("launching").and_then(Value::as_bool).unwrap_or(false);
    // (And for Ctrl+L or "/" with none picked: the bar is there for its game box, which they focus.)
    let wanted = cx.try_global::<menu::Ui>().is_some_and(|u| u.place_wanted == Some(root.nav));
    let play = (k > 0 || opening || wanted).then(|| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .size(px(34.))
                    .flex_none()
                    .rounded(px(R_M))
                    .overflow_hidden()
                    .bg(ca(WHITE, 0.06))
                    .flex()
                    .items_center()
                    .justify_center()
                    .map(|d| match gpic {
                        Some(i) => d.child(img(i).size_full().object_fit(ObjectFit::Cover)),
                        None => d.child(icon("gamepad", 16., c(MUTED))),
                    }),
            )
            // Choose a game: the ones played lately, the pinned, the saved servers, favourites, popular.
            .child(crate::pages::picker::button(root, cx))
            .children(crate::pages::picker::save_button(root, cx))
            .child(div().id("bar-place").flex_1().min_w(px(200.)).child(field(&root.place, None)).tooltip({
                // What the box takes (Accounts.tsx's title).
                let tip: SharedString = root.tr("dock.placeTip").into();
                move |_, cx| cx.new(|_| Tip(tip.clone())).into()
            }))
            .child(job_box(root, compact, cx))
            .children(browse_servers(root, compact, cx))
            .children(recents.into_iter().enumerate().map(|(i, r)| {
                let place = r.get("placeId").and_then(Value::as_i64).unwrap_or(0);
                let pic = r.get("iconUrl").and_then(Value::as_str).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, 34., cx));
                let name: SharedString = crate::pages::home::clean(s(&r, "name")).into();
                focusable(div().id(("recent", i)))
                    .size(px(34.))
                    .flex_none()
                    .rounded(px(R_M))
                    .overflow_hidden()
                    .bg(ca(WHITE, 0.06))
                    .cursor_pointer()
                    .hover(|d| d.opacity(0.85))
                    .when_some(pic, |d, p| d.child(img(p).size_full().object_fit(ObjectFit::Cover)))
                    .tooltip(move |_, cx| cx.new(|_| Tip(name.clone())).into())
                    .on_click(cx.listener(move |r, _, window, cx| {
                        let url = format!("https://www.roblox.com/games/{place}");
                        clear_job(window, cx);
                        r.place.update(cx, |st, cx| st.set_value(url, window, cx));
                        r.resolve_place(cx);
                    }))
            }))
            // While accounts are being opened one after another: Stop, in Play's place.
            .child(div().flex_none().child(if opening {
                Button::new("bar-stop", root.tr("dock.stop")).icon("stop").danger().on_click(cx.listener(|r, _, _, _| r.send("cancelLaunch", json!({}))))
            } else {
                Button::new("bar-play", if k > 1 { format!("{verb} · {k}") } else { verb })
                    .icon(if has_game { "play" } else { "screens" })
                    .primary()
                    .on_click(cx.listener(move |r, _, _, cx| play_picked(r, &playable, cx)))
            }))
    });

    let acts = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .when(!dropped.is_empty(), |d| {
            let label = root.i18n.tv("ui.rejoinN", &[("n", &dropped.len().to_string())]);
            d.child(Button::new("bar-rejoin", label).icon("refresh").tint().on_click(cx.listener(move |r, _, _, cx| r.rejoin(&dropped, cx))))
        })
        .when(!open.is_empty(), |d| {
            let label = format!("{} · {}", root.tr("card.close"), open.len());
            d.child(Button::new("bar-close", label).icon("power").on_click(cx.listener(move |r, _, _, cx| close(r, open.clone(), false, cx))))
        })
        // Join a player: these accounts into the server someone is in.
        .child({
            let ids: Vec<String> = picked.iter().filter(|a| !foreign(a)).map(id_of).collect();
            let go = cx.listener(move |r: &mut Root, _: &ClickEvent, window: &mut Window, cx: &mut Context<Root>| crate::pages::join::open(r, ids.clone(), window, cx));
            if compact {
                IconButton::new("bar-join-player", "userGo").size(32.).tip(root.tr("jp.buttonTip")).on_click(go).into_any_element()
            } else {
                div().id("bar-join-wrap").flex_none().child(Button::new("bar-join-player", root.tr("jp.button")).icon("userGo").quiet().on_click(go)).tooltip({
                    let tip: SharedString = root.tr("jp.buttonTip").into();
                    move |_, cx| cx.new(|_| Tip(tip.clone())).into()
                }).into_any_element()
            }
        })
        // The presets to run, and Save as preset… with these accounts (pages/preset_menu.rs).
        // (Named while there is any room: as an icon among the others nobody found it, the owner, 2026-11-18.)
        .child(groups::bar_button(root, width < 880., cx))
        .child(crate::pages::preset_menu::button(root, !compact, cx))
        .child(more(root, picked, cx))
        .child(IconButton::new("bar-clear", "x").size(32.).tip(root.tr("ui.clearPick")).on_click(cx.listener(|r, _, _, cx| {
            r.selected.clear();
            cx.notify();
        })));

    // What the game box holds (Accounts.tsx's DockInfo): looking it up, why not, a private-server link, or the game's
    // name (its page on a click), the kind of server, its maker and how many play it.
    // (In a low window the line that only says how to use the box is left out: the bar took what was left of the
    // list's room, 860×500 with accounts selected.)
    let info = (k > 0 || opening || wanted).then(|| dock_info(root, has_game, cx)).filter(|_| has_game || root.window_h.get() >= 620.);
    // Two lines at any width: who and what to do with them, then where to play (one line ran out of room).
    div()
        .flex_none()
        .mx(px(PAD))
        // (Clear of the cards: the scrolling part ends a gap above it, with a hairline round the bar, so a card cut
        // at that edge reads as "more above", not as the bar lying over it.)
        .mt(px(8.))
        .mb(px(PAD))
        .p(px(10.))
        .pl(px(14.))
        .rounded(px(R_L))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(div().flex().items_center().gap(px(12.)).child(who).child(div().flex_1()).child(acts))
        .children(play)
        .children(info)
}

fn dock_info(root: &Root, has_game: bool, cx: &mut Context<Root>) -> Div {
    let line = div().h(px(18.)).min_w_0().flex().items_center().gap(px(8.)).pl(px(42.)).text_size(px(T_S)).text_color(c(TEXT_2));
    let g = root.game.as_ref();
    if !has_game {
        return line.text_color(c(MUTED)).child(crate::i18n::plain(&root.tr("dock.hint")));
    }
    if root.place_asking {
        return line.child(icon("refresh", 12., c(MUTED))).child(root.tr("dock.finding"));
    }
    let Some(g) = g else { return line };
    if g.get("ok").and_then(Value::as_bool) != Some(true) {
        let why = s(g, "error").to_string();
        return line.text_color(c(WARN)).child(icon("alert", 12., c(WARN))).child(div().truncate().child(why));
    }
    let kind = s(g, "kind");
    if kind == "share" {
        return line.child(tag(root.tr("dock.share"), FACE)).child(div().truncate().child(root.tr("dock.shareNote")));
    }
    let place = g.get("placeId").and_then(Value::as_i64).unwrap_or(0);
    let name = Some(crate::pages::home::clean(s(g, "name"))).filter(|n| !n.is_empty()).unwrap_or_else(|| root.i18n.tv("dock.placeN", &[("id", &place.to_string())]));
    let public = place > 0 && kind != "private" && kind != "server";
    let tip: SharedString = root.tr("gp.open").into();
    let (n2, p2) = (name.clone(), place);
    line.child(
        div()
            .id("dock-game")
            .min_w_0()
            .truncate()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(TEXT))
            .child(name)
            .when(public, |d| {
                d.cursor_pointer().hover(|d| d.underline()).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()).on_click(cx.listener(move |r, _, _, cx| {
                    crate::pages::games::open_place(r, p2, &n2);
                    cx.notify();
                }))
            }),
    )
    .when(kind == "private", |d| d.child(tag(root.tr("dock.private"), FACE)))
    .when(kind == "server", |d| d.child(tag(root.tr("dock.server"), FACE)))
    .when(!s(g, "creator").is_empty(), |d| d.child(div().flex_none().text_color(c(MUTED)).child(root.i18n.tv("dock.by", &[("c", s(g, "creator"))]))))
    .when_some(g.get("playing").and_then(Value::as_i64).filter(|n| *n > 0), |d, n| {
        d.child(div().flex_none().text_color(c(MUTED)).child(root.i18n.tv("dock.playing", &[("n", &crate::kit::thousands(n))])))
    })
}

/// An account's name as the page shows it, by its id (None when it is gone).
pub(crate) fn label_by_id(root: &Root, id: &str) -> Option<String> {
    root.accounts().iter().find(|a| s(a, "id") == id).map(label)
}
