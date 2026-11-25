// The account's own settings, inside Games (its header's switch), as Roblox's Settings page has them: its email and
// phone (verified or not), birthday and country to read; its About words, who may send it trades and the least a trade
// must be worth to change. Only what Roblox's published API docs (accountinformation, accountsettings) have; the rest
// of Roblox's Settings (privacy, security, parental controls) is on Roblox's own page, opened in the account's website
// window. All of it answers only that account's own sign-in, lent by the app; asked from here (roblox.rs).
use gpui_kit::{prelude::*, *};
use serde_json::json;
use crate::time::now_secs as now;
use crate::ask::reply;

use crate::{
    kit::*,
    pages::{
        accounts::{avatar, label},
        friends,
    },
    roblox::{self, AccountSettings, TRADE_PRIVACY, TRADE_VALUE},
    shell::Root,
    theme::*,
};

/// The page's parts, one shown at a time and picked on the left, as Bloxkeep's own Settings has its parts
/// (settings.rs): everything on one long page was too much (the owner, 2026-11-25).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Part {
    #[default]
    Account,
    Privacy,
    Places,
    Trades,
    More,
}

impl Part {
    const ALL: [Part; 5] = [Part::Account, Part::Privacy, Part::Places, Part::Trades, Part::More];
    fn title(self) -> &'static str {
        match self {
            Part::Account => "rs.account",
            Part::Privacy => "rs.privacy",
            Part::Places => "rs.places",
            Part::Trades => "rs.trades",
            Part::More => "rs.moreHead",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Part::Account => "user",
            Part::Privacy => "shield",
            Part::Places => "monitor",
            Part::Trades => "repeat",
            Part::More => "external",
        }
    }
}

#[derive(Default)]
pub struct Data {
    /// The part shown (kept when another account's settings are read).
    pub part: Part,
    /// The Roblox user these are of, the settings (None while they are read), and whether they are being read.
    of: i64,
    got: Option<Result<AccountSettings, String>>,
    reading: bool,
    /// A save on its way ("about", "trade", "value"), and what came of the last one (saved, or why not).
    saving: Option<&'static str>,
    said: Option<Result<(), String>>,
    /// The About words to put in the box (set when they come; the frame puts them in, as that needs the window).
    fill: Option<String>,
    /// Which voice-lock countdown is the live one (a new read stops the old), and whether one runs.
    ticks: u64,
    ticking: bool,
    /// Who sees it online and who may join it (Roblox's words and the choices it offers this account; each None when
    /// Roblox has no such setting for it), None while read.
    vis: Option<(Option<roblox::Choice>, Option<roblox::Choice>)>,
    /// Roblox didn't answer who sees the account (the Privacy card says so instead of "Reading" for good).
    /// Why the visibility read failed (Roblox's code, or the network's words), shown as it is.
    vis_failed: Option<String>,
    /// Account status, as Roblox's own page has it: nothing against it (Ok None), a ban or a warning, or the read
    /// failed. It was only behind the card's ⋮ menu, where the owner never found it (2026-11-15).
    held: Option<Result<Option<roblox::Held>, String>>,
    /// The account's violations (Roblox's Account status page lists them), None while read; every one shown or the
    /// first five.
    violations: Option<Result<Vec<roblox::Violation>, String>>,
    vio_all: bool,
    /// Where the account is signed in (Roblox's Settings › Security › Where you're logged in), None while read; every
    /// one shown or the first ten (Roblox's page shows ten, then Show more); the places being logged out now.
    places: Option<Result<roblox::Places, String>>,
    places_all: bool,
    leaving: Vec<String>,
    /// The people the account has blocked (Roblox's Settings › Privacy › Blocked users), None while not asked; read
    /// while `reading_blocked`; the ones being unblocked now.
    blocked: Option<Result<Vec<roblox::Friend>, String>>,
    reading_blocked: bool,
    unblocking: Vec<i64>,
    /// Content maturity (Roblox's Settings › Privacy & content maturity), None while read; the level picked that needs
    /// what Bloxkeep can't do (an age check, a parent), said under the levels.
    maturity: Option<Result<Option<roblox::Maturity>, String>>,
    gated: Option<(String, String)>,
    /// The age group Roblox has for the account (Account info › Age group), and its account PIN: on, and unlocked for
    /// how long (Settings can't be changed while it is locked); an Unlock on its way and Roblox's word on the last.
    age: Option<roblox::Age>,
    pin: Option<roblox::Pin>,
    pin_at: Option<std::time::Instant>,
    unlocking: bool,
    pin_said: Option<String>,
}

impl Data {
    /// The blocked list is read again next time (someone was blocked or unblocked elsewhere in the app).
    pub fn forget_blocked(&mut self) {
        if !self.reading_blocked {
            self.blocked = None;
        }
    }
}

pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    friends::want_sign(r, link, cx);
    let Some((_, user, _)) = friends::signed(r) else { return };
    if r.rset.of != user {
        r.rset = Data { of: user, part: r.rset.part, ..Default::default() };
    }
    // (BLOXKEEP_SHOT_RSET=privacy|places|trades|more: a hidden shot of that part, picked once.)
    #[cfg(feature = "shot")]
    {
        static PICKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !PICKED.swap(true, std::sync::atomic::Ordering::Relaxed)
            && let Some(p) = std::env::var("BLOXKEEP_SHOT_RSET").ok().and_then(|k| Part::ALL.into_iter().find(|p| p.title().ends_with(&k) || (k == "more" && *p == Part::More)))
        {
            r.rset.part = p;
        }
    }
    if r.rset.got.is_none() && !r.rset.reading {
        load(r, cx);
    }
    if r.rset.blocked.is_none() && !r.rset.reading_blocked {
        load_blocked(r, cx);
    }
    // A voice lock's countdown, again on coming back.
    let until = r.rset.got.as_ref().and_then(|g| g.as_ref().ok()).and_then(|s| s.voice.as_ref()).filter(|v| v.banned).and_then(|v| v.until);
    if let (Some(until), false) = (until, r.rset.ticking) {
        tick_voice(r, user, until, cx);
    }
}

fn load(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    r.rset.reading = true;
    let c3 = cookie.clone();
    let held = roblox::run(move || roblox::held(&c3));
    reply(held, cx, move |r, got, cx| {
        if r.rset.of == user {
            r.rset.held = Some(got);
            cx.notify();
        }
    });
    let c6 = cookie.clone();
    let vio = roblox::run(move || roblox::violations(&c6, user));
    reply(vio, cx, move |r, got, cx| {
        if r.rset.of == user {
            r.rset.violations = Some(got);
            cx.notify();
        }
    });
    let c4 = cookie.clone();
    let places = roblox::run(move || roblox::sessions(&c4).map(roblox::places));
    reply(places, cx, move |r, got, cx| {
        if r.rset.of == user {
            r.rset.places = Some(got);
            cx.notify();
        }
    });
    let c5 = cookie.clone();
    let more = roblox::run(move || (roblox::maturity(&c5), roblox::age(&c5), roblox::pin(&c5)));
    crate::ask::answer(more, cx, move |r, got, cx| {
        let Ok((maturity, age, pin)) = got else { return };
        if r.rset.of == user {
            r.rset.maturity = Some(maturity);
            r.rset.age = age.ok();
            r.rset.pin = pin.ok();
            r.rset.pin_at = Some(std::time::Instant::now());
            cx.notify();
        }
    });
    let c2 = cookie.clone();
    let vis = roblox::run(move || roblox::visibility(&c2));
    reply(vis, cx, move |r, got, cx| {
        if r.rset.of == user {
            match got {
                Ok(v) => r.rset.vis = Some(v),
                Err(e) => r.rset.vis_failed = Some(if e.is_empty() { "no answer".into() } else { e }),
            }
            cx.notify();
        }
    });
    let rx = roblox::run(move || roblox::account_settings(&cookie));
    reply(rx, cx, move |r, got, cx| {
        if r.rset.of == user {
            r.rset.fill = Some(got.as_ref().ok().and_then(|s| s.about.clone()).unwrap_or_default());
            let until = got.as_ref().ok().and_then(|s| s.voice.as_ref()).filter(|v| v.banned).and_then(|v| v.until);
            r.rset.got = Some(got);
            r.rset.reading = false;
            if let Some(until) = until {
                tick_voice(r, user, until, cx);
            }
            cx.notify();
        }
    });
}

/// A voice lock's countdown, redrawn every 20 s while these settings are shown; at its end Roblox is asked again.
/// The people the account has blocked.
fn load_blocked(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    r.rset.reading_blocked = true;
    let rx = roblox::run(move || roblox::blocked(&cookie));
    reply(rx, cx, move |r, got, cx| {
        if r.rset.of == user {
            r.rset.reading_blocked = false;
            r.rset.blocked = Some(got);
            cx.notify();
        }
    });
}

/// Unblocks one of them, after the question Roblox's own list asks.
fn unblock(r: &mut Root, who: i64, name: String, window: &mut Window, cx: &mut Context<Root>) {
    if r.rset.unblocking.contains(&who) {
        return;
    }
    let title = r.i18n.tv("blk.unTitle", &[("name", &name)]);
    let (text, ok) = (r.tr("blk.unText"), r.tr("blk.unblock"));
    r.confirm(title, text, ok, false, None, window, cx, move |r, cx| {
        let Some((_, user, cookie)) = friends::signed(r) else { return };
        r.rset.unblocking.push(who);
        cx.notify();
        let rx = roblox::run(move || roblox::set_blocked(&cookie, who, false));
        reply(rx, cx, move |r, got, cx| {
            if r.rset.of != user {
                return;
            }
            r.rset.unblocking.retain(|u| *u != who);
            match got {
                Ok(()) => {
                    if let Some(Ok(l)) = r.rset.blocked.as_mut() {
                        l.retain(|f| f.id != who);
                    }
                    r.toast = Some((r.tr("blk.undone"), "ok".into()));
                }
                Err(why) => r.toast = Some((r.i18n.tv("blk.failed", &[("why", crate::net::why_words(&r.i18n, &why).as_str())]), "bad".into())),
            }
            cx.notify();
        });
    });
}

/// The blocked people as Roblox's Settings › Privacy lists them: each face, name and @name, with Unblock.
fn blocked_card(root: &Root, cx: &mut Context<Root>) -> Div {
    let rs = &root.rset;
    let note = |text: String| div().px(px(20.)).py(px(18.)).text_size(px(T_M)).text_color(c(MUTED)).child(text);
    let card = surface().overflow_hidden().flex().flex_col();
    let list = match &rs.blocked {
        None => return card.child(note(root.tr("vis.reading"))),
        Some(Err(why)) => return card.child(note(root.i18n.tv("blk.listFailed", &[("why", crate::net::why_words(&root.i18n, why).as_str())]))),
        Some(Ok(l)) if l.is_empty() => return card.child(note(root.tr("blk.none"))),
        Some(Ok(l)) => l,
    };
    let rows: Vec<AnyElement> = list
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let pic = (!f.face.is_empty()).then(|| crate::images::pic(&f.face, cx)).flatten();
            let shown = if f.display.is_empty() { f.name.clone() } else { f.display.clone() };
            let (who, name) = (f.id, shown.clone());
            div()
                .min_h(px(60.))
                .px(px(20.))
                .py(px(10.))
                .flex()
                .items_center()
                .gap(px(12.))
                .when(i > 0, |d| d.border_t_1().border_color(c(LINE)))
                .child(face(&shown, pic, 40.))
                .child(title(shown, (!f.name.is_empty()).then(|| format!("@{}", f.name))))
                .child(Button::new(("blk-un", i), root.tr("blk.unblock")).small().quiet().disabled(rs.unblocking.contains(&who)).on_click(cx.listener(move |r, _, window, cx| unblock(r, who, name.clone(), window, cx))))
                .into_any_element()
        })
        .collect();
    card.children(rows)
}

fn tick_voice(r: &mut Root, user: i64, until: i64, cx: &mut Context<Root>) {
    r.rset.ticks += 1;
    r.rset.ticking = true;
    let run = r.rset.ticks;
    cx.spawn(async move |this, cx| loop {
        cx.background_executor().timer(std::time::Duration::from_secs(20)).await;
        let on = this
            .update(cx, |r, cx| {
                // (Only while it is shown: back on it, want() starts it again.)
                let shown = r.page == crate::pages::Page::Games && r.games.view == crate::pages::games::View::Settings;
                if r.rset.of != user || r.rset.ticks != run || r.rset.got.is_none() || !shown {
                    if r.rset.ticks == run {
                        r.rset.ticking = false;
                    }
                    return false;
                }
                if now() >= until {
                    again(r, cx);
                    return false;
                }
                cx.notify();
                true
            })
            .unwrap_or(false);
        if !on {
            break;
        }
    })
    .detach();
}

/// What's left of a lock, the two largest parts ("2 d 4 h", "3 h 12 m", "5 m").
fn left(root: &Root, secs: i64) -> String {
    let m = (secs.max(60) + 59) / 60;
    let (d, h, m) = (m / 1440, m / 60 % 24, m % 60);
    let key = if d > 0 { "rs.leftDH" } else if h > 0 { "rs.leftHM" } else { "rs.leftM" };
    root.i18n.tv(key, &[("d", &d.to_string()), ("h", &h.to_string()), ("m", &m.to_string())])
}

/// The frame puts the About words in their box.
pub fn settle(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if let Some(words) = r.rset.fill.take() {
        r.rset_about.update(cx, |s, cx| s.set_value(words, window, cx));
    }
}

fn again(r: &mut Root, cx: &mut Context<Root>) {
    if r.rset.reading {
        return;
    }
    r.rset.got = None;
    r.rset.said = None;
    r.rset.places = None;
    load(r, cx);
    cx.notify();
}

/// Logs the account out of these places (Roblox's Log out, one after another), after asking: `title` and `text` say
/// which. Each gone one leaves the list; one Roblox refused stays, and why is said.
fn log_out(r: &mut Root, tokens: Vec<String>, title: String, text: String, window: &mut Window, cx: &mut Context<Root>) {
    if tokens.is_empty() || !r.rset.leaving.is_empty() {
        return;
    }
    r.confirm(title, text, r.tr("rs.logOut"), true, None, window, cx, move |r, cx| {
        let Some((_, user, cookie)) = friends::signed(r) else { return };
        r.rset.leaving = tokens.clone();
        cx.notify();
        let list = tokens.clone();
        let rx = roblox::run(move || list.into_iter().map(|t| { let done = roblox::log_out_session(&cookie, &t); (t, done) }).collect::<Vec<_>>());
        cx.spawn(async move |this, cx| {
            let Ok(done) = rx.await else { return };
            let _ = this.update(cx, |r, cx| {
                if r.rset.of != user {
                    return;
                }
                r.rset.leaving.clear();
                let gone: Vec<&String> = done.iter().filter(|(_, d)| d.is_ok()).map(|(t, _)| t).collect();
                if let Some(Ok(p)) = r.rset.places.as_mut() {
                    p.known.retain(|s| !gone.contains(&&s.token));
                    p.unknown.retain(|s| !gone.contains(&&s.token));
                }
                let failed = done.iter().filter_map(|(_, d)| d.as_ref().err()).count();
                r.toast = Some(match done.iter().find_map(|(_, d)| d.as_ref().err()) {
                    None => (r.i18n.tn("rs.loggedOut", gone.len() as i64, &[]), "ok".into()),
                    Some(why) => (
                        r.i18n.tv("rs.logOutFailed", &[("n", &failed.to_string()), ("why", crate::net::why_words(&r.i18n, why).as_str())]),
                        "bad".into(),
                    ),
                });
                cx.notify();
            });
        })
        .detach();
    });
}

/// A place's device in words, as Roblox's page names it (getDeviceDetails): the Roblox app or Studio on a system, a
/// browser on a system, or an unknown device.
fn device(root: &Root, s: &roblox::Session) -> String {
    let on = |what: String| if s.os.is_empty() { what } else { format!("{what} · {}", s.os) };
    match s.kind.as_str() {
        "App" => on(root.tr("rs.robloxApp")),
        "Studio" => on("Roblox Studio".into()),
        "Browser" => on(if s.app.is_empty() { root.tr("rs.browser") } else { s.app.clone() }),
        _ if !s.os.is_empty() => on(root.tr("rs.robloxApp")),
        _ => root.tr("rs.unknownDevice"),
    }
}

/// The mark beside a place: a console, a browser, the Roblox app, or a PC.
fn device_mark(s: &roblox::Session) -> &'static str {
    let os = s.os.to_ascii_lowercase();
    if os.contains("xbox") || os.contains("playstation") {
        "gamepad"
    } else if s.kind == "Browser" {
        "globe"
    } else if s.kind == "App" {
        "roblox"
    } else {
        "monitor"
    }
}

/// Where, roughly (Roblox's city, region, country), or that Roblox doesn't know.
fn place(root: &Root, s: &roblox::Session) -> String {
    if s.place.is_empty() { root.tr("rs.unknownPlace") } else { s.place.join(", ") }
}

/// The About words in the box, saved; the box then holds them as Roblox kept them.
fn save_about(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    if r.rset.saving.is_some() {
        return;
    }
    let words = r.rset_about.read(cx).value().to_string();
    r.rset.saving = Some("about");
    r.rset.said = None;
    let rx = roblox::run(move || roblox::set_about(&cookie, &words));
    reply(rx, cx, move |r, done, cx| {
        if r.rset.of != user {
            return;
        }
        r.rset.saving = None;
        match done {
            Ok(kept) => {
                if let Some(Ok(s)) = r.rset.got.as_mut() {
                    s.about = Some(kept.clone());
                }
                r.rset.fill = Some(kept);
                r.rset.said = Some(Ok(()));
            }
            Err(why) => r.rset.said = Some(Err(why)),
        }
        cx.notify();
    });
}

/// Who may trade (`privacy`) or the least a trade must be worth, set to Roblox's `name` for it: shown at once, put
/// back as it was when Roblox refuses.
fn set_trade(r: &mut Root, privacy: bool, name: String, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let list: &[(&str, i64)] = if privacy { &TRADE_PRIVACY } else { &TRADE_VALUE };
    let Some(n) = list.iter().find(|(k, _)| *k == name).map(|(_, n)| *n) else { return };
    let Some(Ok(s)) = r.rset.got.as_mut() else { return };
    if r.rset.saving.is_some() {
        return;
    }
    let slot = if privacy { &mut s.trade_privacy } else { &mut s.trade_value };
    let before = std::mem::replace(slot, Some(name));
    r.rset.saving = Some(if privacy { "trade" } else { "value" });
    r.rset.said = None;
    let rx = roblox::run(move || if privacy { roblox::set_trade_privacy(&cookie, n) } else { roblox::set_trade_value(&cookie, n) });
    reply(rx, cx, move |r, done, cx| {
        if r.rset.of != user {
            return;
        }
        r.rset.saving = None;
        if let Err(why) = done {
            if let Some(Ok(s)) = r.rset.got.as_mut() {
                *(if privacy { &mut s.trade_privacy } else { &mut s.trade_value }) = before;
            }
            r.rset.said = Some(Err(why));
        } else {
            r.rset.said = Some(Ok(()));
        }
        cx.notify();
    });
}

/// Who sees it online (`online`) or may join it, set to Roblox's `v`: shown at once, put back when Roblox refuses.
fn set_vis(r: &mut Root, online: bool, v: String, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    if r.rset.saving.is_some() {
        return;
    }
    let Some(now) = r.rset.vis.as_mut() else { return };
    let Some(slot) = (if online { now.0.as_mut() } else { now.1.as_mut() }) else { return };
    let before = std::mem::replace(&mut slot.now, v.clone());
    r.rset.saving = Some(if online { "online" } else { "join" });
    r.rset.said = None;
    let rx = roblox::run(move || if online { roblox::set_visibility(&cookie, Some(&v), None) } else { roblox::set_visibility(&cookie, None, Some(&v)) });
    reply(rx, cx, move |r, done, cx| {
        if r.rset.of != user {
            return;
        }
        r.rset.saving = None;
        if let Err(why) = done {
            if let Some(slot) = r.rset.vis.as_mut().and_then(|now| if online { now.0.as_mut() } else { now.1.as_mut() }) {
                slot.now = before;
            }
            r.toast = Some((r.i18n.tv("rs.failed", &[("why", if why.is_empty() { "Roblox" } else { &why })]), "bad".into()));
        }
        cx.notify();
    });
}

/// Content maturity set to Roblox's `level`: shown at once, put back when Roblox refuses. A level that needs an age
/// check or a parent first isn't sent: what it needs is said under the levels, with Roblox's own page.
fn set_maturity(r: &mut Root, level: String, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    if r.rset.saving.is_some() {
        return;
    }
    let Some(Ok(Some(m))) = r.rset.maturity.as_mut() else { return };
    if let Some(need) = m.options.iter().find(|(v, _)| *v == level).and_then(|(_, need)| need.clone()) {
        r.rset.gated = Some((level, need));
        cx.notify();
        return;
    }
    r.rset.gated = None;
    let before = std::mem::replace(&mut m.now, level.clone());
    r.rset.saving = Some("maturity");
    let rx = roblox::run(move || roblox::set_maturity(&cookie, &level));
    reply(rx, cx, move |r, done, cx| {
        if r.rset.of != user {
            return;
        }
        r.rset.saving = None;
        if let Err(why) = done {
            if let Some(Ok(Some(m))) = r.rset.maturity.as_mut() {
                m.now = before;
            }
            r.toast = Some((r.i18n.tv("rs.failed", &[("why", if why.is_empty() { "Roblox" } else { &why })]), "bad".into()));
        }
        cx.notify();
    });
}

/// The settings unlocked with the PIN typed (Roblox's "Account PIN Required" › Unlock).
pub fn unlock_pin(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let pin: String = r.rs_pin.read(cx).value().trim().to_string();
    if r.rset.unlocking || pin.is_empty() {
        return;
    }
    r.rset.unlocking = true;
    r.rset.pin_said = None;
    r.rs_pin.update(cx, |s, cx| s.set_value("", window, cx));
    let rx = roblox::run(move || roblox::unlock_pin(&cookie, &pin));
    reply(rx, cx, move |r, done, cx| {
        if r.rset.of != user {
            return;
        }
        r.rset.unlocking = false;
        match done {
            Ok(p) => {
                r.rset.pin = Some(p);
                r.rset.pin_at = Some(std::time::Instant::now());
            }
            Err(why) => r.rset.pin_said = Some(if why.contains("403") || why.to_lowercase().contains("incorrect") { r.tr("rs.pinWrong") } else { r.i18n.tv("rs.failed", &[("why", if why.is_empty() { "Roblox" } else { &why })]) }),
        }
        cx.notify();
    });
}

/// Locked again before the unlock runs out (Roblox's padlock).
fn lock_pin(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let rx = roblox::run(move || roblox::lock_pin(&cookie));
    reply(rx, cx, move |r, done, cx| {
        if r.rset.of == user && done.is_ok() {
            r.rset.pin = Some(roblox::Pin { on: true, left: None });
            cx.notify();
        }
    });
}

/// The seconds the PIN's unlock still lasts now (None locked or off).
fn pin_left(rs: &Data) -> Option<i64> {
    let p = rs.pin.filter(|p| p.on)?;
    let gone = rs.pin_at.map_or(0, |t| t.elapsed().as_secs() as i64);
    p.left.map(|l| l - gone).filter(|l| *l > 0)
}

/// A name (14 px, medium) and what it is under it in muted, as Settings' rows have them.
fn title(text: String, sub: Option<String>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(div().text_size(px(T_L)).line_height(px(20.)).font_weight(FontWeight::MEDIUM).child(text))
        .when_some(sub, |d, s| d.child(div().text_size(px(T_M)).line_height(px(18.)).text_color(c(MUTED)).child(s)))
}

/// A row of the account card: what, and its value (words, or a state word), a hairline above all but the first.
fn row(i: usize, what: String, value: AnyElement) -> Div {
    div()
        .h(px(52.))
        .px(px(20.))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(14.))
        .text_size(px(T_L))
        .when(i > 0, |d| d.border_t_1().border_color(c(LINE)))
        .child(div().flex_none().text_color(c(TEXT_2)).child(what))
        .child(div().min_w_0().flex().items_center().gap(px(10.)).child(value))
}

/// A section: its head (kit::section_head), then its card.
pub(super) fn section(head: impl Into<SharedString>, card: impl IntoElement) -> Div {
    div().flex().flex_col().gap(px(8.)).child(section_head(head, None)).child(card)
}

pub fn page(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let rs = &root.rset;
    let (able, viewer) = friends::looking_ref(root);
    let shell = div().id("rsettings").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let header = div()
        .flex_none()
        .px(px(PAGE_X))
        .pt(px(22.))
        .pb(px(20.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(crate::pages::games::brand(root))
        .relative().child(crate::pages::games::island(root, cx))
        .child(div().flex_1())
        .when(viewer.is_some(), |d| d.child(IconButton::new("rs-refresh", "refresh").size(34.).busy(rs.reading).tip(root.tr("fr.refresh")).on_click(cx.listener(|r, _, _, cx| again(r, cx)))));

    let body: AnyElement = if able.is_empty() || friends::nobody(root) {
        friends::need_account(root, "rs-add-acc", "rs.noAccount", "sliders", cx)
    } else {
        match &rs.got {
            None => empty_note(root.tr("rs.loading")).into_any_element(),
            Some(Err(why)) => empty_note(root.i18n.tv("rs.readFailed", &[("why", crate::net::why_words(&root.i18n, why).as_str())])).into_any_element(),
            Some(Ok(s)) => {
                let compact = width < 760.;
                let nav_w = if compact { 64. } else if width < 1100. { 198. } else { 230. };
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .flex()
                    .child(nav(root, compact, nav_w, cx))
                    .child(div().flex_1().min_w_0().h_full().child(filled(root, s, viewer, cx)))
                    .into_any_element()
            }
        }
    };
    shell.child(header).child(body)
}

/// The parts' list, as Settings' own (settings.rs: the same rows, the same column and widths; icons only when narrow).
fn nav(root: &Root, compact: bool, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let part = root.rset.part;
    let mut list = div().w(px(width)).flex_none().flex().flex_col().gap(px(4.)).px(px(10.)).pt(px(4.)).pb(px(20.));
    for (i, p) in Part::ALL.into_iter().enumerate() {
        let on = p == part;
        let tip: SharedString = root.tr(p.title()).into();
        list = list.child(
            menu_row(("rs-nav", i))
                .relative()
                .h(px(39.))
                .w_full()
                .px(px(if compact { 0. } else { 12. }))
                .when(compact, |d| d.justify_center().tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
                .border_1()
                .edge(if on { ca(FACE, 0.25) } else { ca(WHITE, 0.) }, if on { ca(FACE, 0.40) } else { ca(WHITE, 0.10) })
                .tones(if on { ca(FACE, 0.11) } else { ca(WHITE, 0.) }, if on { ca(FACE, 0.17) } else { ca(WHITE, 0.05) })
                .text_color(c(if on { TEXT } else { TEXT_2 }))
                .text_size(px(T_L))
                .font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                .child(icon(p.icon(), 17., if on { c(FACE) } else { c(MUTED) }))
                .when(!compact, |d| d.child(div().flex_1().min_w_0().truncate().child(root.tr(p.title()))))
                .when(!on, |d| {
                    d.on_click(cx.listener(move |r, _, _, cx| {
                        r.rset.part = p;
                        crate::scrolled::handle("rs-body").set_offset(point(px(0.), px(0.)));
                        cx.notify();
                    }))
                }),
        );
    }
    if crate::theme::glass_blocks() {
        div()
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .pl(px(PAGE_X))
            .pb(px(16.))
            .child(crate::kit::block_of(R_L, ca(GROUND, 0.34)).min_h_0().flex().flex_col().overflow_hidden().pt(px(12.)).child(div().id("rs-nav").min_h_0().overflow_y_scroll().child(list)))
            .into_any_element()
    } else {
        div().id("rs-nav").h_full().flex_none().overflow_y_scroll().border_r_1().border_color(c(LINE)).bg(ca(GROUND, 0.34)).pl(px(PAGE_X - 10.)).pt(px(12.)).child(list).into_any_element()
    }
}

/// A part's own head, as Settings' (its mark in a tile, its name).
fn part_head(root: &Root, p: Part) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(14.))
        .pb(px(6.))
        .child(div().size(px(46.)).flex_none().rounded(px(R_M)).bg(ca(FACE, 0.12)).flex().items_center().justify_center().child(icon(p.icon(), 21., c(FACE))))
        .child(div().min_w_0().text_size(px(T_FIG)).line_height(px(28.)).font_weight(FontWeight::BOLD).child(root.tr(p.title())))
}

fn filled(root: &Root, s: &AccountSettings, viewer: Option<&serde_json::Value>, cx: &mut Context<Root>) -> impl IntoElement {
    let rs = &root.rset;
    let dash = || div().text_color(c(MUTED)).child(root.tr("rs.unknown")).into_any_element();
    let words = |t: String| div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).font_features(tnum()).child(t).into_any_element();

    // The account: who, then what Roblox has of it.
    let mut rows: Vec<(String, AnyElement)> = Vec::new();
    // Account status first: good standing, or what Roblox holds against it and until when, and Roblox's own page.
    let account_id = viewer.and_then(|a| a.get("id")).and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
    let open = Button::new("rs-status-open", root.tr("rs.statusOpen")).small().quiet().icon_end("external").on_click(cx.listener(move |r, _, _, _| r.send("openAppeal", serde_json::json!({ "id": account_id }))));
    rows.push((
        root.tr("m.status"),
        match &rs.held {
            None => div().text_color(c(MUTED)).child(root.tr("vis.reading")).into_any_element(),
            Some(Err(_)) => div().flex().items_center().gap(px(10.)).child(dash()).child(open).into_any_element(),
            Some(Ok(None)) => div().flex().items_center().gap(px(10.)).child(badge(root.tr("rs.statusGood"), Tone::Ok)).child(open).into_any_element(),
            Some(Ok(Some(h))) => div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(badge(if h.kind.is_empty() { root.tr("rs.statusHeld") } else { h.kind.clone() }, if h.kind.to_lowercase().contains("warn") { Tone::Warn } else { Tone::Bad }))
                .when_some(h.ends, |d, t| d.child(div().text_color(c(MUTED)).font_features(tnum()).child(root.i18n.tv("rs.statusEnds", &[("t", &crate::time::day_time(t))]))))
                .child(open)
                .into_any_element(),
        },
    ));
    rows.push((
        root.tr("rs.email"),
        match &s.email {
            // (Names hidden: an email names its owner as a name does.)
            Some((mail, _)) if !mail.is_empty() && root.hides_names() => badge(root.tr("rs.hidden"), Tone::Muted).into_any_element(),
            Some((mail, ok)) if !mail.is_empty() => div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(words(mail.clone()))
                .child(badge(root.tr(if *ok { "rs.verified" } else { "rs.unverified" }), if *ok { Tone::Ok } else { Tone::Warn }))
                .into_any_element(),
            Some(_) => badge(root.tr("rs.none"), Tone::Muted).into_any_element(),
            None => dash(),
        },
    ));
    rows.push((
        root.tr("rs.phone"),
        match s.phone {
            Some(true) => badge(root.tr("rs.verified"), Tone::Ok).into_any_element(),
            Some(false) => badge(root.tr("rs.none"), Tone::Muted).into_any_element(),
            None => dash(),
        },
    ));
    rows.push((
        root.tr("rs.birthday"),
        match s.birthday {
            Some(_) if root.hides_names() => badge(root.tr("rs.hidden"), Tone::Muted).into_any_element(),
            Some((d, m, y)) if root.i18n.lang != "en" => words(crate::lang::cur().dmy(d as i64, m as i64, y as i64)),
            Some((d, m, y)) => words(format!("{y}-{m:02}-{d:02}")),
            None => dash(),
        },
    ));
    if let Some(a) = rs.age.as_ref().filter(|a| !roblox::age_words(&a.group).is_empty()) {
        rows.push((
            root.tr("rs.ageGroup"),
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(words(roblox::age_words(&a.group)))
                .when(a.checked, |d| d.child(badge(root.tr("rs.ageChecked"), Tone::Ok)))
                .into_any_element(),
        ));
    }
    rows.push((root.tr("rs.country"), s.country.clone().map(words).unwrap_or_else(dash)));
    // Voice chat: on, off (why), or locked: how long is left and when it ends, or for good.
    rows.push((
        root.tr("rs.voice"),
        match &s.voice {
            None => dash(),
            Some(v) if v.banned => match v.until {
                Some(t) => {
                    let ends = root.i18n.tv("rs.voiceEnds", &[("t", &crate::time::day_time(t))]);
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(div().text_color(c(MUTED)).font_features(tnum()).child(ends))
                        .child(badge(root.i18n.tv("rs.voiceLocked", &[("left", &left(root, t - now()))]), Tone::Warn))
                        .into_any_element()
                }
                None => badge(root.tr("rs.voiceForever"), Tone::Bad).into_any_element(),
            },
            Some(v) if v.on => badge(root.tr("rs.voiceOn"), Tone::Ok).into_any_element(),
            Some(v) if v.eligible && !v.opted => badge(root.tr("rs.voiceOff"), Tone::Muted).into_any_element(),
            Some(_) => badge(root.tr("rs.voiceNo"), Tone::Muted).into_any_element(),
        },
    ));
    // The account PIN: off, locked, or unlocked and for how long (Lock locks it again).
    if let Some(p) = rs.pin {
        rows.push((
            root.tr("rs.pin"),
            match pin_left(rs) {
                _ if !p.on => badge(root.tr("rs.pinOff"), Tone::Muted).into_any_element(),
                None => badge(root.tr("rs.pinLocked"), Tone::Warn).into_any_element(),
                Some(l) => div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(badge(root.i18n.tv("rs.pinOpen", &[("t", &left(root, l))]), Tone::Ok))
                    .child(Button::new("rs-pin-lock", root.tr("rs.pinLock")).small().quiet().icon("lock").on_click(cx.listener(|r, _, _, cx| lock_pin(r, cx))))
                    .into_any_element(),
            },
        ));
    }
    let who = viewer.map(|a| {
        let user = a.get("username").and_then(serde_json::Value::as_str).filter(|u| !u.is_empty()).map(|u| format!("@{u}"));
        let (presence, tone) = roblox_presence(a);
        let said = vec![(user.unwrap_or_else(|| root.tr("rs.account")), None), (root.tr(presence), Some(c(tone)))];
        div().px(px(20.)).pt(px(18.)).pb(px(16.)).border_b_1().border_color(c(LINE)).child(group_head(avatar(a, GROUP_PIC, RAISED, cx), label(a), facts(said)))
    });
    let account = surface().overflow_hidden().flex().flex_col().children(who)
        .child(div().flex().flex_col().children(rows.into_iter().enumerate().map(|(i, (k, v))| row(i, k, v))));

    let busy = rs.saving.is_some();
    let trades = trades_card(root, s.trade_privacy.as_deref(), s.trade_value.as_deref(), busy, set_trade, cx);
    let vis = rs.vis.as_ref().map(|(o, j)| (o.as_ref(), j.as_ref()));
    let privacy = privacy_card(root, vis, rs.vis_failed.as_deref(), busy, set_vis, cx);
    let maturity = maturity_card(root, busy, viewer, cx);
    // Settings locked by the account PIN: Roblox refuses every change until it is typed (its "Account PIN Required").
    let locked = rs.pin.is_some_and(|p| p.on) && pin_left(rs).is_none();
    let pin_bar = locked.then(|| {
        surface()
            .border_1()
            .border_color(ca(WARN, 0.55))
            .px(px(20.))
            .py(px(14.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(14.))
            .child(icon("lock", 19., c(WARN)))
            .child(title(root.tr("rs.pinBar"), Some(rs.pin_said.clone().unwrap_or_else(|| root.tr("rs.pinSub")))))
            .child(div().w(px(150.)).child(field(&root.rs_pin, Some("key"))))
            .child(Button::new("rs-pin-unlock", root.tr("rs.pinUnlock")).primary().small().disabled(rs.unlocking).on_click(cx.listener(|r, _, window, cx| unlock_pin(r, window, cx))))
    });

    // About me: the box, and Save when it holds something else.
    let typed = root.rset_about.read(cx).value().to_string();
    let changed = s.about.as_deref().unwrap_or_default() != typed;
    // As Settings' rows: its name and what it is, the box under them from the same edge; a foot (a hairline above) only
    // while there is something to say or save. (Save shows once the words differ from Roblox's: a greyed yellow block
    // read as broken.)
    let said = rs.said.as_ref().filter(|_| !(changed && matches!(rs.said, Some(Ok(())))));
    let about = surface()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(div().px(px(20.)).pt(px(16.)).pb(px(18.)).flex().flex_col().gap(px(14.)).child(title(root.tr("rs.bio"), Some(root.tr("rs.aboutSub")))).child(crate::kit::text_area(&root.rset_about)))
        .when(changed || said.is_some(), |d| {
            d.child(
                div()
                    .min_h(px(56.))
                    .px(px(20.))
                    .py(px(12.))
                    .border_t_1()
                    .border_color(c(LINE))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().flex_1().min_w_0().map(|d| match said {
                        Some(Ok(())) => d.child(div().flex().child(badge(root.tr("rs.saved"), Tone::Ok))),
                        Some(Err(why)) => d.text_size(px(T_S)).text_color(c(BAD)).child(root.i18n.tv("rs.failed", &[("why", crate::net::why_words(&root.i18n, why).as_str())])),
                        None => d.text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("rs.unsaved")),
                    }))
                    .when(changed, |d| d.child(Button::new("rs-save", root.tr("rs.save")).primary().small().disabled(busy).on_click(cx.listener(|r, _, _, cx| save_about(r, cx))))),
            )
        });

    // The rest is Roblox's own page, in the account's website window.
    let id = viewer.and_then(|a| a.get("id")).and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
    let more = surface()
        .px(px(20.))
        .py(px(16.))
        .flex()
        .items_center()
        .gap(px(14.))
        .child(div().flex_none().child(icon("shield", 19., c(TEXT_2))))
        .child(title(root.tr("rs.more"), Some(root.tr("rs.moreSub"))))
        .child(Button::new("rs-web", root.tr("rs.open")).icon("external").small().on_click(cx.listener(move |r, _, _, _| r.send("openWebsite", json!({ "id": id, "page": "settings" })))));

    // The part picked: its head, its card, and what goes with it (About me with the account, the blocked people with
    // privacy).
    let part = rs.part;
    let mut inner = div().px(px(PAGE_X)).pt(px(12.)).pb(px(32.)).w_full().flex().flex_col().gap(px(SECTION_GAP)).child(part_head(root, part)).children(pin_bar);
    inner = match part {
        Part::Account => inner.child(account).child(section(root.tr("rs.vio"), violations_card(root, cx))).child(section(root.tr("rs.about"), about)),
        Part::Privacy => inner.child(privacy).child(section(root.tr("rs.maturity"), maturity)).child(section(root.tr("blk.title"), blocked_card(root, cx))),
        Part::Places => inner.child(places_card(root, cx)),
        Part::Trades => inner.child(trades),
        Part::More => inner.child(more),
    };
    crate::scrolled::body("rs-body", inner)
}

/// A place's row: its mark, what it is over where and when (the place cut short when long, never the time), and what
/// can be done to it.
fn place_row(i: usize, mark: &'static str, what: String, sub: (String, Option<String>), end: Option<AnyElement>) -> AnyElement {
    let (at, when) = sub;
    div()
        .min_h(px(62.))
        .px(px(20.))
        .py(px(11.))
        .flex()
        .items_center()
        .gap(px(14.))
        .when(i > 0, |d| d.border_t_1().border_color(c(LINE)))
        .child(div().flex_none().child(icon(mark, 19., c(TEXT_2))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(div().truncate().text_size(px(T_L)).line_height(px(20.)).font_weight(FontWeight::MEDIUM).child(what))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .text_size(px(T_M))
                        .line_height(px(18.))
                        .text_color(c(MUTED))
                        .font_features(tnum())
                        .child(div().min_w_0().truncate().child(at))
                        .when_some(when, |d, w| d.child(div().flex_none().whitespace_nowrap().child(format!(" · {w}")))),
                ),
        )
        .children(end.map(|e| div().flex_none().flex().items_center().gap(px(10.)).child(e)))
        .into_any_element()
}

/// The account's violations, as Roblox's Account status page lists them: what was moderated, when and for which
/// rules, and where its appeal stands (or until when it may be appealed: on Roblox's page, above). Five, then Show more.
fn violations_card(root: &Root, cx: &mut Context<Root>) -> Div {
    let rs = &root.rset;
    let note = |text: String| div().px(px(20.)).py(px(18.)).text_size(px(T_M)).text_color(c(MUTED)).child(text);
    let card = surface().overflow_hidden().flex().flex_col();
    let list = match &rs.violations {
        None => return card.child(note(root.tr("vis.reading"))),
        Some(Err(why)) => return card.child(note(root.i18n.tv("rs.vioFailed", &[("why", crate::net::why_words(&root.i18n, why).as_str())]))),
        Some(Ok(l)) if l.is_empty() => return card.child(note(root.tr("rs.vioNone"))),
        Some(Ok(l)) => l,
    };
    let shown = if rs.vio_all { list.len() } else { list.len().min(5) };
    let mut rows: Vec<AnyElement> = list
        .iter()
        .take(shown)
        .enumerate()
        .map(|(i, v)| {
            let mark = match v.kind {
                "chat" => "message",
                "avatar" | "accessory" | "look" | "bundle" => "user",
                "audio" => "music",
                "image" => "image",
                "experience" => "gamepad",
                "behavior" => "shield",
                _ => "layers",
            };
            let mut rules: Vec<String> = v.rules.iter().map(|k| super::games::rule_of(k)).filter(|k| *k != "other").map(|k| root.tr(&format!("rn.rule.{k}"))).collect();
            rules.dedup();
            let mut sub: Vec<String> = v.at.map(|t| date(root, t)).into_iter().collect();
            sub.extend(rules.first().map(|r| capital(r)).into_iter().chain(rules.iter().skip(1).cloned()));
            let end = match v.state {
                "denied" => Some(badge(root.tr("rs.vioDenied"), Tone::Bad).into_any_element()),
                "accepted" => Some(badge(root.tr("rs.vioAccepted"), Tone::Ok).into_any_element()),
                "inactive" => Some(badge(root.tr("rs.vioInactive"), Tone::Muted).into_any_element()),
                "unavailable" => Some(badge(root.tr("rs.vioNoAppeal"), Tone::Muted).into_any_element()),
                _ => v.appeal_by.map(|t| {
                    div().text_size(px(T_M)).text_color(c(MUTED)).font_features(tnum()).whitespace_nowrap().child(root.i18n.tv("rs.vioBy", &[("t", &date(root, t))])).into_any_element()
                }),
            };
            place_row(i, mark, root.tr(&format!("rs.vk.{}", v.kind)), (sub.join(" · "), None), end)
        })
        .collect();
    if list.len() > 5 {
        let all = rs.vio_all;
        let words = if all { root.tr("gp.less") } else { root.i18n.tv("rs.placesMore", &[("n", &(list.len() - shown).to_string())]) };
        let more = Button::new("rs-vio-more", words)
            .small()
            .quiet()
            .icon_end(if all { "chevronUp" } else { "chevronDown" })
            .on_click(cx.listener(move |r, _, _, cx| {
                r.rset.vio_all = !all;
                cx.notify();
            }));
        rows.push(div().px(px(20.)).py(px(10.)).border_t_1().border_color(c(LINE)).flex().justify_center().child(more).into_any_element());
    }
    card.children(rows)
}

/// A day with its year, as the birthday row says one (this PC's clock).
fn date(root: &Root, secs: i64) -> String {
    let (y, m, d) = crate::time::civil((secs + crate::time::utc_offset_secs()).div_euclid(86_400));
    if root.i18n.lang != "en" { crate::lang::cur().dmy(d, m, y) } else { format!("{y}-{m:02}-{d:02}") }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// Where the account is signed in, as Roblox's Settings › Security has it and Discord's Devices lays it out: this PC
/// first (Bloxkeep's own sign-in, with the Roblox windows opened from it: never logged out from here), then every other
/// place, the latest used first (ten, then Show more), each with Log out; old ones Roblox knows nothing of as one row;
/// at the foot, Log out of every other place.
fn places_card(root: &Root, cx: &mut Context<Root>) -> Div {
    let rs = &root.rset;
    let note = |text: String| div().px(px(20.)).py(px(18.)).text_size(px(T_M)).text_color(c(MUTED)).child(text);
    let card = surface().overflow_hidden().flex().flex_col();
    let p = match &rs.places {
        None => return card.child(note(root.tr("vis.reading"))),
        Some(Err(why)) => return card.child(note(root.i18n.tv("rs.placesFailed", &[("why", crate::net::why_words(&root.i18n, why).as_str())]))),
        Some(Ok(p)) => p,
    };
    let leaving = !rs.leaving.is_empty();
    let now = now();
    let mut rows: Vec<AnyElement> = Vec::new();
    if let Some(t) = &p.this {
        let sub = (place(root, t), (p.here > 0).then(|| root.i18n.tn("rs.thisWindows", p.here as i64, &[])));
        rows.push(place_row(rows.len(), "monitor", root.tr("rs.thisDevice"), sub, Some(badge(root.tr("rs.inUse"), Tone::Ok).into_any_element())));
    }
    let shown = if rs.places_all { p.known.len() } else { p.known.len().min(10) };
    for (i, s) in p.known.iter().take(shown).enumerate() {
        let (what, at) = (device(root, s), place(root, s));
        let sub = (at.clone(), Some(crate::time::ago(now - s.last.unwrap_or(now), &root.i18n)));
        let token = s.token.clone();
        let title = root.tr("rs.logOutOneTitle");
        let text = root.i18n.tv("rs.logOutOne", &[("device", &what), ("place", &at)]);
        let out = Button::new(("rs-out", i), root.tr("rs.logOut"))
            .small()
            .quiet()
            .disabled(leaving)
            .on_click(cx.listener(move |r, _, window, cx| log_out(r, vec![token.clone()], title.clone(), text.clone(), window, cx)));
        let end = div().flex().items_center().gap(px(10.)).when(s.trusted, |d| d.child(badge(root.tr("rs.trusted"), Tone::Muted))).child(out);
        rows.push(place_row(rows.len(), device_mark(s), what, sub, Some(end.into_any_element())));
    }
    // Show more, and once all are out Show less (it folds back to the first 10).
    if p.known.len() > 10 {
        let all = rs.places_all;
        let words = if all { root.tr("gp.less") } else { root.i18n.tv("rs.placesMore", &[("n", &(p.known.len() - shown).to_string())]) };
        let more = Button::new("rs-places-more", words)
            .small()
            .quiet()
            .icon_end(if all { "chevronUp" } else { "chevronDown" })
            .on_click(cx.listener(move |r, _, _, cx| {
                r.rset.places_all = !all;
                cx.notify();
            }));
        rows.push(div().px(px(20.)).py(px(10.)).border_t_1().border_color(c(LINE)).flex().justify_center().child(more).into_any_element());
    }
    if !p.unknown.is_empty() {
        let n = p.unknown.len();
        let tokens: Vec<String> = p.unknown.iter().map(|s| s.token.clone()).collect();
        let title = root.i18n.tn("rs.logOutManyTitle", n as i64, &[]);
        let text = root.tr("rs.logOutUnknown");
        let out = Button::new("rs-out-unknown", root.tr("rs.logOut"))
            .small()
            .quiet()
            .disabled(leaving)
            .on_click(cx.listener(move |r, _, window, cx| log_out(r, tokens.clone(), title.clone(), text.clone(), window, cx)));
        rows.push(place_row(rows.len(), "monitor", root.i18n.tn("rs.unknownCount", n as i64, &[]), (root.tr("rs.unknownSub"), None), Some(out.into_any_element())));
    }
    let others: Vec<String> = p.known.iter().chain(&p.unknown).map(|s| s.token.clone()).collect();
    if rows.is_empty() {
        return card.child(note(root.tr("rs.placesNone")));
    }
    let foot = if others.is_empty() {
        div().px(px(20.)).py(px(14.)).border_t_1().border_color(c(LINE)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("rs.onlyHere"))
    } else {
        let n = others.len();
        let title = root.i18n.tn("rs.logOutManyTitle", n as i64, &[]);
        let text = root.tr("rs.logOutOthers");
        div()
            .min_h(px(56.))
            .px(px(20.))
            .py(px(12.))
            .border_t_1()
            .border_color(c(LINE))
            .flex()
            .items_center()
            .gap(px(12.))
            .child(div().flex_1().min_w_0().text_size(px(T_M)).line_height(px(18.)).text_color(c(MUTED)).child(root.tr("rs.othersNote")))
            .child(
                Button::new("rs-out-others", root.tr("rs.logOutAll"))
                    .small()
                    .danger()
                    .disabled(leaving)
                    .on_click(cx.listener(move |r, _, window, cx| log_out(r, others.clone(), title.clone(), text.clone(), window, cx))),
            )
    };
    card.children(rows).child(foot)
}

/// Sets one of the account's choices: privacy or not (trades: who or the least worth; privacy: online or joining),
/// to Roblox's word for it.
pub(super) type Pick = fn(&mut Root, bool, String, &mut Context<Root>);

/// Trades: who may send them and the least one must be worth (Roblox's words for each now, None unknown).
pub(super) fn trades_card(root: &Root, privacy: Option<&str>, value: Option<&str>, busy: bool, pick: Pick, cx: &mut Context<Root>) -> Div {
    let choice = |is_privacy: bool| {
        let (list, at): (&[(&str, i64)], Option<&str>) = if is_privacy { (&TRADE_PRIVACY, privacy) } else { (&TRADE_VALUE, value) };
        let mut seg = Segmented::new(if is_privacy { "rs-tp" } else { "rs-tv" }, at.unwrap_or_default().to_string());
        for (name, _) in list {
            seg = seg.item(*name, root.tr(&format!("{}.{name}", if is_privacy { "rs.tp" } else { "rs.tv" })));
        }
        let this = cx.entity().downgrade();
        seg.on_select(move |v, _, cx| {
            if busy {
                return;
            }
            let name = v.to_string();
            let _ = this.update(cx, |r, cx| pick(r, is_privacy, name, cx));
        })
    };
    surface()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(div().px(px(20.)).pt(px(16.)).pb(px(18.)).flex().flex_col().gap(px(12.)).child(title(root.tr("rs.tradeWho"), None)).child(div().flex().child(choice(true))))
        .child(div().px(px(20.)).pt(px(16.)).pb(px(18.)).border_t_1().border_color(c(LINE)).flex().flex_col().gap(px(12.)).child(title(root.tr("rs.tradeValue"), Some(root.tr("rs.tradeValueSub")))).child(div().flex().child(choice(false))))
}

/// Privacy: who sees it online and who may join it, with the choices Roblox offers this account (None while read,
/// `failed` why the read failed); under each, what the picked one means (the line used to explain "No one" whatever
/// was picked, and read as if it were).
pub(super) fn privacy_card(root: &Root, vis: Option<(Option<&roblox::Choice>, Option<&roblox::Choice>)>, failed: Option<&str>, busy: bool, pick: Pick, cx: &mut Context<Root>) -> Div {
    let seen = |online: bool, choice: &roblox::Choice| {
        let mut seg = Segmented::new(if online { "rs-online" } else { "rs-join" }, choice.now.clone());
        for v in &choice.options {
            seg = seg.item(v.clone(), vis_word(root, v));
        }
        let this = cx.entity().downgrade();
        seg.on_select(move |v, _, cx| {
            if busy {
                return;
            }
            let v = v.to_string();
            let _ = this.update(cx, |r, cx| pick(r, online, v, cx));
        })
    };
    let means = |online: bool, now: &str| -> Option<String> {
        let key = format!("{}.{now}", if online { "rs.onlineIs" } else { "rs.joinIs" });
        root.i18n.has(&key).then(|| root.tr(&key))
    };
    let note = |text: String| div().px(px(20.)).py(px(18.)).text_size(px(T_M)).text_color(c(MUTED)).child(text);
    surface().overflow_hidden().flex().flex_col().map(|d| match vis {
        None => d.child(note(match failed { Some(why) => root.i18n.tv("rs.visFailed", &[("why", why)]), None => root.tr("vis.reading") })),
        Some((online, join)) => {
            let rows: Vec<Div> = [(true, online), (false, join)]
                .into_iter()
                .filter_map(|(o, c)| c.filter(|c| !c.options.is_empty()).map(|c| (o, c)))
                .enumerate()
                .map(|(i, (o, ch))| {
                    div()
                        .px(px(20.))
                        .pt(px(16.))
                        .pb(px(18.))
                        .when(i > 0, |d| d.border_t_1().border_color(c(LINE)))
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(title(root.tr(if o { "rs.online" } else { "rs.join" }), means(o, &ch.now)))
                        .child(div().flex().flex_wrap().child(seen(o, ch)))
                })
                .collect();
            if rows.is_empty() { d.child(note(root.tr("rs.visNone"))) } else { d.children(rows) }
        }
    })
}

/// Content maturity as Roblox's Settings has it: the four levels (the ones Roblox offers this account), what the one
/// picked may contain, and, when a level needs an age check or a parent, that said with Roblox's own page. Where the
/// place's own age ratings decide (Vietnam), only Roblox's note.
fn maturity_card(root: &Root, busy: bool, viewer: Option<&serde_json::Value>, cx: &mut Context<Root>) -> Div {
    let rs = &root.rset;
    let note = |text: String| div().px(px(20.)).py(px(18.)).text_size(px(T_M)).text_color(c(MUTED)).child(text);
    let card = surface().overflow_hidden().flex().flex_col();
    let m = match &rs.maturity {
        None => return card.child(note(root.tr("vis.reading"))),
        Some(Err(why)) => return card.child(note(root.i18n.tv("rs.matFailed", &[("why", crate::net::why_words(&root.i18n, why).as_str())]))),
        Some(Ok(None)) => return card.child(note(root.tr("rs.matNone"))),
        Some(Ok(Some(m))) => m,
    };
    if m.local {
        return card.child(note(root.tr("rs.matLocal")));
    }
    let mut seg = Segmented::new("rs-maturity", m.now.clone());
    for (v, _) in &m.options {
        seg = seg.item(v.clone(), root.tr(&format!("rs.mat.{v}")));
    }
    let this = cx.entity().downgrade();
    let seg = seg.on_select(move |v, _, cx| {
        if busy {
            return;
        }
        let v = v.to_string();
        let _ = this.update(cx, |r, cx| set_maturity(r, v, cx));
    });
    let means = format!("rs.matIs.{}", m.now);
    let id = viewer.and_then(|a| a.get("id")).and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
    let gate = rs.gated.as_ref().map(|(_, need)| {
        let words = if need.contains("Parent") || need == "Inherited" { root.tr("rs.matParent") } else { root.tr("rs.matVerify") };
        div()
            .px(px(20.))
            .py(px(12.))
            .border_t_1()
            .border_color(c(LINE))
            .flex()
            .items_center()
            .gap(px(12.))
            .child(div().flex_1().min_w_0().text_size(px(T_M)).line_height(px(18.)).text_color(c(WARN)).child(words))
            .child(Button::new("rs-mat-verify", root.tr("rs.verify")).small().icon("external").on_click(cx.listener(move |r, _, _, _| r.send("openWebsite", json!({ "id": id, "page": "settings" })))))
    });
    card.child(
        div()
            .px(px(20.))
            .pt(px(16.))
            .pb(px(18.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(title(root.tr("rs.matLevel"), Some(root.tr("rs.maturitySub"))))
            .child(div().flex().flex_wrap().child(seg))
            .when(root.i18n.has(&means), |d| d.child(div().text_size(px(T_M)).line_height(px(18.)).text_color(c(TEXT_2)).child(root.tr(&means)))),
    )
    .children(gate)
}

/// The account's status as Roblox shows it to others (its presence): the words' key and their colour.
pub(super) fn roblox_presence(a: &serde_json::Value) -> (&'static str, u32) {
    match a.pointer("/presence/type").and_then(serde_json::Value::as_str) {
        Some("ingame") => ("rs.presence.playing", OK),
        Some("online" | "studio") => ("rs.presence.online", ONLINE),
        Some("offline") if !crate::pages::accounts::is_local(a) => ("rs.presence.offline", TEXT_2),
        Some("invisible") => ("rs.presence.invisible", TEXT_2),
        _ if crate::pages::accounts::is_local(a) => ("rs.presence.local", TEXT_2),
        _ => ("rs.presence.unknown", TEXT_2),
    }
}

/// A privacy choice in words: Roblox's word for it ("All", "FriendsAndFollowing"…), as its Settings page names it.
pub fn vis_word(root: &Root, v: &str) -> String {
    let key = format!("vis.{v}");
    if root.i18n.has(&key) { root.tr(&key) } else { v.to_string() }
}
