// Auto: Presets and Macros on one page, a switch between them (the old Presets and Macros tabs, PresetsView.tsx and
// MacrosView.tsx). A preset row: its game's icon, name, game, what it sets, when it runs next and last, its accounts,
// the schedule switch, Run or Close, Duplicate, Delete (a second click to be sure) and its place in the order. Over
// the list, today's scheduled runs on one ruler. A macro row: what it is doing on each account (or when it runs and on
// whom), its steps as small chips, its panel shown or not, Run or Stop, Duplicate, Export, pause or resume (one that runs
// by itself), Delete and its place. New preset and a row's Edit open the preset form (preset_form.rs) here; New macro
// and a macro's Edit open the macro form (macro_form.rs). The
// commands are the app's: runPreset, closePreset, setPresetSchedule, duplicatePreset, deletePreset, setPresetOrder,
// presetIcons, runMacro, stopMacros, showPanel, hidePanel, duplicateMacro, exportMacro, saveMacro, moveMacro, deleteMacro.
use std::collections::{HashMap, HashSet};

use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};
use crate::val::s;
use crate::val::int;
use crate::time::{civil, day_short, dm, hm, hm_text, weekday};
use crate::pages::macro_form::{live, step_icon};

use crate::{
    images,
    kit::*,

    shell::Root,
    theme::*,
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Presets,
    Macros,
}

#[derive(Default)]
pub struct Data {
    pub tab: Tab,
    /// The row whose Delete was clicked once: its button asks to be clicked again.
    pub confirm: Option<String>,
    /// The preset form, open in place of the list.
    pub form: Option<crate::pages::preset_form::Form>,
    /// The macro form, open in place of the list.
    pub macro_form: Option<crate::pages::macro_form::Form>,
    /// When the presets' game icons were last asked for (asked again 5 minutes on while one is still missing).
    asked_icons: Option<std::time::Instant>,
    /// The macro whose "…" menu is open, and the one a click outside just closed (so its own button doesn't reopen it).
    menu: Option<String>,
    menu_closed: Option<(String, std::time::Instant)>,
    /// A row dragged by its grip: the list's order as it is now, and the row moving through it (PresetsView.tsx's and
    /// MacrosView.tsx's drag to reorder); let go, the order is saved.
    drag: Option<Drag>,
    pub scroll: ScrollHandle,
}

struct Drag {
    macros: bool,
    /// The section it is in (a row moves within its own).
    group: u8,
    id: String,
    order: Vec<String>,
}

/// The web page's accent on its default theme: today's ruler, where the day is now.
const ACCENT: u32 = 0x22c55e;

const AVATARS: usize = 4;
/// The page's sides (the Accounts page's).
const SIDE: f32 = PAGE_X;
const SHAPE_MAX: usize = 14;

/// The app's presets or macros, as its state holds them (borrowed: the page is drawn every frame).
fn list<'a>(root: &'a Root, k: &str) -> &'a [Value] {
    root.state.pointer(&format!("/settings/{k}")).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// The Presets tab's search box is on the page (Ctrl+F goes to it; else to the accounts' search, as app.js).
pub fn preset_search_shown(r: &Root) -> bool {
    r.auto.form.is_none() && r.auto.macro_form.is_none() && r.auto.tab == Tab::Presets && !list(r, "presets").is_empty()
}

// ───────────────────────── times ─────────────────────────

/// Today on the PC's clock: days since 1970 and minutes after midnight.
fn local_now() -> (i64, i64) {
    let t = crate::time::now_secs() + crate::time::utc_offset_secs();
    (t.div_euclid(86400), t.rem_euclid(86400) / 60)
}

/// "T2–T6 20:00", "Mỗi ngày 20:00", "T4, 24/12 07:30" (a one-off), as app.js's scheduleText.
pub(crate) fn schedule_text(root: &Root, sc: &Value) -> String {
    let time = hm_text(s(sc, "time"));
    let date = s(sc, "date");
    if date.len() == 10 {
        let n = |a: usize, b: usize| date.get(a..b).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
        let (y, m, d) = (n(0, 4), n(5, 7), n(8, 10));
        let days = crate::time::iso_secs(&format!("{y:04}-{m:02}-{d:02}T00:00:00Z")).unwrap_or(0) / 86400;
        return format!("{}, {} {time}", day_short(weekday(days)), dm(d, m));
    }
    let picked: Vec<i64> = sc.get("days").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
    let order = [1, 2, 3, 4, 5, 6, 0];
    let days: Vec<i64> = order.iter().copied().filter(|d| picked.contains(d)).collect();
    let pos = |d: i64| order.iter().position(|x| *x == d).unwrap_or(0);
    let in_row = days.len() >= 3 && pos(*days.last().unwrap()) - pos(days[0]) == days.len() - 1;
    let text = if days.len() == 7 {
        root.tr("pv.everyDay")
    } else if in_row {
        format!("{}–{}", day_short(days[0]), day_short(*days.last().unwrap()))
    } else {
        days.iter().map(|d| day_short(*d)).collect::<Vec<_>>().join(", ")
    };
    format!("{text} {time}").trim().to_string()
}

/// "Lần tới: Hôm nay 20:00", as app.js's nextRunText.
pub(crate) fn next_text(root: &Root, p: &Value) -> Option<String> {
    let at = crate::time::iso_secs(s(p, "nextRun"))? + crate::time::utc_offset_secs();
    let (today, _) = local_now();
    let day = at.div_euclid(86400);
    let time = hm(at.rem_euclid(86400) / 3600, at.rem_euclid(3600) / 60);
    let when = if day == today {
        root.tr("ps.today")
    } else if day == today + 1 {
        root.tr("ps.tomorrow")
    } else {
        let (_, m, d) = civil(day);
        format!("{}, {}", day_short(weekday(day)), dm(d, m))
    };
    Some(root.i18n.tv("ps.next", &[("when", &format!("{when} {time}"))]))
}

/// The minute after midnight a preset runs today, if it does (its days, or a one-off's date).
fn run_today(p: &Value, today: i64) -> Option<i64> {
    let sc = p.get("schedule").filter(|v| v.is_object())?;
    if !sc.get("enabled").and_then(Value::as_bool).unwrap_or(false) {
        return None;
    }
    let (h, m) = s(sc, "time").split_once(':')?;
    let (h, m) = (h.trim().parse::<i64>().ok()?, m.trim().parse::<i64>().ok()?);
    let date = s(sc, "date");
    let runs = if date.len() == 10 {
        let (y, mo, d) = civil(today);
        date == format!("{y:04}-{mo:02}-{d:02}")
    } else {
        sc.get("days").and_then(Value::as_array).is_some_and(|l| l.iter().any(|d| d.as_i64() == Some(weekday(today))))
    };
    runs.then_some(h * 60 + m)
}

// ───────────────────────── the page ─────────────────────────

pub fn auto(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    if root.auto.macro_form.is_some() {
        return crate::pages::macro_form::form(root, width, cx);
    }
    if root.auto.form.is_some() {
        return crate::pages::preset_form::form(root, width, cx).into_any_element();
    }
    let presets = list(root, "presets");
    let macros = list(root, "macros");
    let tab = root.auto.tab;
    let narrow = width < 900.;
    let wide = width >= 1300.;
    let runs: Vec<Value> = root
        .state
        .get("macroRuns")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter(|r| !r.get("trial").and_then(Value::as_bool).unwrap_or(false)).cloned().collect())
        .unwrap_or_default();
    let any_live = runs.iter().any(live) || !root.state.get("macroRecording").is_none_or(Value::is_null);

    // The place at a glance, in muted words under the title: how many, what is scheduled or running, and when the
    // next scheduled run is.
    let summary: Vec<(String, Option<Hsla>)> = match tab {
        Tab::Presets => {
            let n = presets.len() as i64;
            let scheduled = presets.iter().filter(|p| p.pointer("/schedule/enabled").and_then(Value::as_bool).unwrap_or(false)).count();
            let open: i64 = presets.iter().map(|p| int(p, "running")).sum();
            let mut v = vec![(if n == 1 { root.tr("pv.count1") } else { root.i18n.tn("pv.countN", n, &[]) }, None)];
            if n >= 50 {
                v.push((root.tr("pv.atMax"), Some(c(WARN))));
            }
            if scheduled > 0 {
                v.push((root.i18n.tv("pv.scheduled", &[("n", &scheduled.to_string())]), None));
            }
            if open > 0 {
                v.push((root.i18n.tn("pv.running", open, &[]), Some(c(OK))));
            }
            // The soonest next run of a repeating schedule.
            let next = presets
                .iter()
                .filter(|p| p.pointer("/schedule/enabled").and_then(Value::as_bool).unwrap_or(false))
                .filter_map(|p| crate::time::iso_secs(s(p, "nextRun")).map(|t| (t, p)))
                .min_by_key(|(t, _)| *t)
                .and_then(|(_, p)| next_text(root, p));
            if let Some(t) = next {
                v.push((t, None));
            }
            if n == 0 { Vec::new() } else { v }
        }
        Tab::Macros => {
            let n = macros.len();
            let on = runs.iter().filter(|r| live(r)).map(|r| s(r, "macroId")).collect::<HashSet<_>>().len();
            let mut v = vec![(if n == 1 { root.tr("macro.count1") } else { root.i18n.tv("macro.countN", &[("n", &n.to_string())]) }, None)];
            if on > 0 {
                v.push((root.i18n.tv("macro.runningN", &[("n", &on.to_string())]), Some(c(OK))));
            }
            if n == 0 { Vec::new() } else { v }
        }
    };

    // Presets | Macros: the page's two lists, as the Accounts page's filters (the chosen one the yellow block).
    let switch = crate::kit::slide("auto-tabs", div().flex().items_center().gap(px(4.)).children([(Tab::Presets, root.tr("ps.listTitle"), presets.len(), "layers"), (Tab::Macros, root.tr("macro.title"), macros.len(), "keyboard")].into_iter().map(
        |(t, text, n, mark)| {
            let on = tab == t;
            chip(SharedString::from(format!("auto-tab-{text}")), on)
                .h(px(TOOL_H))
                .child(icon(mark, 15., if on { crate::theme::on_text() } else { c(MUTED) }))
                .child(text)
                .child(chip_count(n, on))
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.auto.tab = t;
                    r.auto.confirm = None;
                    r.auto.menu = None;
                    cx.notify();
                }))
        },
    )));
    // Macros are still in Beta (MacrosView.tsx's title pill).
    let beta = (tab == Tab::Macros).then(|| {
        let tip: SharedString = format!("{} {}", root.tr("macro.betaTip"), root.tr("macro.honest")).into();
        div()
            .id("mc-beta")
            .flex_none()
            .h(px(22.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(px(R_S - 2.))
            .bg(ca(WARN, 0.16))
            .text_size(px(T_XS))
            .font_weight(FontWeight::BOLD)
            .text_color(c(WARN))
            .child(root.tr("macro.beta"))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
    });

    let actions = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(12.))
        .when(tab == Tab::Presets, |d| {
            let full = presets.len() >= 50;
            d.when(!presets.is_empty(), |d| d.child(div().w(px(if narrow { 250. } else { 280. })).min_w(px(140.)).flex_shrink(1.).child(field(&root.preset_search, Some("search")))))
                .when(!presets.is_empty(), |d| d.child(Button::new("pv-new", root.tr("ps.new")).icon("plus").primary().on_click(cx.listener(move |r, _, window, cx| {
                    if full {
                        r.toast = Some((r.i18n.tv("pv.full", &[("n", "50")]), "warn".into()));
                        cx.notify();
                        return;
                    }
                    r.auto.confirm = None;
                    crate::pages::preset_form::open(r, None, window, cx);
                }))))
        })
        .when(tab == Tab::Macros, |d| {
            let full = macros.len() >= 100;
            // Stop all only while something runs; Import and New only once there is a list (the empty one has both).
            d.when(any_live, |d| d.child(Button::new("mc-stop-all", root.tr("macro.stopAll")).icon("stop").tip(root.tr("macro.stopAllTip")).on_click(cx.listener(|r, _, _, cx| {
                r.ask("stopMacros", json!({}), cx, |r, _| r.ok(r.tr("macro.stoppedAll")))
            }))))
            .when(!macros.is_empty(), |d| d.child(Button::new("mc-import", root.tr("macro.import")).icon("folder").quiet().on_click(cx.listener(move |r, _, window, cx| {
                if !macros_full(r, full, cx) {
                    import_macro(r, window, cx)
                }
            })))
            .child(Button::new("mc-new", root.tr("macro.new")).icon("plus").primary().on_click(cx.listener(move |r, _, window, cx| {
                if macros_full(r, full, cx) {
                    return;
                }
                r.auto.confirm = None;
                crate::pages::macro_form::open(r, None, window, cx);
            }))))
        });

    let header = div()
        .flex_none()
        .px(px(SIDE))
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
                        .child(div().text_size(px(T_TITLE)).line_height(px(38.)).font_weight(FontWeight::BOLD).child(root.tr("nav.auto")))
                        .child(if summary.is_empty() { div().h(px(18.)) } else { facts(summary) }),
                )
                .child(actions),
        )
        .child(div().flex().items_center().gap(px(12.)).child(switch).children(beta));

    let body = match tab {
        Tab::Presets => preset_list(root, &presets, narrow, wide, width, cx).into_any_element(),
        Tab::Macros => macro_list(root, &macros, &runs, narrow, wide, width, cx).into_any_element(),
    };
    // The list scrolls under the head, with a bar at its side that says there is more below.
    div()
        .id("auto")
        .size_full()
        .rounded(px(R_PANEL))
        .bg(c(PANEL))
        .overflow_hidden()
        .relative()
        .flex()
        .flex_col()
        .child(header)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(div().id("auto-body").size_full().overflow_y_scroll().track_scroll(&root.auto.scroll).child(body))
                .child(scroll_bar("auto-bar", &root.auto.scroll)),
        )
        .into_any_element()
}

/// A list's scroll bar: shown while it scrolls, then it fades (owner 2026-11-12: a bar always there looked ugly).
pub(crate) fn scroll_bar(id: &'static str, handle: &ScrollHandle) -> Div {
    scroll_bar_named(id, handle)
}

/// [`scroll_bar`] for an id made at run time.
pub(crate) fn scroll_bar_named<H: gpui_kit::component::scroll::ScrollbarHandle + Clone>(id: impl Into<ElementId>, handle: &H) -> Div {
    div().absolute().top_0().bottom_0().right(px(2.)).w(px(10.)).child(Scrollbar::vertical(handle).id(id).mode(ScrollbarMode::Scrolling).viewport_from_layout())
}

/// Asks for the presets' game icons the app doesn't know yet; again after 5 minutes while one is still missing (a
/// preset added later, a first asking that failed offline: PresetsView.tsx).
pub fn want(r: &mut Root) {
    if r.auto.asked_icons.is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(300)) {
        return;
    }
    if list(r, "presets").iter().any(|p| !s(p, "placeInput").is_empty() && s(p, "iconUrl").is_empty()) {
        r.auto.asked_icons = Some(std::time::Instant::now());
        r.send("presetIcons", json!({}));
    }
}

/// At the most macros there may be: says so instead (MacrosView.tsx's full()).
fn macros_full(r: &mut Root, full: bool, cx: &mut Context<Root>) -> bool {
    if full {
        r.toast = Some((r.i18n.tv("macro.full", &[("n", "100")]), "warn".into()));
        cx.notify();
    }
    full
}

/// A macro saved to a file (Export) opens in the form as a new one (MacrosView.tsx's importFile).
fn import_macro(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call("importMacro", json!({}));
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            match answer {
                // (A form opened while the file box was up keeps its edits: the file isn't put over it.)
                Ok(Ok(m)) if m.is_object() && r.auto.macro_form.is_none() => {
                    r.auto.confirm = None;
                    crate::pages::macro_form::open(r, Some(m), window, cx);
                }
                Ok(Err(e)) => r.toast = Some((e, "bad".into())),
                _ => {}
            }
            cx.notify();
        });
    })
    .detach();
}

/// No macros yet, in Home's welcome look: three ways to start as blocks, Record, From a template, Blank (each with its
/// button), and a file to import (MacrosView.tsx's mc-empty). Record needs a Roblox window open (it says so); a
/// template is picked in the form, on its templates card.
fn macro_start(root: &Root, width: f32, cx: &mut Context<Root>) -> Div {
    let wide = |b: AnyElement| div().relative().flex().flex_col().child(b);
    let record = Button::new("mc-rec-b", root.tr("macro.record")).icon("record").small().on_click(cx.listener(|r, _, window, cx| {
        r.auto.confirm = None;
        // Several windows: which one first (MacrosView.tsx's record menu).
        if crate::pages::macro_form::record_windows(r).len() > 1 {
            let just = r.auto.menu_closed.as_ref().is_some_and(|(k, at)| k == REC_MENU && at.elapsed().as_millis() < 300);
            r.auto.menu_closed = None;
            r.auto.menu = if just || r.auto.menu.as_deref() == Some(REC_MENU) { None } else { Some(REC_MENU.into()) };
            cx.notify();
        } else {
            crate::pages::macro_form::record_new(r, window, cx);
        }
    }));
    let template = Button::new("mc-tpl-b", root.tr("macro.fromTemplate")).icon("layers").small().on_click(cx.listener(|r, _, window, cx| {
        r.auto.confirm = None;
        crate::pages::macro_form::open(r, None, window, cx);
    }));
    let blank = Button::new("mc-blank-b", root.tr("macro.empty")).icon("plus").small().on_click(cx.listener(|r, _, window, cx| {
        r.auto.confirm = None;
        crate::pages::macro_form::open(r, None, window, cx);
    }));
    let blocks = vec![
        crate::pages::home::stage_block(
            root.tr("macro.record").trim_end_matches(['…', '.']).to_string(),
            crate::pages::home::block_text("record", None, root.tr("macro.tile.record"), None),
            Some(wide(record.into_any_element()).children(record_menu(root, cx)).into_any_element()),
            false,
        ),
        crate::pages::home::stage_block(
            root.tr("macro.fromTemplate"),
            crate::pages::home::block_text("layers", None, root.tr("macro.tile.template"), None),
            Some(wide(template.into_any_element()).into_any_element()),
            false,
        ),
        crate::pages::home::stage_block(
            root.tr("macro.empty"),
            crate::pages::home::block_text("plus", None, root.tr("macro.tile.blank"), None),
            Some(wide(blank.into_any_element()).into_any_element()),
            false,
        ),
    ];
    let import = Button::new("mc-import-link", root.tr("macro.importLink")).icon("folder").quiet().small().on_click(cx.listener(|r, _, window, cx| import_macro(r, window, cx)));
    crate::pages::home::empty_stage(
        root,
        3,
        None,
        root.tr("macro.emptyTitle"),
        root.tr("macro.emptyText"),
        None,
        blocks,
        Some(div().flex().child(import).into_any_element()),
        width - 2. * SIDE,
        cx,
    )
}

const REC_MENU: &str = "\u{1}record";

/// Record's "On which window?" list under its tile.
fn record_menu(root: &Root, cx: &mut Context<Root>) -> Option<Div> {
    if root.auto.menu.as_deref() != Some(REC_MENU) {
        return None;
    }
    let mut menu = div()
        .id("mc-rec-menu")
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
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            if r.auto.menu.as_deref() == Some(REC_MENU) {
                r.auto.menu = None;
                r.auto.menu_closed = Some((REC_MENU.into(), std::time::Instant::now()));
                cx.notify();
            }
        }))
        .child(div().px(px(10.)).py(px(6.)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(root.tr("macro.recordWhich")));
    for (k, (acc, name)) in crate::pages::macro_form::record_windows(root).into_iter().enumerate() {
        menu = menu.child(
            menu_row(("mc-rec-i", k))
                .text_color(c(TEXT_2))
                .child(icon("record", 15., c(TEXT_2)))
                .child(div().flex_1().whitespace_nowrap().child(name))
                .on_click(cx.listener(move |r, _, window, cx| {
                    r.auto.menu = None;
                    crate::pages::macro_form::record_on(r, acc.clone(), window, cx);
                    cx.notify();
                })),
        );
    }
    Some(popup(PopAt::Under, "mc-rec-pop", true, menu.mt(px(6.))))
}

/// A list of rows on one calm panel (a section's presets or macros): rows a hairline apart, each easing in under the
/// mouse, as the Accounts page's panel of what needs a look.
fn list_panel() -> Div {
    div().w_full().flex().flex_col().p(px(4.)).rounded(px(R_L + 2.)).bg(ca(WHITE, 0.03)).border_1().border_color(c(LINE))
}

/// The hairline between a [`list_panel`]'s rows.
fn row_line() -> Div {
    div().h(px(1.)).mx(px(12.)).flex_none().bg(ca(WHITE, 0.05))
}

/// A row of a [`list_panel`]. (Its ground eases under the mouse; what orders it shows only then: the web page's rows.)
fn row_shell(id: (&'static str, usize)) -> Hot {
    hover_only(id).group("auto-row").w_full().min_h(px(68.)).px(px(12.)).py(px(10.)).flex().items_center().gap(px(14.)).rounded(px(R_M)).tones(ca(WHITE, 0.), ca(WHITE, 0.04))
}

/// A section of the list: its head (title and count) over its rows on one panel.
fn list_section(title: String, n: usize, rows: Vec<AnyElement>) -> Div {
    let mut panel = list_panel();
    for (k, row) in rows.into_iter().enumerate() {
        if k > 0 {
            panel = panel.child(row_line());
        }
        panel = panel.child(row);
    }
    div().flex().flex_col().gap(px(12.)).child(section_head(title, Some(n))).child(panel)
}

/// A game's picture as a flat block, or a mark on the raised ground.
fn game_block(url: &str, size: f32, fallback: &'static str, cx: &mut App) -> Div {
    let url = Some(url).filter(|u| !u.is_empty());
    let pic = url.and_then(|u| images::pic_sq(u, size, cx));
    div().size(px(size)).flex_none().rounded(px(R_M)).overflow_hidden().bg(c(RAISED_2)).flex().items_center().justify_center().map(|d| match pic {
        Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
        None => d.child(icon(fallback, size * 0.42, c(TEXT_2))),
    })
}

/// A row's grip: pressed, the row follows the mouse through its section until it is let go (the web page's ord-grip).
fn grip(root: &Root, key: &'static str, i: usize, held: bool, cx: &mut Context<Root>, start: impl Fn(&mut Root) + 'static) -> Stateful<Div> {
    let tip: SharedString = root.tr("lo.drag").into();
    div()
        .id((key, i))
        .w(px(18.))
        .h(px(44.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_grab()
        .child(icon("grip", 15., if held { c(TEXT) } else { c(DIM) }))
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |r, _, _, cx| {
                r.auto.menu = None;
                start(r);
                cx.stop_propagation();
                cx.notify();
            }),
        )
}

/// The dragged row passed over row `over`: it takes that row's place at once.
fn drag_over(r: &mut Root, over: &str) -> bool {
    let Some(d) = r.auto.drag.as_mut() else { return false };
    let (Some(from), Some(to)) = (d.order.iter().position(|x| *x == d.id), d.order.iter().position(|x| x == over)) else { return false };
    if from == to {
        return false;
    }
    let moved = d.order.remove(from);
    d.order.insert(to, moved);
    true
}

/// The grip let go: the new order is saved (presets as a whole order, a macro as its new place).
fn drag_end(r: &mut Root, cx: &mut Context<Root>) {
    let Some(d) = r.auto.drag.take() else { return };
    let key = if d.macros { "macros" } else { "presets" };
    let before: Vec<String> = list(r, key).iter().map(|v| s(v, "id").to_string()).collect();
    if before != d.order {
        if d.macros {
            if let Some(to) = d.order.iter().position(|x| *x == d.id) {
                r.ask("moveMacro", json!({ "id": d.id, "to": to }), cx, |_, _| {});
            }
        } else {
            r.ask("setPresetOrder", json!({ "ids": d.order }), cx, |_, _| {});
        }
    }
    cx.notify();
}

/// While a row is dragged, the rows of its section it passes over make way; a move with the button up (let go
/// outside) ends it.
fn follow_drag<E: InteractiveElement>(el: E, id: String, group: u8, cx: &mut Context<Root>) -> E {
    el.on_mouse_move(cx.listener(move |r, ev: &MouseMoveEvent, _, cx| {
        let Some(d) = r.auto.drag.as_ref() else { return };
        if ev.pressed_button != Some(MouseButton::Left) {
            return drag_end(r, cx);
        }
        if d.group != group {
            return;
        }
        if drag_over(r, &id) {
            cx.notify();
        }
    }))
}

/// Delete: the box asks first (the old page's confirmBox).
#[allow(clippy::too_many_arguments)]
fn delete_button(
    root: &Root,
    key: &'static str,
    i: usize,
    cx: &mut Context<Root>,
    title: String,
    text: String,
    remember: Option<&'static str>,
    del: impl Fn(&mut Root, &mut Context<Root>) + 'static,
) -> AnyElement {
    let del = std::rc::Rc::new(del);
    IconButton::new((key, i), "trash")
        .size(30.)
        .tip(root.tr("ps.delete"))
        .on_click(cx.listener(move |r, _, window, cx| {
            let del = del.clone();
            r.confirm(title.clone(), text.clone(), r.tr("ps.delete"), true, remember, window, cx, move |r, cx| del(r, cx));
        }))
        .into_any_element()
}

// ───────────────────────── presets ─────────────────────────

/// Which section a preset is in: open now, scheduled, the rest.
fn preset_group(p: &Value) -> u8 {
    if int(p, "running") > 0 {
        0
    } else if p.pointer("/schedule/enabled").and_then(Value::as_bool).unwrap_or(false) {
        1
    } else {
        2
    }
}

fn preset_list(root: &Root, presets: &[Value], narrow: bool, wide: bool, width: f32, cx: &mut Context<Root>) -> Div {
    let mut body = div().flex().flex_col().gap(px(SECTION_GAP)).px(px(SIDE)).pt(px(8.)).pb(px(32.));
    if presets.is_empty() {
        // In Home's welcome look: what a preset keeps, a block each.
        let main = Button::new("pv-new-empty", root.tr("ps.new")).icon("plus").primary().on_click(cx.listener(|r, _, window, cx| {
            crate::pages::preset_form::open(r, None, window, cx)
        }));
        let blocks = vec![
            crate::pages::home::stage_block(root.tr("nav.accounts"), crate::pages::home::block_text("users", Some(root.tr("pv.w.acc")), root.tr("pv.w.accSub"), None), None, false),
            crate::pages::home::stage_block(root.tr("au.sec.game"), crate::pages::home::block_text("gamepad", Some(root.tr("pv.w.game")), root.tr("pv.w.gameSub"), None), None, false),
            crate::pages::home::stage_block(root.tr("pv.w.whenHead"), crate::pages::home::block_text("clock", Some(root.tr("pv.w.when")), root.tr("pv.w.whenSub"), None), None, false),
        ];
        return body.child(crate::pages::home::empty_stage(
            root,
            2,
            None,
            root.tr("pv.emptyTitle"),
            root.tr("pv.emptyText"),
            Some(main.into_any_element()),
            blocks,
            None,
            width - 2. * SIDE,
            cx,
        ));
    }
    // The search: over each preset's name and its game (accents don't matter), as PresetsView.tsx's.
    let typed = root.preset_search.read(cx).value().trim().to_string();
    let q = folded(&typed);
    let no_game = root.tr("ps.noGame");
    let shown: Vec<&Value> = presets
        .iter()
        .filter(|p| {
            let game = if s(p, "placeInput").is_empty() { no_game.as_str() } else if s(p, "gameName").is_empty() { s(p, "placeInput") } else { s(p, "gameName") };
            q.is_empty() || folded(&format!("{}\n{game}", s(p, "name"))).contains(&q)
        })
        .collect();
    if shown.is_empty() {
        return body.child(
            div()
                .py(px(40.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(12.))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("pv.noMatch", &[("q", &typed)])))
                .child(Button::new("pv-clear-search", root.tr("pv.clearSearch")).on_click(cx.listener(|r, _, window, cx| {
                    r.preset_search.update(cx, |s, cx| s.set_value("", window, cx));
                    cx.notify();
                }))),
        );
    }
    // (Today's runs, and the order buttons, only for the whole list.)
    if q.is_empty() {
        if let Some(r) = ruler(root, presets, width, cx) {
            body = body.child(r);
        }
    }
    let by_id: HashMap<&str, &Value> = root.accounts().iter().map(|a| (s(a, "id"), a)).collect();
    let ids: Vec<String> = if q.is_empty() { presets.iter().map(|p| s(p, "id").to_string()).collect() } else { Vec::new() };
    // A row being dragged: the list in the order it has now.
    let mut shown = shown;
    if let Some(d) = root.auto.drag.as_ref().filter(|d| !d.macros && q.is_empty()) {
        shown.sort_by_key(|p| d.order.iter().position(|x| x == s(p, "id")).unwrap_or(usize::MAX));
    }
    // In sections (open now, scheduled, the rest) once there is more than one kind; a row moves within its own.
    let titles = ["au.grp.running", "au.grp.scheduled", "au.grp.rest"];
    let groups: Vec<(u8, Vec<&Value>)> = (0..3u8).map(|g| (g, shown.iter().copied().filter(|p| preset_group(p) == g).collect::<Vec<_>>())).filter(|(_, l)| !l.is_empty()).collect();
    let single = groups.len() == 1;
    let mut k = 0;
    for (g, items) in groups {
        let n = items.len();
        let mut rows = Vec::new();
        for p in items {
            rows.push(preset_row(root, p, k, g, &ids, &by_id, narrow, wide, cx).into_any_element());
            k += 1;
        }
        body = body.child(list_section(if single { root.tr("ps.listTitle") } else { root.tr(titles[g as usize]) }, n, rows));
    }
    body.on_mouse_up(MouseButton::Left, cx.listener(|r, _, _, cx| drag_end(r, cx))).on_mouse_up_out(MouseButton::Left, cx.listener(|r, _, _, cx| drag_end(r, cx)))
}

#[allow(clippy::too_many_arguments)]
fn preset_row(root: &Root, p: &Value, i: usize, group: u8, ids: &[String], by_id: &HashMap<&str, &Value>, narrow: bool, _wide: bool, cx: &mut Context<Root>) -> impl IntoElement {
    let id = s(p, "id").to_string();
    let name = s(p, "name").to_string();
    let place_input = s(p, "placeInput");
    let server = s(p, "server");
    let game = if server == "follow" {
        root.i18n.tv("ps.following", &[("name", s(p, "followPlayer"))])
    } else if !place_input.is_empty() {
        crate::pages::home::clean(Some(s(p, "gameName")).filter(|g| !g.is_empty()).unwrap_or(place_input))
    } else {
        root.tr("ps.noGame")
    };
    let accounts: Vec<&Value> = p.get("accountIds").and_then(Value::as_array).into_iter().flatten().filter_map(|x| x.as_str()).filter_map(|x| by_id.get(x).copied()).collect();

    // What it sets: the server, the wait between accounts, the options it sets differently (app.js's presetOptionsText).
    let delay = p.get("delaySeconds").and_then(Value::as_i64);
    let default_delay = root.state.pointer("/settings/launchDelaySeconds").and_then(Value::as_i64).unwrap_or(8).to_string();
    let mut summary: Vec<String> = Vec::new();
    if !place_input.is_empty() && server != "follow" && !server.is_empty() {
        summary.push(root.tr(&format!("ps.srv.{server}")));
    }
    summary.push(match delay {
        None => root.i18n.tv("pv.delayDefault", &[("n", &default_delay)]),
        Some(0) => root.tr("pv.delay0"),
        Some(n) => root.i18n.tv("pv.delayN", &[("n", &n.to_string())]),
    });
    let tri = |k: &str, on: &str, off: &str| match p.get(k).and_then(Value::as_bool) {
        Some(true) => root.tr(on),
        Some(false) => root.tr(off),
        None => String::new(),
    };
    summary.push(tri("antiAfk", "pv.optAfkOn", "pv.optAfkOff"));
    summary.push(tri("autoRejoin", "pv.optRejoinOn", "pv.optRejoinOff"));
    let with = s(p, "rejoinWith");
    if with == "any" {
        summary.push(root.tr("pv.optWithAny"));
    } else if let Some(a) = by_id.get(with) {
        summary.push(root.i18n.tv("pv.optWith", &[("name", &crate::pages::accounts::label(a))]));
    }
    if s(p, "profile") == "afk" {
        summary.push(root.tr("pv.optLight"));
    }
    if int(p, "restartHours") > 0 && !place_input.is_empty() {
        summary.push(root.i18n.tv("pv.optRestart", &[("n", &int(p, "restartHours").to_string())]));
    }
    summary.push(tri("arrange", "pv.optArrangeOn", "pv.optArrangeOff"));
    if p.get("discord").is_some_and(|d| d.as_str().is_some_and(|x| !x.is_empty())) {
        summary.push(root.tr("pv.optDiscord"));
    }
    let summary = summary.into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");

    // Under the name, quieter: what is open, when it is set to run, when next, when last.
    let sc = p.get("schedule").filter(|v| v.is_object());
    let sc_on = sc.and_then(|v| v.get("enabled")).and_then(Value::as_bool).unwrap_or(false);
    let has_time = sc.is_some_and(|v| !s(v, "time").is_empty());
    let running = int(p, "running");
    let mut when: Vec<(String, Option<Hsla>)> = Vec::new();
    if running > 0 {
        when.push((root.i18n.tn("pv.running", running, &[]), Some(c(OK))));
    }
    if let Some(sc) = sc.filter(|_| has_time) {
        let at = schedule_text(root, sc);
        when.push(if sc_on { (at, Some(c(TEXT_2))) } else { (format!("{} · {at}", root.tr("pv.schOff")), None) });
        if sc_on && s(sc, "date").is_empty() {
            if let Some(t) = next_text(root, p) {
                when.push((t, None));
            }
        }
    }
    // No time set: said so, as the others say theirs (a row with no schedule line read as missing one, review 4.3).
    if !has_time {
        when.push((root.tr("au.noSchedule"), None));
    }
    if let Some(t) = crate::time::iso_secs(s(p, "lastRun")) {
        let ago = crate::time::ago(crate::time::now_secs() - t, &root.i18n);
        when.push((root.i18n.tv("pv.lastRun", &[("t", &ago)]), None));
    }
    // (Nothing to say: what it sets, in its place.)
    let said_summary = when.is_empty();
    let line = if said_summary { div().text_size(px(T_M)).line_height(px(18.)).text_color(c(MUTED)).child(summary.clone()) } else { facts(when) };

    let tile = game_block(s(p, "iconUrl"), 44., if place_input.is_empty() { "monitor" } else { "gamepad" }, cx);

    let faces = if accounts.is_empty() {
        let tip: SharedString = root.tr("pv.noAccountsTip").into();
        div()
            .id(("pv-none", i))
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(T_S))
            .text_color(c(WARN))
            .child(icon("alert", 14., c(WARN)))
            .child(root.tr("pv.noAccounts"))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
    } else {
        let more = accounts.len().saturating_sub(AVATARS);
        let mut names: Vec<String> = accounts.iter().take(AVATARS).map(|a| {
            let name = crate::pages::accounts::label(a);
            let short: String = name.chars().take(36).collect();
            if short.len() < name.len() { format!("{short}…") } else { short }
        }).collect();
        if more > 0 {
            names.push(root.i18n.tv("home.moreN", &[("n", &more.to_string())]));
        }
        let tip: SharedString = names.join(", ").into();
        div()
            .id(("pv-faces", i))
            .flex()
            .items_center()
            .children(accounts.iter().take(AVATARS).enumerate().map(|(k, a)| div().when(k > 0, |d| d.ml(px(-8.))).child(crate::pages::accounts::stacked(a, 28., PANEL, cx))))
            .when(more > 0, |d| d.child(div().ml(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).font_weight(FontWeight::MEDIUM).font_features(tnum()).child(format!("+{more}"))))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
    };

    // The schedule's switch, its words its tip. No time set: a calendar in its place that opens the editor to set one
    // (an empty place in the column read as a missing switch, review 4.3).
    let sch_add = (!has_time).then(|| {
        let p = p.clone();
        IconButton::new(("pv-sch-add", i), "calendar").size(30.).tip(root.tr("pv.schAdd")).on_click(cx.listener(move |r, _, window, cx| {
            r.auto.confirm = None;
            crate::pages::preset_form::open(r, Some(p.clone()), window, cx);
        }))
    });
    let sch = has_time.then(|| {
        let (pid, pname) = (id.clone(), name.clone());
        let tip: SharedString = root.tr(if sc_on { "pv.schTurnOff" } else { "pv.schTurnOn" }).into();
        div().id(("pv-sch-tip", i)).flex().items_center().tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()).child(Switch::new(("pv-sch", i), sc_on).on_toggle({
            let this = cx.entity().downgrade();
            move |on, _, cx| {
                let (pid, pname, on) = (pid.clone(), pname.clone(), *on);
                let _ = this.update(cx, |r, cx| {
                    r.ask("setPresetSchedule", json!({ "id": pid, "enabled": on }), cx, move |r, _| {
                        r.ok(r.i18n.tv(if on { "t.schOn" } else { "t.schOff" }, &[("name", &pname)]))
                    })
                });
            }
        }))
    });

    // Run (or Close while its windows are open): quiet, so the page's one yellow block stays New preset.
    let launching = root.state.get("launching").and_then(Value::as_bool).unwrap_or(false);
    let run = if running > 0 {
        let pid = id.clone();
        Button::new(("pv-close", i), root.tr("pv.close")).icon("power").small().tip(root.i18n.tn("pv.runningTip", running as i64, &[])).on_click(cx.listener(move |r, _, _, cx| {
            r.ask("closePreset", json!({ "id": pid }), cx, |r, v| {
                let n = v.as_i64().unwrap_or(0);
                r.toast = Some(if n > 0 { (r.i18n.tn("t.closing", n, &[]), "ok".into()) } else { (r.tr("t.noneRunning"), "info".into()) });
            })
        }))
    } else {
        let pid = id.clone();
        Button::new(("pv-run", i), root.tr("ps.run")).icon("play").small().disabled(accounts.is_empty() || launching).on_click(cx.listener(move |r, _, _, cx| {
            r.launch("runPreset", json!({ "id": pid }), false, cx)
        }))
    };

    let edit = {
        let p = p.clone();
        IconButton::new(("pv-edit", i), "edit").size(30.).tip(root.tr("ps.edit")).on_click(cx.listener(move |r, _, window, cx| {
            r.auto.confirm = None;
            crate::pages::preset_form::open(r, Some(p.clone()), window, cx);
        }))
    };
    let dup = {
        let (pid, was_on) = (id.clone(), sc_on);
        IconButton::new(("pv-dup", i), "copy").size(30.).tip(root.tr("pv.duplicate")).on_click(cx.listener(move |r, _, _, cx| {
            r.ask_once("duplicatePreset", "duplicatePreset", json!({ "id": pid }), cx, move |r, _| r.ok(r.tr(if was_on { "pv.duplicatedOff" } else { "pv.duplicated" })))
        }))
    };
    let del = {
        let pid = id.clone();
        delete_button(root, "pv-del", i, cx, root.tr("ps.deleteTitle"), root.i18n.tv("ps.deleteQ", &[("name", s(p, "name"))]), Some("deletePreset"), move |r, cx| r.ask_once("deletePreset", "deletePreset", json!({ "id": pid }), cx, |r, _| r.ok(r.tr("ps.deleted"))))
    };
    // Up and Down, beside the grip (PresetsView.tsx's move): a place within its section, without dragging.
    // (Only among the rows of its own section: the sections are by state, a move across them would show nothing.)
    let mates: Vec<String> = list(root, "presets").iter().filter(|q| preset_group(q) == group).map(|q| s(q, "id").to_string()).filter(|x| ids.contains(x)).collect();
    let moves = (mates.len() > 1).then(|| {
        let at = mates.iter().position(|x| *x == id).unwrap_or(0);
        let step = |key: &'static str, glyph: &'static str, to: Option<usize>, cx: &mut Context<Root>| {
            let (pid, other) = (id.clone(), to.and_then(|j| mates.get(j)).cloned());
            IconButton::new((key, i), glyph).size(30.).tip(root.tr(if key == "pv-up" { "lo.up" } else { "lo.down" })).disabled(other.is_none()).on_click(cx.listener(move |r, _, _, cx| {
                let Some(other) = other.clone() else { return };
                swap_presets(r, &pid, &other, cx);
            }))
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .child(step("pv-up", "chevronUp", at.checked_sub(1), cx))
            .child(step("pv-down", "chevronDown", Some(at + 1).filter(|j| *j < mates.len()), cx))
    });
    let held = root.auto.drag.as_ref().is_some_and(|d| !d.macros && d.id == id);
    let handle = (mates.len() > 1).then(|| {
        let (pid, all) = (id.clone(), ids.to_vec());
        grip(root, "pv-grip", i, held, cx, move |r| r.auto.drag = Some(Drag { macros: false, group, id: pid.clone(), order: all.clone() }))
    });
    let tip: SharedString = summary.clone().into();

    // A row alone in its section has no grip, but its picture stays in line with the other sections' rows.
    let gap = (handle.is_none() && ids.len() > 1).then(|| div().w(px(18.)).flex_none());
    follow_drag(row_shell(("pv-row", i)), id.clone(), group, cx)
        .pl(px(if ids.len() > 1 { 4. } else { 12. }))
        .when(held, |d| d.opacity(0.6))
        .children(handle)
        .children(gap)
        .child(tile)
        .child(
            div()
                .id(("pv-info", i))
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(10.))
                        .min_w_0()
                        // (The name keeps its room, the game gives way first.)
                        .child(div().flex_shrink_0().max_w(relative(1.)).truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(name.clone()))
                        .child(div().flex_shrink(1.).min_w_0().truncate().text_size(px(T_M)).text_color(c(MUTED)).child(game)),
                )
                .child(line)
                // What it sets, whole: under the rest, wrapping onto a second line rather than cut (the Presets list
                // lost "Anti-AFK o…" at the side).
                .when(!said_summary, |d| d.child(div().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(summary)))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
        )
        .when(!narrow, |d| d.child(div().flex_none().w(px(128.)).flex().justify_start().child(faces)))
        // (The faces start at one x in every row: right-aligned, a row of one face and a row of four began apart.)
        .child(div().flex_none().w(px(44.)).flex().justify_end().children(sch).children(sch_add))
        .child(div().flex_none().w(px(84.)).flex().justify_end().child(run))
        // (Its room kept when it has none, so Run and the rest stay in one column down the sections.)
        .child(moves.unwrap_or_else(|| div().flex_none().w(px(62.))))
        .child(div().flex_none().flex().items_center().gap(px(2.)).child(edit).child(dup).child(del))
}

/// Two presets change places in the whole order.
fn swap_presets(r: &mut Root, a: &str, b: &str, cx: &mut Context<Root>) {
    let mut order: Vec<String> = list(r, "presets").iter().map(|v| s(v, "id").to_string()).collect();
    let (Some(i), Some(j)) = (order.iter().position(|x| x == a), order.iter().position(|x| x == b)) else { return };
    order.swap(i, j);
    r.ask_once("setPresetOrder", "setPresetOrder", json!({ "ids": order }), cx, |_, _| {});
}

/// Today on one ruler, midnight to midnight: each preset that runs today as its game's icon at its time (those gone by
/// faded), and now. None on a day nothing is scheduled.
fn ruler(root: &Root, presets: &[Value], width: f32, cx: &mut Context<Root>) -> Option<Div> {
    let (today, now) = local_now();
    let mut runs: Vec<(i64, &Value)> = presets.iter().filter_map(|p| run_today(p, today).map(|at| (at, p))).collect();
    if runs.is_empty() {
        return None;
    }
    runs.sort_by_key(|(at, _)| *at);
    let pct = |m: i64| relative(m as f32 / 1440.);
    let mut track = div().relative().h(px(76.)).mx(px(20.));
    // The line, the part gone by, the hours.
    track = track
        .child(div().absolute().left_0().right_0().top(px(37.)).h(px(2.)).rounded_full().bg(ca(WHITE, 0.08)))
        .child(div().absolute().left_0().top(px(37.)).h(px(2.)).w(pct(now)).rounded_full().bg(ca(WHITE, 0.24)));
    // (An hour's words give way to the time now only where they would touch it, in pixels: review 4.2, "now" had no
    // time on the line; a fixed 80 minutes hid 6 PM at 4:46 PM with 90 px between them, review round 5 4.1. An hour's
    // words start at its tick, about 48 px wide; now's time is centred on its line, about 56 px.)
    let per_min = ((width - 120.).max(400.)) / 1440.;
    let near_now = |m: i64| {
        let gap = (m - now) as f32 * per_min;
        gap > -(48. + 28. + 6.) && gap < 28. + 6.
    };
    for h in [0, 3, 6, 9, 12, 15, 18, 21] {
        let mark = crate::lang::cur().hour(h);
        track = track
            .child(div().absolute().left(pct(h * 60)).top(px(34.)).w(px(1.)).h(px(8.)).bg(ca(WHITE, 0.12)))
            .when(!near_now(h * 60), |d| d.child(div().absolute().left(pct(h * 60)).top(px(56.)).text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(mark)));
    }
    // (The day's end, so the ruler doesn't stop at 9 PM.)
    let l = crate::lang::cur();
    let end = if l.meridiem.is_some() { l.hour(0) } else { format!("24{}00", l.time_sep) };
    track = track
        .child(div().absolute().right_0().top(px(34.)).w(px(1.)).h(px(8.)).bg(ca(WHITE, 0.12)))
        .when((1440 - now) as f32 * per_min > 48. + 28. + 6., |d| d.child(div().absolute().right_0().top(px(56.)).text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(end)));
    // Runs close together take turns above and below the line.
    let (mut last, mut lift) = (i64::MIN / 2, false);
    let mut next: Option<(i64, String)> = None;
    for (k, (at, p)) in runs.into_iter().enumerate() {
        lift = if at - last < 58 { !lift } else { false };
        last = at;
        if at >= now && next.is_none() {
            next = Some((at, s(p, "name").to_string()));
        }
        let pic = Some(s(p, "iconUrl")).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, 32., cx));
        let tip: SharedString = format!("{} · {}", s(p, "name"), hm(at / 60, at % 60)).into();
        track = track.child(
            div()
                .id(("pvr", k))
                .absolute()
                .left(pct(at))
                .ml(px(-18.))
                .top(px(if lift { 0. } else { 20. }))
                .size(px(36.))
                .rounded(px(8.))
                .overflow_hidden()
                .border_2()
                .border_color(c(PANEL))
                .bg(c(RAISED_2))
                .flex()
                .items_center()
                .justify_center()
                .when(at < now, |d| d.opacity(0.55))
                .map(|d| match pic {
                    Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
                    None => d.child(icon("layers", 16., c(TEXT_2))),
                })
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
        );
    }
    // Now: its line and, under it on the hours' row, its time in its colour (centred on the line, kept inside the
    // ruler at the day's two ends).
    let shift = if now < 40 { 0. } else if now > 1400 { -84. } else { -42. };
    track = track
        .child(div().absolute().left(pct(now)).top(px(20.)).h(px(36.)).w(px(2.)).ml(px(-1.)).rounded_full().bg(c(ACCENT)))
        .child(
            div()
                .absolute()
                .left(pct(now))
                .ml(px(shift))
                .w(px(84.))
                .top(px(56.))
                .flex()
                .justify_center()
                .when(now < 40, |d| d.justify_start())
                .when(now > 1400, |d| d.justify_end())
                .text_size(px(T_S))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(ACCENT))
                .font_features(tnum())
                .child(hm(now / 60, now % 60)),
        );
    // The head: Today, and at its right the next run's time and name (the time now is on the ruler).
    let mut parts: Vec<(String, Option<Hsla>)> = Vec::new();
    if let Some((at, name)) = next {
        parts.push((root.i18n.tv("ps.next", &[("when", &hm(at / 60, at % 60))]), Some(c(TEXT_2))));
        parts.push((name, None));
    }
    let head = section_head(root.tr("pv.today"), None).child(facts(parts));
    Some(div().flex().flex_col().gap(px(12.)).child(head).child(list_panel().pt(px(8.)).pb(px(6.)).child(track)))
}

// ───────────────────────── macros ─────────────────────────

/// Why a run stopped, in words: a known reason's (macro.why.*), else as it came (a Fail step's own text).
pub(crate) fn why_text(root: &Root, reason: &str) -> String {
    let key = format!("macro.why.{reason}");
    if root.i18n.has(&key) { root.tr(&key) } else { reason.to_string() }
}

/// One account's run in words: "Main: running", "Farm 1: waiting its turn", "Farm 2: stopped (didn't see it)".
fn run_text(root: &Root, r: &Value, name: &str) -> String {
    let (status, reason) = (s(r, "status"), s(r, "reason"));
    let why = if reason.is_empty() { String::new() } else { why_text(root, reason) };
    if status == "waiting" && (reason.is_empty() || reason == "busy") {
        return root.i18n.tv("macro.line.turn", &[("a", name)]);
    }
    if status == "running" && reason == "recovering" {
        return root.i18n.tv("macro.line.recovering", &[("a", name)]);
    }
    let key = if !why.is_empty() && status != "running" && status != "done" { format!("macro.line.{status}Why") } else { format!("macro.line.{status}") };
    root.i18n.tv(&key, &[("a", name), ("why", &why)])
}

fn dur_text(root: &Root, secs: i64) -> String {
    if secs < 60 {
        return root.i18n.tv("macro.dur.s", &[("n", &secs.to_string())]);
    }
    let (h, m, r) = (secs / 3600, secs % 3600 / 60, secs % 60);
    let mut v = Vec::new();
    if h > 0 {
        v.push(root.i18n.tv("macro.dur.h", &[("n", &h.to_string())]));
    }
    if m > 0 {
        v.push(root.i18n.tv("macro.dur.min", &[("n", &m.to_string())]));
    }
    if r > 0 {
        v.push(root.i18n.tv("macro.dur.s", &[("n", &r.to_string())]));
    }
    v.join(" ")
}

/// When it runs and on whom: "Khi bạn bấm Chạy · Tài khoản đang chọn · Ctrl+Alt+2".
pub(crate) fn when_text(root: &Root, m: &Value) -> String {
    let kind = m.pointer("/trigger/kind").and_then(Value::as_str).unwrap_or("manual");
    let when = if kind == "interval" {
        let every = m.pointer("/trigger/everySeconds").and_then(Value::as_i64).unwrap_or(60);
        root.i18n.tv("macro.trig.every", &[("d", &dur_text(root, every))])
    } else {
        root.tr(&format!("macro.when.{kind}"))
    };
    let targets = s(m, "targets");
    let who = if targets == "accounts" {
        let n = m.get("accountIds").and_then(Value::as_array).map_or(0, Vec::len);
        if n == 1 { root.tr("macro.tg.n1") } else { root.i18n.tv("macro.tg.n", &[("n", &n.to_string())]) }
    } else {
        root.tr(&format!("macro.tg.{targets}"))
    };
    let mut v = vec![when, who];
    if let Some(k) = m.get("hotkey").and_then(Value::as_i64) {
        v.push(format!("Ctrl+Alt+{k}"));
    }
    v.join(" · ")
}

/// Which section a macro is in: running now, running by itself (a join, a rejoin, a time), by hand.
fn macro_group(m: &Value, live_runs: bool) -> u8 {
    if live_runs {
        0
    } else if m.pointer("/trigger/kind").and_then(Value::as_str).unwrap_or("manual") != "manual" {
        1
    } else {
        2
    }
}

fn macro_list(root: &Root, macros: &[Value], runs: &[Value], narrow: bool, wide: bool, width: f32, cx: &mut Context<Root>) -> Div {
    let mut body = div().flex().flex_col().gap(px(SECTION_GAP)).px(px(SIDE)).pt(px(8.)).pb(px(32.));
    if let Some(rec) = root.state.get("macroRecording").filter(|v| v.is_object()) {
        body = body.child(recording(root, rec, cx));
    }
    if macros.is_empty() {
        return body.child(macro_start(root, width, cx));
    }
    let by_id: HashMap<&str, &Value> = root.accounts().iter().map(|a| (s(a, "id"), a)).collect();
    let panels: HashSet<&str> = root.state.get("panelsShown").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    let can_run: HashSet<&str> = root.state.get("macroCanRun").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    // A row being dragged: the list in the order it has now.
    let mut order: Vec<&Value> = macros.iter().collect();
    if let Some(d) = root.auto.drag.as_ref().filter(|d| d.macros) {
        order.sort_by_key(|m| d.order.iter().position(|x| x == s(m, "id")).unwrap_or(usize::MAX));
    }
    let ids: Vec<String> = macros.iter().map(|m| s(m, "id").to_string()).collect();
    let mine = |id: &str| -> Vec<&Value> { runs.iter().filter(|r| s(r, "macroId") == id).collect() };
    // In sections (running now, by itself, by hand) once there is more than one kind; a row moves within its own.
    let titles = ["au.grp.running", "au.grp.auto", "macro.trig.manual"];
    let groups: Vec<(u8, Vec<&Value>)> = (0..3u8)
        .map(|g| (g, order.iter().copied().filter(|m| macro_group(m, mine(s(m, "id")).iter().any(|r| live(r))) == g).collect::<Vec<_>>()))
        .filter(|(_, l)| !l.is_empty())
        .collect();
    let single = groups.len() == 1;
    let mut k = 0;
    for (g, items) in groups {
        let n = items.len();
        let mut rows = Vec::new();
        for m in items {
            let id = s(m, "id");
            rows.push(macro_row(root, m, k, g, &ids, &mine(id), &by_id, panels.contains(id), can_run.contains(id), narrow, wide, cx).into_any_element());
            k += 1;
        }
        body = body.child(list_section(if single { root.tr("macro.title") } else { root.tr(titles[g as usize]) }, n, rows));
    }
    body.on_mouse_up(MouseButton::Left, cx.listener(|r, _, _, cx| drag_end(r, cx))).on_mouse_up_out(MouseButton::Left, cx.listener(|r, _, _, cx| drag_end(r, cx)))
}

/// The recording under way: its countdown, then how many steps so far; F8 or Stop ends it.
fn recording(root: &Root, rec: &Value, cx: &mut Context<Root>) -> impl IntoElement {
    let name = root.accounts().iter().find(|a| s(a, "id") == s(rec, "accountId")).map(crate::pages::accounts::label).unwrap_or_default();
    let left = crate::time::iso_secs(s(rec, "startsAt")).map_or(0, |t| t - crate::time::now_secs());
    let text = if left > 0 {
        root.i18n.tv("macro.rec.starting", &[("name", &name), ("n", &left.to_string())])
    } else {
        root.i18n.tv("macro.rec.on", &[("name", &name), ("n", &int(rec, "steps").to_string())])
    };
    div()
        .min_h(px(56.))
        .px(px(16.))
        .py(px(10.))
        .rounded(px(R_L + 2.))
        .bg(tinted(PANEL, BAD, 0.08))
        .border_1()
        .border_color(ca(BAD, 0.28))
        .flex()
        .items_center()
        .gap(px(12.))
        .child(div().size(px(10.)).flex_none().rounded_full().bg(c(BAD)))
        .child(div().flex_none().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).child(text))
        .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("macro.rec.hint")))
        .child(Button::new("mc-rec-stop", root.tr("macro.rec.stop")).icon("stop").small().on_click(cx.listener(|r, _, _, cx| {
            r.ask("stopRecording", json!({}), cx, |_, _| {})
        })))
}

#[allow(clippy::too_many_arguments)]
fn macro_row(
    root: &Root,
    m: &Value,
    i: usize,
    group: u8,
    ids: &[String],
    runs: &[&Value],
    by_id: &HashMap<&str, &Value>,
    shown: bool,
    can_run: bool,
    narrow: bool,
    wide: bool,
    cx: &mut Context<Root>,
) -> impl IntoElement {
    let id = s(m, "id").to_string();
    let auto = m.pointer("/trigger/kind").and_then(Value::as_str).unwrap_or("manual") != "manual";
    let enabled = m.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let live_runs = runs.iter().filter(|r| live(r)).count();
    let failed = runs.iter().any(|r| s(r, "status") == "failed");
    let targets = s(m, "targets");
    let accs = m.get("accountIds").and_then(Value::as_array).map_or(0, |l| l.iter().filter_map(Value::as_str).filter(|x| by_id.contains_key(x)).count());
    let sel: Vec<String> = root.selected.iter().filter(|x| by_id.contains_key(x.as_str())).cloned().collect();
    let cant = (targets == "selected" && sel.is_empty() && !can_run) || (targets == "accounts" && accs == 0);

    // One line: each account's run with a dot in its colour, or (none) when it runs and on whom.
    let line = if runs.is_empty() {
        let mut parts: Vec<(String, Option<Hsla>)> = when_text(root, m).split(" · ").map(|x| (x.to_string(), None)).collect();
        if auto && !enabled {
            parts.push((root.tr("macro.autoOff"), Some(c(WARN))));
        }
        facts(parts)
    } else {
        let mut d = div().flex().items_center().gap(px(14.)).overflow_hidden().text_size(px(T_M));
        for r in runs {
            let name = by_id.get(s(r, "accountId")).map(|a| crate::pages::accounts::label(a)).unwrap_or_else(|| "?".into());
            let col = match s(r, "status") {
                "running" => OK,
                "paused" | "waiting" => WARN,
                "failed" => BAD,
                _ => DIM,
            };
            d = d.child(
                div()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_color(if s(r, "status") == "failed" { c(BAD) } else { c(TEXT_2) })
                    .child(div().flex_none().size(px(7.)).rounded_full().bg(c(col)))
                    .child(div().min_w_0().truncate().child(run_text(root, r, &name))),
            );
        }
        d
    };

    // Its block: a running one green, a stopped run's red dot on its corner.
    let tile = {
        div()
            .relative()
            .size(px(26.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(icon(if auto { "repeat" } else { "keyboard" }, 22., if live_runs > 0 { c(OK) } else { c(TEXT_2) }))
            .when(failed, |d| {
                let tip: SharedString = root.tr("macro.fixTip").into();
                d.child(div().id(("mc-bad", i)).absolute().top(px(-3.)).right(px(-3.)).size(px(11.)).rounded_full().border_2().border_color(c(PANEL)).bg(c(BAD)).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
            })
    };
    // Where its run stopped, for the menu's Fix (MacrosView.tsx's editAt: the step, in its list).
    let stopped_at = runs.iter().find(|r| s(r, "status") == "failed").map(|r| {
        let step = r.get("lastStep").and_then(Value::as_u64).or_else(|| r.get("step").and_then(Value::as_u64)).map(|x| x as usize);
        (step, Some(s(r, "section").to_string()).filter(|x| !x.is_empty()))
    });

    // Its panel beside the game, shown or not; a narrow window keeps only its icon.
    let panel = {
        let mid = id.clone();
        let toggle = move |r: &mut Root, cx: &mut Context<Root>| r.ask(if shown { "hidePanel" } else { "showPanel" }, json!({ "id": mid }), cx, |_, _| {});
        if narrow {
            IconButton::new(("mc-panel", i), "screens")
                .size(30.)
                .on(shown)
                .tip(root.tr(if shown { "macro.panelHide" } else { "macro.panelShow" }))
                .on_click(cx.listener(move |r, _, _, cx| toggle(r, cx)))
                .into_any_element()
        } else {
            // Shown: the panel's icon stays (a lone green dot in its place read as an unexplained mark, review 5.2),
            // in green, and the words say it is on; the filled "picked" look read as a choice nobody explained.
            let tip: SharedString = root.tr(if shown { "macro.panelHide" } else { "macro.panelShow" }).into();
            tool(("mc-panel", i), (!shown).then_some("screens"), false)
                .when(shown, |d| d.text_color(c(OK)).child(icon("screens", 15., c(OK))))
                .child(root.tr(if shown { "macro.panelOn" } else { "macro.panel" }))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .on_click(cx.listener(move |r, _, _, cx| toggle(r, cx)))
                .into_any_element()
        }
    };
    let run = if live_runs > 0 {
        let mid = id.clone();
        Button::new(("mc-stop", i), root.tr("macro.stop")).icon("stop").small().on_click(cx.listener(move |r, _, _, cx| {
            r.ask("stopMacros", json!({ "id": mid }), cx, |r, _| r.ok(r.tr("macro.stopped")))
        }))
    } else {
        let mid = id.clone();
        let on = (targets == "selected" && !sel.is_empty()).then(|| sel.clone());
        Button::new(("mc-run", i), root.tr("macro.run")).icon("play").small().disabled(cant).on_click(cx.listener(move |r, _, _, cx| {
            r.run_macro(mid.clone(), on.clone(), cx)
        }))
    };

    let count = ids.len();
    let held = root.auto.drag.as_ref().is_some_and(|d| d.macros && d.id == id);
    let handle = (count > 1).then(|| {
        let (mid, all) = (id.clone(), ids.to_vec());
        grip(root, "mc-grip", i, held, cx, move |r| r.auto.drag = Some(Drag { macros: true, group, id: mid.clone(), order: all.clone() }))
    });
    let steps = m.get("steps").and_then(Value::as_array).map_or(0, Vec::len) as i64;

    follow_drag(row_shell(("mc-row", i)), id.clone(), group, cx)
        .pl(px(if handle.is_some() { 4. } else { 12. }))
        .when(held, |d| d.opacity(0.6))
        .children(handle)
        .child(tile)
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
                        .items_baseline()
                        .gap(px(10.))
                        .min_w_0()
                        .child(div().flex_shrink(1.).min_w_0().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(s(m, "name").to_string()))
                        .child(div().flex_none().text_size(px(T_M)).text_color(c(MUTED)).font_features(tnum()).child(root.i18n.tn("au.stepsN", steps, &[]))),
                )
                .child(line),
        )
        .when(!narrow, |d| d.child(div().flex_none().w(px(if wide { 300. } else { 200. })).flex().justify_end().child(shape(root, m, i, if wide { 300. } else { 200. }, runs.iter().find(|r| s(r, "status") == "running").and_then(|r| r.get("step")).and_then(Value::as_u64).map(|x| x as usize)))))
        .child(div().flex_none().w(px(if narrow { 34. } else { 124. })).flex().justify_end().child(panel))
        .child(div().flex_none().w(px(84.)).flex().justify_end().child(run))
        // (Its place in the whole list, not in its section: Up and Down move it there.)
        .child(more_menu(root, m, ids.iter().position(|x| *x == id).unwrap_or(i), count, stopped_at, cx))
}

type MenuAct = Box<dyn Fn(&mut Root, &mut Window, &mut Context<Root>)>;

/// A macro's "…": Fix (its last run stopped: the editor at that step), Edit, Duplicate, Export, pause or resume (one
/// that runs by itself), its place, Delete (MacrosView.tsx's row menu). A second click on the button, or a click
/// outside, closes it.
fn more_menu(root: &Root, m: &Value, i: usize, count: usize, stopped_at: Option<(Option<usize>, Option<String>)>, cx: &mut Context<Root>) -> Div {
    let id = s(m, "id").to_string();
    let open = root.auto.menu.as_deref() == Some(id.as_str());
    let auto = m.pointer("/trigger/kind").and_then(Value::as_str).unwrap_or("manual") != "manual";
    let enabled = m.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let button = {
        let mid = id.clone();
        IconButton::new(("mc-more", i), "more").size(32.).selected(open).tip(root.tr("macro.moreActs")).on_click(cx.listener(move |r, _, _, cx| {
            let just = r.auto.menu_closed.as_ref().is_some_and(|(k, at)| *k == mid && at.elapsed().as_millis() < 300);
            r.auto.menu_closed = None;
            r.auto.menu = if just || r.auto.menu.as_deref() == Some(mid.as_str()) { None } else { Some(mid.clone()) };
            cx.notify();
        }))
    };
    let wrap = div().relative().flex_none().child(button);
    if !open {
        return wrap;
    }
    // (None: a hairline between groups.)
    let mut items: Vec<Option<(&'static str, String, bool, bool, MenuAct)>> = Vec::new();
    if let Some((step, sec)) = stopped_at {
        let whole = m.clone();
        items.push(Some(("wrench", root.tr("macro.fix"), false, false, Box::new(move |r, window, cx| crate::pages::macro_form::open_at(r, whole.clone(), step, sec.clone(), window, cx)))));
    }
    {
        let whole = m.clone();
        items.push(Some(("edit", root.tr("macro.edit"), false, false, Box::new(move |r, window, cx| crate::pages::macro_form::open(r, Some(whole.clone()), window, cx)))));
    }
    {
        let mid = id.clone();
        items.push(Some(("copy", root.tr("macro.duplicate"), false, false, Box::new(move |r, _, cx| r.ask("duplicateMacro", json!({ "id": mid }), cx, |r, _| r.ok(r.tr("macro.duplicated")))))));
    }
    {
        let mid = id.clone();
        items.push(Some((
            "download",
            root.tr("macro.export"),
            false,
            false,
            Box::new(move |r, _, cx| {
                r.ask("exportMacro", json!({ "id": mid }), cx, |r, v| {
                    if v.as_bool().unwrap_or(!v.is_null()) {
                        r.ok(r.tr("macro.exported"));
                    }
                })
            }),
        )));
    }
    if auto {
        let mut changed = m.clone();
        changed["enabled"] = json!(!enabled);
        items.push(Some((
            if enabled { "pause" } else { "play" },
            root.tr(if enabled { "macro.autoPause" } else { "macro.autoResume" }),
            false,
            false,
            Box::new(move |r, _, cx| r.ask("saveMacro", json!({ "macro": changed }), cx, |_, _| {})),
        )));
    }
    if count > 1 {
        items.push(None);
        for (dir, mark, key) in [(-1_i32, "chevronUp", "lo.up"), (1, "chevronDown", "lo.down")] {
            let mid = id.clone();
            let to = i as i32 + dir;
            let off = to < 0 || to as usize >= count;
            items.push(Some((mark, root.tr(key), off, false, Box::new(move |r, _, cx| r.ask("moveMacro", json!({ "id": mid, "to": to }), cx, |_, _| {})))));
        }
    }
    items.push(None);
    {
        let mid = id.clone();
        let (title, text) = (root.tr("macro.deleteTitle"), root.i18n.tv("macro.deleteQ", &[("name", s(m, "name"))]));
        items.push(Some((
            "trash",
            root.tr("macro.delete"),
            false,
            true,
            Box::new(move |r, window, cx| {
                let mid = mid.clone();
                let yes = r.tr("ps.delete");
                r.confirm(title.clone(), text.clone(), yes, true, None, window, cx, move |r, cx| r.ask("deleteMacro", json!({ "id": mid }), cx, |r, _| r.ok(r.tr("macro.deleted"))));
            }),
        )));
    }
    let mut menu = div()
        .id(("mc-menu", i))
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
        .on_mouse_down_out({
            let mid = id.clone();
            cx.listener(move |r, _, _, cx| {
                if r.auto.menu.as_deref() == Some(mid.as_str()) {
                    r.auto.menu = None;
                    r.auto.menu_closed = Some((mid.clone(), std::time::Instant::now()));
                    cx.notify();
                }
            })
        });
    for (k, item) in items.into_iter().enumerate() {
        let Some((mark, label, off, danger, act)) = item else {
            menu = menu.child(div().my(px(4.)).mx(px(6.)).h(px(1.)).bg(c(LINE)));
            continue;
        };
        let tone = if danger { c(BAD) } else if off { c(DIM) } else { c(TEXT_2) };
        let row = menu_row(("mc-menu-i", i * 32 + k)).text_color(tone).child(icon(mark, 15., tone)).child(div().flex_1().whitespace_nowrap().child(label));
        menu = menu.child(if off {
            row.cursor_default().hover(|d| d.text_color(c(DIM))).into_any_element()
        } else {
            row.when(danger, |d| d.hover(|d| d.text_color(c(BAD))))
                .on_click(cx.listener(move |r, _, window, cx| {
                    r.auto.menu = None;
                    r.auto.confirm = None;
                    act(r, window, cx);
                    cx.notify();
                }))
                .into_any_element()
        });
    }
    wrap.child(popup(PopAt::BelowRight(36.), SharedString::from(format!("mc-pop-{id}")), open, menu))
}

/// A macro at a glance, as a track: its steps in order on one line, a key's own label, the others their mark, joined by
/// a thread; a wait is no chip of its own but the thread's length between them (longer for a longer wait, its seconds
/// on it), so the macro's rhythm shows: a burst of keys, then a long pause. A block's end a short blue bar. The step a
/// run is on is ringed green.
/// Each mark says what it is under the mouse ("Press a key: W", "Wait: 2s").
fn shape(root: &Root, m: &Value, row_i: usize, max_w: f32, at: Option<usize>) -> Div {
    let steps: &[Value] = m.get("steps").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    // A value set by one of the macro's choices ("$o2") shows the choice's value.
    let value = |v: Option<&Value>| -> Option<Value> {
        let v = v?;
        match v.as_str().and_then(|x| x.strip_prefix('$')) {
            Some(name) => m.get("options").and_then(Value::as_array)?.iter().find(|o| s(o, "name") == name).and_then(|o| o.get("value")).cloned(),
            None => Some(v.clone()),
        }
    };
    let thread = |w: f32| div().flex_none().w(px(w)).h(px(26.)).flex().items_center().child(div().w_full().h(px(1.)).bg(ca(TEXT, 0.22)));
    let mut row = div().flex_none().max_w(px(max_w)).flex().items_center().overflow_hidden();
    // Whether the last thing put down was a mark (the next mark is joined to it by a short thread).
    let mut after_mark = false;
    for (k, st) in steps.iter().take(SHAPE_MAX).enumerate() {
        let kind = s(st, "kind");
        let chip_id = SharedString::from(format!("mc-sh-{row_i}-{k}"));
        let what = root.tr(&format!("macro.kind.{kind}"));
        if kind == "wait" {
            let ms = value(st.get("ms")).and_then(|v| v.as_f64()).unwrap_or(0.);
            let secs = format!("{}s", (ms / 100.).round() / 10.);
            // 0.5 s a short thread, each doubling a little longer (a 30 s wait doesn't take the whole line).
            let w = (18. + 9. * (1. + ms as f32 / 500.).log2()).clamp(18., 64.);
            let tip: SharedString = format!("{}: {secs}", what.trim_end_matches('…')).into();
            row = row.child(
                div()
                    .id(chip_id)
                    .flex_none()
                    .w(px(w))
                    .h(px(26.))
                    .relative()
                    .child(thread(w))
                    .child(
                        div()
                            .absolute()
                            .top(px(-3.))
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .text_size(px(10.))
                            .font_features(tnum())
                            .text_color(c(MUTED))
                            .child(secs),
                    )
                    .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
            );
            after_mark = false;
            continue;
        }
        if after_mark {
            row = row.child(thread(8.));
        }
        after_mark = true;
        if kind == "endIf" || kind == "endLoop" {
            let tip: SharedString = what.into();
            row = row.child(
                div()
                    .id(chip_id)
                    .px(px(1.))
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .child(div().w(px(3.)).h(px(16.)).rounded(px(2.)).bg(ca(ONLINE, 0.6)))
                    .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
            );
            continue;
        }
        let label = match kind {
            "key" | "down" | "up" => value(st.get("key")).and_then(|k| k.as_str().map(|k| key_name(root, k))),
            "loop" => Some(match st.get("times").and_then(Value::as_i64) {
                Some(n) if n > 0 => format!("×{n}"),
                _ => "∞".into(),
            }),
            _ => None,
        };
        let opens = matches!(kind, "if" | "loop" | "loopUntil");
        let now = at == Some(k);
        let tip: SharedString = match &label {
            Some(l) => format!("{}: {l}", what.trim_end_matches('…')),
            None => what,
        }
        .into();
        let tone = if now {
            c(OK)
        } else if opens {
            c(ONLINE)
        } else {
            c(TEXT_2)
        };
        row = row.child(
            div()
                .id(chip_id)
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .h(px(24.))
                .min_w(px(24.))
                .px(px(5.))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .gap(px(3.))
                .rounded(px(6.))
                .bg(ca(WHITE, 0.06))
                .when(now, |d| d.border_1().border_color(c(OK)))
                .text_size(px(T_S))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tone)
                .when(label.is_none() || kind == "loop", |d| d.child(icon(step_icon(kind), 13., tone)))
                .when_some(label, |d, l| d.child(l)),
        );
    }
    if steps.len() > SHAPE_MAX {
        row = row.child(thread(8.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(format!("+{}", steps.len() - SHAPE_MAX)));
    }
    row
}

fn key_name(root: &Root, k: &str) -> String {
    if matches!(k, "LButton" | "RButton" | "MButton") {
        return root.tr(&format!("macro.mk.{k}"));
    }
    k.to_string()
}

