// How Arrange windows lays the Roblox windows out, picked from small screen pictures like Windows' snap layouts (the
// web page's ui/src/Snap.tsx): "fill" (Auto), grids whose cells are the windows' size whatever their number, and a size
// of one's own. Pointing at one lights the cells the open windows would take. It is in Accounts' Windows menu (the web
// dock's: Open Roblox only, hide or show them all, then the layouts; picking one arranges at once, on the screen asked
// when there are several) and in Settings › Launching. The commands are the app's: setSettings { windowLayout },
// arrangeWindows { monitor }.
use std::time::Instant;

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{kit::*, shell::Root, theme::*};

/// The layouts in the order the web page lists them; "custom" (a size of one's own) comes after.
pub const LAYOUTS: [&str; 8] = ["fill", "2x1", "3x1", "2x2", "3x2", "3x3", "4x3", "4x4"];

/// The Windows menu: open, still built while it fades out, closed by a click outside just now (that click on its own
/// button must not open it again), and on its second step (which screen, when there are several).
#[derive(Default)]
pub struct Data {
    pub open: bool,
    closed: Option<Instant>,
    screens: bool,
}

/// A grid's columns and rows ("3x2"); "fill" as many as the windows need (one at least); None for a size of one's own.
fn grid_of(layout: &str, count: usize) -> Option<(usize, usize)> {
    if layout == "fill" {
        let cols = ((count as f64).sqrt().ceil() as usize).max(1);
        return Some((cols, count.div_ceil(cols).max(1)));
    }
    let (a, b) = layout.split_once('x')?;
    let (cols, rows) = (a.parse::<usize>().ok()?, b.parse::<usize>().ok()?);
    ((1..=8).contains(&cols) && (1..=8).contains(&rows)).then_some((cols, rows))
}

/// A layout's name under its picture: Auto, "3 × 2", Custom.
pub fn name_of(root: &Root, layout: &str) -> String {
    match layout {
        "fill" => root.tr("snap.fill"),
        "custom" => root.tr("snap.customName"),
        g => g.replace('x', " × "),
    }
}

/// The layout kept in the settings, and the size of one's own (Settings › Launching).
fn kept(root: &Root) -> (String, i64, i64) {
    let st = root.state.get("settings");
    let layout = st.and_then(|s| s.get("windowLayout")).and_then(Value::as_str).filter(|l| !l.is_empty()).unwrap_or("fill").to_string();
    let num = |k: &str, or: i64| st.and_then(|s| s.get(k)).and_then(Value::as_i64).unwrap_or(or);
    (layout, num("windowWidth", 640), num("windowHeight", 480))
}

/// What picking a layout does.
pub type Pick = std::rc::Rc<dyn Fn(&mut Root, &'static str, &mut Window, &mut Context<Root>)>;

/// The pictures: `cols` to a row, each `tile` wide. `value` is the layout chosen (ringed); `w`×`h` the size of one's
/// own, drawn as one window of that size in the corner of a 1920 × 1080 screen.
pub fn picker(root: &Root, key: &'static str, value: &str, w: i64, h: i64, cols: usize, tile: f32, pick: Pick, cx: &mut Context<Root>) -> Div {
    let count = root.metrics.get("robloxCount").and_then(Value::as_i64).unwrap_or(0).max(0) as usize;
    let screen_w = tile - 12. - 2.;
    let screen_h = (screen_w * 10. / 16.).round();
    let inner_w = screen_w - 8.;
    let inner_h = screen_h - 8.;
    let all = LAYOUTS.iter().copied().chain(std::iter::once("custom"));
    let tiles: Vec<AnyElement> = all
        .enumerate()
        .map(|(i, layout)| {
            let current = value == layout;
            let group = SharedString::from(format!("{key}-{layout}"));
            let lit = |d: Div, on: bool| d.when(on, |d| d.group_hover(group.clone(), |s| s.bg(c(TEXT_2))));
            let screen = div().w(px(screen_w)).h(px(screen_h)).flex_none().p(px(3.)).rounded(px(6.)).border_1().border_color(ca(WHITE, 0.1)).bg(c(GROUND));
            let screen = match grid_of(layout, count) {
                Some((gc, gr)) => {
                    let on = count.min(gc * gr);
                    let (cw, ch) = ((inner_w - 2. * (gc as f32 - 1.)) / gc as f32, (inner_h - 2. * (gr as f32 - 1.)) / gr as f32);
                    let fill = layout == "fill";
                    screen.child(div().size_full().flex().flex_col().gap(px(2.)).children((0..gr).map(|row| {
                        div().flex().gap(px(2.)).children((0..gc).map(|col| {
                            let n = row * gc + col;
                            let cell = div().w(px(cw)).h(px(ch)).flex_none().rounded(px(2.));
                            // Auto's cells are only outlines: the windows take whatever room there is.
                            let cell = if fill {
                                cell.border_1().border_dashed().border_color(ca(WHITE, 0.17)).when(n < on, |d| d.group_hover(group.clone(), |s| s.border_color(c(TEXT_2))))
                            } else {
                                lit(cell.bg(ca(WHITE, 0.17)), n < on)
                            };
                            cell
                        }))
                    })))
                }
                // A size of one's own: one window of that size in the corner (as wide a share of a 1920 × 1080 screen).
                None => {
                    let ow = (w as f32 / 1920.).clamp(0.05, 1.) * inner_w;
                    let oh = (h as f32 / 1080.).clamp(0.05, 1.) * inner_h;
                    screen.child(lit(div().w(px(ow)).h(px(oh)).rounded(px(2.)).bg(ca(WHITE, 0.17)), count > 0))
                }
            };
            let tip: SharedString = if layout == "custom" { format!("{} · {}", root.tr("snap.customName"), root.i18n.tv("snap.custom", &[("w", &w.to_string()), ("h", &h.to_string())])) } else { name_of(root, layout) }.into();
            let pick = pick.clone();
            hot((key, i))
                .group(group.clone())
                .w(px(tile))
                .flex_none()
                .p(px(6.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(5.))
                .rounded(px(R_M + 1.))
                .border_1()
                // The chosen layout is the solid yellow block (1.1's "chosen" look), its little screen dark on it.
                .map(|d| if current { d.tones(c(FACE), c(FACE_HI)).edge(ca(FACE, 0.), ca(FACE, 0.)).text_color(c(INK)).font_weight(FontWeight::SEMIBOLD) } else { d.tones(ca(WHITE, 0.), ca(WHITE, 0.06)).edge(ca(WHITE, 0.), ca(WHITE, 0.)).text_color(c(TEXT_2)).hover(|d| d.text_color(c(TEXT))) })
                .cursor_pointer()
                .text_size(px(T_S))
                .child(screen)
                .child(div().w_full().text_center().truncate().child(name_of(root, layout)))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .on_click(cx.listener(move |r, _, window, cx| pick(r, layout, window, cx)))
                .into_any_element()
        })
        .collect();
    let mut grid = div().flex().flex_col().gap(px(4.));
    let mut tiles = tiles.into_iter().peekable();
    while tiles.peek().is_some() {
        grid = grid.child(div().flex().gap(px(4.)).children(tiles.by_ref().take(cols.max(1))));
    }
    grid
}

/// "Arranged 3 Roblox windows: grid 2 × 2." — the layout said too: a 4 × 4 grid chosen long ago makes two windows
/// small, with nothing else telling why (app.js's arrangedText).
fn arranged_text(r: &Root, n: i64, layout: &str) -> String {
    let (_, w, h) = kept(r);
    let how = match layout {
        "fill" => Some(r.tr("snap.fill")),
        "custom" => Some(r.i18n.tv("snap.custom", &[("w", &w.to_string()), ("h", &h.to_string())])),
        g if grid_of(g, 1).is_some() => Some(r.i18n.tv("snap.grid", &[("g", &g.replace('x', " × "))])),
        _ => None,
    };
    match how {
        Some(how) => r.i18n.tn("t.arrangedAs", n, &[("n", &n.to_string()), ("how", &how)]),
        None => r.i18n.tn("t.arranged", n, &[("n", &n.to_string())]),
    }
}

/// Every Roblox window laid out on this screen (its index), and how many were, in a toast.
pub fn arrange(r: &mut Root, monitor: i64, layout: String, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call("arrangeWindows", json!({ "monitor": monitor }));
    cx.spawn(async move |this, cx| {
        let answer = rx.await;
        let _ = this.update(cx, |r, cx| {
            match answer {
                Ok(Ok(v)) => {
                    let n = v.as_i64().unwrap_or(0);
                    r.toast = Some(if n > 0 { (arranged_text(r, n, &layout), "ok".into()) } else { (r.tr("t.noWindows"), "info".into()) });
                }
                Ok(Err(e)) => r.toast = Some((e, "bad".into())),
                Err(_) => {}
            }
            cx.notify();
        });
    })
    .detach();
}

/// Every Roblox window laid out on the main screen with the layout kept (the palette's Arrange, app.js's arrange(null)).
pub fn arrange_main(r: &mut Root, cx: &mut Context<Root>) {
    let main = screens(r).into_iter().find(|s| s.3).map_or(0, |s| s.0);
    let (layout, _, _) = kept(r);
    arrange(r, main, layout, cx);
}

/// The screens the app knows: (index, width, height, main).
fn screens(root: &Root) -> Vec<(i64, i64, i64, bool)> {
    root.state
        .get("monitors")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .map(|m| {
                    let n = |k: &str| m.get(k).and_then(Value::as_i64).unwrap_or(0);
                    (n("index"), n("width"), n("height"), m.get("primary").and_then(Value::as_bool) == Some(true))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A layout picked in the menu: kept first (when it changed), then the windows arranged with it; with several screens
/// the menu asks which one.
fn picked(r: &mut Root, layout: &'static str, cx: &mut Context<Root>) {
    let (was, _, _) = kept(r);
    let several = screens(r).len() > 1;
    if several {
        r.snap.screens = true;
    } else {
        r.snap.open = false;
    }
    let Some(link) = r.link.clone() else { return };
    let saved = (layout != was).then(|| link.call("setSettings", json!({ "windowLayout": layout })));
    cx.notify();
    if several {
        // (Saved meanwhile: the screen is picked next.)
        drop(saved);
        return;
    }
    let main = screens(r).into_iter().find(|s| s.3).map_or(0, |s| s.0);
    cx.spawn(async move |this, cx| {
        // (The layout is saved before the windows are arranged with it.)
        if let Some(rx) = saved {
            let _ = rx.await;
        }
        let _ = this.update(cx, |r, cx| arrange(r, main, layout.to_string(), cx));
    })
    .detach();
}

/// Accounts' Windows button and its menu (the web dock's "Windows").
pub fn button(root: &Root, compact: bool, cx: &mut Context<Root>) -> Div {
    let open = root.snap.open;
    let key = "acc-windows";
    // A quiet tool as the toolbar's others; short of room, its icon alone (its name is the tip then too).
    let tip: SharedString = root.tr("dock.moreTip").into();
    let mut wrap = div().relative().flex_none().child(
        tool(key, Some("screens"), open)
            .when(!compact, |d| d.child(root.tr("dock.more")))
            .child(tool_chev())
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            .on_click(cx.listener(|r, _, _, cx| {
                // A click outside the open menu closed it just now: this click on its own button only closes it.
                let just = r.snap.closed.take().is_some_and(|at| at.elapsed().as_millis() < 300);
                r.snap.open = !just && !r.snap.open;
                r.snap.screens = false;
                cx.notify();
            })),
    );
    // (Built while open or fading out only, not every frame after its first opening.)
    if crate::pages::accounts::pop_alive("acc-windows-pop", open) {
        wrap = wrap.child(div().absolute().top(px(TOOL_H + 4.)).right_0().child(deferred(anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(pop("acc-windows-pop", open, menu(root, cx)))).with_priority(1)));
    }
    wrap
}

fn menu(root: &Root, cx: &mut Context<Root>) -> Stateful<Div> {
    let shell = div()
        .id("acc-windows-menu")
        .occlude()
        .w(px(340.))
        .p(px(6.))
        .rounded(px(R_L))
        .bg(c(RAISED))
        .border_1()
        .border_color(ca(WHITE, 0.1))
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            if r.snap.open {
                r.snap.open = false;
                r.snap.closed = Some(Instant::now());
                cx.notify();
            }
        }));
    // Which screen: the second step, once a layout is picked and there are several.
    if root.snap.screens {
        let (layout, _, _) = kept(root);
        let mut m = shell.child(div().px(px(8.)).pt(px(4.)).pb(px(8.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("m.arrange")));
        for (i, (index, w, h, main)) in screens(root).into_iter().enumerate() {
            let mut text = root.i18n.tv("m.monitor", &[("n", &(index + 1).to_string()), ("w", &w.to_string()), ("h", &h.to_string())]);
            if main {
                text = format!("{text} · {}", root.tr("m.primary"));
            }
            let layout = layout.clone();
            m = m.child(row(("snap-screen", i), "monitor", text).on_click(cx.listener(move |r, _, _, cx| {
                r.snap.open = false;
                arrange(r, index, layout.clone(), cx);
                cx.notify();
            })));
        }
        return m;
    }
    let picked_ids: Vec<String> = root.accounts().iter().filter_map(|a| a.get("id").and_then(Value::as_str)).filter(|id| root.selected.contains(*id)).map(str::to_string).collect();
    let any = !picked_ids.is_empty();
    let all = root.accounts();
    let can_hide = all.iter().any(|a| crate::pages::accounts::is_local(a) && a.pointer("/session/hidden").and_then(Value::as_bool) != Some(true));
    let can_show = all.iter().any(|a| a.pointer("/session/hidden").and_then(Value::as_bool) == Some(true));
    // Roblox's own app for the picked accounts, without a game: off with none picked, and says so.
    let mut open_app = row("snap-app", "monitor", root.tr("dock.openAppTip"));
    if any {
        open_app = open_app.on_click(cx.listener(move |r, _, _, cx| {
            r.snap.open = false;
            r.launch("launch", json!({ "ids": picked_ids, "appOnly": true, "placeInput": "", "jobId": "" }), false, cx);
            cx.notify();
        }));
    } else {
        let tip: SharedString = root.tr("dock.pick").into();
        open_app = open_app.opacity(0.38).cursor_default().tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into());
    }
    let mut m = shell.child(open_app);
    if can_hide {
        m = m.child(row("snap-hide", "eyeOff", root.tr("m.hideAll")).on_click(cx.listener(|r, _, _, cx| {
            r.snap.open = false;
            r.send("hideWindows", json!({}));
            cx.notify();
        })));
    }
    if can_show {
        m = m.child(row("snap-show", "eye", root.tr("m.showAll")).on_click(cx.listener(|r, _, _, cx| {
            r.snap.open = false;
            r.send("showWindows", json!({}));
            cx.notify();
        })));
    }
    let (layout, w, h) = kept(root);
    let pick: Pick = std::rc::Rc::new(|r: &mut Root, layout: &'static str, _: &mut Window, cx: &mut Context<Root>| picked(r, layout, cx));
    m.child(div().h(px(1.)).my(px(6.)).mx(px(4.)).bg(c(LINE)))
        .child(div().px(px(8.)).pt(px(2.)).pb(px(8.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("m.arrange")))
        .child(div().px(px(1.)).pb(px(2.)).child(picker(root, "snap-menu", &layout, w, h, 3, 106., pick, cx)))
}

/// A line of the menu.
fn row(id: impl Into<ElementId>, icon_name: &'static str, label: String) -> Hot {
    menu_row(id).h(px(36.)).child(icon(icon_name, 15., c(TEXT_2))).child(div().flex_1().min_w_0().truncate().child(label))
}

/// A shot opens Accounts with the Windows menu open.
pub fn shot(r: &mut Root, cx: &mut Context<Root>) {
    r.go(crate::pages::Page::Accounts);
    r.snap.open = true;
    cx.notify();
}
