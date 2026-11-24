// A date box's calendar (TimeField.tsx's Calendar): a month at a time under the box's calendar button, the week
// starting as the interface's language has it (Monday in Vietnamese, Sunday in English), the previous and next month
// at its head, today ringed, the chosen day filled yellow; days already past can't be picked where the box says so.
// No picker of the system's: Bloxkeep draws its own. Its open state (the month shown) is the page's own; a day picked,
// a click outside or its button again closes it.
use gpui_kit::{prelude::*, *};

use crate::{i18n::I18n, kit::*, theme::*};

/// What the calendar asks of its page.
pub enum Act {
    /// The month shown, this many months on (-1 the previous one).
    Shift(i32),
    /// A day picked, as the app writes dates: "2026-11-04".
    Pick(String),
    /// A press outside it.
    Close,
}

const CELL: f32 = 36.;
const GAP: f32 = 2.;
/// time.rs's arithmetic in the calendar's types.
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    crate::time::days_from_civil(y.into(), m.into(), d.into())
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let (y, m, d) = crate::time::civil(days);
    (y as i32, m as u32, d as u32)
}

fn iso(y: i32, m: u32, d: u32) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

/// Today on this PC's clock, as the app writes dates.
pub fn today() -> String {
    let (y, m, d) = civil_from_days((crate::time::now_secs() + crate::time::utc_offset_secs()).div_euclid(86400));
    iso(y, m, d)
}

/// The month a calendar opens on: the chosen day's ("2026-12-24"), else today's.
pub fn start_month(chosen: Option<&str>) -> (i32, u32) {
    let at = |v: &str| {
        let y = v.get(0..4)?.parse::<i32>().ok()?;
        let m = v.get(5..7)?.parse::<u32>().ok().filter(|m| (1..=12).contains(m))?;
        Some((y, m))
    };
    chosen.and_then(at).or_else(|| at(&today())).unwrap_or((2026, 1))
}

/// A month this many months on.
pub fn shift((y, m): (i32, u32), n: i32) -> (i32, u32) {
    let k = y * 12 + m as i32 - 1 + n;
    (k.div_euclid(12), k.rem_euclid(12) as u32 + 1)
}

fn days_in((y, m): (i32, u32)) -> u32 {
    let (ny, nm) = shift((y, m), 1);
    (days_from_civil(ny, nm, 1) - days_from_civil(y, m, 1)) as u32
}

fn id(key: &str, part: &str, n: u64) -> ElementId {
    ElementId::NamedInteger(SharedString::from(format!("{key}-{part}")), n)
}

/// The calendar's panel: `shown` is the month on it, `chosen` the box's day ("2026-12-24"), `past_off` greys the days
/// before today. What it is asked to do goes to `act`.
pub fn panel<V: 'static>(
    key: &'static str,
    i18n: &I18n,
    shown: (i32, u32),
    chosen: Option<&str>,
    past_off: bool,
    cx: &mut Context<V>,
    act: impl Fn(&mut V, Act, &mut Window, &mut Context<V>) + 'static,
) -> Stateful<Div> {
    let act = std::rc::Rc::new(act);
    let (y, m) = shown;
    let today = today();
    // (The week starts on Monday but in English.)
    let first = if i18n.lang == "en" { 0 } else { 1 };
    // (1970-01-01 was a Thursday.)
    let lead = ((days_from_civil(y, m, 1) + 4).rem_euclid(7) as u32 + 7 - first) % 7;
    let count = days_in(shown);

    let title = i18n.tv("tf.monthYear", &[("month", crate::time::month_name(m.into())), ("n", &m.to_string()), ("year", &y.to_string())]);
    let step = |n: i32, icon_name: &'static str, tip: String, cx: &mut Context<V>| {
        let act = act.clone();
        IconButton::new(id(key, "step", (n + 1) as u64), icon_name).size(28.).tip(tip).on_click(cx.listener(move |v, _, w, cx| act(v, Act::Shift(n), w, cx)))
    };
    let head = div()
        .flex()
        .items_center()
        .gap(px(4.))
        .pb(px(6.))
        .child(step(-1, "chevronLeft", i18n.t("tf.prevMonth"), cx))
        .child(div().flex_1().min_w_0().text_center().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title))
        .child(step(1, "chevronRight", i18n.t("tf.nextMonth"), cx));

    let row = || div().flex().gap(px(GAP));
    let blank = || div().size(px(CELL)).flex_none();
    let mut names = row();
    for i in 0..7 {
        names = names.child(div().w(px(CELL)).h(px(24.)).flex_none().flex().items_center().justify_center().text_size(px(T_XS)).font_weight(FontWeight::MEDIUM).text_color(c(MUTED)).child(crate::time::day_short(((i + first) % 7).into())));
    }
    let mut grid = div().flex().flex_col().gap(px(GAP)).child(names);
    let mut week = row();
    for _ in 0..lead {
        week = week.child(blank());
    }
    let past_tip: SharedString = i18n.t("tf.past").into();
    for d in 1..=count {
        let day = iso(y, m, d);
        let on = chosen == Some(day.as_str());
        let is_today = day == today;
        let past = past_off && day < today;
        let cell = if past {
            let tip = past_tip.clone();
            div()
                .id(id(key, "day", u64::from(d)))
                .size(px(CELL))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(R_S))
                .text_size(px(T_M))
                .font_features(tnum())
                .text_color(c(DIM))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .child(d.to_string())
                .into_any_element()
        } else {
            let act = act.clone();
            let h = hot(id(key, "day", u64::from(d)))
                .size(px(CELL))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(R_S))
                .text_size(px(T_M))
                .font_features(tnum())
                .cursor_pointer();
            // The chosen day filled yellow (the app's "picked"), today a faint ring.
            let h = if on {
                h.tones(c(FACE), c(FACE_HI)).text_color(c(INK)).font_weight(FontWeight::SEMIBOLD)
            } else {
                h.tones(ca(WHITE, 0.), ca(WHITE, 0.07)).text_color(if is_today { c(TEXT) } else { c(TEXT_2) }).hover(|s| s.text_color(c(TEXT)))
            };
            h.when(is_today && !on, |s| s.border_color(ca(TEXT, 0.3)))
                .child(d.to_string())
                .on_click(cx.listener(move |v, _, w, cx| act(v, Act::Pick(day.clone()), w, cx)))
                .into_any_element()
        };
        week = week.child(cell);
        if (lead + d) % 7 == 0 {
            grid = grid.child(week);
            week = row();
        }
    }
    if (lead + count) % 7 != 0 {
        grid = grid.child(week);
    }

    let out = act.clone();
    div()
        .id(SharedString::from(format!("{key}-menu")))
        .occlude()
        .p(px(10.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .flex()
        .flex_col()
        .child(head)
        .child(grid)
        .on_mouse_down_out(cx.listener(move |v, _, w, cx| out(v, Act::Close, w, cx)))
}

/// The panel laid over the page under its box, its right edge on the box's (TimeField.tsx's Popup), `top` below the
/// top of the box's wrapper (which is `relative`).
pub fn popover(key: &'static str, top: f32, panel: Stateful<Div>) -> Div {
    popup(PopAt::BelowRight(top), SharedString::from(format!("{key}-pop")), true, panel)
}

#[cfg(test)]
mod tests {
    use super::*;
    // (gpui_kit's glob brings gpui's own `test` macro, which expands into `#[test]` again: Rust's is named.)
    use std::prelude::rust_2024::test;

    #[test]
    fn months_and_days() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(days_from_civil(2026, 11, 13)), (2026, 11, 13));
        assert_eq!(days_in((2024, 2)), 29);
        assert_eq!(days_in((2026, 2)), 28);
        assert_eq!(days_in((2026, 12)), 31);
        assert_eq!(shift((2026, 1), -1), (2025, 12));
        assert_eq!(shift((2026, 12), 1), (2027, 1));
        // 2026-11-01 is a Sunday.
        assert_eq!((days_from_civil(2026, 11, 1) + 4).rem_euclid(7), 0);
        assert_eq!(start_month(Some("2026-12-24")), (2026, 12));
    }
}
