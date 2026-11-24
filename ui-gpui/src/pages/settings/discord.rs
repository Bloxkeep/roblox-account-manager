// Settings › Discord (ui/src/settings/DiscordPage.tsx): sending and the webhook link, how messages will look, the
// play-time summary, alerts and who they ping, the status message, quiet hours, how messages look; then "Playing
// Bloxkeep" on the profile and control from Discord. What the old page asked in a box is done in place: the link is
// pasted under its row, Remove asks under it, the summary's parts and accounts are ticked under theirs. The link itself
// never comes to the window unless asked for (Show), and goes again after half a minute.
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};

use super::*;
use crate::pages::preset_form::parse_time;

const PING_MODES: [&str; 5] = ["none", "here", "everyone", "user", "role"];
const PARTS: [&str; 5] = ["total", "accounts", "games", "sessions", "kicks"];

thread_local! {
    /// The buttons whose command is on its way (Send test, Send summary, Show, New code, Save): pressed again they do
    /// nothing until it answers, as the old page's busy buttons did.
    static BUSY: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
    /// Which Show the link is shown for: the half-minute of an earlier Show mustn't hide a later one early.
    static SHOWN_FOR: Cell<u64> = const { Cell::new(0) };
    /// The remote code's minutes are being counted down (the page drawn again every 15 s while it shows).
    static TICKING: Cell<bool> = const { Cell::new(false) };
    /// The last ID said to be wrong.
    static BAD_ID: RefCell<String> = const { RefCell::new(String::new()) };
}

fn busy(key: &str) -> bool {
    BUSY.with(|b| b.borrow().contains(key))
}

/// `ask` for a button: it stays off until the app answers.
fn ask_busy(r: &mut Root, key: &'static str, cmd: &str, args: Value, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Value) + 'static) {
    let Some(link) = r.link.clone() else { return };
    if busy(key) {
        return;
    }
    BUSY.with(|b| b.borrow_mut().insert(key));
    let rx = link.call(cmd, args);
    cx.spawn(async move |this, cx| {
        let answer = rx.await;
        BUSY.with(|b| b.borrow_mut().remove(key));
        let _ = this.update(cx, |r, cx| {
            match answer {
                Ok(Ok(v)) => done(r, v),
                Ok(Err(e)) => r.toast = Some((e, "bad".into())),
                Err(_) => {}
            }
            cx.notify();
        });
    })
    .detach();
    cx.notify();
}

/// The page's text boxes, made when the page is first opened (a box needs the window).
pub struct Boxes {
    link: Entity<InputState>,
    daily: Entity<InputState>,
    weekly: Entity<InputState>,
    from: Entity<InputState>,
    to: Entity<InputState>,
    name: Entity<InputState>,
    avatar: Entity<InputState>,
    ping_id: Entity<InputState>,
}

fn dc(root: &Root) -> &Value {
    root.state.get("discord").unwrap_or(&Value::Null)
}
fn put(e: &Entity<InputState>, v: &str, window: &mut Window, cx: &mut Context<Root>) {
    let v = v.to_string();
    e.update(cx, |st, cx| st.set_value(v, window, cx));
}

/// What a box shows when saved, from the app's state.
fn saved(r: &Root, which: &str) -> String {
    let d = dc(r);
    let p = match which {
        "daily" => "/summary/dailyTime",
        "weekly" => "/summary/weeklyTime",
        "from" => "/quiet/from",
        "to" => "/quiet/to",
        "name" => "/username",
        "avatar" => "/avatarUrl",
        _ => return String::new(),
    };
    d.pointer(p).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// Settings › Discord opened: its boxes made once, and showing what is saved.
pub(super) fn opened(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.settings.dc.is_none() {
        let mk = |hint: &str, window: &mut Window, cx: &mut Context<Root>| {
            let hint = hint.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(hint))
        };
        let boxes = Boxes {
            link: mk("https://discord.com/api/webhooks/…", window, cx),
            daily: mk("22:00", window, cx),
            weekly: mk("21:00", window, cx),
            from: mk("23:00", window, cx),
            to: mk("07:00", window, cx),
            name: mk("Bloxkeep", window, cx),
            avatar: mk("https://…", window, cx),
            ping_id: mk(&r.tr("dc.idPh"), window, cx),
        };
        for (e, which) in [(&boxes.daily, "daily"), (&boxes.weekly, "weekly"), (&boxes.from, "from"), (&boxes.to, "to"), (&boxes.name, "name"), (&boxes.avatar, "avatar")] {
            cx.subscribe_in(e, window, move |r: &mut Root, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    commit(r, which, window, cx);
                }
            })
            .detach();
        }
        cx.subscribe_in(&boxes.link, window, |r: &mut Root, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => save_link(r, window, cx),
            InputEvent::Change => {
                r.settings.dc_link_err = None;
                cx.notify();
            }
            _ => {}
        })
        .detach();
        cx.subscribe_in(&boxes.ping_id, window, |r: &mut Root, _, ev: &InputEvent, _, cx| match ev {
            InputEvent::PressEnter { .. } | InputEvent::Blur => commit_ping(r, matches!(ev, InputEvent::Blur), cx),
            _ => {}
        })
        .detach();
        // A shown link goes back to masked when the window loses focus (as the old page's did).
        cx.observe_window_activation(window, |r: &mut Root, window, cx| {
            if !window.is_window_active() && r.settings.dc_shown.is_some() {
                r.settings.dc_shown = None;
                cx.notify();
            }
        })
        .detach();
        r.settings.dc = Some(boxes);
    }
    let Some(bx) = r.settings.dc.as_ref() else { return };
    let pairs = [(bx.daily.clone(), "daily"), (bx.weekly.clone(), "weekly"), (bx.from.clone(), "from"), (bx.to.clone(), "to"), (bx.name.clone(), "name"), (bx.avatar.clone(), "avatar")];
    for (e, which) in pairs {
        let v = saved(r, which);
        put(&e, &v, window, cx);
    }
}

/// The box for one of the saved values.
fn box_for(bx: &Boxes, which: &str) -> Entity<InputState> {
    match which {
        "daily" => bx.daily.clone(),
        "weekly" => bx.weekly.clone(),
        "from" => bx.from.clone(),
        "to" => bx.to.clone(),
        "name" => bx.name.clone(),
        _ => bx.avatar.clone(),
    }
}

/// `setDiscord` for a box: refused by the app, the reason is said and the box shows what is saved again
/// (DiscordPage.tsx's `again`).
fn ask_box(r: &mut Root, which: &'static str, args: Value, window: &mut Window, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call("setDiscord", args);
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            if let Ok(Err(e)) = answer {
                r.toast = Some((e, "bad".into()));
                if let Some(e) = r.settings.dc.as_ref().map(|bx| box_for(bx, which)) {
                    let v = saved(r, which);
                    put(&e, &v, window, cx);
                }
                cx.notify();
            }
        });
    })
    .detach();
}

/// A box left or Enter pressed: its value saved when it is a good one and new; else what is saved comes back.
fn commit(r: &mut Root, which: &'static str, window: &mut Window, cx: &mut Context<Root>) {
    let Some(bx) = r.settings.dc.as_ref() else { return };
    let (from_box, to_box) = (bx.from.clone(), bx.to.clone());
    let e = box_for(bx, which);
    let v = input_text(&e, cx);
    let was = saved(r, which);
    match which {
        "name" | "avatar" => {
            let bad = if which == "name" {
                let l = v.to_lowercase();
                v.chars().count() > 80 || l.contains("discord") || l.contains("clyde")
            } else {
                !v.is_empty() && !(v.to_lowercase().starts_with("https://") && !v.contains(char::is_whitespace) && v.len() <= 2048)
            };
            if bad {
                r.toast = Some((r.tr(if which == "name" { "dc.badName" } else { "dc.badAvatar" }), "warn".into()));
                put(&e, &was, window, cx);
            } else if v != was {
                let key = if which == "name" { "username" } else { "avatarUrl" };
                ask_box(r, which, json!({ key: v }), window, cx);
            }
        }
        _ => match parse_time(&v) {
            Ok(Some(t)) => {
                put(&e, &t, window, cx);
                if t != was {
                    let key = match which {
                        "daily" => "summaryDailyTime",
                        "weekly" => "summaryWeeklyTime",
                        "from" => "quietFrom",
                        _ => "quietTo",
                    };
                    ask_box(r, which, json!({ key: t }), window, cx);
                    // From and To the same never applies: said so when it happens.
                    let other = if which == "from" { input_text(&to_box, cx) } else if which == "to" { input_text(&from_box, cx) } else { String::new() };
                    if !other.is_empty() && parse_time(&other) == Ok(Some(t.clone())) {
                        r.toast = Some((r.tr("dc.quietSame"), "warn".into()));
                    }
                }
            }
            // Not a time, or emptied: the saved one comes back.
            _ => put(&e, &was, window, cx),
        },
    }
    cx.notify();
}

/// A ping for "alerts" is the same ping for every kind of alert, sent one at a time.
fn set_ping(r: &mut Root, kind: &str, value: String, cx: &mut Context<Root>) {
    if kind != "alerts" {
        r.ask("setDiscord", json!({ "ping": { "kind": kind, "value": value } }), cx, |_, _| {});
        return;
    }
    let Some(link) = r.link.clone() else { return };
    cx.spawn(async move |this, cx| {
        for k in NOTIFY_KINDS {
            if let Ok(Err(e)) = link.call("setDiscord", json!({ "ping": { "kind": k, "value": value } })).await {
                let _ = this.update(cx, |r, cx| {
                    r.toast = Some((e, "bad".into()));
                    cx.notify();
                });
                return;
            }
        }
    })
    .detach();
}

/// The ID box: saved once it looks like an ID; left empty, the saved choice comes back.
fn commit_ping(r: &mut Root, left: bool, cx: &mut Context<Root>) {
    let Some((kind, mode)) = r.settings.dc_ping.clone() else { return };
    let Some(bx) = r.settings.dc.as_ref() else { return };
    let v = input_text(&bx.ping_id, cx);
    if v.is_empty() {
        if left {
            r.settings.dc_ping = None;
            cx.notify();
        }
        return;
    }
    if !(17..=20).contains(&v.len()) || !v.bytes().all(|b| b.is_ascii_digit()) {
        // (Enter and then leaving the box is the same mistake: said once.)
        if BAD_ID.with(|b| b.replace(v.clone())) != v {
            r.toast = Some((r.tr(if mode == "role" { "dc.badRole" } else { "dc.badUser" }), "warn".into()));
        }
        // Left the box: the next try is a new one, told again even with the same text.
        if left {
            BAD_ID.with(|b| b.take());
        }
        cx.notify();
        return;
    }
    BAD_ID.with(|b| b.take());
    r.settings.dc_ping = None;
    set_ping(r, &kind, format!("{mode}:{v}"), cx);
    cx.notify();
}

/// Discord's webhook links: https://discord.com/api/webhooks/<17–20 digits>/<token> (canary., ptb., discordapp.com, /v10).
fn webhook_ok(u: &str) -> bool {
    let Some(rest) = u.strip_prefix("https://") else { return false };
    let rest = rest.strip_prefix("canary.").or_else(|| rest.strip_prefix("ptb.")).unwrap_or(rest);
    let Some(rest) = rest.strip_prefix("discord.com").or_else(|| rest.strip_prefix("discordapp.com")) else { return false };
    let Some(mut rest) = rest.strip_prefix("/api") else { return false };
    if let Some(r) = rest.strip_prefix("/v") {
        let n = r.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 || n > 2 {
            return false;
        }
        rest = &r[n..];
    }
    let Some(rest) = rest.strip_prefix("/webhooks/") else { return false };
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let Some((id, token)) = rest.split_once('/') else { return false };
    (17..=20).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_digit())
        && (50..=100).contains(&token.len())
        && token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn open_link(r: &mut Root, then_on: bool, window: &mut Window, cx: &mut Context<Root>) {
    r.settings.dc_link = Some(then_on);
    r.settings.dc_link_err = None;
    r.settings.asking = None;
    if let Some(bx) = r.settings.dc.as_ref() {
        let e = bx.link.clone();
        e.update(cx, |st, cx| {
            st.set_value("", window, cx);
            st.focus(window, cx);
        });
    }
    cx.notify();
}

fn save_link(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let (Some(then_on), Some(bx)) = (r.settings.dc_link, r.settings.dc.as_ref()) else { return };
    let url = input_text(&bx.link, cx);
    if !webhook_ok(&url) {
        r.settings.dc_link_err = Some(r.tr("dc.badLink"));
        cx.notify();
        return;
    }
    let Some(link) = r.link.clone() else { return };
    if busy("link") {
        return;
    }
    BUSY.with(|b| b.borrow_mut().insert("link"));
    cx.notify();
    cx.spawn_in(window, async move |this, cx| {
        let answer = link.call("setDiscordWebhook", json!({ "url": url })).await;
        if let Ok(Err(e)) = &answer {
            let e = e.clone();
            BUSY.with(|b| b.borrow_mut().remove("link"));
            let _ = this.update(cx, |r, cx| {
                r.settings.dc_link_err = Some(e);
                cx.notify();
            });
            return;
        }
        // No answer (the app's pipe dropped): nothing is known saved, so the box stays as it is.
        if answer.is_err() {
            BUSY.with(|b| b.borrow_mut().remove("link"));
            let _ = this.update(cx, |_, cx| cx.notify());
            return;
        }
        let mut on_failed = None;
        if then_on {
            if let Ok(Err(e)) = link.call("setDiscord", json!({ "enabled": true })).await {
                on_failed = Some(e);
            }
        }
        BUSY.with(|b| b.borrow_mut().remove("link"));
        let _ = this.update_in(cx, |r, window, cx| {
            r.settings.dc_link = None;
            if let Some(bx) = r.settings.dc.as_ref() {
                let e = bx.link.clone();
                e.update(cx, |st, cx| st.set_value("", window, cx));
            }
            match on_failed {
                // The link is kept; only turning sending on failed.
                Some(e) => r.toast = Some((e, "bad".into())),
                None => r.ok(r.tr(if then_on { "dc.savedOn" } else { "dc.saved" })),
            }
            cx.notify();
        });
    })
    .detach();
}

fn time_box(e: &Entity<InputState>) -> Div {
    div().w(px(120.)).child(field(e, None))
}

fn ping_key(kind: &str) -> &'static str {
    match kind {
        "summary" => "dcp-summary",
        "alerts" => "dcp-alerts",
        "drop" => "dcp-drop",
        "schedule" => "dcp-schedule",
        "signin" => "dcp-signin",
        "ram" => "dcp-ram",
        _ => "dcp-launch",
    }
}

/// Who a message pings: nobody, @here and @everyone save at once; a user or a role waits for its ID in the box under it.
fn ping(root: &Root, kind: &'static str, cx: &mut Context<Root>) -> Div {
    let pings = dc(root).get("pings").unwrap_or(&Value::Null);
    let of = |k: &str| pings.get(k).and_then(Value::as_str).filter(|v| !v.is_empty()).unwrap_or("none").to_string();
    let saved = if kind != "alerts" {
        of(kind)
    } else {
        let all: Vec<String> = NOTIFY_KINDS.iter().map(|k| of(k)).collect();
        if all.iter().all(|p| *p == all[0]) { all[0].clone() } else { "mixed".into() }
    };
    let (saved_mode, saved_id) = saved.split_once(':').map_or((saved.clone(), String::new()), |(m, i)| (m.to_string(), i.to_string()));
    let editing = root.settings.dc_ping.as_ref().filter(|(k, _)| k == kind).map(|(_, m)| m.clone());
    let current = editing.clone().unwrap_or(saved_mode.clone());
    let mut items: Vec<(String, String, bool)> = PING_MODES.iter().map(|m| (m.to_string(), root.tr(&format!("dc.ping.{m}")), false)).collect();
    // Alerts whose pings differ read "Varies by alert", not to be picked.
    if current == "mixed" {
        items.push(("mixed".into(), root.tr("dc.pingMixed"), true));
    }
    let line = chips_with(root, cx, ping_key(kind), items, &current, move |r, v, window, cx| {
        if v == "user" || v == "role" {
            // The ID box starts empty: a person's ID kept as a role's (or the other way round) would ping the wrong one.
            r.settings.dc_ping = Some((kind.to_string(), v));
            if let Some(bx) = r.settings.dc.as_ref() {
                let e = bx.ping_id.clone();
                e.update(cx, |st, cx| {
                    st.set_value("", window, cx);
                    st.focus(window, cx);
                });
            }
            cx.notify();
            return;
        }
        r.settings.dc_ping = None;
        set_ping(r, kind, v, cx);
        cx.notify();
    });
    // The ID beside the menu: a box while one is being typed, else the one saved.
    let beside: Option<AnyElement> = if editing.is_some() {
        root.settings.dc.as_ref().map(|bx| div().w(px(200.)).child(field(&bx.ping_id, None)).into_any_element())
    } else if !saved_id.is_empty() {
        Some(div().max_w(px(180.)).truncate().text_size(px(T_S)).text_color(c(MUTED)).child(format!("ID {saved_id}")).into_any_element())
    } else {
        None
    };
    div().flex().items_center().gap(px(10.)).children(beside).child(line)
}

pub(super) fn discord(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let d = dc(root);
    let has_hook = flag(d, "hasWebhook");
    let on = flag(d, "enabled");
    let hint = s(d, "webhookHint").to_string();
    let Some(bx) = root.settings.dc.as_ref() else { return vec![] };
    let mut out = Vec::new();

    // ── the connection card, the old page's first group: sending on or off (what it does), the webhook link with what
    // to do with it and where to get one, then the last message sent with Send test.
    let shown = root.settings.dc_shown.as_ref().filter(|(_, h)| has_hook && *h == hint).map(|(u, _)| u.clone());
    let (state, tone) = if !has_hook {
        (root.tr("set.dc.state.none"), Tone::Muted)
    } else if on {
        (root.tr("set.dc.state.on"), Tone::Ok)
    } else {
        (root.tr("set.dc.state.off"), Tone::Muted)
    };
    let this = cx.entity().downgrade();
    let pasting = root.settings.dc_link.is_some();
    // The connection at a glance: Discord's mark on its block, the state in words (its dot) and what sending does; at
    // the right the one thing to do: link a channel (the main action) or, once linked, sending on or off.
    let mark = div().flex_none().child(icon("discord", 30., c(TEXT)));
    let head = div()
        .px(px(ROW_X))
        .pt(px(20.))
        .pb(px(20.))
        .flex()
        .items_center()
        .gap(px(16.))
        .child(mark)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(div().size(px(8.)).flex_none().rounded_full().bg(c(match tone { Tone::Ok => OK, _ => DIM })))
                        .child(div().text_size(px(T_GROUP)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).child(state)),
                )
                .child(div().text_size(px(T_M)).line_height(px(18.)).text_color(c(TEXT_2)).child(root.tr("dc.onDesc"))),
        )
        .map(|d| {
            if has_hook {
                // The master switch says what it does ("Send to Discord"), as the old page's row did.
                d.child(div().flex_none().flex().items_center().gap(px(10.)).child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(root.tr("dc.on"))).child(Switch::new("s-dc-on", on).on_toggle(move |v, window, cx| {
                    let v = *v;
                    let _ = this.update(cx, |r, cx| {
                        if v && !flag(dc(r), "hasWebhook") {
                            open_link(r, true, window, cx);
                        } else {
                            r.ask("setDiscord", json!({ "enabled": v }), cx, |_, _| {});
                        }
                    });
                })))
            } else if !pasting {
                // No link yet: linking one turns sending on once it is saved (as the switch did).
                d.child(Button::new("s-dc-paste", root.tr("dc.paste")).icon("link").primary().on_click(cx.listener(|r, _, window, cx| open_link(r, true, window, cx))))
            } else {
                d
            }
        });
    let mut card: Vec<AnyElement> = vec![found(&format!("{} {} Discord", root.tr("dc.on"), root.tr("dc.onDesc")), head.into_any_element())];
    if !s(d, "stopped").is_empty() {
        card.push(note("alert", root.tr("dc.gone"), WARN));
    }
    // A line of the card: its words on the left (a name, then muted lines), its buttons on the right.
    let line = |words: Div, ctl: Div| div().px(px(ROW_X)).py(px(14.)).flex().items_center().gap(px(24.)).child(words).child(ctl.flex_none().flex().items_center().gap(px(6.)));
    let title_t = |t: String| div().text_size(px(T_L)).line_height(px(20.)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).child(t);
    let desc_t = |t: String, col: u32| div().text_size(px(T_M)).line_height(px(18.)).text_color(c(col)).child(t);
    if has_hook {
        // The webhook link: masked, or the whole link while shown; where to get one under it.
        let showing = shown.is_some();
        let words = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(title_t(root.tr("dc.webhook")))
            .child(desc_t(shown.clone().unwrap_or_else(|| hint.clone()), TEXT_2).font_features(tnum()))
            .child(desc_t(root.tr("dc.webhookHelp"), MUTED));
        let ctl = div()
            .child(Button::new("s-dc-paste", root.tr("dc.replace")).small().icon("link").on_click(cx.listener(|r, _, window, cx| open_link(r, false, window, cx))))
            .child(
                Button::new("s-dc-reveal", root.tr(if showing { "dc.hide" } else { "dc.show" })).small().quiet().icon(if showing { "eyeOff" } else { "eye" }).disabled(busy("reveal")).on_click(cx.listener(move |r, _, _, cx| {
                    if showing {
                        r.settings.dc_shown = None;
                        cx.notify();
                        return;
                    }
                    let hint = s(dc(r), "webhookHint").to_string();
                    let n = SHOWN_FOR.with(|f| {
                        f.set(f.get() + 1);
                        f.get()
                    });
                    ask_busy(r, "reveal", "revealDiscordWebhook", Value::Null, cx, move |r, v| {
                        let Some(url) = v.as_str().map(str::to_string) else { return };
                        r.settings.dc_shown = Some((url, hint));
                    });
                    // Shown for half a minute, then masked again (unless shown again since).
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(Duration::from_secs(30)).await;
                        if SHOWN_FOR.with(Cell::get) != n {
                            return;
                        }
                        let _ = this.update(cx, |r, cx| {
                            r.settings.dc_shown = None;
                            cx.notify();
                        });
                    })
                    .detach();
                })),
            )
            .child(Button::new("s-dc-remove", root.tr("dc.remove")).small().quiet().icon("trash").on_click(cx.listener(|r, _, _, cx| {
                r.settings.asking = Some("dc-remove");
                r.settings.dc_link = None;
                cx.notify();
            })));
        card.push(found(&format!("{} {}", root.tr("dc.webhook"), root.tr("dc.webhookHelp")), line(words, ctl).into_any_element()));
        if root.settings.asking == Some("dc-remove") {
            card.push(question(root, "dc-remove-q", root.tr("dc.removeTitle"), root.tr("dc.removeConfirm"), root.tr("dc.remove"), true, cx, |r, cx| {
                r.settings.dc_shown = None;
                r.ask("setDiscordWebhook", json!({ "url": "" }), cx, |_, _| {});
            }));
        }
    } else if !pasting {
        // No link yet: where to get one (the old page said it under the link row), before it is pasted.
        card.push(found(&root.tr("dc.webhookHelp"), note("info", root.tr("dc.webhookHelp"), MUTED)));
    }
    if pasting {
        let saving = busy("link");
        card.push(
            div()
                .px(px(ROW_X))
                .py(px(14.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .bg(c(INSET))
                .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.tr("dc.pasteTitle")))
                .child(div().text_size(px(T_S)).text_color(c(TEXT_2)).child(crate::i18n::plain(&root.tr("dc.pasteText"))))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().flex_1().min_w_0().child(field(&bx.link, Some("link"))))
                        .child(Button::new("s-dc-link-no", root.tr("common.cancel")).small().quiet().on_click(cx.listener(|r, _, _, cx| {
                            r.settings.dc_link = None;
                            cx.notify();
                        })))
                        .child(Button::new("s-dc-link-ok", root.tr("dc.save")).small().primary().disabled(saving).on_click(cx.listener(|r, _, window, cx| save_link(r, window, cx)))),
                )
                .child(match root.settings.dc_link_err.clone() {
                    Some(e) => div().text_size(px(T_S)).text_color(c(BAD)).child(e),
                    None => div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("dc.webhookHelp")),
                })
                .into_any_element(),
        );
    }
    if has_hook {
        // The last message: when it went (or nothing yet), what went wrong, what waits; Send test beside it.
        let last = s(d, "lastSent");
        let when = match crate::time::iso_secs(last) {
            Some(at) if !last.is_empty() => root.i18n.tv("dc.last", &[("t", &crate::time::ago(crate::time::now_secs() - at, &root.i18n))]),
            _ => root.tr("dc.never"),
        };
        let err = s(d, "error");
        let queued = d.get("queued").and_then(Value::as_i64).unwrap_or(0);
        let words = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(title_t(when).font_features(tnum()))
            .when(!err.is_empty(), |w| w.child(desc_t(err.to_string(), BAD)))
            .when(queued > 0, |w| w.child(desc_t(root.i18n.tv("dc.queued", &[("n", &queued.to_string())]), MUTED)));
        let ctl = div().child(Button::new("s-dc-test", root.tr("dc.test")).small().icon("message").disabled(busy("test")).on_click(cx.listener(|r, _, _, cx| {
            ask_busy(r, "test", "discordTest", Value::Null, cx, |r, _| r.ok(r.tr("dc.testOk")));
        })));
        card.push(found(&root.tr("dc.test"), line(words, ctl).into_any_element()));
    }
    out.push(group(None, card));

    // ── how messages will look, once there is a link to send them to: right under the connection, above the tabs,
    // as the old page had it (it was hidden in the Messages tab).
    if has_hook && !super::finding() {
        out.push(preview(root, cx));
    }

    // ── the tabs: one kind of thing at a time
    let tab = root.settings.dc_tab;
    let tab = if tab.is_empty() { "summary" } else { tab };
    // (The app's one way of picking a part of a page: the chosen one the yellow block, as Appearance's and the
    // Inbox's are; an underline here was a second style for the same control.)
    let mut strip = div().w_full().flex().flex_wrap().items_center().gap(px(4.));
    for (i, (key, mark)) in [("summary", "activity"), ("alerts", "bell"), ("messages", "message"), ("control", "monitor")].into_iter().enumerate() {
        let picked = key == tab;
        strip = strip.child(chip(("s-dc-tab", i), picked).h(px(TOOL_H)).px(px(12.)).child(icon(mark, 15., c(if picked { INK } else { MUTED }))).child(root.tr(&format!("set.dc.tab.{key}"))).when(!picked, |d| {
            d.on_click(cx.listener(move |r, _, _, cx| {
                r.settings.dc_tab = key;
                r.settings.menu = None;
                r.settings.dc_pick = None;
                r.settings.dc_ping = None;
                r.settings.dc_shown = None;
                cx.notify();
            }))
        }));
    }
    if !super::finding() {
        out.push(strip);
    }
    // A search reads every tab's rows, not only the open one's ("discord" found two rows before).
    if super::finding() {
        out.extend(summary_tab(root, bx, cx));
        out.extend(alerts_tab(root, cx));
        out.extend(messages_tab(root, bx, cx));
        out.extend(control_tab(root, cx));
        return out;
    }
    match tab {
        "alerts" => out.extend(alerts_tab(root, cx)),
        "messages" => out.extend(messages_tab(root, bx, cx)),
        "control" => out.extend(control_tab(root, cx)),
        _ => out.extend(summary_tab(root, bx, cx)),
    }
    out
}

/// The play-time summary: when it goes, what it covers, who it pings.
fn summary_tab(root: &Root, bx: &Boxes, cx: &mut Context<Root>) -> Vec<Div> {
    let d = dc(root);
    let sm = d.get("summary").unwrap_or(&Value::Null);
    let (daily, weekly) = (flag(sm, "daily"), flag(sm, "weekly"));
    let when = match (daily, weekly) {
        (true, true) => "both",
        (true, false) => "daily",
        (false, true) => "weekly",
        _ => "none",
    };
    let whens = [("none", "dc.send.none"), ("daily", "dc.daily"), ("weekly", "dc.weekly"), ("both", "dc.send.both")].into_iter().map(|(v, k)| (v, root.tr(k), false)).collect();
    let mut sched: Vec<AnyElement> = vec![row(
        root.tr("dc.sendAuto"),
        None,
        chips(root, cx, "s-dc-when", whens, &when, |r, v, cx| {
            r.ask("setDiscord", json!({ "summaryDaily": v == "daily" || v == "both", "summaryWeekly": v == "weekly" || v == "both" }), cx, |_, _| {});
        }),
    )];
    if daily {
        sched.push(row(root.tr("dc.dailyAt"), None, time_box(&bx.daily)));
    }
    if weekly {
        let wd = sm.get("weekday").and_then(Value::as_i64).unwrap_or(0);
        let days = [1, 2, 3, 4, 5, 6, 0].into_iter().map(|n| (n, crate::time::day_long(n).to_string(), false)).collect();
        sched.push(row(
            root.tr("dc.weeklyAt"),
            None,
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(chips(root, cx, "s-dc-weekday", days, &wd, |r, v, cx| r.ask("setDiscord", json!({ "summaryWeekday": v }), cx, |_, _| {})))
                .child(time_box(&bx.weekly)),
        ));
    }
    sched.push(row(
        root.tr("dc.after"),
        Some(root.tr("dc.afterDesc")),
        switch("s-dc-after", flag(sm, "afterSchedule"), cx, |r, on, cx| r.ask("setDiscord", json!({ "summaryAfterSchedule": on }), cx, |_, _| {})),
    ));

    let period = sm.get("days").and_then(Value::as_i64).unwrap_or(1);
    let periods = [1, 7, 30].into_iter().map(|n| (n, root.tr(&format!("dc.days.{n}")), false)).collect();
    let mut what: Vec<AnyElement> = vec![row(
        root.tr("dc.period"),
        Some(root.tr("dc.periodDesc")),
        chips(root, cx, "s-dc-days", periods, &period, |r, v, cx| r.ask("setDiscord", json!({ "summaryDays": v }), cx, |_, _| {})),
    )];
    let parts: Vec<String> = sm.get("parts").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let picking = root.settings.dc_pick.as_ref().map(|(k, _)| *k);
    let parts_words = parts.iter().map(|p| root.tr(&format!("dc.part.{p}"))).collect::<Vec<_>>().join(", ");
    what.push(row(
        root.tr("dc.include"),
        Some(parts_words),
        Button::new("s-dc-parts", root.i18n.tn("dc.partsN", parts.len() as i64, &[])).small().icon_end("chevronDown").disabled(picking == Some("parts")).on_click(cx.listener(move |r, _, _, cx| {
            r.settings.dc_pick = Some(("parts", parts.iter().cloned().collect()));
            cx.notify();
        })),
    ));
    if picking == Some("parts") {
        what.push(dc_picker(root, cx));
    }
    let picked = sm.get("accounts").and_then(Value::as_array).map(|l| l.len()).unwrap_or(0);
    what.push(row(
        root.tr("dc.accounts"),
        None,
        Button::new("s-dc-acc", if picked > 0 { root.i18n.tv("dc.accN", &[("n", &picked.to_string())]) } else { root.tr("dc.accAll") })
            .small()
            .icon_end("chevronDown")
            .disabled(picking == Some("accounts"))
            .on_click(cx.listener(|r, _, _, cx| {
                if r.accounts().is_empty() {
                    r.toast = Some((r.tr("rc.noAccounts"), "info".into()));
                } else {
                    let chosen: HashSet<String> = r.state.pointer("/discord/summary/accounts").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                    r.settings.dc_pick = Some(("accounts", chosen));
                }
                cx.notify();
            })),
    ));
    if picking == Some("accounts") {
        what.push(dc_picker(root, cx));
    }
    what.push(row(root.tr("dc.ping"), None, ping(root, "summary", cx)));
    what.push(row(
        root.tr("dc.sendNow"),
        Some(root.tr("dc.sendNowDesc")),
        Button::new("s-dc-now", root.tr("dc.sendNowBtn")).small().icon("message").disabled(busy("now")).on_click(cx.listener(|r, _, _, cx| {
            ask_busy(r, "now", "discordSummaryNow", Value::Null, cx, |r, _| r.ok(r.tr("dc.sentOk")));
        })),
    ));
    vec![group(None, sched), group(Some(root.tr("dc.summary")), what)]
}

/// Alerts: on or off as a whole (every kind follows, one at a time), the ping for all, then each kind on its own line.
fn alerts_tab(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let d = dc(root);
    let events: Vec<String> = d.get("events").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let want = root.settings.dc_alerts_want;
    let any_on = NOTIFY_KINDS.iter().any(|k| events.iter().any(|e| e == k));
    let ev_list = events.clone();
    let all = vec![
        row(
            root.tr("dc.alertsOn"),
            Some(root.tr("dc.eventsHint")),
            switch("s-dc-alerts", want.unwrap_or(any_on), cx, move |r, on, cx| {
                let Some(link) = r.link.clone() else { return };
                r.settings.dc_alerts_want = Some(on);
                cx.notify();
                let todo: Vec<&'static str> = NOTIFY_KINDS.into_iter().filter(|k| ev_list.iter().any(|e| e == k) != on).collect();
                cx.spawn(async move |this, cx| {
                    let mut failed = None;
                    for kind in todo {
                        if let Ok(Err(e)) = link.call("setDiscord", json!({ "event": { "kind": kind, "enabled": on } })).await {
                            failed = Some(e);
                            break;
                        }
                    }
                    let _ = this.update(cx, |r, cx| {
                        r.settings.dc_alerts_want = None;
                        if let Some(e) = failed {
                            r.toast = Some((e, "bad".into()));
                        }
                        cx.notify();
                    });
                })
                .detach();
            })
            .disabled(want.is_some()),
        ),
        row(root.tr("dc.ping"), None, ping(root, "alerts", cx)),
    ];
    let mut kinds: Vec<AnyElement> = Vec::new();
    for (i, k) in NOTIFY_KINDS.into_iter().enumerate() {
        let on = events.iter().any(|e| e == k);
        let id: &'static str = ["s-dc-ev-drop", "s-dc-ev-schedule", "s-dc-ev-signin", "s-dc-ev-ram", "s-dc-ev-launch"][i];
        kinds.push(row(
            root.tr(&format!("set.notify.{k}")),
            None,
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .child(div().when(!on, |d| d.opacity(0.45)).child(ping(root, k, cx)))
                .child(switch(id, on, cx, move |r, on, cx| r.ask("setDiscord", json!({ "event": { "kind": k, "enabled": on } }), cx, |_, _| {}))),
        ));
    }
    kinds.push(note("info", root.tr("dc.idHelp"), MUTED));
    vec![group(None, all), group(Some(root.tr("dc.kinds")), kinds)]
}

/// How messages look: the status message, quiet hours, the sender's name and picture, language and style (the
/// preview of them stands above the tabs).
fn messages_tab(root: &Root, bx: &Boxes, cx: &mut Context<Root>) -> Vec<Div> {
    let d = dc(root);
    let mut out = Vec::new();
    let board = d.get("board").unwrap_or(&Value::Null);
    let minutes = board.get("minutes").and_then(Value::as_i64).unwrap_or(5);
    let every = [2, 5, 10, 15, 30, 60].into_iter().map(|m| (m, root.i18n.tv("dc.everyN", &[("n", &m.to_string())]), false)).collect();
    let quiet = d.get("quiet").unwrap_or(&Value::Null);
    out.push(group(
        None,
        vec![
            row(
                root.tr("dc.boardOn"),
                Some(root.tr("dc.boardDesc")),
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(chips(root, cx, "s-dc-minutes", every, &minutes, |r, v, cx| r.ask("setDiscord", json!({ "boardMinutes": v }), cx, |_, _| {})))
                    .child(switch("s-dc-board", flag(board, "enabled"), cx, |r, on, cx| r.ask("setDiscord", json!({ "board": on }), cx, |_, _| {}))),
            ),
            row(
                root.tr("dc.quietOn"),
                Some(root.tr("dc.quietDesc")),
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(time_box(&bx.from))
                    .child(div().text_color(c(MUTED)).child("–"))
                    .child(time_box(&bx.to))
                    .child(div().w(px(6.)))
                    .child(switch("s-dc-quiet", flag(quiet, "enabled"), cx, |r, on, cx| r.ask("setDiscord", json!({ "quiet": on }), cx, |_, _| {}))),
            ),
        ],
    ));
    let lang = { let l = s(d, "language"); if l.is_empty() { "app" } else { l } }.to_string();
    let style = { let l = s(d, "style"); if l.is_empty() { "embed" } else { l } }.to_string();
    let mut langs = vec![("app".to_string(), root.tr("dc.lang.app"), false)];
    langs.extend(crate::lang::picker_items().into_iter().map(|(code, name, off)| (code.to_string(), name, off)));
    let styles = ["embed", "plain"].into_iter().map(|v| (v.to_string(), root.tr(&format!("dc.style.{v}")), false)).collect();
    out.push(group(
        Some(root.tr("dc.look")),
        vec![
            row(root.tr("dc.name"), Some(root.tr("dc.nameDesc")), div().w(px(220.)).child(field(&bx.name, None))),
            row(root.tr("dc.avatar"), Some(root.tr("dc.avatarDesc")), div().w(px(220.)).child(field(&bx.avatar, None))),
            row(root.tr("dc.lang"), None, chips(root, cx, "s-dc-lang", langs, &lang, |r, v, cx| r.ask("setDiscord", json!({ "language": v }), cx, |_, _| {}))),
            row(root.tr("dc.style"), Some(root.tr("dc.styleDesc")), chips(root, cx, "s-dc-style", styles, &style, |r, v, cx| r.ask("setDiscord", json!({ "style": v }), cx, |_, _| {}))),
            row(
                root.tr("dc.hideNames"),
                Some(root.tr("dc.hideNamesDesc")),
                switch("s-dc-hidenames", flag(d, "hideNames"), cx, |r, on, cx| r.ask("setDiscord", json!({ "hideNames": on }), cx, |_, _| {})),
            ),
        ],
    ));
    out
}

/// Control from Discord, and "Playing Bloxkeep" on the profile (once Bloxkeep's Discord application exists).
fn control_tab(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let d = dc(root);
    let mut rows = remote(root, cx);
    if flag(d, "presenceReady") {
        rows.push(row(
            root.tr("dc.presence"),
            Some(root.tr("dc.presenceDesc")),
            switch("s-dc-presence", flag(d, "presence"), cx, |r, on, cx| r.ask("setDiscordPresence", json!({ "enabled": on }), cx, |_, _| {})),
        ));
    }
    vec![group(None, rows)]
}

/// The summary's parts or accounts, ticked under their row.
fn dc_picker(root: &Root, cx: &mut Context<Root>) -> AnyElement {
    let Some((what, ticked)) = root.settings.dc_pick.clone() else { return div().into_any_element() };
    let toggle = |r: &mut Root, id: String, on: bool| {
        if let Some((_, set)) = r.settings.dc_pick.as_mut() {
            if on { set.insert(id) } else { set.remove(&id) };
        }
    };
    let cancel = |r: &mut Root| r.settings.dc_pick = None;
    let saved = |r: &mut Root, v: Value, cx: &mut Context<Root>| {
        ask_busy(r, "pick", "setDiscord", v, cx, |r, _| r.settings.dc_pick = None);
    };
    if what == "parts" {
        let items = PARTS.iter().map(|p| (p.to_string(), root.tr(&format!("dc.part.{p}")), String::new())).collect();
        return picker(root, "dc-parts", root.tr("dc.partsText"), items, &ticked, cx, toggle, vec![], move |r, cx| {
            let set = r.settings.dc_pick.as_ref().map(|(_, s)| s.clone()).unwrap_or_default();
            let list: Vec<&str> = PARTS.into_iter().filter(|p| set.contains(*p)).collect();
            if list.is_empty() {
                r.toast = Some((r.tr("dc.partsText"), "warn".into()));
                cx.notify();
                return;
            }
            saved(r, json!({ "summaryParts": list }), cx);
        }, cancel);
    }
    let mut list: Vec<Value> = root.accounts().to_vec();
    list.sort_by_key(|a| crate::pages::accounts::label(a).to_lowercase());
    let items = list.iter().map(|a| (s(a, "id").to_string(), crate::pages::accounts::label(a), roblox::at_name(a))).collect();
    let all = Button::new("dc-acc-all", root.tr("dc.allAccounts")).small().quiet().on_click(cx.listener(move |r, _, _, cx| saved(r, json!({ "summaryAccounts": [] }), cx)));
    picker(root, "dc-acc", root.tr("dc.accText"), items, &ticked, cx, toggle, vec![all.into_any_element()], move |r, cx| {
        let ids: Vec<String> = r.settings.dc_pick.as_ref().map(|(_, s)| s.iter().cloned().collect()).unwrap_or_default();
        saved(r, json!({ "summaryAccounts": ids }), cx);
    }, cancel)
}

/// How the page's choices come out in Discord, with made-up accounts: an alert or the status message, in the words
/// the app sends, as a card (colour by meaning) or plain text.
fn preview(root: &Root, cx: &mut Context<Root>) -> Div {
    let d = dc(root);
    let lang = s(d, "language");
    let lang = if crate::lang::by_code(lang).is_some() { crate::lang::known(lang) } else { root.i18n.lang };
    let w = |k: &str| root.i18n.t_in(lang, &format!("dcv.{k}"));
    let hide = flag(d, "hideNames");
    let who = |n: i64| if hide { root.i18n.t_in(lang, "hide.account").replace("{n}", &n.to_string()) } else { format!("Player_0{n}") };
    let board = root.settings.dc_board_preview;
    let plain = s(d, "style") == "plain";
    let minutes = d.pointer("/board/minutes").and_then(Value::as_i64).filter(|m| *m > 0).unwrap_or(5);
    // (name, lines, the first line a link)
    type Field = (String, Vec<String>, bool);
    let (author, title, bar, text, small, fields, footer): (String, String, Hsla, Vec<String>, String, Vec<Field>, String) = if !board {
        (
            who(1),
            w("alertTitle"),
            c(WARN),
            vec![],
            String::new(),
            vec![(w("game"), vec!["Obby".into()], true), (w("played"), vec![w("long")], false), (w("reason"), vec![w("reasonSample")], false)],
            w("alertFoot"),
        )
    } else {
        (
            String::new(),
            w("boardTitle"),
            hsla(0.6, 0.5, 0.58, 1.),
            vec![format!("1 {}", w("boardOff"))],
            w("ram"),
            vec![
                (who(1), vec!["Obby".into(), format!("{} · {}", w("inGame"), w("long")), "1228 MB".into()], true),
                (who(2), vec!["Obby".into(), format!("{} · {}", w("off"), w("short")), "1229 MB".into()], true),
            ],
            w("boardFoot").replace("{n}", &minutes.to_string()),
        )
    };
    let ping = d.pointer("/pings/drop").and_then(Value::as_str).unwrap_or("none");
    let mention = if board { None } else {
        match ping.split(':').next().unwrap_or("none") {
            "here" => Some("@here"),
            "everyone" => Some("@everyone"),
            "user" => Some("@user"),
            "role" => Some("@role"),
            _ => None,
        }
    };
    let at = crate::lang::cur().hm(14, 30);
    let name = { let n = s(d, "username"); if n.is_empty() { "Bloxkeep" } else { n } }.to_string();
    // The sender's picture: the one linked once it has loaded, else Bloxkeep's logo (as Discord shows it then).
    let avatar = s(d, "avatarUrl").to_string();
    let face = if avatar.is_empty() { None } else { crate::images::pic_sq(&avatar, 36., cx) };
    let face = match face {
        Some(src) => img(src).size(px(36.)).flex_none().rounded_full(),
        None => img("logo.svg").size(px(36.)).flex_none().rounded_full(),
    };
    let small_t = |t: String| div().text_size(px(T_XS)).text_color(c(MUTED)).child(t);
    let body: AnyElement = if plain {
        let mut col = div().flex().flex_col().gap(px(2.)).text_size(px(T_S)).text_color(c(TEXT_2));
        col = col.child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(if author.is_empty() { title.clone() } else { format!("{author} · {title}") }));
        for l in &text {
            col = col.child(l.clone());
        }
        if !small.is_empty() {
            col = col.child(small_t(small.clone()));
        }
        for (n, lines, _) in &fields {
            col = col.child(format!("{n}: {}", lines.join(" · ")));
        }
        col.child(small_t(footer.clone())).into_any_element()
    } else {
        let mut grid = div().flex().flex_wrap().gap_x(px(24.)).gap_y(px(8.));
        for (n, lines, is_link) in &fields {
            let mut f = div().flex().flex_col().gap(px(1.)).text_size(px(T_S)).child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(n.clone()));
            for (i, l) in lines.iter().enumerate() {
                f = f.child(div().text_color(if *is_link && i == 0 { hsla(0.57, 0.6, 0.65, 1.) } else { c(TEXT_2) }).child(l.clone()));
            }
            grid = grid.child(f);
        }
        div()
            .flex()
            .rounded(px(R_S))
            .overflow_hidden()
            .bg(ca(WHITE, 0.04))
            .child(div().w(px(4.)).flex_none().bg(bar))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .p(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .when(!author.is_empty(), |d| d.child(div().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).child(author.clone())))
                    .child(div().text_size(px(T_M)).font_weight(FontWeight::BOLD).child(title.clone()))
                    .children(text.iter().map(|l| div().text_size(px(T_S)).text_color(c(TEXT_2)).child(l.clone())))
                    .when(!small.is_empty(), |d| d.child(small_t(small.clone())))
                    .child(grid)
                    .child(small_t(footer.clone())),
            )
            .into_any_element()
    };
    let this = cx.entity().downgrade();
    let kinds = Segmented::new("s-dc-preview", if board { "board" } else { "alert" })
        .item("alert", root.tr("dc.events"))
        .item("board", root.tr("dc.board"))
        .on_select(move |v, _, cx| {
            let board = v.as_ref() == "board";
            let _ = this.update(cx, |r, cx| {
                r.settings.dc_board_preview = board;
                cx.notify();
            });
        });
    let card = div()
        .px(px(16.))
        .py(px(14.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(div().flex().child(kinds))
        .child(
            div()
                .p(px(14.))
                .rounded(px(R_M))
                .bg(c(INSET))
                .flex()
                .gap(px(12.))
                .child(face)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(name))
                                .child(div().px(px(4.)).rounded(px(3.)).bg(hsla(0.65, 0.55, 0.6, 1.)).text_size(px(10.)).font_weight(FontWeight::BOLD).text_color(c(WHITE)).child("APP"))
                                .child(div().text_size(px(T_XS)).text_color(c(MUTED)).child(at)),
                        )
                        .when_some(mention, |d, m| d.child(div().flex().child(div().px(px(3.)).rounded(px(3.)).bg(hsla(0.65, 0.5, 0.6, 0.25)).text_size(px(T_S)).text_color(hsla(0.65, 0.7, 0.8, 1.)).child(m))))
                        .child(body),
                ),
        )
        .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(w("sample")));
    group(Some(w("title")), vec![card.into_any_element()])
}

/// Control from Discord: on, this PC takes /pc commands from the Discord account that typed its code into /pc link.
fn remote(root: &Root, cx: &mut Context<Root>) -> Vec<AnyElement> {
    let r = root.state.get("remote").unwrap_or(&Value::Null);
    let on = flag(r, "on");
    let mut rows = vec![row(
        root.tr("dc.remote"),
        Some(root.tr("dc.remoteDesc")),
        switch("s-dc-remote", on, cx, |r, on, cx| r.ask("setRemote", json!({ "enabled": on }), cx, |_, _| {})),
    )];
    if on {
        let connected = flag(r, "connected");
        let linked = flag(r, "linked");
        let code = s(r, "code");
        let until = r.get("codeUntil").and_then(Value::as_i64).unwrap_or(0);
        let left = if until > 0 { ((until - crate::time::now_secs() * 1000) as f64 / 60_000.).ceil().max(0.) as i64 } else { 0 };
        let t = |x: String| div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(x);
        let dsc = |x: String, col: u32| div().text_size(px(T_S)).text_color(c(col)).child(x);
        let mut lines = div().flex_1().min_w_0().flex().flex_col().gap(px(3.));
        if !connected {
            lines = lines.child(t(root.tr("dc.remoteConnecting")));
        } else if linked {
            let user = s(r, "user");
            lines = lines
                .child(t(if user.is_empty() { root.tr("dc.remoteLinked") } else { root.i18n.tv("dc.remoteLinkedTo", &[("user", user)]) }))
                .child(dsc(root.i18n.tv("dc.remoteUse", &[("pc", s(r, "pc"))]), MUTED));
        } else if !code.is_empty() && left > 0 {
            // The minutes left counted down: the page drawn again every 15 s while the code shows.
            if !TICKING.with(Cell::get) && !super::finding() {
                TICKING.with(|t| t.set(true));
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_secs(15)).await;
                    TICKING.with(|t| t.set(false));
                    let _ = this.update(cx, |_, cx| cx.notify());
                })
                .detach();
            }
            lines = lines
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(t(root.tr("dc.remoteCode")))
                        .child(div().px(px(8.)).py(px(2.)).rounded(px(R_S)).bg(c(RAISED_2)).text_size(px(T_M)).font_weight(FontWeight::BOLD).child(code.to_string())),
                )
                .child(dsc(root.i18n.tv("dc.remoteCodeHelp", &[("n", &left.to_string())]), MUTED));
        } else {
            lines = lines.child(t(root.tr("dc.remoteNoCode")));
            if flag(r, "busy") {
                lines = lines.child(dsc(root.tr("dc.remoteBusy"), WARN));
            }
        }
        let ctl = div()
            .flex()
            .gap(px(6.))
            .when(connected && !linked, |d| {
                d.child(Button::new("s-dc-remote-code", root.tr("dc.remoteNewCode")).small().disabled(busy("code")).on_click(cx.listener(|r, _, _, cx| ask_busy(r, "code", "remoteCode", Value::Null, cx, |_, _| {}))))
            })
            .when(!linked, |d| d.child(Button::new("s-dc-remote-add", root.tr("dc.remoteAdd")).small().quiet().on_click(cx.listener(|r, _, _, _| r.send("openDiscordApp", Value::Null)))));
        rows.push(div().px(px(ROW_X)).py(px(14.)).flex().items_center().gap(px(24.)).child(lines).child(ctl).into_any_element());
    }
    rows
}
