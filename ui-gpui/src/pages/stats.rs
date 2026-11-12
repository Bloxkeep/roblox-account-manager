// Statistics: play time (the old StatsView.tsx's Statistics) and Activity (what the app saw), a switch between them.
// Statistics: four figures with the change from the period before; today as a timeline (an account a row, its stays in
// its game's colour, a red end where one dropped), or for 7 and 30 days a bar a day and the hours of each day; why
// windows dropped; the games as boxes sized by their play time (a click counts only that game); each account as a card
// (7 and 30 days: a click opens its own timeline); by preset. Activity: the events newest first by day, filtered by
// account (a small menu) and kind (chips), exported as CSV, a page more at a time. Figures and events are asked for when
// the page opens and every 30 s while it is shown. The commands are the app's: playStats, activity, exportActivity.
use std::time::{Duration, Instant};

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{images, kit::*, shell::Root, theme::*};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Stats,
    Activity,
}

const PERIODS: [i64; 3] = [1, 7, 30];
const GROUPS: [&str; 5] = ["play", "drops", "schedule", "signin", "updates"];
const PAGE: i64 = 100;
const NO_PRESET: &str = "00000000-0000-0000-0000-000000000000";
const REFRESH: Duration = Duration::from_secs(30);

pub struct Data {
    pub tab: Tab,
    pub days: i64,
    /// One game's figures only (picked on the games' boxes), by its key.
    pub game: Option<String>,
    pub data: Option<Value>,
    failed: bool,
    seq: u64,
    /// 7 and 30 days: the account whose own timeline is open under the cards.
    pub open: Option<String>,
    pub log: Option<Value>,
    log_failed: bool,
    log_seq: u64,
    pub account: String,
    pub group: String,
    limit: i64,
    /// The account filter's menu is open.
    pub menu: bool,
    /// Today's clock: the account pointed at (its ring stands out, the middle says its time).
    active: Option<String>,
    last: Option<Instant>,
    pub scroll: ScrollHandle,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            tab: Tab::Stats,
            days: 7,
            game: None,
            data: None,
            failed: false,
            seq: 0,
            open: None,
            log: None,
            log_failed: false,
            log_seq: 0,
            account: String::new(),
            group: String::new(),
            limit: PAGE,
            menu: false,
            active: None,
            last: None,
            scroll: ScrollHandle::new(),
        }
    }
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}
fn int(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_f64).map(|x| x.round() as i64).unwrap_or(0)
}
fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}
fn secs(v: &Value, k: &str) -> i64 {
    crate::time::iso_secs(s(v, k)).unwrap_or(0)
}

// ───────────────────────── asking the app ─────────────────────────

/// Fresh figures and events when the page opens and every 30 s while it is shown.
pub fn want(r: &mut Root, cx: &mut Context<Root>) {
    if r.stats.last.is_some_and(|t| t.elapsed() < REFRESH) {
        return;
    }
    r.stats.last = Some(Instant::now());
    load_stats(r, cx);
    load_log(r, cx);
    // While the page stays open, the next round comes even if nothing else redraws it.
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(REFRESH + Duration::from_millis(50)).await;
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

/// The figures for the period (and game) picked; a reply to an older request is dropped, a failed one keeps the figures
/// already shown.
fn load_stats(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    r.stats.seq += 1;
    let mine = r.stats.seq;
    let rx = link.call("playStats", json!({ "days": r.stats.days, "game": r.stats.game }));
    cx.spawn(async move |this, cx| {
        let got = rx.await;
        let _ = this.update(cx, |r, cx| {
            if r.stats.seq != mine {
                return;
            }
            match got {
                Ok(Ok(v)) if v.is_object() => {
                    r.stats.data = Some(v);
                    r.stats.failed = false;
                    // BLOXKEEP_SHOT_SCROLL=900: a shot of the page scrolled down that far, once there is something to scroll.
                    #[cfg(feature = "shot")]
                    if let Ok(y) = std::env::var("BLOXKEEP_SHOT_SCROLL").map(|v| v.parse::<f32>().unwrap_or(0.)) {
                        r.stats.scroll.set_offset(point(px(0.), px(-y)));
                    }
                }
                _ => r.stats.failed = true,
            }
            cx.notify();
        });
    })
    .detach();
}

fn load_log(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    r.stats.log_seq += 1;
    let mine = r.stats.log_seq;
    let opt = |v: &str| if v.is_empty() { Value::Null } else { json!(v) };
    let rx = link.call("activity", json!({ "account": opt(&r.stats.account), "group": opt(&r.stats.group), "limit": r.stats.limit }));
    cx.spawn(async move |this, cx| {
        let got = rx.await;
        let _ = this.update(cx, |r, cx| {
            if r.stats.log_seq != mine {
                return;
            }
            match got {
                Ok(Ok(v)) if v.is_object() => {
                    r.stats.log = Some(v);
                    r.stats.log_failed = false;
                }
                _ => r.stats.log_failed = true,
            }
            cx.notify();
        });
    })
    .detach();
}

pub fn export(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let opt = |v: &str| if v.is_empty() { Value::Null } else { json!(v) };
    let rx = link.call("exportActivity", json!({ "account": opt(&r.stats.account), "group": opt(&r.stats.group) }));
    cx.spawn(async move |this, cx| {
        let got = rx.await;
        let _ = this.update(cx, |r, cx| {
            match got {
                Ok(Ok(v)) => {
                    let n = v.as_i64().unwrap_or(0);
                    if n > 0 {
                        r.toast = Some((r.i18n.tn("alog.exported", n, &[("n", &num(n, r.i18n.vietnamese))]), "ok".into()));
                    }
                }
                Ok(Err(e)) => r.toast = Some((e, "bad".into())),
                Err(_) => {}
            }
            cx.notify();
        });
    })
    .detach();
}

// ───────────────────────── words for numbers and times ─────────────────────────

/// 1.234 in Vietnamese, 1,234 in English.
fn num(n: i64, vi: bool) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(if vi { '.' } else { ',' });
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// "35 phút", "2 giờ 5 phút", "2 giờ" (the old page's playedText).
pub fn played(root: &Root, seconds: i64) -> String {
    let m = seconds.max(0) / 60;
    let h = m / 60;
    if m < 60 {
        root.i18n.tv("dur.min", &[("m", &m.to_string())])
    } else if m % 60 > 0 {
        root.i18n.tv("dur.hour", &[("h", &num(h, root.i18n.vietnamese)), ("m", &(m % 60).to_string())])
    } else {
        root.i18n.tv("dur.hours", &[("h", &num(h, root.i18n.vietnamese))])
    }
}

/// "3h06" on a bar, the same in both languages (never "0h00" for some play).
fn bar_time(s: i64) -> String {
    let m = if s > 0 { (s as f64 / 60.).round().max(1.) as i64 } else { 0 };
    format!("{}h{:02}", m / 60, m % 60)
}

fn offset() -> i64 {
    crate::time::utc_offset_secs()
}

/// A moment's local time of day: "07:05" in Vietnamese, "7:05 AM" in English.
fn clock_at(root: &Root, at: i64) -> String {
    let t = (at + offset()).rem_euclid(86400);
    let (h, m) = (t / 3600, t / 60 % 60);
    if root.i18n.vietnamese {
        format!("{h:02}:{m:02}")
    } else {
        format!("{}:{m:02} {}", if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "AM" } else { "PM" })
    }
}

/// Hour h as a mark on a scale: "07:00", "7 AM".
fn hour_mark(root: &Root, h: i64) -> String {
    if root.i18n.vietnamese {
        format!("{h:02}:00")
    } else {
        format!("{} {}", if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "AM" } else { "PM" })
    }
}

fn local_day(at: i64) -> i64 {
    (at + offset()).div_euclid(86400)
}

/// A day since 1970 as (year, month, day).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}
fn weekday(days: i64) -> usize {
    (days + 4).rem_euclid(7) as usize
}

/// A day's short name under a bar: "T2"…"CN" / "Mon"…; or its date ("9/11", "11/9") when the period is long.
fn day_label(root: &Root, day: i64, many: bool) -> String {
    let vi = root.i18n.vietnamese;
    if many {
        let (_, m, d) = civil(day);
        return if vi { format!("{d}/{m}") } else { format!("{m}/{d}") };
    }
    let w = weekday(day);
    if vi {
        if w == 0 { "CN".into() } else { format!("T{}", w + 1) }
    } else {
        ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][w].into()
    }
}

/// An Activity day's heading: Today, Yesterday, "Thứ Hai, 9 tháng 11", "Monday, November 9" (the year when not this one).
fn day_heading(root: &Root, day: i64, today: i64) -> String {
    if day == today {
        return root.tr("alog.today");
    }
    if day == today - 1 {
        return root.tr("alog.yesterday");
    }
    let (y, m, d) = civil(day);
    let (ty, _, _) = civil(today);
    let w = weekday(day);
    if root.i18n.vietnamese {
        let name = ["Chủ Nhật", "Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy"][w];
        if y != ty { format!("{name}, {d} tháng {m}, {y}") } else { format!("{name}, {d} tháng {m}") }
    } else {
        let name = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"][w];
        let month = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"]
            [(m - 1).clamp(0, 11) as usize];
        if y != ty { format!("{name}, {month} {d}, {y}") } else { format!("{name}, {month} {d}") }
    }
}

// ───────────────────────── colours ─────────────────────────

/// The web page's accent on its default theme (app.css --accent): the charts' bars and the clock's hand.
const ACCENT: u32 = 0x22c55e;
/// The busiest hour of the heat map: the accent leaned 40 % towards the text colour (app.css --heat).
const HEAT: u32 = 0x72d59a;
/// The alts board's medals (app.css .cm-board).
const GOLD: u32 = 0xe8b931;
const SILVER: u32 = 0xb8c0cc;
const BRONZE: u32 = 0xc98a55;
const EYE: u32 = 0x1d1a14;

/// A game's own hue in degrees, the same everywhere it shows (clock, boxes, cards), kept off red, which says "dropped".
fn hue_of(key: &str) -> f32 {
    let mut h: u32 = 0;
    for u in key.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u as u32);
    }
    (40 + h % 280) as f32
}

/// oklch(l c h / alpha) as sRGB (Björn Ottosson's OKLab), clamped into the screen's colours.
fn oklch(l: f32, chroma: f32, deg: f32, alpha: f32) -> Hsla {
    let (a, b) = (chroma * deg.to_radians().cos(), chroma * deg.to_radians().sin());
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let lin = [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ];
    let gamma = |x: f32| {
        let x = x.clamp(0., 1.);
        if x <= 0.003_130_8 { 12.92 * x } else { 1.055 * x.powf(1. / 2.4) - 0.055 }
    };
    Rgba { r: gamma(lin[0]), g: gamma(lin[1]), b: gamma(lin[2]), a: alpha }.into()
}

/// A game's colour: its hue at the web page's lightness and strength (app.css --g-l .66, --g-c .14).
fn game_color(key: &str) -> Hsla {
    oklch(0.66, 0.14, hue_of(key), 1.)
}

// ───────────────────────── the page ─────────────────────────

pub fn stats(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let st = &root.stats;
    let on_stats = st.tab == Tab::Stats;
    let data = st.data.as_ref();
    let never = data.is_some_and(|d| int(d, "sessions") == 0 && !d.get("recorded").and_then(Value::as_bool).unwrap_or(false));

    let switch = Segmented::new("st-tab", if on_stats { "s" } else { "a" })
        .item("s", root.tr("stats.title"))
        .item("a", root.tr("alog.title"))
        .on_select({
            let this = cx.entity().downgrade();
            move |v, _, cx| {
                let t = if v.as_ref() == "a" { Tab::Activity } else { Tab::Stats };
                let _ = this.update(cx, |r, cx| {
                    r.stats.tab = t;
                    r.stats.menu = false;
                    cx.notify();
                });
            }
        });
    let period = PERIODS
        .iter()
        .fold(Segmented::new("st-period", st.days.to_string()), |seg, d| seg.item(d.to_string(), root.tr(&format!("stats.period.{d}"))))
        .on_select({
            let this = cx.entity().downgrade();
            move |v, _, cx| {
                let d: i64 = v.parse().unwrap_or(7);
                let _ = this.update(cx, |r, cx| {
                    if r.stats.days != d {
                        r.stats.days = d;
                        r.stats.open = None;
                        r.stats.failed = false;
                        load_stats(r, cx);
                    }
                    cx.notify();
                });
            }
        });

    let header = div()
        .flex_none()
        .px(px(SIDE))
        .pt(px(22.))
        .pb(px(16.))
        .flex()
        .items_center()
        .gap(px(12.))
        // The title with what is counted under it (StatsView.tsx's page head), then the switch and the period.
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_size(px(T_TITLE)).line_height(px(38.)).font_weight(FontWeight::BOLD).child(root.tr("nav.stats")))
                .child(div().truncate().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr(if on_stats { "stats.note" } else { "alog.note" }))),
        )
        .child(switch)
        .when(on_stats && !never, |d| d.child(period));

    let inner = width - 2. * SIDE;
    let body = if on_stats { figures(root, inner, cx).into_any_element() } else { activity(root, cx).into_any_element() };
    div()
        .id("stats")
        .size_full()
        .rounded(px(R_PANEL))
        .bg(c(PANEL))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(header)
        .child(crate::scrolled::with_bar("stats-body", &st.scroll, div().id("stats-body").size_full().overflow_y_scroll().track_scroll(&st.scroll).child(div().px(px(SIDE)).pt(px(4.)).pb(px(32.)).child(body))))
        .into_any_element()
}

/// The page's sides (the Accounts page's).
const SIDE: f32 = 24.;

/// A section of the page: a calm panel, a hairline round it.
fn card() -> Div {
    surface().p(px(20.)).flex().flex_col().gap(px(16.))
}
/// A section's head: its title at 16 px (the kit's), what it does at the right.
fn card_title(title: String) -> Div {
    section_head(title, None)
}

fn empty(icon_name: &'static str, title: String, text: Option<String>) -> Div {
    div()
        // (As Automation's empty lists: the same block, size and drop from the head, so the places read as one app.)
        .pt(px(96.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .child(div().size(px(56.)).flex_none().rounded(px(R_L)).bg(c(RAISED_2)).flex().items_center().justify_center().child(icon(icon_name, 24., c(TEXT_2))).mb(px(6.)))
        .child(div().text_size(px(T_SEC)).font_weight(FontWeight::SEMIBOLD).child(title))
        .when_some(text, |d, t| d.child(div().max_w(px(420.)).text_size(px(T_M)).text_color(c(MUTED)).text_center().child(t)))
}

/// An account's name as the page shows it: names hidden, the list's own "Account N"; a deleted account its old name.
fn name_of(root: &Root, a: &Value) -> String {
    let hide = root.state.get("hideNames").and_then(Value::as_bool).unwrap_or(false);
    let id = s(a, "id");
    let shown = if hide { root.accounts().iter().find(|x| s(x, "id") == id).map(|x| s(x, "label").to_string()) } else { Some(s(a, "label").to_string()) };
    shown.filter(|n| !n.is_empty()).unwrap_or_else(|| root.tr("stats.deleted"))
}

/// An account's picture; a deleted account's is a plain figure (its old picture may be someone else's by now).
fn face(a: &Value, size: f32, cx: &mut App) -> Div {
    let deleted = a.get("deleted").and_then(Value::as_bool).unwrap_or(false);
    let pic = if deleted { None } else { images::pic_sq(s(a, "avatarUrl"), size, cx) };
    div()
        .size(px(size))
        .flex_none()
        .rounded_full()
        .overflow_hidden()
        .bg(c(RAISED_2))
        .flex()
        .items_center()
        .justify_center()
        .map(|d| match pic {
            Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
            None => d.child(icon("user", size * 0.5, c(MUTED))),
        })
}

fn game_pic(url: &str, size: f32, fallback: &'static str, cx: &mut App) -> Div {
    let pic = Some(url).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, size, cx));
    div()
        .size(px(size))
        .flex_none()
        .rounded(px(size * 0.24))
        .overflow_hidden()
        .bg(c(RAISED_2))
        .flex()
        .items_center()
        .justify_center()
        .map(|d| match pic {
            Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
            None => d.child(icon(fallback, size * 0.5, c(MUTED))),
        })
}

// ───────────────────────── Statistics ─────────────────────────

fn figures(root: &Root, w: f32, cx: &mut Context<Root>) -> Div {
    let st = &root.stats;
    let Some(d) = st.data.as_ref() else {
        if st.failed {
            return empty("alert", root.tr("stats.error"), None).child(
                div().mt(px(8.)).child(Button::new("st-retry", root.tr("srv.retry")).icon("refresh").on_click(cx.listener(|r, _, _, cx| {
                    r.stats.failed = false;
                    load_stats(r, cx);
                    cx.notify();
                }))),
            );
        }
        return div().pt(px(80.)).flex().justify_center().text_color(c(MUTED)).text_size(px(T_M)).child("…");
    };
    if int(d, "sessions") == 0 {
        let recorded = d.get("recorded").and_then(Value::as_bool).unwrap_or(false);
        let e = if recorded { empty("clock", root.tr("stats.emptyPeriod"), None) } else { empty("clock", root.tr("stats.emptyTitle"), Some(root.tr("stats.emptyText"))) };
        return e.when(st.game.is_some(), |e| e.child(div().mt(px(8.)).child(Button::new("st-all", root.tr("stats.allGames")).on_click(cx.listener(|r, _, _, cx| pick_game(r, None, cx))))));
    }
    let hourly = d.get("hourly").and_then(Value::as_bool).unwrap_or(false);
    let mut page = div().flex().flex_col().gap(px(GRID_GAP)).child(tiles(root, d, w));
    let drops = drops(root, d, cx);
    if hourly {
        // On a wide window the day's clock and why they dropped stand side by side (no empty half under the clock's list).
        // The top games go under it on that side: the column no longer ends in an empty half beside the clock.
        let pair = w >= 1280.;
        let clock_w = if pair { (w * 0.6).floor() } else { w };
        let clock_card = card().child(card_title(root.tr("stats.timeline"))).child(clock(root, d, clock_w - 42., cx));
        if pair {
            let side_w = w - clock_w - GRID_GAP;
            let _ = side_w;
            let side = div().flex_1().min_w_0().flex().flex_col().gap(px(GRID_GAP)).children(drops).child(game_list(root, d, 1, cx));
            return page
                .child(div().flex().items_start().gap(px(GRID_GAP)).child(clock_card.w(px(clock_w)).flex_none()).child(side))
                .child(account_cards(root, d, w, cx))
                .children(by_preset(root, d, cx))
                .child(alts_board(root, w, cx));
        }
        page = page.child(clock_card).children(drops);
    } else {
        page = page.child(days(root, d, w));
        if !arr(d, "heat").is_empty() {
            page = page.child(heat(root, d, w));
        }
        page = page.children(drops);
    }
    page.child(game_list(root, d, if w >= 1000. { 2 } else { 1 }, cx)).child(account_cards(root, d, w, cx)).children(by_preset(root, d, cx)).child(alts_board(root, w, cx))
}

/// One alt on the alts board: a little block head like Bloxkeep's own (Community.tsx's cm-head), eyes and a smile once
/// it is big enough to show them.
fn alt_head(size: f32, tone: Hsla) -> Div {
    let u = size / 20.;
    let eye = |x: f32| div().absolute().left(px(x * u)).top(px(7. * u)).w(px(2.2 * u)).h(px(3.4 * u)).rounded(px(1.1 * u)).bg(c(EYE));
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(div().absolute().left(px(u)).top(px(u)).size(px(18. * u)).rounded(px(4.5 * u)).bg(tone))
        .when(size >= 10., |d| d.child(eye(6.)).child(eye(11.8)))
        .when(size >= 16., |d| d.child(div().absolute().left(px(6.6 * u)).top(px(11.2 * u)).w(px(6.8 * u)).h(px(2.4 * u)).border_b_1().border_color(c(EYE)).rounded_b(px(3. * u))))
}

/// The alts board (Community.tsx's AltsBoard): the PCs running the most accounts now, the first three in gold, silver
/// and bronze, this PC marked; it is joined under a name typed here (shown to everyone; empty: off the board). Each PC
/// is a row with a bar of its accounts against the leader's: the block head stays as the board's mark (a row of up to
/// 200 little heads each was noise, review 6.2).
fn alts_board(root: &Root, w: f32, cx: &mut Context<Root>) -> Div {
    let vi = root.i18n.vietnamese;
    let cm = root.state.pointer("/community/data").filter(|v| v.is_object());
    let counted = root.state.get("countOnline").and_then(Value::as_bool) != Some(false);
    let name = s(&root.state, "boardName").to_string();
    let board: Vec<Value> = cm.map(|c| arr(c, "board").to_vec()).unwrap_or_default();
    let rank = cm.and_then(|c| c.pointer("/you/rank")).and_then(Value::as_i64).unwrap_or(0);
    let of = cm.and_then(|c| c.pointer("/you/of")).and_then(Value::as_i64).unwrap_or(0);
    let most = board.iter().map(|b| int(b, "alts")).max().unwrap_or(0).max(1);
    let narrow = w < 900.;

    // The head: the board's own block head, its title and liveness, and the sums on the right.
    let mut head = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(14.))
        .child(alt_head(30., c(GOLD)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr("cm.board")))
                .when(cm.is_some(), |d| {
                    d.child(div().flex().items_center().gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(div().size(px(7.)).rounded_full().bg(c(OK))).child(format!("{} · {}", root.tr("cm.live"), root.tr("cm.liveSub"))))
                }),
        );
    if let Some(data) = cm {
        let (alts, pcs) = (int(data, "alts"), int(data, "pcs"));
        let per = if pcs > 0 { format!("{:.1}", alts as f64 / pcs as f64) } else { "0".into() };
        let per = if vi { per.replace('.', ",") } else { per };
        let fact = |n: String, label: String| {
            div()
                .min_w(px(84.))
                .flex()
                .flex_col()
                .gap(px(1.))
                .px(px(12.))
                .py(px(7.))
                .rounded(px(R_M))
                .bg(c(RAISED))
                .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).child(n))
                .child(div().text_size(px(T_XS)).text_color(c(MUTED)).child(label))
        };
        head = head.child(div().flex_1()).child(
            div().flex().flex_wrap().gap(px(8.)).child(fact(num(alts, vi), root.tr("cm.accounts"))).child(fact(num(pcs, vi), root.tr("cm.pcs"))).child(fact(per, root.tr("cm.perPc"))),
        );
    }
    let mut card = card().child(head);
    if cm.is_none() {
        card = card.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("news.connecting")));
    } else if board.is_empty() {
        card = card.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("cm.empty")));
    } else {
        let mut rows = div().flex().flex_col();
        for (i, b) in board.iter().enumerate() {
            let me = !name.is_empty() && rank == i as i64 + 1;
            let first = i == 0;
            let alts = int(b, "alts");
            let medal = [GOLD, SILVER, BRONZE].get(i).copied();
            let tone = if me { c(OK) } else { medal.map_or(ca(TEXT_2, 0.45), c) };
            let rank_cell = div().w(px(24.)).flex_none().flex().justify_center().map(|d| match medal {
                Some(m) => d.child(
                    div().size(px(22.)).rounded(px(R_S)).bg(c(m)).flex().items_center().justify_center().text_size(px(T_S)).font_weight(FontWeight::BOLD).text_color(c(EYE)).child((i + 1).to_string()),
                ),
                None => d.text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).font_features(tnum()).child((i + 1).to_string()),
            });
            let you = div().flex_none().px(px(6.)).py(px(1.)).rounded(px(R_S)).bg(ca(OK, 0.16)).text_color(c(OK)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).child(root.tr("cm.you"));
            let name_cell = div()
                .w(px(if narrow { 150. } else { 220. }))
                .flex_none()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .min_w_0()
                        .child(div().min_w_0().truncate().text_size(px(if first { T_L } else { T_M })).font_weight(FontWeight::SEMIBOLD).child(s(b, "name").to_string()))
                        .when(me, |d| d.child(you)),
                )
                .when(first, |d| d.child(div().text_size(px(T_XS)).text_color(mix(c(GOLD), c(TEXT), 0.3)).child(root.tr("cm.leader"))));
            let part = (alts as f32 / most as f32).clamp(0.02, 1.);
            let bar = div().flex_1().min_w_0().h(px(8.)).rounded_full().bg(c(RAISED_2)).child(div().h_full().w(relative(part)).rounded_full().bg(tone));
            let count = div()
                .w(px(76.))
                .flex_none()
                .flex()
                .items_baseline()
                .justify_end()
                .gap(px(4.))
                .font_features(tnum())
                .child(div().text_size(px(if first { T_L } else { T_M })).font_weight(FontWeight::SEMIBOLD).child(num(alts, vi)))
                .child(div().text_size(px(T_XS)).text_color(c(MUTED)).child(root.tr("cm.alts")));
            rows = rows.child(
                div()
                    .min_h(px(if first { 52. } else { 40. }))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .rounded(px(R_M))
                    .when(first, |d| d.bg(ca(GOLD, 0.07)))
                    .when(me && !first, |d| d.bg(ca(OK, 0.06)))
                    .when(i > 1, |d| d.border_t_1().border_color(c(LINE)))
                    .child(rank_cell)
                    .child(name_cell)
                    .child(bar)
                    .child(count),
            );
        }
        card = card.child(rows);
    }
    // This PC at the foot: what it is on the board, and the name it joins under, kept with Save (or Enter).
    let foot = if !counted {
        div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("cm.joinOff"))
    } else {
        let on_board = !name.is_empty() && rank > 0 && rank as usize <= board.len();
        let mine = if on_board { int(&board[rank as usize - 1], "alts") } else { root.accounts().iter().filter(|a| s(a, "state") == "running").count() as i64 };
        let line = if on_board {
            Some(root.i18n.tv("cm.youLine", &[("name", &name), ("r", &rank.to_string()), ("n", &num(of, vi)), ("a", &num(mine, vi))]))
        } else if !name.is_empty() && rank > 0 {
            Some(root.i18n.tv("cm.yourRank", &[("r", &rank.to_string()), ("n", &num(of, vi))]))
        } else if !name.is_empty() && cm.is_some_and(|c| c.get("you").is_some_and(|y| !y.is_null())) {
            Some(root.tr("cm.notYet"))
        } else {
            None
        };
        let typed: String = root.board_name.read(cx).value().trim().chars().take(20).collect();
        let changed = typed != name;
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x(px(24.))
            .gap_y(px(12.))
            .child(
                div()
                    .flex_1()
                    .min_w(px(260.))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.tr(if name.is_empty() { "cm.join" } else { "cm.nameOn" })))
                    .when_some(line, |d, text| d.child(div().text_size(px(T_S)).text_color(c(TEXT_2)).child(text)))
                    .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("cm.joinNote"))),
            )
            .child(
                div().flex().items_center().gap(px(8.)).child(div().w(px(220.)).child(field(&root.board_name, None))).child(
                    Button::new("cm-save", root.tr("rs.save")).primary().disabled(!changed).on_click(cx.listener(|r, _, _, cx| {
                        let name: String = r.board_name.read(cx).value().trim().chars().take(20).collect();
                        r.send("setBoardName", json!({ "name": name }));
                        cx.notify();
                    })),
                ),
            )
    };
    card.child(div().pt(px(16.)).border_t_1().border_color(c(LINE)).child(foot))
}

fn pick_game(r: &mut Root, key: Option<String>, cx: &mut Context<Root>) {
    r.stats.game = key;
    r.stats.open = None;
    load_stats(r, cx);
    cx.notify();
}

/// Play time, accounts played, drops, the longest stay; the first and third with the change from the period before.
fn tiles(root: &Root, d: &Value, w: f32) -> Div {
    let vi = root.i18n.vietnamese;
    let hourly = d.get("hourly").and_then(Value::as_bool).unwrap_or(false);
    let period = if hourly { 1 } else { Some(int(d, "days")).filter(|n| *n > 0).unwrap_or(arr(d, "buckets").len() as i64) };
    let prev = d.get("previous").filter(|p| p.is_object());
    let change = |now: i64, before: Option<i64>, more_is_good: bool| -> Option<Div> {
        let before = before.filter(|b| *b > 0)?;
        let pct = ((now - before) as f64 / before as f64 * 100.).round() as i64;
        let vs = root.tr(&format!("stats.vs.{period}"));
        let line = div().flex().items_center().gap(px(5.)).text_size(px(T_S));
        Some(if pct == 0 {
            line.text_color(c(MUTED)).child(root.tr("stats.noChange")).child(vs)
        } else {
            let good = (pct > 0) == more_is_good;
            line.child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(if good { OK } else { BAD }))
                    .child(format!("{} {} %", if pct > 0 { "↑" } else { "↓" }, num(pct.abs(), vi))),
            )
            .child(div().text_color(c(MUTED)).child(vs))
        })
    };
    let all = root.accounts().len() as i64;
    let played_n = arr(d, "accounts").iter().filter(|a| int(a, "seconds") > 0 && !a.get("deleted").and_then(Value::as_bool).unwrap_or(false)).count() as i64;
    let longest = d.get("longest").filter(|l| l.is_object());
    let total = int(d, "totalSeconds");
    let kicks = int(d, "kicks");
    // The period's play time leads (twice as wide, its figure at title size); the others follow, their figures smaller,
    // what they compare with muted under them.
    let tile = |hero: bool, label: String, value: Div, sub: Option<Div>, aside: Option<Div>| {
        let words = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .justify_between()
            .gap(px(6.))
            .child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(MUTED)).child(label))
            .child(value.text_size(px(if hero { T_TITLE + 6. } else { T_FIG + 2. })).line_height(px(if hero { 42. } else { 30. })).font_weight(FontWeight::BOLD).font_features(tnum()).truncate())
            .child(sub.unwrap_or_else(|| div().text_size(px(T_S)).child(" ")));
        div()
            .when(w >= 700., |d| d.flex_basis(px(0.)).flex_grow(if hero { 2. } else { 1. }))
            .min_w(px(if hero { 260. } else { 160. }))
            .rounded(px(R_L))
            .bg(ca(WHITE, 0.045))
            .border_1()
            .border_color(c(LINE))
            .px(px(20.))
            .py(px(16.))
            .flex()
            .gap(px(20.))
            .child(words)
            .children(aside)
    };
    // The lead tile is twice as wide as its figure needs: the period's days as little bars beside it, the last (today)
    // bright, so the width says something (review 6.3). Over a day, a bar per hour (none brighter); over 30 days, the bars
    // thinner.
    let spark = {
        let b = arr(d, "buckets");
        let mut sec: Vec<i64> = b.iter().map(|x| int(x, "seconds")).collect();
        // Today's hours still to come are not drawn (they read as hours with no play).
        if hourly {
            while sec.last() == Some(&0) {
                sec.pop();
            }
        }
        let top = sec.iter().copied().max().unwrap_or(0);
        // (From 1200 px: on the owner's 1199 px window the bars cut "16 h 24 min" to "16 h 24…", 2026-11-12.)
        (w >= 1200. && sec.len() >= 2 && top > 0).then(|| {
            let n = sec.len();
            let bar_w = if n > 10 { 6. } else { 12. };
            div().flex_none().self_end().h(px(56.)).flex().items_end().gap(px(if n > 10 { 3. } else { 6. })).children(sec.iter().enumerate().map(|(i, v)| {
                let h = (*v as f32 / top as f32 * 56.).max(3.);
                div().w(px(bar_w)).h(px(h)).rounded(px(2.)).bg(if !hourly && i + 1 == n { c(ACCENT) } else { ca(WHITE, 0.16) })
            }))
        })
    };
    let long_sub = longest.map(|l| {
        let id = s(l, "account");
        let who = root.accounts().iter().find(|a| s(a, "id") == id);
        let hide = root.state.get("hideNames").and_then(Value::as_bool).unwrap_or(false);
        let name = if hide { who.map(|a| s(a, "label").to_string()) } else { Some(s(l, "label").to_string()) }
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| root.tr("stats.deleted"));
        let text = [name, s(l, "game").to_string()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
        div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(text)
    });
    div()
        .flex()
        .flex_wrap()
        .gap(px(GRID_GAP))
        .when(w < 700., |d| d.flex_col())
        .child(tile(true, root.tr("stats.total"), div().child(played(root, total)), change(total, prev.map(|p| int(p, "totalSeconds")), true), spark))
        .child(tile(
            false,
            root.tr("stats.played"),
            div().flex().items_baseline().child(num(played_n, vi)).child(div().text_size(px(T_H)).text_color(c(MUTED)).child(format!("/{}", num(all, vi)))),
            (all > 0).then(|| {
                let idle = (all - played_n).max(0);
                div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(if idle == 0 { root.tr("stats.allPlayed") } else { root.i18n.tv("stats.idleN", &[("n", &num(idle, vi))]) })
            }),
            None,
        ))
        .child(tile(false, root.tr("stats.kicks"), div().child(num(kicks, vi)), change(kicks, prev.map(|p| int(p, "kicks")), false), None))
        .child(tile(false, root.tr("stats.longest"), div().child(longest.map(|l| played(root, int(l, "seconds"))).unwrap_or("—".into())), long_sub, None))
}

/// Each account's stays from the period's start up to now (two hours at least), each in its game's colour with its icon
/// (its name too where it fits), a red end where one dropped. Today every account's; `only` one account's.
fn timeline(root: &Root, d: &Value, only: Option<&str>, w: f32, cx: &mut Context<Root>) -> Div {
    let hourly = d.get("hourly").and_then(Value::as_bool).unwrap_or(false);
    let since = secs(d, "since");
    let now = crate::time::now_secs();
    let span = (now - since).max(2 * 3600) as f32;
    let at = |t: i64| ((t - since) as f32 / span).clamp(0., 1.);
    let name_w = if only.is_some() { 0. } else { 150. };
    let track_w = (w - name_w - if only.is_some() { 0. } else { 12. }).max(100.);
    let accounts = arr(d, "accounts");
    let icons = d.get("icons");
    let mut rows = div().flex().flex_col().gap(px(8.));
    let mut any_drop = false;
    for (k, row) in arr(d, "timeline").iter().enumerate() {
        let id = s(row, "id");
        if only.is_some_and(|o| o != id) {
            continue;
        }
        let Some(a) = accounts.iter().find(|x| s(x, "id") == id) else { continue };
        let mut track = div().relative().h(px(24.)).w(px(track_w)).rounded(px(6.)).bg(ca(WHITE, 0.04));
        for (i, seg) in arr(row, "segments").iter().enumerate() {
            let (from, to) = (secs(seg, "from"), secs(seg, "to"));
            let (x0, x1) = (at(from), at(to));
            let seg_w = ((x1 - x0) * track_w).max(3.);
            let key = s(seg, "game");
            let dropped = !s(seg, "reason").is_empty();
            any_drop |= dropped;
            let pic = icons.and_then(|m| m.get(key)).and_then(Value::as_str).filter(|_| seg_w >= 26.).and_then(|u| images::pic(u, cx));
            let name = s(seg, "name").to_string();
            let mut tip = vec![name.clone(), format!("{} – {}", stay_clock(root, hourly, from), stay_clock(root, hourly, to)), played(root, to - from)];
            if dropped {
                tip.push(root.tr(&format!("stats.why.{}", s(seg, "reason"))));
            }
            let tip: SharedString = tip.into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ").into();
            track = track.child(
                div()
                    .id(SharedString::from(format!("tl-{k}-{i}")))
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(x0 * track_w))
                    .w(px(seg_w))
                    .rounded(px(6.))
                    .overflow_hidden()
                    .bg(game_color(key))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(3.))
                    .when_some(pic, |d, p| d.child(img(p).size(px(18.)).flex_none().rounded(px(4.))))
                    .when(seg_w >= 110., |d| d.child(div().min_w_0().truncate().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(name)))
                    .when(dropped, |d| d.child(div().absolute().right_0().top_0().bottom_0().w(px(3.)).bg(c(BAD))))
                    .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
            );
        }
        let label = name_of(root, a);
        rows = rows.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .when(only.is_none(), |d| {
                    d.child(
                        div()
                            .w(px(name_w))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(face(a, 22., cx))
                            .child(div().min_w_0().truncate().text_size(px(T_M)).child(label)),
                    )
                })
                .child(track),
        );
    }
    // The hours (today) or days under the tracks.
    let buckets = arr(d, "buckets");
    let hours = span / 3600.;
    let every = if hourly {
        if hours <= 6. { 1 } else if hours <= 12. { 2 } else if hours <= 18. { 3 } else { 6 }
    } else if buckets.len() > 10 {
        5
    } else {
        1
    };
    let mut axis = div().relative().h(px(16.)).w(px(track_w)).ml(px(if only.is_some() { 0. } else { name_w + 12. }));
    for (i, b) in buckets.iter().enumerate() {
        let start = secs(b, "start");
        let n = if hourly { i } else { buckets.len() - 1 - i };
        if n % every != 0 || at(start) > 0.95 {
            continue;
        }
        let label = if hourly { hour_mark(root, (start + offset()).rem_euclid(86400) / 3600) } else { day_label(root, local_day(start), every > 1) };
        axis = axis.child(div().absolute().left(px(at(start) * track_w)).top_0().text_size(px(T_S)).text_color(c(MUTED)).child(label));
    }
    rows.child(axis).when(any_drop, |d| {
        d.child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(T_S))
                .text_color(c(MUTED))
                .child(div().w(px(3.)).h(px(12.)).rounded_full().bg(c(BAD)))
                .child(root.tr("stats.kicks")),
        )
    })
}

// ───────────────────────── today as a clock ─────────────────────────
// StatsViz.tsx's Clock, on its own 400×400 dial scaled to the box: midnight at the top, clockwise.

const DIAL: f32 = 380.;
const C_MID: f32 = 200.;
const R_OUT: f32 = 158.;
const R_IN: f32 = 66.;
const MAX_RINGS: usize = 24;

/// Where on the dial (in its own 400 units) a moment of the day is.
fn dial_point(r: f32, turn: f32) -> (f32, f32) {
    let a = turn * std::f32::consts::TAU;
    (C_MID + r * a.sin(), C_MID - r * a.cos())
}

/// One account's ring: its radius, how strongly it shows, and its stays (from, to as parts of the day, colour, dropped).
struct Ring {
    r: f32,
    alpha: f32,
    stays: Vec<(f32, f32, Hsla, bool)>,
}

/// Today: a ring per account (the most played outside), its stays as arcs in their game's colour, a red notch where one
/// dropped, the hand at now. The middle says today's play time and the windows open now; an account pointed at in the
/// list beside it says its own, and the other rings step back.
fn clock(root: &Root, d: &Value, w: f32, cx: &mut Context<Root>) -> Div {
    let since = secs(d, "since");
    let turn = |t: i64| ((t - since) as f32 / 86400.).clamp(0., 1.);
    let accounts = arr(d, "accounts");
    let all_rows = arr(d, "timeline");
    let shown: Vec<&Value> = all_rows.iter().filter(|r| accounts.iter().any(|a| s(a, "id") == s(r, "id"))).take(MAX_RINGS).collect();
    let n = shown.len().max(1) as f32;
    let band = 16_f32.min((R_OUT - R_IN) / n);
    let stroke = (band - if n > 12. { 1. } else { 2. }).max(2.);
    let now = turn(crate::time::now_secs());
    let active = root.stats.active.clone();
    let icons = d.get("icons");
    let k = DIAL / 400.;

    let mut rings = Vec::new();
    let mut marks: Vec<AnyElement> = Vec::new();
    let mut games: Vec<(String, String)> = Vec::new();
    let mut any_drop = false;
    for (i, row) in shown.iter().enumerate() {
        let r = R_OUT - band * i as f32 - band / 2.;
        let dim = active.as_deref().is_some_and(|a| a != s(row, "id"));
        let mut stays = Vec::new();
        for seg in arr(row, "segments") {
            let key = s(seg, "game");
            let (a0, a1) = (turn(secs(seg, "from")), turn(secs(seg, "to")));
            let dropped = !s(seg, "reason").is_empty();
            any_drop |= dropped;
            if !games.iter().any(|(g, _)| g == key) {
                games.push((key.to_string(), s(seg, "name").to_string()));
            }
            stays.push((a0, a1, game_color(key), dropped));
            // Its game's icon at the stay's start, where the arc is long and wide enough: a little over the arc's width
            // (at the arc's own width it came out ~12 px, unreadable, review 6.1), on a dark hairline so it parts from
            // the arc under it.
            let size = (stroke + 6.).min(20.);
            if stroke >= 8. && a1 - a0 > 0.05 {
                if let Some(p) = icons.and_then(|m| m.get(key)).and_then(Value::as_str).and_then(|u| images::pic(u, cx)) {
                    let (x, y) = dial_point(r, a0 + 0.025_f32.min((a1 - a0) / 2.));
                    marks.push(
                        img(p)
                            .absolute()
                            .left(px((x - size / 2.) * k))
                            .top(px((y - size / 2.) * k))
                            .size(px(size * k))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(ca(0x000000, 0.55))
                            .when(dim, |i| i.opacity(0.25))
                            .into_any_element(),
                    );
                }
            }
        }
        rings.push(Ring { r, alpha: if dim { 0.25 } else { 1. }, stays });
    }
    let inner_edge = R_OUT - band * shown.len() as f32 - 6.;

    let dial_paint = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
            let k = f32::from(bounds.size.width) / 400.;
            let at = |(x, y): (f32, f32)| point(px(ox + x * k), px(oy + y * k));
            let line = |window: &mut Window, from: (f32, f32), to: (f32, f32), width: f32, color: Hsla| {
                let mut pb = PathBuilder::stroke(px(width * k));
                pb.move_to(at(from));
                pb.line_to(at(to));
                if let Ok(path) = pb.build() {
                    window.paint_path(path, color);
                }
            };
            // An arc from one part of the day to another (a whole ring as two halves).
            let arc = |window: &mut Window, r: f32, from: f32, to: f32, width: f32, color: Hsla| {
                let to = to.max(from + 0.0015).min(from + 0.9995);
                let mut pb = PathBuilder::stroke(px(width * k));
                pb.move_to(at(dial_point(r, from)));
                if to - from > 0.5 {
                    let mid = (from + to) / 2.;
                    pb.arc_to(point(px(r * k), px(r * k)), px(0.), false, true, at(dial_point(r, mid)));
                }
                pb.arc_to(point(px(r * k), px(r * k)), px(0.), false, true, at(dial_point(r, to)));
                if let Ok(path) = pb.build() {
                    window.paint_path(path, color);
                }
            };
            for h in 0..24 {
                let t = h as f32 / 24.;
                let big = h % 6 == 0;
                line(window, dial_point(R_OUT + 8., t), dial_point(R_OUT + if big { 17. } else { 13. }, t), 1.5, if big { c(MUTED) } else { ca(WHITE, 0.12) });
            }
            for ring in &rings {
                let well = ca(0x000000, 0.28 * ring.alpha);
                arc(window, ring.r, 0., 0.5, stroke, well);
                arc(window, ring.r, 0.5, 1., stroke, well);
                for (a0, a1, color, _) in &ring.stays {
                    arc(window, ring.r, *a0, *a1, stroke, color.opacity(ring.alpha));
                }
                for (_, a1, _, dropped) in &ring.stays {
                    if *dropped {
                        let (r0, r1) = (ring.r - stroke / 2. - 2., ring.r + stroke / 2. + 2.);
                        line(window, dial_point(r0, *a1), dial_point(r1, *a1), 2.5, c(BAD).opacity(ring.alpha));
                    }
                }
            }
            // The hand crosses the rings only, never the words in the middle; its dot sits at the hour ring.
            line(window, dial_point(inner_edge, now), dial_point(R_OUT + 10., now), 2., c(ACCENT));
            let (px_, py_) = dial_point(R_OUT + 10., now);
            let pin = Bounds::new(point(px(ox + (px_ - 3.5) * k), px(oy + (py_ - 3.5) * k)), size(px(7. * k), px(7. * k)));
            window.paint_quad(fill(pin, c(ACCENT)).corner_radii(px(3.5 * k)));
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();

    // The hours at the quarters, further out at the sides so they clear their ticks.
    let mut dial = div().relative().flex_none().size(px(DIAL)).child(dial_paint).children(marks);
    for h in [0, 6, 12, 18] {
        let (x, y) = dial_point(R_OUT + if h % 12 != 0 { 40. } else { 30. }, h as f32 / 24.);
        dial = dial.child(
            div()
                .absolute()
                .left(px(x * k - 32.))
                .top(px(y * k - 8.))
                .w(px(64.))
                .h(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(T_S))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(MUTED))
                .font_features(tnum())
                .child(hour_mark(root, h)),
        );
    }
    let focus = active.as_deref().and_then(|id| accounts.iter().find(|a| s(a, "id") == id));
    let windows = root.metrics.get("robloxCount").and_then(Value::as_i64).unwrap_or(0);
    let middle_small = match focus {
        Some(a) => name_of(root, a),
        None if windows > 0 => root.i18n.tn("stats.runningNow", windows, &[]),
        None => root.tr("alog.today"),
    };
    dial = dial.child(
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .child(div().text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).child(played(root, focus.map_or(int(d, "totalSeconds"), |a| int(a, "seconds")))))
            // As wide as the space inside the rings (the inner ring's width left "now" alone on a second line, review
            // 6.1); wrapped onto a second line rather than cut only when that is still too narrow.
            .child(div().max_w(px((inner_edge.max(R_IN) * 2. - 20.) * k)).text_center().line_height(px(16.)).line_clamp(2).text_size(px(T_S)).text_color(c(MUTED)).child(middle_small)),
    );

    // Beside it: the accounts with their drops and time, the games' key under them.
    let mut list = div().flex().flex_col().gap(px(2.));
    for (i, row) in shown.iter().enumerate() {
        let id = s(row, "id").to_string();
        let Some(a) = accounts.iter().find(|x| s(x, "id") == id) else { continue };
        let kicks = int(a, "kicks");
        let on = active.as_deref() == Some(id.as_str());
        let kicks_tip: SharedString = root.i18n.tn("stats.kicksN", kicks, &[]).into();
        list = list.child(
            div()
                .id(("clk-acc", i))
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(8.))
                .py(px(5.))
                .rounded(px(R_S))
                .when(on, |d| d.bg(ca(WHITE, 0.06)))
                .on_hover(cx.listener(move |r, over: &bool, _, cx| {
                    let next = if *over { Some(id.clone()) } else { None };
                    if *over || r.stats.active.as_deref() == Some(id.as_str()) {
                        r.stats.active = next;
                        cx.notify();
                    }
                }))
                .child(face(a, 34., cx))
                .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(name_of(root, a)))
                .when(kicks > 0, |d| {
                    d.child(
                        div()
                            .id(("clk-kicks", i))
                            .flex_none()
                            .min_w(px(18.))
                            .h(px(18.))
                            .px(px(5.))
                            .rounded_full()
                            .bg(ca(BAD, 0.18))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(T_XS))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(BAD))
                            .child(kicks.to_string())
                            .tooltip(move |_, cx| cx.new(|_| Tip(kicks_tip.clone())).into()),
                    )
                })
                .child(div().flex_none().text_size(px(T_M)).text_color(c(TEXT_2)).font_features(tnum()).child(played(root, int(a, "seconds")))),
        );
    }
    let mut key = div().flex().flex_wrap().gap_x(px(14.)).gap_y(px(6.)).text_size(px(T_S)).text_color(c(TEXT_2));
    for (g, name) in &games {
        let name = if name.is_empty() { "…".to_string() } else { name.clone() };
        key = key.child(div().flex().items_center().gap(px(6.)).child(div().size(px(10.)).rounded(px(3.)).bg(game_color(g))).child(name));
    }
    if any_drop {
        key = key.child(div().flex().items_center().gap(px(6.)).child(div().w(px(3.)).h(px(10.)).rounded(px(1.)).bg(c(BAD))).child(root.tr("stats.kicks")));
    }
    let more = all_rows.len().saturating_sub(shown.len());
    let wide = w >= DIAL + 300.;
    // Beside the dial it runs the dial's height: the accounts from its top, the games' key at its foot (centred, a
    // short list left the card's lower right empty, review 6.1).
    let side = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(14.))
        .when(wide, |d| d.self_stretch().justify_between().py(px(12.)))
        .child(div().flex().flex_col().gap(px(14.)).child(list).when(more > 0, |d| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tn("stats.moreRings", more as i64, &[])))))
        .child(key);
    div()
        .flex()
        .gap(px(32.))
        .map(|d| if wide { d.items_center() } else { d.flex_col().items_center() })
        .child(dial)
        .child(if wide { side.flex_1().max_w(px(560.)) } else { side.w_full() })
}

/// A stay's ends: the time today; its weekday and time over 7 and 30 days.
fn stay_clock(root: &Root, hourly: bool, at: i64) -> String {
    if hourly { clock_at(root, at) } else { format!("{} {}", day_label(root, local_day(at), false), clock_at(root, at)) }
}

/// 7 and 30 days: a bar a day on one baseline, today brighter, the day's average beside the title. 7 days: every bar's
/// time on top; 30 days: today's and the longest day's, a date under every 5th day counted back from today.
fn days(root: &Root, d: &Value, w: f32) -> Div {
    let buckets = arr(d, "buckets");
    let n = buckets.len();
    if n == 0 {
        return div();
    }
    let last = n - 1;
    let sec: Vec<i64> = buckets.iter().map(|b| int(b, "seconds")).collect();
    let many = n > 10;
    let every = if many { 5 } else { 1 };
    let top = (0..n).fold(0, |best, i| if sec[i] > sec[best] { i } else { best });
    let top_shown = many && top != last && last - top >= 3 && sec[top] > 0;
    let first = if d.get("previous").is_some_and(Value::is_null) { sec.iter().position(|x| *x > 0) } else { Some(0) };
    let counted = match first {
        Some(f) => n - f,
        None => n,
    };
    let avg = int(d, "totalSeconds") / counted.max(1) as i64;
    const BAR_H: f32 = 132.;
    struct Day {
        band: SharedString,
        seconds: i64,
        today: bool,
        time: SharedString,
        full: SharedString,
        played: SharedString,
    }
    let now_day = local_day(crate::time::now_secs());
    let rows: Vec<Day> = buckets
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let today = i == last;
            let v = sec[i];
            let show_time = !many || today || (top_shown && i == top);
            let day = local_day(secs(b, "start"));
            Day {
                band: if today { root.tr("alog.today") } else { day_label(root, day, many) }.into(),
                seconds: v,
                today,
                time: if show_time { if v > 0 { bar_time(v) } else { "0".into() } } else { String::new() }.into(),
                full: day_heading(root, day, now_day).into(),
                played: played(root, v).into(),
            }
        })
        .collect();
    // The bars drawn here, not by the chart kit: its words are a fixed 10 px, too small for the day names and times,
    // and today couldn't stand out. Today's bar is the bright one, its time and name in bold; the others' quiet. The
    // axis names every `every`-th day, counted from the first; a bar's whole day and time are under the mouse.
    let most = sec.iter().copied().max().unwrap_or(0).max(1) as f32;
    const LABEL_H: f32 = 20.;
    let mut bars = div().h(px(BAR_H + LABEL_H + 26.)).flex().items_end().gap(px(if many { 4. } else { 12. }));
    for (i, day) in rows.into_iter().enumerate() {
        let part = (day.seconds as f32 / most).clamp(0., 1.) * BAR_H / (BAR_H + LABEL_H);
        let named = !many || day.today || i % every == 0;
        let tip: SharedString = format!("{} · {}", day.full, day.played).into();
        let bar_el = div()
            .w_full()
            .max_w(px(56.))
            .rounded_t(px(4.))
            .bg(if day.today { c(ACCENT) } else { ca(ACCENT, 0.45) })
            .when(day.seconds > 0, |d| d.min_h(px(3.)));
        bars = bars.child(
            div()
                .id(("st-day", i))
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .w_full()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_end()
                        .gap(px(4.))
                        .when(!day.time.is_empty(), |d| {
                            d.child(
                                div()
                                    .flex_none()
                                    .text_size(px(T_S))
                                    .font_features(tnum())
                                    .when(day.today, |d| d.font_weight(FontWeight::SEMIBOLD))
                                    .text_color(if day.today { c(TEXT) } else { c(MUTED) })
                                    .child(day.time.clone()),
                            )
                        })
                        .child(grow_in("st-days", i, part, bar_el)),
                )
                .child(
                    div()
                        .h(px(18.))
                        .flex_none()
                        .text_size(px(T_S))
                        .whitespace_nowrap()
                        .when(day.today, |d| d.font_weight(FontWeight::SEMIBOLD))
                        .text_color(if day.today { c(TEXT) } else { c(MUTED) })
                        .when(named, |d| d.child(day.band.clone())),
                )
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
        );
    }
    let _ = w;
    card()
        .child(
            card_title(root.tr("stats.byDay"))
                .when(avg >= 60, |t| t.child(div().flex_1()).child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("stats.avgDay", &[("t", &played(root, avg))])))),
        )
        .child(bars)
}

/// 7 and 30 days by hour: a row a day (today last), a cell an hour, the brighter the more windows ran in it.
fn heat(root: &Root, d: &Value, w: f32) -> Div {
    let heat = arr(d, "heat");
    let vi = root.i18n.vietnamese;
    let most = heat.iter().flat_map(|r| r.as_array().into_iter().flatten()).filter_map(Value::as_f64).fold(0.01_f64, f64::max);
    let first_day = local_day(secs(d, "since"));
    let last = heat.len().saturating_sub(1);
    let many = heat.len() > 10;
    let level = |v: f64| if v <= 0. { 0 } else { ((v / most * 4.).ceil() as usize).min(4) };
    // Five steps of the heat colour (app.css .heat): an empty hour the well, the others the heat faded into it, so an hour
    // played still stands out from an empty one.
    let tone = |l: usize| if l == 0 { ca(0x000000, 0.28) } else { ca(HEAT, [0., 0.55, 0.7, 0.85, 1.][l]) };
    let label_w = 58.;
    let cell_h = if many { 9. } else { 16. };
    let cells_w = (w - 42. - label_w - 8.).max(240.);
    let mut grid = div().flex().flex_col().gap(px(2.));
    let mut hours = div().relative().h(px(20.)).ml(px(label_w + 8.)).w(px(cells_w));
    for k in 0..8 {
        hours = hours.child(div().absolute().left(px(k as f32 * 3. / 24. * cells_w)).text_size(px(T_S)).text_color(c(MUTED)).child(hour_mark(root, k * 3)));
    }
    grid = grid.child(hours);
    for (i, row) in heat.iter().enumerate() {
        let day = first_day + i as i64;
        let day_text = if i == last { root.tr("alog.today") } else { day_label(root, day, many) };
        let shown = !many || (last - i) % 5 == 0;
        let mut line = div().h(px(cell_h)).flex().items_center().gap(px(8.)).child(
            div()
                .w(px(label_w))
                .flex_none()
                .whitespace_nowrap()
                .text_size(px(T_S))
                .text_color(c(if i == last { TEXT } else { MUTED }))
                .child(if shown { day_text.clone() } else { String::new() }),
        );
        let mut cells = div().flex().gap(px(2.)).w(px(cells_w));
        for (h, v) in row.as_array().into_iter().flatten().enumerate() {
            let v = v.as_f64().unwrap_or(0.);
            let tip: SharedString =
                root.i18n.tv("stats.heatTip", &[("d", &day_text), ("h", &hour_mark(root, h as i64)), ("n", &format!("{}", (v * 10.).round() / 10.).replace('.', if vi { "," } else { "." }))]).into();
            cells = cells.child(
                div()
                    .id(SharedString::from(format!("ht-{i}-{h}")))
                    .flex_1()
                    .h(px(cell_h))
                    .rounded(px(2.))
                    .bg(tone(level(v)))
                    .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
            );
        }
        line = line.child(cells);
        grid = grid.child(line);
    }
    let mut key = div().flex().items_center().gap(px(3.)).text_size(px(T_S)).text_color(c(MUTED)).child(div().mr(px(3.)).child(root.tr("stats.heatLess")));
    for l in 0..5 {
        key = key.child(div().size(px(10.)).rounded(px(2.)).bg(tone(l)));
    }
    key = key.child(div().ml(px(3.)).child(root.tr("stats.heatMore")));
    card().child(card_title(root.tr("stats.byHour")).child(div().flex_1()).child(key)).child(grid)
}

/// The time disconnects cost, by why they happened.
fn drops(root: &Root, d: &Value, cx: &mut Context<Root>) -> Option<Div> {
    let losses = arr(d, "losses");
    if losses.is_empty() {
        return None;
    }
    let vi = root.i18n.vietnamese;
    let total: i64 = losses.iter().map(|l| int(l, "seconds")).sum();
    let accounts = arr(d, "accounts");
    let mut list = div().flex().flex_col();
    for (i, l) in losses.iter().enumerate() {
        let names: Vec<String> = arr(l, "accounts")
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|id| accounts.iter().find(|a| s(a, "id") == id))
            .map(|a| name_of(root, a))
            .collect();
        let times = int(l, "times");
        // What would have prevented it, when it is off: Anti-AFK for idling, Auto-rejoin for the rest (StatsView.tsx).
        let fix = match s(l, "kind") {
            "idle" if !root.state.pointer("/settings/antiAfk").and_then(Value::as_bool).unwrap_or(false) => Some("stats.do.antiAfk"),
            "other" if arr(l, "accounts").iter().filter_map(Value::as_str).any(|id| root.accounts().iter().any(|a| s(a, "id") == id && a.get("autoRejoin").and_then(Value::as_bool) == Some(false))) => Some("stats.do.rejoin"),
            _ => None,
        };
        let detail = [root.i18n.tn("stats.timesN", times, &[("n", &num(times, vi))]), names.join(", ")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(12.))
                .when(i > 0, |d| d.border_t_1().border_color(c(LINE)))
                .child(div().size(px(8.)).flex_none().rounded_full().bg(c(if s(l, "kind") == "idle" { WARN } else { BAD })))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.tr(&format!("stats.why.{}", s(l, "kind")))))
                        .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(detail)),
                )
                .child(div().flex_none().text_size(px(T_M)).text_color(c(TEXT_2)).child(root.i18n.tv("stats.lost", &[("t", &played(root, int(l, "seconds")))])))
                .when_some(fix, |d, key| {
                    d.child(Button::new(("sd-fix", i), root.tr(key)).small().on_click(cx.listener(|r, _, _, cx| {
                        r.settings.part = crate::pages::settings::Part::Perf;
                        r.go(crate::pages::Page::Settings);
                        cx.notify();
                    })))
                }),
        );
    }
    let lost_tip: SharedString = root.tr("stats.lostTip").into();
    Some(
        card()
            .child(
                card_title(root.tr("stats.drops")).child(div().flex_1()).child(
                    div()
                        .id("sd-lost")
                        .text_size(px(T_H))
                        .font_weight(FontWeight::SEMIBOLD)
                        .font_features(tnum())
                        .text_color(c(BAD))
                        .child(root.i18n.tv("stats.lost", &[("t", &played(root, total))]))
                        .tooltip(move |_, cx| cx.new(|_| Tip(lost_tip.clone())).into()),
                ),
            )
            .child(list),
    )
}

/// The top games as rows (a box map left a few games as big empty boxes with a tiny icon): each its icon, name, time
/// and share, and a bar of its share in its colour; `cols` side by side on a wide page. A click counts only that game
/// in every figure on the page; the row again, or the chip by the title, counts all again.
fn game_list(root: &Root, d: &Value, cols: u16, cx: &mut Context<Root>) -> Div {
    let vi = root.i18n.vietnamese;
    let games: Vec<&Value> = arr(d, "games").iter().filter(|g| int(g, "seconds") > 0).collect();
    let total = d.get("gamesSeconds").and_then(Value::as_i64).unwrap_or(int(d, "totalSeconds")).max(1);
    let most = games.iter().map(|g| int(g, "seconds")).max().unwrap_or(1).max(1);
    let picked = root.stats.game.clone();
    let mut list = if cols > 1 { div().grid().grid_cols(cols).gap_x(px(24.)).gap_y(px(4.)) } else { div().flex().flex_col().gap(px(4.)) };
    for (i, g) in games.iter().take(6 * cols as usize).enumerate() {
        let key = s(g, "key").to_string();
        let share = (int(g, "seconds") as f64 / total as f64 * 100.).round() as i64;
        let on = picked.as_deref() == Some(key.as_str());
        let pic = Some(s(g, "iconUrl")).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, 32., cx));
        let next = if on { None } else { Some(key.clone()) };
        let part = int(g, "seconds") as f32 / most as f32;
        list = list.child(
            hot(("gl", i))
                .relative()
                .px(px(8.))
                .py(px(8.))
                .rounded(px(R_M))
                .flex()
                .items_center()
                .gap(px(12.))
                .cursor_pointer()
                .tones(ca(WHITE, 0.), ca(WHITE, 0.05))
                .child(
                    div()
                        .size(px(32.))
                        .flex_none()
                        .rounded(px(R_S))
                        .overflow_hidden()
                        .bg(mix(c(RAISED), game_color(&key), 0.4))
                        .when_some(pic, |d, p| d.child(img(p).size_full().object_fit(ObjectFit::Cover))),
                )
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
                                .items_baseline()
                                .gap(px(10.))
                                .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(s(g, "name").to_string()))
                                .child(div().flex_none().text_size(px(T_S)).text_color(c(TEXT_2)).font_features(tnum()).child(format!("{} · {} %", played(root, int(g, "seconds")), num(share, vi)))),
                        )
                        .child(div().h(px(4.)).rounded_full().bg(c(RAISED_2)).child(grow_bar(i, part, game_color(&key)))),
                )
                .when(on, |d| d.child(div().absolute().inset_0().rounded(px(R_M)).border_2().border_color(c(FACE))))
                .on_click(cx.listener(move |r, _, _, cx| pick_game(r, next.clone(), cx))),
        );
    }
    let chip = picked.as_ref().map(|k| {
        let name = arr(d, "games").iter().find(|g| s(g, "key") == k).map(|g| s(g, "name").to_string()).unwrap_or("…".into());
        Button::new("gl-chip", root.i18n.tv("stats.onlyGame", &[("game", &name)])).icon_end("x").small().tint().on_click(cx.listener(|r, _, _, cx| pick_game(r, None, cx)))
    });
    card().child(section_head(root.tr("stats.topGames"), Some(games.len())).children(chip)).child(list)
}

/// A share bar's fill, easing out to its length.
fn grow_bar(i: usize, part: f32, col: Hsla) -> impl IntoElement {
    div().h_full().rounded_full().bg(col).with_spring(("gl-bar", i), SpringAnimation::new(EASE).to(part.clamp(0., 1.)), |d, v| d.w(relative(v.clamp(0., 1.))))
}

fn account_cards(root: &Root, d: &Value, w: f32, cx: &mut Context<Root>) -> Div {
    let tn = |k: &str, n: i64| root.i18n.tn(k, n, &[]);
    let hourly = d.get("hourly").and_then(Value::as_bool).unwrap_or(false);
    let accounts = arr(d, "accounts");
    let most = accounts.iter().flat_map(|a| arr(a, "buckets").iter().filter_map(Value::as_f64)).fold(1_f64, f64::max);
    let inner = w - 42.;
    // As many a row as fit at 230 px, but no more than there are accounts: a few share the row out (up to 460 px each)
    // instead of leaving its right half empty.
    let per_row = ((inner + 10.) / (230. + 10.)).floor().clamp(1., accounts.len().max(1) as f32);
    let card_w = ((inner - (per_row - 1.) * 10.) / per_row).floor().min(460.);
    let open = root.stats.open.clone();
    let mut grid = div().flex().flex_wrap().gap(px(10.));
    for (i, a) in accounts.iter().enumerate() {
        let id = s(a, "id").to_string();
        let can = !hourly && arr(d, "timeline").iter().any(|r| s(r, "id") == id);
        let on = can && open.as_deref() == Some(id.as_str());
        let label = name_of(root, a);
        let deleted = a.get("deleted").and_then(Value::as_bool).unwrap_or(false);
        let mut bars = div().h(px(34.)).flex().items_end().gap(px(2.));
        for v in arr(a, "buckets").iter().filter_map(Value::as_f64) {
            let h = if v > 0. { (v / most * 34.).max(3.) } else { 0. };
            bars = bars.child(div().flex_1().h(px(h as f32)).rounded_t(px(2.)).bg(c(ACCENT)));
        }
        let top = a.get("topGame").filter(|g| g.is_object());
        let kicks = int(a, "kicks");
        let streak = int(a, "streak");
        let streak_tip: SharedString = root.tr("stats.streakTip").into();
        let body = div()
            .id(("sa", i))
            .w(px(card_w))
            .rounded(px(R_L))
            .bg(if on { ca(WHITE, 0.06) } else { ca(WHITE, 0.045) })
            .border_1()
            .border_color(if on { ca(WHITE, 0.4) } else { c(LINE) })
            .p(px(14.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(face(a, 34., cx))
                    .child(div().min_w_0().flex_1().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(label.clone()))
                    .when(deleted && label != root.tr("stats.deleted"), |d| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("stats.deletedTag"))))
                    .when(can, |d| d.child(icon(if on { "chevronUp" } else { "chevronDown" }, 14., c(MUTED)))),
            )
            .child(div().text_size(px(T_FIG)).font_weight(FontWeight::BOLD).font_features(tnum()).child(played(root, int(a, "seconds"))))
            .child(bars)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .text_size(px(T_S))
                    .min_w_0()
                    .when_some(top, |d, g| {
                        d.child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().size(px(8.)).flex_none().rounded_full().bg(game_color(s(g, "key"))))
                                .child(div().min_w_0().truncate().text_color(c(TEXT_2)).child(s(g, "name").to_string())),
                        )
                    })
                    .when(top.is_none(), |d| d.child(div().flex_1()))
                    .when(kicks > 0, |d| d.child(div().flex_none().text_color(c(BAD)).child(tn("stats.kicksN", kicks)))),
            )
            .when(streak >= 2, |d| {
                d.child(
                    div()
                        .id(("sa-streak", i))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(T_S))
                        .text_color(c(TEXT_2))
                        .child(icon("calendar", 12., c(TEXT_2)))
                        .child(root.i18n.tn("stats.streak", streak, &[]))
                        .tooltip(move |_, cx| cx.new(|_| Tip(streak_tip.clone())).into()),
                )
            });
        let tip: SharedString = root.tr("stats.showPlayed").into();
        grid = grid.child(if can {
            focusable(body)
                .cursor_pointer()
                .hover(|d| d.bg(ca(WHITE, 0.06)))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.stats.open = if r.stats.open.as_deref() == Some(id.as_str()) { None } else { Some(id.clone()) };
                    cx.notify();
                }))
                .into_any_element()
        } else {
            body.into_any_element()
        });
    }
    let below = open.filter(|id| !hourly && arr(d, "timeline").iter().any(|r| s(r, "id") == id)).map(|id| {
        surface().p(px(14.)).child(timeline(root, d, Some(&id), inner - 28., cx))
    });
    card().child(card_title(root.tr("stats.byAccount"))).child(grid).children(below)
}

/// Play time by preset (and what wasn't opened from one), each with its accounts, drops and share.
fn by_preset(root: &Root, d: &Value, cx: &mut Context<Root>) -> Option<Div> {
    let presets = arr(d, "presets");
    if presets.is_empty() {
        return None;
    }
    let vi = root.i18n.vietnamese;
    let total = int(d, "totalSeconds").max(1);
    let mut list = div().flex().flex_col().gap(px(4.));
    for (i, p) in presets.iter().enumerate() {
        let own = s(p, "id") != NO_PRESET;
        let name = if own { s(p, "name").to_string() } else { root.tr("stats.noPreset") };
        let kicks = int(p, "kicks");
        let share = (int(p, "seconds") as f64 / total as f64 * 100.).round() as i64;
        let detail = [
            root.i18n.tn("stats.accountsN", int(p, "accounts"), &[]),
            if kicks > 0 { root.i18n.tn("stats.kicksN", kicks, &[]) } else { String::new() },
            format!("{} %", num(share, vi)),
        ]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        let tip: Option<SharedString> = (!own).then(|| root.tr("stats.noPresetTip").into());
        list = list.child(
            div()
                .id(("sp", i))
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(10.))
                .py(px(8.))
                .rounded(px(R_M))
                .hover(|d| d.bg(ca(WHITE, 0.045)))
                .child(if own { game_pic(s(p, "iconUrl"), 34., "layers", cx) } else { game_pic("", 34., "user", cx) })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(name))
                        .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(detail)),
                )
                .child(div().flex_none().text_size(px(T_M)).font_features(tnum()).text_color(c(TEXT_2)).child(played(root, int(p, "seconds"))))
                .when_some(tip, |d, t| d.tooltip(move |_, cx| cx.new(|_| Tip(t.clone())).into())),
        );
    }
    let only_none = presets.iter().all(|p| s(p, "id") == NO_PRESET);
    Some(
        card()
            .child(card_title(root.tr("stats.byPreset")))
            .child(list)
            .when(only_none, |d| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("stats.presetNote")))),
    )
}

// ───────────────────────── Activity ─────────────────────────

fn activity(root: &Root, cx: &mut Context<Root>) -> Div {
    let st = &root.stats;
    let log = st.log.as_ref();
    let named: Vec<(String, String)> = log.map(|l| arr(l, "accounts").iter().map(|a| (s(a, "id").to_string(), s(a, "label").to_string())).collect()).unwrap_or_default();

    // The account filter: a button with a small list under it (a second click closes it).
    let current = if st.account.is_empty() {
        root.tr("alog.allAccounts")
    } else {
        named.iter().find(|(id, _)| *id == st.account).map(|(_, l)| l.clone()).unwrap_or_else(|| root.tr("alog.allAccounts"))
    };
    let mut picker = div().relative().flex_none().child(
        tool("al-acc", Some("user"), st.menu).child(tool_words(None, current)).child(tool_chev()).on_click(cx.listener(|r, _, _, cx| {
            r.stats.menu = !r.stats.menu;
            cx.notify();
        })),
    );
    {
        let mut menu = div()
            .id("al-acc-menu")
            .occlude()
            .w(px(240.))
            .max_h(px(320.))
            .overflow_y_scroll()
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|r, _, _, cx| {
                r.stats.menu = false;
                cx.notify();
            }));
        // Each account with its face (a deleted one's a plain figure), everyone's with the people mark.
        let mut items: Vec<(String, String, Option<Value>)> = vec![(String::new(), root.tr("alog.allAccounts"), None)];
        items.extend(named.iter().map(|(id, l)| (id.clone(), l.clone(), Some(root.accounts().iter().find(|a| s(a, "id") == id).cloned().unwrap_or_else(|| json!({ "deleted": true }))))));
        for (i, (id, label, who)) in items.into_iter().enumerate() {
            let on = id == st.account;
            let lead = match &who {
                Some(a) => face(a, 22., cx),
                None => div().size(px(22.)).flex_none().rounded_full().bg(c(RAISED_2)).flex().items_center().justify_center().child(icon("users", 12., c(TEXT_2))),
            };
            menu = menu.child(
                menu_row(("al-acc-i", i))
                    .h(px(34.))
                    .gap(px(10.))
                    .text_color(if on { c(TEXT) } else { c(TEXT_2) })
                    .child(lead)
                    .child(div().min_w_0().flex_1().truncate().child(label))
                    .when(on, |d| d.child(icon("check", 13., c(TEXT))))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        r.stats.menu = false;
                        r.stats.account = id.clone();
                        r.stats.limit = PAGE;
                        load_log(r, cx);
                        cx.notify();
                    })),
            );
        }
        picker = picker.child(div().absolute().top(px(34.)).left_0().child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(pop("al-acc-pop", st.menu, menu))).with_priority(1)));
    }

    // The kinds, as the Accounts page's filters (the chosen one the yellow block).
    let mut kinds = div().flex().flex_wrap().items_center().gap(px(4.)).min_w_0().flex_1();
    let mut all: Vec<(String, String)> = vec![(String::new(), root.tr("alog.allKinds"))];
    all.extend(GROUPS.iter().map(|g| (g.to_string(), root.tr(&format!("alog.g.{g}")))));
    for (i, (g, label)) in all.into_iter().enumerate() {
        let on = g == st.group;
        kinds = kinds.child(
            chip(("al-g", i), on)
                .h(px(TOOL_H))
                .child(label)
                .on_click(cx.listener(move |r, _, _, cx| {
                    if r.stats.group != g {
                        r.stats.group = g.clone();
                        r.stats.limit = PAGE;
                        load_log(r, cx);
                        cx.notify();
                    }
                })),
        );
    }
    let total = log.map(|l| int(l, "total")).unwrap_or(0);
    let tools = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(picker)
        .child(tool_rule())
        .child(kinds)
        .child(Button::new("al-export", root.tr("alog.export")).icon("download").small().disabled(total == 0).on_click(cx.listener(|r, _, _, cx| export(r, cx))));

    div().flex().flex_col().gap(px(GRID_GAP)).child(tools).child(card().child(log_list(root, cx)))
}

fn log_list(root: &Root, cx: &mut Context<Root>) -> Div {
    let st = &root.stats;
    let vi = root.i18n.vietnamese;
    let note = |t: String| div().py(px(24.)).text_center().text_size(px(T_M)).text_color(c(MUTED)).child(t);
    let Some(log) = st.log.as_ref() else {
        if st.log_failed {
            return note(root.tr("alog.error")).flex().justify_center().items_center().gap(px(10.)).child(
                Button::new("al-retry", root.tr("srv.retry")).small().on_click(cx.listener(|r, _, _, cx| {
                    r.stats.log_failed = false;
                    load_log(r, cx);
                    cx.notify();
                })),
            );
        }
        return note("…".into());
    };
    let entries = arr(log, "entries");
    if entries.is_empty() {
        let filtered = !st.account.is_empty() || !st.group.is_empty();
        return note(root.tr(if filtered { "alog.emptyFilter" } else { "alog.empty" }));
    }
    let now = crate::time::now_secs();
    let today = local_day(now);
    let mut list = div().flex().flex_col();
    let mut last_day = i64::MIN;
    for (i, e) in entries.iter().enumerate() {
        let at = secs(e, "at");
        let day = local_day(at);
        let day_first = day != last_day;
        if day != last_day {
            last_day = day;
            list = list.child(
                div()
                    .pt(px(if i == 0 { 2. } else { 16. }))
                    .pb(px(6.))
                    .text_size(px(T_S))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(MUTED))
                    .child(day_heading(root, day, today)),
            );
        }
        let tone = match s(e, "tone") {
            "ok" => c(OK),
            "warn" => c(WARN),
            "bad" => c(BAD),
            _ => c(DIM),
        };
        let account = s(e, "account").to_string();
        let acc = Some(s(e, "accountId")).filter(|x| !x.is_empty()).and_then(|id| root.accounts().iter().find(|a| s(a, "id") == id));
        let tip: SharedString = if day == today { format!("{} · {}", crate::time::ago(now - at, &root.i18n), clock_at(root, at)) } else { clock_at(root, at) }.into();
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(8.))
                .min_h(px(36.))
                .when(!day_first, |d| d.border_t_1().border_color(c(LINE)))
                .hover(|d| d.bg(ca(WHITE, 0.045)))
                .child(div().size(px(7.)).flex_none().rounded_full().bg(tone))
                .child(
                    div()
                        .id(("al-t", i))
                        .w(px(if vi { 40. } else { 62. }))
                        .flex_none()
                        .text_size(px(T_S))
                        .font_features(tnum())
                        .text_color(c(MUTED))
                        .child(clock_at(root, at))
                        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
                )
                .child(
                    div()
                        .w(px(140.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .min_w_0()
                        // A line about no account is the app's own (an update, a backup, the vault): it says so, where
                        // a lone dash left the column empty (review 6.2).
                        .when(account.is_empty(), |d| d.text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(MUTED)).child(div().size(px(20.)).flex_none().rounded(px(6.)).bg(ca(WHITE, 0.06)).flex().items_center().justify_center().child(icon("monitor", 12., c(MUTED)))).child("Bloxkeep"))
                        .when(!account.is_empty(), |d| {
                            d.when_some(acc, |d, a| d.child(face(a, 20., cx)))
                                .child(div().min_w_0().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(account.clone()))
                        }),
                )
                .child(div().flex_1().min_w_0().py(px(6.)).text_size(px(T_M)).text_color(c(TEXT_2)).child(s(e, "text").to_string())),
        );
    }
    let total = int(log, "total");
    if total > entries.len() as i64 {
        list = list.child(
            div()
                .pt(px(12.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(Button::new("al-more", root.tr("alog.more")).small().on_click(cx.listener(|r, _, _, cx| {
                    r.stats.limit += PAGE;
                    load_log(r, cx);
                    cx.notify();
                })))
                .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tv("alog.shown", &[("n", &num(entries.len() as i64, vi)), ("total", &num(total, vi))]))),
        );
    }
    list
}
