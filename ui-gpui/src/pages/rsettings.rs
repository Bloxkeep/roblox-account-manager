// The account's own settings, inside Games (its header's switch), as Roblox's Settings page has them: its email and
// phone (verified or not), birthday and country to read; its About words, who may send it trades and the least a trade
// must be worth to change. Only what Roblox's published API docs (accountinformation, accountsettings) have; the rest
// of Roblox's Settings (privacy, security, parental controls) is on Roblox's own page, opened in the account's website
// window. All of it answers only that account's own sign-in, lent by the app; asked from here (roblox.rs).
use gpui_kit::{prelude::*, *};
use serde_json::json;

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

#[derive(Default)]
pub struct Data {
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
    /// Who sees it online and who may join it (Roblox's words: "AllUsers", "Friends", "NoOne"), None while read or
    /// when Roblox didn't say.
    vis: Option<(String, String)>,
    /// Roblox didn't answer who sees the account (the Privacy card says so instead of "Reading" for good).
    /// Why the visibility read failed (Roblox's code, or the network's words), shown as it is.
    vis_failed: Option<String>,
}

const SEEN: [&str; 3] = ["AllUsers", "Friends", "NoOne"];

pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    friends::want_sign(r, link, cx);
    let Some((_, user, _)) = friends::signed(r) else { return };
    if r.rset.of != user {
        r.rset = Data { of: user, ..Default::default() };
    }
    if r.rset.got.is_none() && !r.rset.reading {
        load(r, cx);
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
    let c2 = cookie.clone();
    let vis = roblox::run(move || roblox::visibility(&c2));
    cx.spawn(async move |this, cx| {
        let got = vis.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.rset.of == user {
                match got {
                    Ok(v) => r.rset.vis = Some(v),
                    Err(e) => r.rset.vis_failed = Some(if e.is_empty() { "no answer".into() } else { e }),
                }
                cx.notify();
            }
        });
    })
    .detach();
    let rx = roblox::run(move || roblox::account_settings(&cookie));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
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
    })
    .detach();
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// A voice lock's countdown, redrawn every 20 s while these settings are shown; at its end Roblox is asked again.
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
    load(r, cx);
    cx.notify();
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
    cx.spawn(async move |this, cx| {
        let done = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
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
    })
    .detach();
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
    cx.spawn(async move |this, cx| {
        let done = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
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
    })
    .detach();
}

/// Who sees it online (`online`) or may join it, set to Roblox's `v`: shown at once, put back when Roblox refuses.
fn set_vis(r: &mut Root, online: bool, v: &'static str, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    if r.rset.saving.is_some() {
        return;
    }
    let Some(now) = r.rset.vis.as_mut() else { return };
    let slot = if online { &mut now.0 } else { &mut now.1 };
    let before = std::mem::replace(slot, v.to_string());
    r.rset.saving = Some(if online { "online" } else { "join" });
    r.rset.said = None;
    let rx = roblox::run(move || if online { roblox::set_visibility(&cookie, Some(v), None) } else { roblox::set_visibility(&cookie, None, Some(v)) });
    cx.spawn(async move |this, cx| {
        let done = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.rset.of != user {
                return;
            }
            r.rset.saving = None;
            if let Err(why) = done {
                if let Some(now) = r.rset.vis.as_mut() {
                    *(if online { &mut now.0 } else { &mut now.1 }) = before;
                }
                r.toast = Some((r.i18n.tv("rs.failed", &[("why", if why.is_empty() { "Roblox" } else { &why })]), "bad".into()));
            }
            cx.notify();
        });
    })
    .detach();
}

fn note(text: String) -> Div {
    div().flex_1().flex().items_center().justify_center().text_size(px(T_M)).text_color(c(MUTED)).child(text)
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
fn section(head: impl Into<SharedString>, card: impl IntoElement) -> Div {
    div().flex().flex_col().gap(px(8.)).child(section_head(head, None)).child(card)
}

pub fn page(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let rs = &root.rset;
    let (able, viewer) = friends::looking_ref(root);
    let shell = div().id("rsettings").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let header = div()
        .flex_none()
        .px(px(24.))
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
            None => note(root.tr("rs.loading")).into_any_element(),
            Some(Err(why)) => note(root.i18n.tv("rs.readFailed", &[("why", if why.is_empty() { "Roblox" } else { why })])).into_any_element(),
            Some(Ok(s)) => filled(root, s, viewer, width, cx).into_any_element(),
        }
    };
    shell.child(header).child(body)
}

fn filled(root: &Root, s: &AccountSettings, viewer: Option<&serde_json::Value>, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let rs = &root.rset;
    let dash = || div().text_color(c(MUTED)).child(root.tr("rs.unknown")).into_any_element();
    let words = |t: String| div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).font_features(tnum()).child(t).into_any_element();

    // The account: who, then what Roblox has of it.
    let mut rows: Vec<(String, AnyElement)> = Vec::new();
    rows.push((
        root.tr("rs.email"),
        match &s.email {
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
            Some((d, m, y)) if root.i18n.vietnamese => words(format!("{d:02}/{m:02}/{y}")),
            Some((d, m, y)) => words(format!("{y}-{m:02}-{d:02}")),
            None => dash(),
        },
    ));
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
    let who = viewer.map(|a| {
        let user = a.get("username").and_then(serde_json::Value::as_str).filter(|u| !u.is_empty()).map(|u| format!("@{u}"));
        let said = vec![(user.unwrap_or_else(|| root.tr("rs.account")), None)];
        div().px(px(20.)).pt(px(18.)).pb(px(16.)).border_b_1().border_color(c(LINE)).child(group_head(avatar(a, GROUP_PIC, RAISED, cx), label(a), facts(said)))
    });
    let account = surface().overflow_hidden().flex().flex_col().children(who).child(div().flex().flex_col().children(rows.into_iter().enumerate().map(|(i, (k, v))| row(i, k, v))));

    // Trades: two choices, each a row of Roblox's settings.
    let busy = rs.saving.is_some();
    let choice = |privacy: bool| {
        let (list, at): (&[(&str, i64)], &Option<String>) = if privacy { (&TRADE_PRIVACY, &s.trade_privacy) } else { (&TRADE_VALUE, &s.trade_value) };
        let mut seg = Segmented::new(if privacy { "rs-tp" } else { "rs-tv" }, at.clone().unwrap_or_default());
        for (name, _) in list {
            seg = seg.item(*name, root.tr(&format!("{}.{name}", if privacy { "rs.tp" } else { "rs.tv" })));
        }
        let this = cx.entity().downgrade();
        seg.on_select(move |v, _, cx| {
            if busy {
                return;
            }
            let name = v.to_string();
            let _ = this.update(cx, |r, cx| set_trade(r, privacy, name, cx));
        })
    };
    let trades = surface()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(div().px(px(20.)).pt(px(16.)).pb(px(18.)).flex().flex_col().gap(px(12.)).child(title(root.tr("rs.tradeWho"), None)).child(div().flex().child(choice(true))))
        .child(div().px(px(20.)).pt(px(16.)).pb(px(18.)).border_t_1().border_color(c(LINE)).flex().flex_col().gap(px(12.)).child(title(root.tr("rs.tradeValue"), Some(root.tr("rs.tradeValueSub")))).child(div().flex().child(choice(false))));

    // Privacy: who sees it online ("No one" is showing offline to all) and who may join it.
    let seen = |online: bool| {
        let at = rs.vis.as_ref().map(|v| if online { v.0.clone() } else { v.1.clone() }).unwrap_or_default();
        let mut seg = Segmented::new(if online { "rs-online" } else { "rs-join" }, at);
        for v in SEEN {
            seg = seg.item(v, root.tr(&format!("vis.{v}")));
        }
        let this = cx.entity().downgrade();
        seg.on_select(move |v, _, cx| {
            if busy {
                return;
            }
            let Some(v) = SEEN.iter().copied().find(|k| *k == v.as_ref()) else { return };
            let _ = this.update(cx, |r, cx| set_vis(r, online, v, cx));
        })
    };
    let privacy = surface()
        .overflow_hidden()
        .flex()
        .flex_col()
        .map(|d| match rs.vis {
            None => d.child(div().px(px(20.)).py(px(18.)).text_size(px(T_M)).text_color(c(MUTED)).child(match &rs.vis_failed { Some(why) => root.i18n.tv("rs.visFailed", &[("why", why)]), None => root.tr("vis.reading") })),
            Some(_) => d
                .child(div().px(px(20.)).pt(px(16.)).pb(px(18.)).flex().flex_col().gap(px(12.)).child(title(root.tr("rs.online"), Some(root.tr("rs.onlineSub")))).child(div().flex().child(seen(true))))
                .child(div().px(px(20.)).pt(px(16.)).pb(px(18.)).border_t_1().border_color(c(LINE)).flex().flex_col().gap(px(12.)).child(title(root.tr("rs.join"), None)).child(div().flex().child(seen(false)))),
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
                        Some(Err(why)) => d.text_size(px(T_S)).text_color(c(BAD)).child(root.i18n.tv("rs.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])),
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

    let account = section(root.tr("rs.account"), account);
    let trades = section(root.tr("rs.trades"), trades);
    let privacy = section(root.tr("rs.privacy"), privacy);
    let about = section(root.tr("rs.about"), about);
    let more = section(root.tr("rs.moreHead"), more);
    // Wide: the account and its trades beside its About words, the whole width; narrow: one under another.
    let two = width >= 1000.;
    let inner = div().px(px(24.)).pb(px(32.)).w_full().flex().map(|d| {
        if two {
            d.items_start()
                .gap(px(24.))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(SECTION_GAP)).child(account).child(trades))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(SECTION_GAP)).child(privacy).child(about).child(more))
        } else {
            d.flex_col().gap(px(SECTION_GAP)).child(account).child(privacy).child(about).child(trades).child(more)
        }
    });
    crate::scrolled::body("rs-body", inner)
}
