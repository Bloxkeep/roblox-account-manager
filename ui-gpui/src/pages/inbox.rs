// The inbox, inside Games (its header's switch): what Roblox keeps for the account whose tab is shown (pages/tabs.rs):
// its chats (read one, say something in it), its messages (opening one marks it read), its notifications and the
// lines of its Robux account, and its trades: each list Roblox keeps them in, a trade's two sides with what each is
// worth by RAP and who comes out ahead (Roblox leaves that sum to the reader), and Accept (two clicks) or Decline.
// All of it answers only that account's own sign-in, lent by the app; asked from here (roblox.rs). Nothing is spent
// from here. Chat isn't live: Refresh reads it again, as does saying something.
use gpui_kit::{prelude::*, *};

use crate::{
    kit::*,
    images,
    pages::{friends, games::thousands, profile::day},
    roblox::{self, Deal, Letter, Line, Notice, Talk, Trade, TradeDetail, TradeSide},
    shell::Root,
    theme::*,
};

#[derive(Default)]
pub struct Data {
    /// The Roblox user these are of, and the tab shown (0 chat, 1 messages, 2 notifications, 3 Robux, 4 trades).
    of: i64,
    pub tab: usize,
    /// Chat: the chats, the one open with its lines (None while they load), and whether something is being said.
    talks: Option<Result<Vec<Talk>, String>>,
    talk_at: Option<String>,
    /// The friend whose chat was asked for from their card (chat_with), until the chats have come.
    want_with: Option<i64>,
    /// The chat shown was put there by the app (the newest, so the pane isn't empty), not picked: Roblox hasn't been
    /// told it was seen, and its unread mark stands until it is picked.
    peek: bool,
    lines: Option<Result<Vec<Line>, String>>,
    saying: bool,
    letters: Option<Result<Vec<Letter>, String>>,
    notices: Option<Result<Vec<Notice>, String>>,
    deals: Option<Result<Vec<Deal>, String>>,
    /// The tabs being read, and the message open.
    reading: [bool; 5],
    open: Option<i64>,
    /// Trades: the list shown (roblox::TRADES), its trades, the one open with its two sides (None while they load),
    /// Accept armed by its first click, an answer being waited for, and why Roblox turned the last one down.
    trade_list: usize,
    trades: Option<Result<Vec<Trade>, String>>,
    trade_at: Option<i64>,
    trade: Option<Result<TradeDetail, String>>,
    trade_armed: bool,
    trade_busy: bool,
    trade_said: Option<String>,
}

impl Data {
    /// Put away with its tab (pages/tabs.rs): nothing is on its way any more, so what is missing is asked again.
    pub fn rest(&mut self) {
        self.reading = [false; 5];
        self.saying = false;
        self.trade_busy = false;
        self.trade_armed = false;
        // The open trade's sides are read again when the tab comes back, should their answer have been dropped.
        if self.trade.is_none() {
            self.trade_at = None;
        }
        // The open chat's lines are read again when the tab comes back, should their answer have been dropped.
        if self.lines.is_none() {
            self.talk_at = None;
        }
    }
}

pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    friends::want_sign(r, link, cx);
    let Some((_, user, _)) = friends::signed(r) else { return };
    if r.inbox.of != user {
        r.inbox = Data { of: user, tab: r.inbox.tab, want_with: r.inbox.want_with, ..Default::default() };
    }
    let tab = r.inbox.tab.min(4);
    let have = match tab {
        0 => r.inbox.talks.is_some(),
        1 => r.inbox.letters.is_some(),
        2 => r.inbox.notices.is_some(),
        3 => r.inbox.deals.is_some(),
        _ => r.inbox.trades.is_some(),
    };
    if !have && !r.inbox.reading[tab] {
        load(r, tab, cx);
    }
    // BLOXKEEP_SHOT_CHAT=c1: a hidden shot opens that chat once the chats have come.
    #[cfg(feature = "shot")]
    if let (Ok(id), None, Some(Ok(_))) = (std::env::var("BLOXKEEP_SHOT_CHAT"), &r.inbox.talk_at, &r.inbox.talks) {
        open_talk(r, id, cx);
    }
}

fn load(r: &mut Root, tab: usize, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    r.inbox.reading[tab] = true;
    // Each tab's answer goes back into its own place; one for an account no longer looked through is dropped.
    macro_rules! read {
        ($call:expr, $field:ident) => {{
            let rx = roblox::run(move || $call);
            cx.spawn(async move |this, cx| {
                let got = rx.await.unwrap_or_else(|_| Err(String::new()));
                let _ = this.update(cx, |r, cx| {
                    if r.inbox.of == user {
                        r.inbox.$field = Some(got);
                        r.inbox.reading[tab] = false;
                        peek(r, cx);
                        cx.notify();
                    }
                });
            })
            .detach();
        }};
    }
    match tab {
        0 => read!(roblox::talks(&cookie, user), talks),
        1 => read!(roblox::letters(&cookie), letters),
        2 => read!(roblox::notices(&cookie), notices),
        3 => read!(roblox::deals(&cookie, user), deals),
        _ => {
            // (An answer for a list no longer shown is dropped.)
            let list = r.inbox.trade_list.min(roblox::TRADES.len() - 1);
            let rx = roblox::run(move || roblox::trades(&cookie, roblox::TRADES[list]));
            cx.spawn(async move |this, cx| {
                let got = rx.await.unwrap_or_else(|_| Err(String::new()));
                let _ = this.update(cx, |r, cx| {
                    if r.inbox.of == user && r.inbox.trade_list == list {
                        r.inbox.trades = Some(got);
                        r.inbox.reading[4] = false;
                        // BLOXKEEP_SHOT_TRADE=1: a hidden shot opens the first trade (=armed: after Accept's
                        // first click; nothing is accepted).
                        #[cfg(feature = "shot")]
                        if let (Ok(how), Some(Ok(l))) = (std::env::var("BLOXKEEP_SHOT_TRADE"), &r.inbox.trades) {
                            if let Some(first) = l.first().map(|t| t.id) {
                                open_trade(r, first, cx);
                                r.inbox.trade_armed = how == "armed";
                            }
                        }
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }
}

/// Another of the trade lists: read anew, with nothing open.
fn pick_trades(r: &mut Root, list: usize, cx: &mut Context<Root>) {
    if r.inbox.trade_list != list {
        r.inbox.trade_list = list;
        r.inbox.trades = None;
        r.inbox.reading[4] = false;
        r.inbox.trade_at = None;
        r.inbox.trade = None;
        r.inbox.trade_armed = false;
        r.inbox.trade_said = None;
        load(r, 4, cx);
        cx.notify();
    }
}

/// Opens a trade (its two sides are read), or closes the one open.
fn open_trade(r: &mut Root, id: i64, cx: &mut Context<Root>) {
    r.inbox.trade_armed = false;
    r.inbox.trade_said = None;
    r.inbox.trade = None;
    if r.inbox.trade_at == Some(id) {
        r.inbox.trade_at = None;
        return;
    }
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let with = match &r.inbox.trades {
        Some(Ok(l)) => l.iter().find(|t| t.id == id).map(|t| t.who_id).unwrap_or(0),
        _ => 0,
    };
    r.inbox.trade_at = Some(id);
    let rx = roblox::run(move || roblox::trade(&cookie, id, with));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.inbox.of == user && r.inbox.trade_at == Some(id) {
                r.inbox.trade = Some(got);
                cx.notify();
            }
        });
    })
    .detach();
}

/// Accept (the second click) or decline the open trade; then its list is read again.
fn trade_act(r: &mut Root, accept: bool, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let Some(id) = r.inbox.trade_at else { return };
    if r.inbox.trade_busy || (accept && !r.inbox.trade_armed) {
        return;
    }
    r.inbox.trade_busy = true;
    r.inbox.trade_armed = false;
    r.inbox.trade_said = None;
    let rx = roblox::run(move || roblox::trade_act(&cookie, id, accept));
    cx.spawn(async move |this, cx| {
        let done = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.inbox.of != user {
                return;
            }
            r.inbox.trade_busy = false;
            match done {
                Ok(()) => {
                    r.inbox.trade_at = None;
                    r.inbox.trade = None;
                    r.inbox.trades = None;
                    if !r.inbox.reading[4] {
                        load(r, 4, cx);
                    }
                }
                Err(why) => r.inbox.trade_said = Some(if why.is_empty() { "Roblox".into() } else { why }),
            }
            cx.notify();
        });
    })
    .detach();
}

fn again(r: &mut Root, cx: &mut Context<Root>) {
    let tab = r.inbox.tab.min(4);
    if !r.inbox.reading[tab] {
        load(r, tab, cx);
        if tab == 0 {
            read_lines(r, cx);
        }
        cx.notify();
    }
}

/// A friend's chat, from their card on Friends: the Inbox's Chat with theirs open. (The owner, 2026-11-13: "sao không
/// có nút chat với bạn bè": chat was only under Inbox, with no way to it from a friend.)
pub fn chat_with(r: &mut Root, user: i64, cx: &mut Context<Root>) {
    r.games.view = crate::pages::games::View::Inbox;
    r.inbox.tab = 0;
    r.inbox.want_with = Some(user);
    settle_with(r, cx);
    cx.notify();
}

/// Their chat is opened once the chats are here. One never chatted with isn't in Roblox's list: that is said (a new
/// chat isn't started from here: Roblox's call for it isn't one this app has seen answered).
fn settle_with(r: &mut Root, cx: &mut Context<Root>) {
    let Some(user) = r.inbox.want_with else { return };
    // (The chats didn't come: the wish goes with them, so a later visit doesn't jump to that chat, or say it is
    // missing, out of the blue. The Chat tab says why it is empty.)
    if matches!(r.inbox.talks, Some(Err(_))) {
        r.inbox.want_with = None;
        return;
    }
    let Some(Ok(l)) = &r.inbox.talks else { return };
    let found = l.iter().find(|t| !t.group && t.people.contains_key(&user)).map(|t| t.id.clone());
    r.inbox.want_with = None;
    match found {
        Some(id) => open_talk(r, id, cx),
        None => r.toast = Some((r.tr("inbox.noTalk"), "info".into())),
    }
}

/// With no chat picked, the newest one is shown beside the list, so the pane isn't an empty box. Only its lines are
/// read: Roblox is told a chat was seen when it is picked (open_talk).
fn peek(r: &mut Root, cx: &mut Context<Root>) {
    if r.inbox.want_with.is_some() && r.inbox.talks.is_some() {
        settle_with(r, cx);
    }
    if r.inbox.talk_at.is_some() {
        return;
    }
    let Some(Ok(l)) = &r.inbox.talks else { return };
    let Some(first) = l.first().map(|t| t.id.clone()) else { return };
    r.inbox.talk_at = Some(first);
    r.inbox.peek = true;
    r.inbox.lines = None;
    read_lines(r, cx);
}

/// A chat opens beside the list: its lines are read, and Roblox is told it was seen.
fn open_talk(r: &mut Root, id: String, cx: &mut Context<Root>) {
    let same = r.inbox.talk_at.as_deref() == Some(id.as_str());
    if same && !r.inbox.peek {
        return;
    }
    r.inbox.peek = false;
    r.inbox.talk_at = Some(id.clone());
    if !same {
        r.inbox.lines = None;
    }
    let mut was = 0;
    if let Some(Ok(l)) = &mut r.inbox.talks {
        if let Some(t) = l.iter_mut().find(|t| t.id == id) {
            was = std::mem::take(&mut t.unread);
        }
    }
    if !same {
        read_lines(r, cx);
    }
    if let Some((_, user, cookie)) = friends::signed(r).filter(|s| !s.2.is_empty()) {
        let ask = id.clone();
        let rx = roblox::run(move || roblox::seen(&cookie, &ask));
        // Roblox not told: the chat's unread count comes back (it is still unread there).
        cx.spawn(async move |this, cx| {
            if matches!(rx.await, Ok(Ok(()))) || was == 0 {
                return;
            }
            let _ = this.update(cx, |r, cx| {
                if r.inbox.of != user {
                    return;
                }
                if let Some(Ok(l)) = &mut r.inbox.talks {
                    if let Some(t) = l.iter_mut().find(|t| t.id == id && t.unread == 0) {
                        t.unread = was;
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
    cx.notify();
}

/// The open chat's lines, read (again); an answer for a chat no longer open is dropped.
fn read_lines(r: &mut Root, cx: &mut Context<Root>) {
    let (Some(id), Some((_, user, cookie))) = (r.inbox.talk_at.clone(), friends::signed(r)) else { return };
    let ask = id.clone();
    let rx = roblox::run(move || roblox::lines(&cookie, &ask));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.inbox.of == user && r.inbox.talk_at.as_deref() == Some(id.as_str()) {
                // Lines that fail to come again leave the ones shown.
                if got.is_ok() || !matches!(r.inbox.lines, Some(Ok(_))) {
                    r.inbox.lines = Some(got);
                }
                cx.notify();
            }
        });
    })
    .detach();
}

/// What the box holds is said in the open chat (Enter); said, the box empties and the lines are read again.
pub fn say(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let words = r.chat_box.read(cx).value().trim().to_string();
    let (Some(id), Some((_, user, cookie))) = (r.inbox.talk_at.clone(), friends::signed(r)) else { return };
    if words.is_empty() || r.inbox.saying || cookie.is_empty() {
        return;
    }
    r.inbox.saying = true;
    cx.notify();
    let (ask, said) = (id.clone(), words.clone());
    let rx = roblox::run(move || roblox::say(&cookie, &ask, &said));
    cx.spawn_in(window, async move |this, cx| {
        let done = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update_in(cx, |r, window, cx| {
            if r.inbox.of != user {
                return;
            }
            r.inbox.saying = false;
            // Said: the box empties (when it still holds those words) and the chat still open reads its lines again.
            // Not said: the words stay in the box to send again, and the chat's lines stay; the failure is a toast.
            let here = r.inbox.talk_at.as_deref() == Some(id.as_str());
            match done {
                Ok(()) => {
                    if here && r.chat_box.read(cx).value().trim() == words {
                        r.chat_box.update(cx, |s, cx| s.set_value("", window, cx));
                    }
                    if here {
                        read_lines(r, cx);
                    }
                }
                Err(why) => r.toast = Some((if why.is_empty() { "Roblox".into() } else { why }, "bad".into())),
            }
            cx.notify();
        });
    })
    .detach();
}

/// A message opens in place (a second click closes it); one not read yet is marked read, here and at Roblox.
fn open(r: &mut Root, id: i64, cx: &mut Context<Root>) {
    r.inbox.open = Some(id);
    let unread = matches!(&r.inbox.letters, Some(Ok(l)) if l.iter().any(|m| m.id == id && !m.read));
    if let (true, Some(_), Some((_, _, cookie))) = (unread, r.inbox.open, friends::signed(r)) {
        if let Some(Ok(l)) = &mut r.inbox.letters {
            if let Some(m) = l.iter_mut().find(|m| m.id == id) {
                m.read = true;
            }
        }
        // A hidden shot's messages aren't Roblox's to mark (no sign-in there).
        if !cookie.is_empty() {
            drop(roblox::run(move || roblox::mark_read(&cookie, id)));
        }
    }
    cx.notify();
}

fn note(text: String) -> Div {
    div().flex_1().flex().items_center().justify_center().text_size(px(T_M)).text_color(c(MUTED)).child(text)
}

pub fn inbox(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let ib = &root.inbox;
    // One column, newest first, as an inbox reads: two columns (the second going on where the first ended) put the
    // dates out of order across the page.
    let _ = width;
    let tab = ib.tab.min(4);
    let (able, viewer) = friends::looking_ref(root);
    let shell = div().id("inbox").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let unread = match &ib.letters {
        Some(Ok(l)) => l.iter().filter(|m| !m.read).count(),
        _ => 0,
    };
    let waiting = match &ib.talks {
        Some(Ok(l)) => l.iter().filter(|t| t.unread > 0).count(),
        _ => 0,
    };
    let tabs = [(root.tr("in.chat"), waiting), (root.tr("in.messages"), unread), (root.tr("in.notices"), 0), (root.tr("in.deals"), 0), (root.tr("in.trades"), 0)];
    let header = div()
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
                .child(crate::pages::games::brand(root))
                .relative().child(crate::pages::games::island(root, cx))
                .child(div().flex_1())
                .when(viewer.is_some(), |d| {
                    d.child(IconButton::new("in-refresh", "refresh").size(34.).busy(ib.reading[tab]).tip(root.tr("fr.refresh")).on_click(cx.listener(|r, _, _, cx| again(r, cx))))
                }),
        )
        .when(!(able.is_empty() || friends::nobody(root)), |d| d.child(div().flex().flex_wrap().items_center().gap(px(4.)).children(tabs.into_iter().enumerate().map(|(i, (text, count))| {
            let on = tab == i;
            chip(("in-tab", i), on)
                .h(px(TOOL_H))
                .px(px(12.))
                .cursor_pointer()
                .child(icon(["message", "news", "bell", "robux", "repeat"][i], 15., c(if on { INK } else { MUTED })))
                .child(text)
                .when(count > 0, |d| d.child(chip_count(count, on)))
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.inbox.tab = i;
                    cx.notify();
                }))
        }))));

    let failed = |why: &String| note(root.i18n.tv("in.failed", &[("why", if why.is_empty() { "Roblox" } else { why })]));
    let body: AnyElement = if able.is_empty() || friends::nobody(root) {
        friends::need_account(root, "in-add-acc", "in.noAccount", "message", cx)
    } else {
        let rows: Result<Option<Vec<AnyElement>>, AnyElement> = match tab {
            0 => match &ib.talks {
                None => Ok(None),
                Some(Err(e)) => Err(failed(e).into_any_element()),
                Some(Ok(l)) if l.is_empty() => Err(note(root.tr("in.noChats")).into_any_element()),
                Some(Ok(l)) => Err(chat(root, l, cx).into_any_element()),
            },
            1 => match &ib.letters {
                None => Ok(None),
                Some(Err(e)) => Err(failed(e).into_any_element()),
                Some(Ok(l)) if l.is_empty() => Err(note(root.tr("in.noMessages")).into_any_element()),
                Some(Ok(l)) => Err(letters(root, l, cx).into_any_element()),
            },
            2 => match &ib.notices {
                None => Ok(None),
                Some(Err(e)) => Err(failed(e).into_any_element()),
                Some(Ok(l)) if l.is_empty() => Err(note(root.tr("in.noNotices")).into_any_element()),
                Some(Ok(l)) => {
                    // Newest first, whatever order they came in.
                    let mut by: Vec<&Notice> = l.iter().collect();
                    by.sort_by(|a, b| b.when.cmp(&a.when));
                    // The same notice again and again (a game's update, one person's requests) is one row with how
                    // many times, dated by the newest: five equal rows read as a fault.
                    let mut rows: Vec<(&Notice, usize)> = Vec::new();
                    for n in by {
                        match rows.last_mut() {
                            Some((last, k)) if last.kind == n.kind && last.name == n.name && last.text == n.text => *k += 1,
                            _ => rows.push((n, 1)),
                        }
                    }
                    Ok(Some(rows.into_iter().enumerate().map(|(i, (n, k))| notice(root, i, n, k).into_any_element()).collect()))
                }
            },
            3 => match &ib.deals {
                None => Ok(None),
                Some(Err(e)) => Err(failed(e).into_any_element()),
                Some(Ok(l)) if l.is_empty() => Err(note(root.tr("in.noDeals")).into_any_element()),
                Some(Ok(l)) => {
                    // The same purchase or sale several times on one day is one row: how many times, and what they come to.
                    let mut rows: Vec<(&Deal, usize)> = Vec::new();
                    for d in l {
                        match rows.last_mut() {
                            Some((last, k)) if last.what == d.what && last.who == d.who && last.amount == d.amount && last.pending == d.pending && day(&last.when) == day(&d.when) => *k += 1,
                            _ => rows.push((d, 1)),
                        }
                    }
                    Ok(Some(rows.into_iter().enumerate().map(|(i, (d, k))| deal(root, i, d, k).into_any_element()).collect()))
                }
            },
            _ => Err(trades(root, cx)),
        };
        match rows {
            Err(e) => e,
            Ok(None) => note(root.tr("in.loading")).into_any_element(),
            Ok(Some(rows)) => {
                let lists = surface().w_full().flex().flex_col().overflow_hidden().children(rows);
                crate::scrolled::body("in-body", div().px(px(PAD + 4.)).pt(px(4.)).pb(px(24.)).child(lists)).into_any_element()
            }
        }
    };
    shell.child(header).child(body)
}

/// Chat: the chats on the left, the one open on the right with the box to say something under its lines.
fn chat(root: &Root, talks: &[Talk], cx: &mut Context<Root>) -> impl IntoElement {
    let ib = &root.inbox;
    let me = ib.of;
    let open = ib.talk_at.as_ref().and_then(|id| talks.iter().find(|t| t.id == *id));
    let list = div().id("in-talks").w(px(340.)).flex_none().h_full().overflow_y_scroll().flex().flex_col().gap(px(2.)).children(talks.iter().enumerate().map(|(i, t)| {
        let id = t.id.clone();
        let on = ib.talk_at.as_deref() == Some(t.id.as_str());
        let face = (!t.face.is_empty()).then(|| images::pic(&t.face, cx)).flatten();
        hot(("in-talk", i))
            .h(px(64.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(12.))
            .rounded(px(R_M))
            .cursor_pointer()
            .map(|d| if on { d.tones(tinted(PANEL, FACE, 0.12), tinted(PANEL, FACE, 0.16)) } else { d.tones(ca(WHITE, 0.), ca(WHITE, 0.05)) })
            .on_click(cx.listener(move |r, _, _, cx| open_talk(r, id.clone(), cx)))
            .child(round_face(face, &t.name, 42.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().truncate().text_size(px(T_M)).font_weight(if t.unread > 0 { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM }).child(t.name.clone()))
                    .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(t.last.clone())),
            )
            .when(t.unread > 0, |d| d.child(div().size(px(8.)).flex_none().rounded_full().bg(c(FACE))))
    }));

    let pane: AnyElement = match (open, &ib.lines) {
        (None, _) => note(root.tr("in.pickChat")).into_any_element(),
        (Some(_), None) => note(root.tr("in.loading")).into_any_element(),
        (Some(_), Some(Err(why))) => note(root.i18n.tv("in.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])).into_any_element(),
        (Some(t), Some(Ok(lines))) => {
            let bubbles = lines.iter().map(|l| {
                let mine = l.sender == me;
                div().flex().when(mine, |d| d.justify_end()).child(
                    div()
                        .max_w(px(560.))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        // In a group, who said it stands over their words.
                        .when(t.group && !mine, |d| d.child(div().px(px(4.)).text_size(px(T_S)).text_color(c(MUTED)).child(t.people.get(&l.sender).cloned().unwrap_or_default())))
                        .child(div().px(px(14.)).py(px(9.)).rounded(px(R_L)).bg(if mine { tinted(RAISED, FACE, 0.2) } else { c(RAISED_2) }).text_size(px(T_M)).line_height(px(21.)).text_color(c(TEXT)).child(l.text.clone())),
                )
            });
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_none()
                        .px(px(18.))
                        .py(px(12.))
                        .border_b_1()
                        .border_color(c(LINE))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(round_face((!t.face.is_empty()).then(|| images::pic(&t.face, cx)).flatten(), &t.name, 36.))
                        .child(div().flex_1().min_w_0().text_size(px(T_GROUP)).font_weight(FontWeight::SEMIBOLD).truncate().child(t.name.clone())),
                )
                .child(crate::scrolled::body("in-lines", div().px(px(18.)).py(px(14.)).flex().flex_col().gap(px(8.)).children(bubbles)))
                .child(div().flex_none().p(px(12.)).border_t_1().border_color(c(LINE)).when(ib.saying, |d| d.opacity(0.6)).child(field(&root.chat_box, Some("message")).h(px(40.)).px(px(14.))))
                .into_any_element()
        }
    };
    div().flex_1().min_h_0().px(px(PAD + 4.)).pb(px(18.)).flex().gap(px(14.)).child(list).child(surface().flex_1().min_w_0().h_full().overflow_hidden().flex().flex_col().child(pane))
}

/// Trades: the four lists Roblox keeps, and under them the list shown; a trade opens in its row.
fn trades(root: &Root, cx: &mut Context<Root>) -> AnyElement {
    let ib = &root.inbox;
    let list = ib.trade_list.min(roblox::TRADES.len() - 1);
    let kinds = div().flex().flex_wrap().items_center().gap(px(4.)).children((0..roblox::TRADES.len()).map(|i| {
        chip(("in-trades", i), list == i).h(px(TOOL_H)).px(px(12.)).cursor_pointer().child(root.tr(&format!("in.tr.{i}"))).on_click(cx.listener(move |r, _, _, cx| pick_trades(r, i, cx)))
    }));
    let body: AnyElement = match &ib.trades {
        None => note(root.tr("in.loading")).into_any_element(),
        Some(Err(why)) => note(root.i18n.tv("in.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])).into_any_element(),
        Some(Ok(l)) if l.is_empty() => note(root.tr("in.noTrades")).into_any_element(),
        Some(Ok(l)) => {
            let rows: Vec<AnyElement> = l.iter().enumerate().map(|(i, t)| trade(root, i, t, list, cx).into_any_element()).collect();
            crate::scrolled::body("in-trades-body", div().px(px(PAD + 4.)).pt(px(4.)).pb(px(24.)).flex().flex_col().gap(px(GRID_GAP)).children(rows))
                .into_any_element()
        }
    };
    div().flex_1().min_h_0().flex().flex_col().child(div().flex_none().px(px(PAD + 4.)).pb(px(14.)).flex().child(kinds)).child(body).into_any_element()
}

/// What a side is worth: its things by RAP, and its Robux.
fn worth(s: &TradeSide) -> i64 {
    s.items.iter().map(|i| i.rap).sum::<i64>() + s.robux
}

/// One side of a trade: each thing with its serial number and RAP, the Robux put in, and what it all comes to.
fn side(root: &Root, title: String, s: &TradeSide, cx: &mut Context<Root>) -> Div {
    let vi = root.i18n.vietnamese;
    let amount = |n: i64, strong: bool| {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(5.))
            .text_size(px(T_M))
            .font_features(tnum())
            .text_color(c(if strong { TEXT } else { TEXT_2 }))
            .when(strong, |d| d.font_weight(FontWeight::SEMIBOLD))
            .child(icon("robux", 13., c(if strong { TEXT } else { TEXT_2 })))
            .child(thousands(n, vi))
    };
    let items: Vec<Div> = s
        .items
        .iter()
        .map(|i| {
            let pic = (!i.pic.is_empty()).then(|| images::pic_sq(&i.pic, 44., cx)).flatten();
            let tone = (!i.pic.is_empty()).then(|| images::tone(&i.pic, cx)).flatten();
            div()
                .h(px(52.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(div().size(px(44.)).flex_none().rounded(px(R_M)).bg(match tone { Some(t) => mix(c(t), c(RAISED), 0.6), None => c(RAISED_2) }).overflow_hidden().flex().items_center().justify_center().map(|d| match pic { Some(p) => d.child(img(p).size(px(44.)).object_fit(ObjectFit::Contain)), None => d.child(icon("tag", 16., c(MUTED))) }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().text_size(px(T_M)).text_color(c(TEXT)).child(i.name.clone()))
                        .when(i.serial > 0, |d| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(format!("#{}", i.serial)))),
                )
                .child(amount(i.rap, false))
        })
        .collect();
    div()
        .flex_1()
        .min_w(px(280.))
        .p(px(14.))
        .rounded(px(R_L))
        .bg(c(INSET))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().pb(px(4.)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(title))
        .children(items)
        .when(s.robux > 0, |d| d.child(div().h(px(36.)).flex().items_center().justify_between().child(div().text_size(px(T_M)).text_color(c(TEXT)).child("Robux")).child(amount(s.robux, false))))
        .when(s.items.is_empty() && s.robux == 0, |d| d.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("in.tr.nothing"))))
        // (The two sides stand as tall as each other, their totals on one line at the foot.)
        .child(div().flex_1())
        .child(div().mt(px(4.)).pt(px(8.)).border_t_1().border_color(c(LINE)).flex().justify_end().child(amount(worth(s), true)))
}

/// A trade's row: who it is with, when, where it stands; open, its two sides, who comes out ahead by RAP, and for one
/// still open Accept (a first click arms it) and Decline, or for one the account sent, taking it back.
fn trade(root: &Root, i: usize, t: &Trade, list: usize, cx: &mut Context<Root>) -> impl IntoElement {
    let ib = &root.inbox;
    let vi = root.i18n.vietnamese;
    let id = t.id;
    let open = ib.trade_at == Some(id);
    let face = (!t.face.is_empty()).then(|| images::pic(&t.face, cx)).flatten();
    let status = {
        let key = format!("in.ts.{}", t.status);
        let said = root.tr(&key);
        if said == key { t.status.clone() } else { said }
    };
    let dates = if t.until.is_empty() || list > 1 { day(&t.when) } else { format!("{} · {}", day(&t.when), root.i18n.tv("in.tr.until", &[("d", &day(&t.until))])) };
    let head = focusable(div().id(("in-trade", i)))
        .flex()
        .items_center()
        .gap(px(12.))
        .cursor_pointer()
        .on_click(cx.listener(move |r, _, _, cx| {
            open_trade(r, id, cx);
            cx.notify();
        }))
        .child(round_face(face, &t.who, 40.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).child(t.who.clone()))
                .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(dates)),
        )
        .child(badge(status, match t.status.as_str() {
            "Open" => Tone::Warn,
            "Completed" => Tone::Ok,
            "Declined" | "RejectedDueToError" => Tone::Bad,
            _ => Tone::Muted,
        }))
        .child(icon(if open { "chevronUp" } else { "chevronDown" }, 14., c(DIM)));

    let detail: Option<Div> = open.then(|| match &ib.trade {
        None => div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("in.loading")),
        Some(Err(why)) => div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("in.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])),
        Some(Ok(d)) => {
            let gain = worth(&d.theirs) - worth(&d.mine);
            let verdict = match gain {
                0 => div().text_color(c(TEXT_2)).child(root.tr("in.tr.even")),
                g if g > 0 => div().text_color(c(OK)).child(root.i18n.tv("in.tr.win", &[("n", &thousands(g, vi))])),
                g => div().text_color(c(WARN)).child(root.i18n.tv("in.tr.lose", &[("n", &thousands(-g, vi))])),
            };
            // Only a trade still open can be answered: one that came in, or one this account sent (taken back).
            let live = t.status == "Open" && list < 2;
            let acts: Option<Div> = live.then(|| {
                if ib.trade_busy {
                    div().flex().child(Button::new("tr-busy", root.tr("in.loading")).disabled(true))
                } else if list == 1 {
                    div().flex().child(Button::new("tr-take", root.tr("in.tr.cancel")).on_click(cx.listener(|r, _, _, cx| trade_act(r, false, cx))))
                } else if ib.trade_armed {
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(Button::new("tr-confirm", root.tr("shop.confirm")).primary().on_click(cx.listener(|r, _, _, cx| trade_act(r, true, cx))))
                        .child(Button::new("tr-back", root.tr("gp.cancel")).on_click(cx.listener(|r, _, _, cx| {
                            r.inbox.trade_armed = false;
                            cx.notify();
                        })))
                } else {
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(Button::new("tr-accept", root.tr("in.tr.accept")).primary().on_click(cx.listener(|r, _, _, cx| {
                            r.inbox.trade_armed = true;
                            r.inbox.trade_said = None;
                            cx.notify();
                        })))
                        .child(Button::new("tr-decline", root.tr("in.tr.decline")).on_click(cx.listener(|r, _, _, cx| trade_act(r, false, cx))))
                }
            });
            // Turned down (Roblox may want its own check first): said, with the way to its page.
            let said = ib.trade_said.as_ref().map(|why| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().text_size(px(T_M)).text_color(c(WARN)).child(root.i18n.tv("shop.refused", &[("why", why)])))
                    .child(Button::new("tr-web", root.tr("shop.open")).small().icon("external").on_click(cx.listener(|r, _, _, _| r.send("openRobloxPage", serde_json::json!({ "kind": "trades" })))))
            });
            div()
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(div().flex().flex_wrap().items_stretch().gap(px(GRID_GAP)).child(side(root, root.tr("in.tr.give"), &d.mine, cx)).child(side(root, root.tr("in.tr.get"), &d.theirs, cx)))
                .child(verdict.text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD))
                .children(acts)
                .children(said)
        }
    });
    // (Open is not "picked": the yellow ring means chosen elsewhere in the app, so an open trade only stands out by a
    // hairline round it and one under its head.)
    block()
        .w_full()
        .px(px(18.))
        .py(px(14.))
        .border_1()
        .border_color(if open { ca(WHITE, 0.12) } else { ca(WHITE, 0.) })
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(head)
        .when(open, |d| d.child(div().h(px(1.)).mx(px(-18.)).bg(c(LINE))))
        .children(detail)
}

/// A row of a list in one card: a hairline above every row but the first.
fn row(i: usize) -> Div {
    div().px(px(18.)).py(px(13.)).when(i > 0, |d| d.border_t_1().border_color(c(LINE)))
}

/// Who something is from, round: their picture, or the first letter of their name.
fn round_face(pic: Option<std::sync::Arc<Image>>, name: &str, size: f32) -> Div {
    let first: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    div().size(px(size)).flex_none().rounded_full().bg(c(RAISED_2)).overflow_hidden().flex().items_center().justify_center().map(|d| match pic {
        Some(p) => d.child(img(p).size_full().rounded_full()),
        None => d.child(div().text_size(px(size * 0.42)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).child(first)),
    })
}

/// Messages, as mail is read: the list on the left, the one picked in full on the right. Until one is picked the
/// newest is shown there (so the pane isn't an empty box); it is marked read only when it is picked.
fn letters(root: &Root, list: &[Letter], cx: &mut Context<Root>) -> impl IntoElement {
    let ib = &root.inbox;
    let shown = ib.open.and_then(|id| list.iter().find(|m| m.id == id)).or(list.first());
    let at = shown.map(|m| m.id);
    let rows = div().id("in-letters").w(px(420.)).flex_none().h_full().overflow_y_scroll().flex().flex_col().gap(px(2.)).children(list.iter().enumerate().map(|(i, m)| letter(root, i, m, at == Some(m.id), cx).into_any_element()).collect::<Vec<_>>());
    let pane: AnyElement = match shown {
        None => div().into_any_element(),
        Some(m) => {
            let face = (!m.face.is_empty()).then(|| images::pic(&m.face, cx)).flatten();
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_none()
                        .px(px(22.))
                        .py(px(18.))
                        .border_b_1()
                        .border_color(c(LINE))
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .child(round_face(face, &m.from, 44.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .child(div().text_size(px(T_GROUP)).font_weight(FontWeight::BOLD).line_height(px(24.)).truncate().child(m.subject.clone()))
                                .child(div().text_size(px(T_M)).text_color(c(MUTED)).truncate().child(format!("{} · {}", m.from, day(&m.when)))),
                        ),
                )
                .child(
                    crate::scrolled::body("in-letter-body", 
                        div().px(px(22.)).py(px(18.)).max_w(px(760.)).text_size(px(T_M)).text_color(c(TEXT_2)).line_height(px(22.)).children(m.body.lines().map(|l| div().min_h(px(22.)).child(l.to_string()))),
                    ),
                )
                .into_any_element()
        }
    };
    div().flex_1().min_h_0().px(px(PAD + 4.)).pb(px(18.)).flex().gap(px(14.)).child(rows).child(surface().flex_1().min_w_0().h_full().overflow_hidden().child(pane))
}

/// A message in the list: who it is from (a face), when, its subject and how it begins. Unread: a dot, the name bold.
fn letter(_root: &Root, i: usize, m: &Letter, on: bool, cx: &mut Context<Root>) -> impl IntoElement {
    let id = m.id;
    let first = m.body.lines().next().unwrap_or_default().to_string();
    let face = (!m.face.is_empty()).then(|| images::pic(&m.face, cx)).flatten();
    hot(("in-letter", i))
        .h(px(78.))
        .flex_none()
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(12.))
        .rounded(px(R_M))
        .cursor_pointer()
        .map(|d| if on { d.tones(ca(WHITE, 0.07), ca(WHITE, 0.09)) } else { d.tones(ca(WHITE, 0.), ca(WHITE, 0.05)) })
        .on_click(cx.listener(move |r, _, _, cx| open(r, id, cx)))
        .child(round_face(face, &m.from, 42.))
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
                        .gap(px(8.))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).font_weight(if m.read { FontWeight::MEDIUM } else { FontWeight::SEMIBOLD }).text_color(c(TEXT)).child(m.from.clone()))
                        .child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(day(&m.when))),
                )
                .child(div().truncate().text_size(px(T_M)).text_color(c(if m.read { TEXT_2 } else { TEXT })).child(m.subject.clone()))
                .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(first)),
        )
        .when(!m.read, |d| d.child(div().size(px(8.)).flex_none().rounded_full().bg(c(FACE))))
}

fn notice(root: &Root, i: usize, n: &Notice, times: usize) -> impl IntoElement {
    // Roblox's own words when it sent them; else worded here by its kind, with the name it is about.
    let words = if !n.text.is_empty() {
        n.text.clone()
    } else {
        let key = format!("in.n.{}", n.kind);
        let known = ["FriendRequestReceived", "FriendRequestAccepted", "PrivateMessageReceived", "ExperienceInvitation"].contains(&n.kind.as_str()) && !n.name.is_empty();
        if known { root.i18n.tv(&key, &[("name", &n.name)]) } else { root.tr("in.n.other") }
    };
    row(i)
        .flex()
        .items_center()
        .gap(px(12.))
        .child(div().size(px(36.)).flex_none().rounded_full().bg(ca(WHITE, 0.05)).flex().items_center().justify_center().child(icon(
            match n.kind.as_str() {
                "FriendRequestReceived" => "userPlus",
                "FriendRequestAccepted" => "users",
                "PrivateMessageReceived" => "message",
                "ExperienceInvitation" => "gamepad",
                _ => "bell",
            },
            16.,
            c(TEXT_2),
        )))
        .child(div().flex_1().min_w_0().text_size(px(T_M)).text_color(c(TEXT)).child(words))
        .when(times > 1, |d| d.child(times_pill(times)))
        .child(div().flex_none().w(px(90.)).text_right().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(day(&n.when)))
}

/// How many times the same line came, as ×5: the × drawn as an icon at the number's size (the font's own × came out
/// a dot smaller than the figure beside it).
fn times_pill(times: usize) -> Div {
    div()
        .flex_none()
        .h(px(20.))
        .px(px(7.))
        .flex()
        .items_center()
        .gap(px(2.))
        .rounded_full()
        .bg(ca(WHITE, 0.07))
        .text_size(px(T_S))
        .font_weight(FontWeight::SEMIBOLD)
        .font_features(tnum())
        .text_color(c(TEXT_2))
        .child(icon("x", 11., c(TEXT_2)))
        .child(times.to_string())
}

fn deal(root: &Root, i: usize, d: &Deal, times: usize) -> impl IntoElement {
    let vi = root.i18n.vietnamese;
    let sum = d.amount * times as i64;
    let amount = format!("{}{}", if sum > 0 { "+" } else { "" }, thousands(sum, vi));
    let income = d.amount > 0;
    row(i)
        .flex()
        .items_center()
        .gap(px(12.))
        .child(div().size(px(36.)).flex_none().rounded_full().bg(if income { ca(OK, 0.14) } else { ca(WHITE, 0.05) }).flex().items_center().justify_center().child(icon("robux", 16., c(if income { OK } else { TEXT_2 }))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().truncate().text_size(px(T_M)).text_color(c(TEXT)).child(d.what.clone()))
                .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(if d.who.is_empty() { day(&d.when) } else { format!("{} · {}", d.who, day(&d.when)) })),
        )
        .when(times > 1, |x| x.child(times_pill(times)))
        .when(d.pending, |x| x.child(badge(root.tr("in.pending"), Tone::Muted)))
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(5.))
                .text_size(px(T_M))
                .font_weight(FontWeight::MEDIUM)
                .font_features(tnum())
                .text_color(c(if d.amount > 0 { OK } else { TEXT }))
                .child(icon("robux", 13., c(if d.amount > 0 { OK } else { TEXT })))
                .child(amount),
        )
}
