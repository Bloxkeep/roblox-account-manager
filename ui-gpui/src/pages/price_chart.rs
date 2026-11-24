// A resold thing's price history on its Marketplace page (pages/profile.rs item_page), laid out as Rolimons' item
// page has it (the reference the owner's no-self-design rule asks for, looked at 2026-11-21): the figures in tiles
// over the chart (RAP, the last day's price, the change over the span shown, how many sold in it), the span as a row
// of short choices (1M 3M 6M 1Y All), the day's average price as a line with the number sold as bars under it, and
// the day under the mouse named with its price and sales. Roblox's own data (roblox::resale), no sign-in.
use std::cell::Cell;

use gpui_kit::{prelude::*, *};

use crate::{kit::*, roblox::Resale, shell::Root, theme::*};

/// The spans offered, in days (0: all of it), with their words.
const SPANS: [(i64, &str); 5] = [(30, "pc.m1"), (90, "pc.m3"), (180, "pc.m6"), (365, "pc.y1"), (0, "pc.all")];

thread_local! {
    /// The span picked (an index of SPANS; a year by default), kept while the app runs.
    static SPAN: Cell<usize> = const { Cell::new(3) };
    /// Where the chart was drawn last (left, top, width, height) and the day under the mouse (an index of the days shown).
    static AREA: Cell<(f32, f32, f32, f32)> = const { Cell::new((0., 0., 0., 0.)) };
    static HOVER: Cell<Option<usize>> = const { Cell::new(None) };
}

const CHART_H: f32 = 240.;
/// The bars' share of the chart's height, at its foot.
const BARS: f32 = 0.24;
fn short_day(secs: i64) -> String {
    let (y, m, d) = crate::time::civil(secs.div_euclid(86_400));
    let _ = d;
    crate::lang::cur().month_year(m, y)
}

fn long_day(secs: i64) -> String {
    let (y, m, d) = crate::time::civil(secs.div_euclid(86_400));
    let l = crate::lang::cur();
    if l.code == "vi" { format!("{d}/{m}/{y}") } else { l.date_short(d, m, Some(y)) }
}

pub fn view(root: &Root, data: &Resale, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let span = SPAN.with(Cell::get).min(SPANS.len() - 1);
    let last = data.days.last().map(|d| d.0).unwrap_or(0);
    let from = match SPANS[span].0 {
        0 => i64::MIN,
        n => last - n * 86_400,
    };
    let days: Vec<(i64, i64, i64)> = data.days.iter().copied().filter(|d| d.0 >= from).collect();
    let sold: i64 = days.iter().map(|d| d.2).sum();
    let latest = days.last().map(|d| d.1);
    let change = match (days.first(), days.last()) {
        (Some(a), Some(b)) if a.1 > 0 && days.len() > 1 => Some((b.1 - a.1) as f64 / a.1 as f64 * 100.),
        _ => None,
    };

    // The span's choices at the head's right.
    let mut head = crate::kit::slide("pc-span", section_head(root.tr("pc.title"), None));
    for (i, (_, key)) in SPANS.iter().enumerate() {
        head = head.child(chip(("pc-span", i), span == i).child(root.tr(key)).on_click(cx.listener(move |_, _, _, cx| {
            SPAN.with(|s| s.set(i));
            HOVER.with(|h| h.set(None));
            cx.notify();
        })));
    }

    let robux = |n: i64| div().flex().items_center().gap(px(6.)).child(icon("robux", 15., c(TEXT))).child(thousands(n));
    let tile = |key: &str, value: AnyElement| {
        div()
            .flex_1()
            .min_w(px(150.))
            .px(px(14.))
            .py(px(12.))
            .rounded(px(R_L))
            .bg(c(PANEL))
            .border_1()
            .border_color(c(LINE))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr(key)))
            .child(div().text_size(px(T_FIG)).line_height(px(28.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(c(TEXT)).child(value))
    };
    let pct = |p: f64| {
        let t = format!("{}{:.1}%", if p > 0. { "+" } else { "" }, p);
        div().text_color(c(if p > 0.05 { OK } else if p < -0.05 { BAD } else { TEXT })).child(crate::lang::cur().decimal(&t))
    };
    let tiles = div()
        .flex()
        .flex_wrap()
        .gap(px(12.))
        .child(tile("pc.rap", robux(data.rap).into_any_element()))
        .child(tile("pc.latest", latest.map(|p| robux(p).into_any_element()).unwrap_or_else(|| div().child("—").into_any_element())))
        .child(tile("pc.change", change.map(|p| pct(p).into_any_element()).unwrap_or_else(|| div().child("—").into_any_element())))
        .child(tile("pc.sold", div().child(thousands(sold)).into_any_element()));

    if days.len() < 2 {
        return div().flex().flex_col().gap(px(12.)).child(head).child(tiles).child(empty_note(root.tr("pc.few"))).into_any_element();
    }

    let hover = HOVER.with(Cell::get).filter(|i| *i < days.len());
    let (lo, hi) = days.iter().fold((i64::MAX, 0), |(lo, hi), d| (lo.min(d.1), hi.max(d.1)));
    let most_sold = days.iter().map(|d| d.2).max().unwrap_or(0).max(1);
    let pts = days.clone();
    let line = c(OK);
    let bar = ca(TEXT, 0.22);
    let bar_on = ca(TEXT, 0.5);
    let grid_line = ca(WHITE, 0.05);
    let plot = canvas(
        |_, _, _| (),
        move |b, _, window, _| {
            let (x0, y0, w, h) = (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height));
            AREA.with(|a| a.set((x0, y0, w, h)));
            let n = pts.len();
            let step = w / (n.max(2) - 1) as f32;
            let price_h = h * (1. - BARS) - 14.;
            let span = (hi - lo).max(1) as f32;
            let y_of = |p: i64| y0 + 6. + price_h * (1. - (p - lo) as f32 / span);
            let x_of = |i: usize| x0 + i as f32 * step;
            // Four faint rules across the price part.
            for k in 0..4 {
                let y = y0 + 6. + price_h * k as f32 / 3.;
                window.paint_quad(fill(Bounds::new(point(px(x0), px(y)), size(px(w), px(1.))), grid_line));
            }
            // The number sold each day as bars at the foot.
            let bars_top = y0 + h * (1. - BARS);
            let bw = (step * 0.7).clamp(1., 8.);
            for (i, d) in pts.iter().enumerate() {
                if d.2 <= 0 {
                    continue;
                }
                let bh = (h * BARS - 2.) * d.2 as f32 / most_sold as f32;
                let colour = if hover == Some(i) { bar_on } else { bar };
                window.paint_quad(fill(Bounds::new(point(px(x_of(i) - bw / 2.), px(bars_top + h * BARS - bh)), size(px(bw), px(bh.max(1.)))), colour));
            }
            // The price: a soft fill under it, then the line.
            let mut area = PathBuilder::fill();
            let mut poly: Vec<Point<Pixels>> = pts.iter().enumerate().map(|(i, d)| point(px(x_of(i)), px(y_of(d.1)))).collect();
            let foot = y0 + 6. + price_h;
            poly.push(point(px(x_of(n - 1)), px(foot)));
            poly.push(point(px(x0), px(foot)));
            area.add_polygon(&poly, true);
            if let Ok(p) = area.build() {
                window.paint_path(p, line.opacity(0.10));
            }
            let mut pb = PathBuilder::stroke(px(2.));
            for (i, d) in pts.iter().enumerate() {
                let at = point(px(x_of(i)), px(y_of(d.1)));
                if i == 0 { pb.move_to(at) } else { pb.line_to(at) }
            }
            if let Ok(p) = pb.build() {
                window.paint_path(p, line);
            }
            // The day under the mouse: a hairline down the chart and a dot on the line.
            if let Some(i) = hover {
                let x = x_of(i);
                window.paint_quad(fill(Bounds::new(point(px(x), px(y0)), size(px(1.), px(h))), ca(WHITE, 0.25)));
                let y = y_of(pts[i].1);
                window.paint_quad(fill(Bounds::new(point(px(x - 4.), px(y - 4.)), size(px(8.), px(8.))), line).corner_radii(px(4.)).border_widths(px(2.)).border_color(c(PANEL)));
            }
        },
    )
    .size_full();

    // The day under the mouse in a label at the top, on the side away from the mouse.
    let inner = width - 28.;
    let label = hover.map(|i| {
        let d = days[i];
        let x = i as f32 / (days.len() - 1) as f32 * inner;
        let at_left = x > inner / 2.;
        div()
            .absolute()
            .top(px(8.))
            .when(at_left, |e| e.left(px((x - 196.).max(0.))))
            .when(!at_left, |e| e.left(px((x + 12.).min(inner - 184.).max(0.))))
            .w(px(184.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(R_M))
            .bg(c(RAISED_2))
            .border_1()
            .border_color(c(LINE))
            .flex()
            .flex_col()
            .gap(px(3.))
            .text_size(px(T_S))
            .child(div().text_color(c(MUTED)).child(long_day(d.0)))
            .child(div().flex().items_center().gap(px(5.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(c(TEXT)).child(icon("robux", 12., c(TEXT))).child(thousands(d.1)))
            .child(div().text_color(c(TEXT_2)).font_features(tnum()).child(root.i18n.tv("pc.soldDay", &[("n", &thousands(d.2))])))
    });

    // The dates under it: five, spread along.
    let ticks: Vec<AnyElement> = (0..5)
        .map(|k| {
            let i = (k * (days.len() - 1)) / 4;
            div().text_size(px(T_XS)).text_color(c(MUTED)).font_features(tnum()).child(short_day(days[i].0)).into_any_element()
        })
        .collect();
    let n_days = days.len();
    let chart = div()
        .id("pc-chart")
        .relative()
        .w_full()
        .h(px(CHART_H))
        .child(plot)
        .children(label)
        // The highest and lowest price of the span, quietly in the corners.
        .child(div().absolute().top(px(0.)).right(px(0.)).text_size(px(T_XS)).text_color(c(MUTED)).font_features(tnum()).child(thousands(hi)))
        .child(div().absolute().top(px(CHART_H * (1. - BARS) - 26.)).right(px(0.)).text_size(px(T_XS)).text_color(c(MUTED)).font_features(tnum()).child(thousands(lo)))
        .on_mouse_move(cx.listener(move |_, e: &MouseMoveEvent, _, cx| {
            let (x0, _, w, _) = AREA.with(Cell::get);
            if w <= 0. {
                return;
            }
            let t = ((f32::from(e.position.x) - x0) / w).clamp(0., 1.);
            let i = (t * (n_days - 1) as f32).round() as usize;
            if HOVER.with(Cell::get) != Some(i) {
                HOVER.with(|h| h.set(Some(i)));
                cx.notify();
            }
        }))
        .on_hover(cx.listener(|_, over: &bool, _, cx| {
            if !*over {
                HOVER.with(|h| h.set(None));
                cx.notify();
            }
        }));
    // The chart's box is solid: over a theme's picture (Sunset's sun and hills) the line and bars were hard to read
    // through a see-through ground (the owner, 2026-11-21).
    let panel = div()
        .w_full()
        .p(px(14.))
        .rounded(px(R_L))
        .bg(crate::theme::solid_panel())
        .border_1()
        .border_color(c(LINE))
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(chart)
        .child(div().flex().justify_between().children(ticks))
        .child(
            div()
                .flex()
                .justify_center()
                .gap(px(18.))
                .text_size(px(T_S))
                .text_color(c(TEXT_2))
                .child(div().flex().items_center().gap(px(6.)).child(div().w(px(14.)).h(px(2.)).bg(line)).child(root.tr("pc.avg")))
                .child(div().flex().items_center().gap(px(6.)).child(div().size(px(8.)).rounded(px(2.)).bg(bar_on)).child(root.tr("pc.volume"))),
        );
    div().flex().flex_col().gap(px(12.)).child(head).child(tiles).child(panel).into_any_element()
}
