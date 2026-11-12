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
}

pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    friends::want_sign(r, link, cx);
    let Some((_, user, _)) = friends::signed(r) else { return };
    if r.rset.of != user {
        r.rset = Data { of: user, ..Default::default() };
    }
    if r.rset.got.is_none() && !r.rset.reading {
        load(r, cx);
    }
}

fn load(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    r.rset.reading = true;
    let rx = roblox::run(move || roblox::account_settings(&cookie));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.rset.of == user {
                r.rset.fill = Some(got.as_ref().ok().and_then(|s| s.about.clone()).unwrap_or_default());
                r.rset.got = Some(got);
                r.rset.reading = false;
                cx.notify();
            }
        });
    })
    .detach();
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
        .text_size(px(T_M + 0.5))
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
    let (able, viewer) = friends::looking(root);
    let shell = div().id("rsettings").size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let header = div()
        .flex_none()
        .px(px(24.))
        .pt(px(22.))
        .pb(px(20.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(div().text_size(px(T_TITLE)).line_height(px(38.)).font_weight(FontWeight::BOLD).child("Roblox"))
        .child(div().ml(px(6.)).child(crate::pages::games::views(root, cx)))
        .child(div().flex_1())
        .when(viewer.is_some(), |d| d.child(IconButton::new("rs-refresh", "refresh").size(34.).busy(rs.reading).tip(root.tr("fr.refresh")).on_click(cx.listener(|r, _, _, cx| again(r, cx)))));

    let body: AnyElement = if able.is_empty() || friends::nobody(root) {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("fr.noAccount")))
            .child(Button::new("rs-add-acc", root.tr("acc.add")).icon("plus").primary().on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
            .into_any_element()
    } else {
        match &rs.got {
            None => note(root.tr("rs.loading")).into_any_element(),
            Some(Err(why)) => note(root.i18n.tv("rs.readFailed", &[("why", if why.is_empty() { "Roblox" } else { why })])).into_any_element(),
            Some(Ok(s)) => filled(root, s, viewer.as_ref(), width, cx).into_any_element(),
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
        .child(div().size(px(36.)).flex_none().rounded(px(R_M)).bg(c(RAISED_2)).flex().items_center().justify_center().child(icon("shield", 17., c(TEXT_2))))
        .child(title(root.tr("rs.more"), Some(root.tr("rs.moreSub"))))
        .child(Button::new("rs-web", root.tr("rs.open")).icon("external").small().on_click(cx.listener(move |r, _, _, _| r.send("openWebsite", json!({ "id": id, "page": "settings" })))));

    let account = section(root.tr("rs.account"), account);
    let trades = section(root.tr("rs.trades"), trades);
    let about = section(root.tr("rs.about"), about);
    let more = section(root.tr("rs.moreHead"), more);
    // Wide: the account and its trades beside its About words, the whole width; narrow: one under another.
    let two = width >= 1000.;
    let inner = div().px(px(24.)).pb(px(32.)).w_full().flex().map(|d| {
        if two {
            d.items_start()
                .gap(px(24.))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(SECTION_GAP)).child(account).child(trades))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(SECTION_GAP)).child(about).child(more))
        } else {
            d.flex_col().gap(px(SECTION_GAP)).child(account).child(about).child(trades).child(more)
        }
    });
    crate::scrolled::body("rs-body", inner)
}
