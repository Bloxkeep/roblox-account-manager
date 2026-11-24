// The tray icon's quick panel (left click) and menu (right click), drawn here in a window of their own: until
// 2026-11-19 they were a page (ui/src/Tray.tsx) in a WebView2 browser of its own, whose processes came and went beside
// Bloxkeep in Task Manager. The core's tray (core/tray_window.rs) still decides when it opens, where it goes, when it
// hides and what its clicks do; this draws what it is sent ("tray" events), says the size it drew, and hands its clicks
// back. The look is Tray.tsx's (app.css .tray-*), part for part.
use std::{cell::Cell, rc::Rc, time::Instant};

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{i18n::I18n, images, kit::*, shell::Root, theme::*};

/// More running windows than this: the last row says how many more (Tray.tsx's MAX_CLIENTS).
const MAX_CLIENTS: usize = 6;
const PANEL_W: f32 = 344.;
const MENU_W: f32 = 292.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Accounts,
    Presets,
    Macros,
    Multi,
    Close,
}

pub struct Tray {
    i18n: I18n,
    mode: String,
    opened: i64,
    state: Value,
    page: Page,
    /// "Close N windows?" until then, back to "Close all" after.
    confirm_until: Option<Instant>,
    focus: FocusHandle,
    /// What was last said to the core: the size drawn, the window's size, the opening.
    said: Rc<Cell<(i32, i32, i32, i32, i64)>>,
    _active: Option<Subscription>,
}

/// A "tray" event from the core: the tray's window drawn with it, made the first time.
pub fn show(r: &mut Root, data: &Value, cx: &mut Context<Root>) {
    let mode = data.get("mode").and_then(Value::as_str).unwrap_or("panel").to_string();
    let opened = data.get("opened").and_then(Value::as_i64).unwrap_or(0);
    let state = data.get("state").cloned().unwrap_or(Value::Null);
    // (Its words in the language the core says; the app's own words, copied once, when the window is made.)
    let lang = data.get("language").and_then(Value::as_str).map_or(r.i18n.lang, crate::lang::known);
    if let Some(h) = r.tray {
        let (m, s) = (mode.clone(), state.clone());
        if h.update(cx, move |t, window, cx| t.set(lang, m, opened, s, window, cx)).is_ok() {
            return;
        }
        r.tray = None;
    }
    let mut i18n = r.i18n.clone();
    i18n.set_language(lang);
    let options = window_options();
    match cx.open_window(options, move |window, cx| {
        let view = cx.new(|cx| Tray::new(i18n, mode, opened, state, window, cx));
        // The core places, shows and hides this window from now on.
        if let Some(h) = crate::hwnd_of(window) {
            to_core(move |t| t.attach(h));
        }
        view
    }) {
        Ok(h) => r.tray = Some(h),
        Err(e) => eprintln!("tray: its window couldn't be made: {e}"),
    }
}

/// The tray's window: a borderless pop-up (WS_POPUP: no frame, so what is drawn is the whole window), on top, not in
/// the taskbar, made hidden; the core gives it its place and size, and shows it.
fn window_options() -> WindowOptions {
    WindowOptions {
        kind: WindowKind::PopUp,
        titlebar: None,
        focus: false,
        show: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(PANEL_W), px(300.)) })),
        ..Default::default()
    }
}

/// Hands something to the core's tray, after this turn: the core moves, shows and hides this very window, which must
/// not happen inside its own drawing or click (Windows would call back into it there).
#[cfg(feature = "core")]
fn to_core(f: impl FnOnce(&Rc<crate::core::tray_window::TrayWindow>) + Send + 'static) {
    crate::core::win_form::invoke(move || {
        if let Some(t) = crate::core::tray_window::TrayWindow::current() {
            f(&t);
        }
    });
}
/// (Without the core, a shot: nothing to tell.)
#[cfg(not(feature = "core"))]
fn to_core(_f: impl FnOnce(&NoCore) + Send + 'static) {}
#[cfg(not(feature = "core"))]
struct NoCore;
#[cfg(not(feature = "core"))]
#[allow(dead_code)]
impl NoCore {
    fn attach(&self, _: isize) {}
    fn act(&self, _: &Value) {}
    fn on_deactivate(&self) {}
    fn on_size(&self, _: i32, _: i32, _: i32, _: i32, _: i32) {}
}

impl Tray {
    fn new(i18n: I18n, mode: String, opened: i64, state: Value, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        // Something else clicked: it goes (the core hides it). Shown: the keyboard on it, for Esc and Tab.
        let active = cx.observe_window_activation(window, |t, window, cx| {
            if window.is_window_active() {
                window.focus(&t.focus, cx);
            } else {
                to_core(|t| t.on_deactivate());
            }
        });
        Tray { i18n, mode, opened, state, page: Page::Home, confirm_until: None, focus, said: Rc::new(Cell::new((0, 0, 0, 0, -1))), _active: Some(active) }
    }

    /// What the core sent: a new opening starts on the first page; the refresh keeps the page, unless its list emptied.
    fn set(&mut self, lang: &str, mode: String, opened: i64, state: Value, window: &mut Window, cx: &mut Context<Self>) {
        if opened != self.opened || mode != self.mode {
            self.page = Page::Home;
            self.confirm_until = None;
        }
        self.i18n.set_language(lang);
        self.mode = mode;
        self.opened = opened;
        self.state = state;
        let open = self.flag("open");
        let empty = |k: &str| self.state.get(k).and_then(Value::as_array).is_none_or(Vec::is_empty);
        if (self.page == Page::Accounts && (!open || empty("accounts"))) || (self.page == Page::Presets && (!open || empty("presets"))) || (self.page == Page::Macros && (!open || empty("macros"))) {
            self.page = Page::Home;
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn flag(&self, k: &str) -> bool {
        self.state.get(k).and_then(Value::as_bool).unwrap_or(false)
    }
    fn num(&self, k: &str) -> i64 {
        self.state.get(k).and_then(Value::as_f64).unwrap_or(0.) as i64
    }
    fn list(&self, k: &str) -> Vec<Value> {
        self.state.get(k).and_then(Value::as_array).cloned().unwrap_or_default()
    }
    fn multi(&self) -> &str {
        self.state.get("multi").and_then(Value::as_str).unwrap_or("off")
    }
    /// Switched from here: with Multi-Roblox only, both ways; with a vault, only off (on asks first, in Bloxkeep).
    fn can_switch(&self, on: bool) -> bool {
        self.flag("multiOnly") || (on && self.flag("open"))
    }
    fn t(&self, k: &str) -> String {
        self.i18n.t(k)
    }
    fn tn(&self, k: &str, n: i64) -> String {
        self.i18n.tn(k, n, &[])
    }

    fn act(&mut self, a: Value) {
        to_core(move |t| t.act(&a));
    }

    fn go(&mut self, p: Page, cx: &mut Context<Self>) {
        self.page = p;
        self.confirm_until = None;
        cx.notify();
    }

    // ── Parts ──────────────────────────────────────────────────────────────────────────────────────────────────────
    /// Bloxkeep's logo and name, with what is going on under it (the menu's own line, with Multi-Roblox's light).
    fn head(&self, line: Option<String>, dot: bool, end: Option<AnyElement>) -> Div {
        let count = self.num("robloxCount");
        let status = line.unwrap_or_else(|| if count > 0 { self.i18n.tn("tray.status", count, &[("ram", &ram(self.num("ramMb")))]) } else { self.t("tray.none") });
        let dot_tone = match self.multi() {
            "ready" => c(OK),
            "notReady" => c(WARN),
            _ => c(MUTED),
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .min_w_0()
            .child(div().size(px(36.)).flex_none().rounded(px(11.)).overflow_hidden().child(crate::crisp::blocks(36.)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(div().text_size(px(15.)).line_height(px(19.)).font_weight(FontWeight::SEMIBOLD).child("Bloxkeep"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .min_w_0()
                            .text_size(px(T_S))
                            .text_color(c(TEXT_2))
                            .when(dot, |d| d.child(div().size(px(7.)).flex_none().rounded_full().bg(dot_tone)))
                            .child(div().min_w_0().truncate().child(status)),
                    ),
            )
            .children(end)
    }

    fn icon_button(&self, id: impl Into<ElementId>, name: &'static str, on: bool, small: bool, danger: bool, tip: String, cx: &mut Context<Self>, f: impl Fn(&mut Self, &mut Context<Self>) + 'static) -> Stateful<Div> {
        let s = if small { 30. } else { 34. };
        let tip: SharedString = tip.into();
        div()
            .id(id)
            .size(px(s))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(if small { 9. } else { 11. }))
            .cursor_pointer()
            .when(on, |d| d.bg(c(RAISED_2)))
            .hover(move |d| if danger { d.bg(ca(BAD, 0.16)) } else { d.bg(ca(WHITE, 0.07)) })
            .child(icon(name, if small { 16. } else { 18. }, if on { c(TEXT) } else { c(TEXT_2) }))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            .on_click(cx.listener(move |t, _, _, cx| f(t, cx)))
    }

    fn back(&self, label: String, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("tray-back")
            .flex()
            .items_center()
            .gap(px(6.))
            .w_full()
            .px(px(8.))
            .py(px(8.))
            .rounded(px(11.))
            .cursor_pointer()
            .font_weight(FontWeight::SEMIBOLD)
            .hover(|d| d.bg(ca(WHITE, 0.06)))
            .child(icon("chevronLeft", 16., c(TEXT_2)))
            .child(label)
            .on_click(cx.listener(|t, _, _, cx| t.go(Page::Home, cx)))
    }

    /// An account's face (its picture, or its initials).
    fn avatar(&self, name: &str, url: Option<&str>, size: f32, cx: &mut Context<Self>) -> Div {
        let pic = url.filter(|u| !u.is_empty()).and_then(|u| images::pic(u, cx));
        div().size(px(size)).flex_none().rounded_full().overflow_hidden().child(face(name, pic, size).rounded_full())
    }

    /// The accounts to play as (a running one can't be picked: "Playing").
    fn account_rows(&self, small: bool, cx: &mut Context<Self>) -> Div {
        let mut list = div().flex().flex_col().gap(px(2.));
        for (i, a) in self.list("accounts").iter().enumerate() {
            let name = a.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
            let id = a.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
            let running = a.get("running").and_then(Value::as_bool).unwrap_or(false);
            let face = self.avatar(&name, a.get("avatar").and_then(Value::as_str), if small { 26. } else { 32. }, cx);
            list = list.child(
                div()
                    .id(("tray-acc", i))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .w_full()
                    .min_h(px(if small { 38. } else { 44. }))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(11.))
                    .when(running, |d| d.opacity(0.55))
                    .when(!running, |d| {
                        d.cursor_pointer().hover(|d| d.bg(ca(WHITE, 0.06))).on_click(cx.listener(move |t, _, _, _| t.act(json!({ "what": "account", "id": id }))))
                    })
                    .child(face)
                    .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).child(name))
                    .when(running, |d| d.child(div().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(OK)).child(self.t("tray.playing")))),
            );
        }
        list
    }

    // ── The quick panel (left click) ───────────────────────────────────────────────────────────────────────────────
    fn panel(&mut self, cx: &mut Context<Self>) -> Div {
        let mini_open = self.flag("miniOpen");
        let head_keys = div()
            .flex()
            .gap(px(2.))
            .child(self.icon_button("tray-mini", "screens", mini_open, false, false, self.t("tray.mini"), cx, |t, _| t.act(json!({ "what": "mini" }))))
            .child(self.icon_button("tray-open", "external", false, false, false, self.t("tray.open"), cx, |t, _| t.act(json!({ "what": "open" }))))
            .into_any_element();
        let head = self.head(None, false, Some(head_keys));
        let body = div().w(px(PANEL_W)).p(px(14.)).flex().flex_col().gap(px(12.));

        if self.page == Page::Accounts || self.page == Page::Presets {
            let accounts = self.page == Page::Accounts;
            let back = self.back(self.t(if accounts { "tray.playAs" } else { "tray.runPreset" }), cx);
            let rows = if accounts {
                self.account_rows(false, cx)
            } else {
                let mut l = div().flex().flex_col().gap(px(2.));
                for (i, p) in self.list("presets").iter().enumerate() {
                    let id = p.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
                    let name = p.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
                    l = l.child(
                        div()
                            .id(("tray-preset", i))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .min_h(px(44.))
                            .px(px(8.))
                            .py(px(6.))
                            .rounded(px(11.))
                            .cursor_pointer()
                            .hover(|d| d.bg(ca(WHITE, 0.06)))
                            .child(div().size(px(32.)).flex_none().rounded(px(9.)).bg(c(RAISED_2)).flex().items_center().justify_center().child(icon("layers", 16., c(TEXT_2))))
                            .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).child(name))
                            .on_click(cx.listener(move |t, _, _, _| t.act(json!({ "what": "preset", "id": id })))),
                    );
                }
                l
            };
            return body.child(head).child(div().flex().flex_col().gap(px(2.)).child(back).child(div().id("tray-list").max_h(px(340.)).overflow_y_scroll().child(rows)));
        }

        let multi = self.multi().to_string();
        let on = multi != "off";
        let open = self.flag("open");
        let multi_only = self.flag("multiOnly");
        let locked = !open && !multi_only;
        let switchable = self.can_switch(on) && !(multi == "notReady" && open);
        let sub = if !self.can_switch(on) && locked {
            self.t("tray.unlockToChange")
        } else {
            self.t(match multi.as_str() {
                "ready" => "tray.multiReady",
                "notReady" => "tray.multiNotReady",
                _ => "tray.multiOff",
            })
        };
        let (tone, ground, edge) = match multi.as_str() {
            "ready" => (c(OK), ca(OK, 0.11), ca(OK, 0.32)),
            "notReady" => (c(WARN), ca(WARN, 0.11), ca(WARN, 0.32)),
            _ => (c(TEXT_2), c(RAISED), c(LINE)),
        };
        // Switched here when it can be; otherwise Bloxkeep opens: its switch asks first, its card fixes "not ready".
        let multi_row = div()
            .id("tray-multi")
            .flex()
            .items_center()
            .gap(px(12.))
            .w_full()
            .px(px(14.))
            .py(px(12.))
            .rounded(px(14.))
            .bg(ground)
            .border_1()
            .border_color(edge)
            .cursor_pointer()
            .hover(|d| d.opacity(0.92))
            .child(icon("copy", 18., tone))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Multi-Roblox"))
                    .child(div().text_size(px(T_S)).text_color(if multi == "ready" { tone } else { c(TEXT_2) }).child(sub)),
            )
            .child(switch_look(on, multi == "notReady"))
            .on_click(cx.listener(move |t, _, _, _| {
                t.act(if switchable {
                    json!({ "what": "multi", "on": on })
                } else if !on && open {
                    // On asks first: Bloxkeep opens with Multi-Roblox's question (owner 2026-11-19: it only opened).
                    json!({ "what": "multiAsk" })
                } else {
                    json!({ "what": "open" })
                })
            }));

        let middle = if locked {
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .pl(px(14.))
                .pr(px(10.))
                .py(px(10.))
                .rounded(px(14.))
                .bg(c(RAISED))
                .child(icon("lock", 18., c(TEXT_2)))
                .child(div().flex_1().child(self.t("tray.isLocked")))
                .child(
                    div()
                        .id("tray-unlock")
                        .px(px(12.))
                        .py(px(6.))
                        .rounded(px(9.))
                        .border_1()
                        .border_color(c(LINE))
                        .bg(c(RAISED_2))
                        .cursor_pointer()
                        .hover(|d| d.bg(ca(WHITE, 0.06)))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(self.t("tray.unlock"))
                        .on_click(cx.listener(|t, _, _, _| t.act(json!({ "what": "open" })))),
                )
        } else {
            let count = self.num("robloxCount");
            let confirming = self.confirm_until.is_some_and(|u| u > Instant::now());
            let can_close = count > 0 && (open || multi_only);
            let clients = self.list("clients");
            let shown: Vec<&Value> = if clients.len() > MAX_CLIENTS { clients.iter().take(MAX_CLIENTS - 1).collect() } else { clients.iter().collect() };
            let label = div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(4.))
                .min_h(px(26.))
                .text_size(px(T_S))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(MUTED))
                .child(self.t("tray.running"))
                .when(can_close, |d| {
                    d.child(
                        div()
                            .id("tray-close-all")
                            .px(px(8.))
                            .py(px(3.))
                            .rounded(px(8.))
                            .cursor_pointer()
                            .text_color(if confirming { c(BAD) } else { c(TEXT_2) })
                            .when(confirming, |d| d.bg(ca(BAD, 0.14)))
                            .hover(|d| d.bg(ca(BAD, 0.14)).text_color(c(BAD)))
                            .child(if confirming { self.tn("tray.closeAsk", count) } else { self.t("tray.closeAll") })
                            .on_click(cx.listener(move |t, _, _, cx| {
                                if t.confirm_until.is_some_and(|u| u > Instant::now()) {
                                    t.confirm_until = None;
                                    t.act(json!({ "what": "closeAll" }));
                                } else {
                                    // "Close N windows?" goes back to "Close all" after four seconds.
                                    t.confirm_until = Some(Instant::now() + std::time::Duration::from_secs(4));
                                    cx.spawn(async move |this, cx| {
                                        cx.background_executor().timer(std::time::Duration::from_secs(4)).await;
                                        this.update(cx, |_, cx| cx.notify()).ok();
                                    })
                                    .detach();
                                }
                                cx.notify();
                            })),
                    )
                });
            let list = if clients.is_empty() {
                div().py(px(12.)).px(px(4.)).flex().justify_center().text_color(c(TEXT_2)).child(if count > 0 { self.tn("tray.openCount", count) } else { self.t("tray.none") })
            } else {
                let mut l = div().flex().flex_col().gap(px(2.));
                for (i, cl) in shown.iter().enumerate() {
                    let name = cl.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
                    let game = cl.get("game").and_then(Value::as_str).unwrap_or_default().to_string();
                    let pid = cl.get("pid").and_then(Value::as_i64).unwrap_or(0);
                    let id = cl.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
                    let mb = cl.get("ramMb").and_then(Value::as_f64).unwrap_or(0.) as i64;
                    let group = SharedString::from(format!("tray-client-{i}"));
                    let face = self.avatar(&name, cl.get("avatar").and_then(Value::as_str), 32., cx);
                    let show_key = self.icon_button(("tray-show", i), "eye", false, true, false, self.t("tray.show"), cx, move |t, _| t.act(json!({ "what": "show", "pid": pid })));
                    let close_key = self.icon_button(("tray-x", i), "x", false, true, true, self.t("tray.close"), cx, move |t, _| t.act(json!({ "what": "closeClient", "id": id })));
                    l = l.child(
                        div()
                            .id(("tray-client", i))
                            .group(group.clone())
                            .flex()
                            .items_center()
                            .w_full()
                            .min_h(px(44.))
                            .pr(px(6.))
                            .rounded(px(11.))
                            .hover(|d| d.bg(ca(WHITE, 0.06)))
                            .child(
                                div()
                                    .id(("tray-client-main", i))
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .px(px(8.))
                                    .py(px(6.))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |t, _, _, _| t.act(json!({ "what": "show", "pid": pid }))))
                                    .child(face)
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .gap(px(1.))
                                            .child(div().truncate().font_weight(FontWeight::SEMIBOLD).child(name))
                                            .child(div().truncate().text_size(px(T_S)).text_color(c(TEXT_2)).child(format!("{game} · {}", ram(mb)))),
                                    ),
                            )
                            // (Show and Close come up with the mouse on the row, as the page's did.)
                            .child(div().flex().gap(px(2.)).opacity(0.).group_hover(group, |s| s.opacity(1.)).child(show_key).child(close_key)),
                    );
                }
                if clients.len() > MAX_CLIENTS {
                    let more = clients.len() - MAX_CLIENTS + 1;
                    l = l.child(
                        div()
                            .id("tray-more")
                            .p(px(8.))
                            .rounded(px(9.))
                            .flex()
                            .justify_center()
                            .text_size(px(T_S))
                            .text_color(c(TEXT_2))
                            .cursor_pointer()
                            .hover(|d| d.bg(ca(WHITE, 0.06)))
                            .child(self.i18n.tv("tray.more", &[("n", &more.to_string())]))
                            .on_click(cx.listener(|t, _, _, _| t.act(json!({ "what": "open" })))),
                    );
                }
                l
            };
            div().flex().flex_col().gap(px(4.)).child(label).child(div().id("tray-clients").max_h(px(340.)).overflow_y_scroll().child(list))
        };

        let has = |k: &str| open && !self.list(k).is_empty();
        let tile = |id: &'static str, name: &'static str, label: String, primary: bool| {
            div()
                .id(id)
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .pt(px(12.))
                .pb(px(10.))
                .px(px(6.))
                .rounded(px(14.))
                .cursor_pointer()
                .bg(if primary { c(FACE) } else { c(RAISED) })
                .when(!primary, |d| d.border_1().border_color(c(LINE)))
                .hover(move |d| d.bg(if primary { c(FACE_HI) } else { c(RAISED_2) }))
                .text_color(if primary { c(INK) } else { c(TEXT) })
                .child(icon(name, 20., if primary { c(INK) } else { c(TEXT_2) }))
                .child(div().max_w_full().truncate().font_weight(FontWeight::SEMIBOLD).child(label))
        };
        let new_tip: SharedString = self.t("tray.newTip").into();
        let tiles = div()
            .flex()
            .gap(px(8.))
            .when(has("accounts"), |d| d.child(tile("tray-play", "play", self.t("tray.play"), true).on_click(cx.listener(|t, _, _, cx| t.go(Page::Accounts, cx)))))
            .when(has("presets"), |d| d.child(tile("tray-presets", "layers", self.t("tray.presets"), false).on_click(cx.listener(|t, _, _, cx| t.go(Page::Presets, cx)))))
            .child(
                tile("tray-new", "userPlus", self.t("tray.new"), false)
                    .tooltip(move |_, cx| cx.new(|_| Tip(new_tip.clone())).into())
                    .on_click(cx.listener(|t, _, _, _| t.act(json!({ "what": "newRoblox" })))),
            );
        body.child(head).child(multi_row).child(middle).child(tiles)
    }

    // ── The menu (right click) ─────────────────────────────────────────────────────────────────────────────────────
    #[allow(clippy::too_many_arguments)]
    fn item(&self, id: impl Into<ElementId>, name: &'static str, label: String, value: Option<String>, sub: bool, checked: bool, strong: bool, danger: bool, cx: &mut Context<Self>, f: impl Fn(&mut Self, &mut Context<Self>) + 'static) -> Stateful<Div> {
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(11.))
            .w_full()
            .px(px(10.))
            .py(px(8.))
            .rounded(px(9.))
            .cursor_pointer()
            .hover(move |d| if danger { d.bg(ca(BAD, 0.14)).text_color(c(BAD)) } else { d.bg(ca(WHITE, 0.06)) })
            .child(icon(name, 17., if danger { c(BAD) } else { c(TEXT_2) }))
            .child(div().flex_1().min_w_0().truncate().when(strong, |d| d.font_weight(FontWeight::SEMIBOLD)).child(label))
            .when(checked, |d| d.child(icon("check", 15., c(FACE))))
            .when_some(value, |d, v| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(v)))
            .when(sub, |d| d.child(icon("chevronRight", 15., c(MUTED))))
            .on_click(cx.listener(move |t, _, _, cx| f(t, cx)))
    }

    fn sep() -> Div {
        div().h(px(1.)).mx(px(8.)).my(px(5.)).bg(c(LINE))
    }

    fn menu(&mut self, cx: &mut Context<Self>) -> Div {
        let body = div().w(px(MENU_W)).p(px(6.)).flex().flex_col();
        let multi = self.multi().to_string();
        let on = multi != "off";
        let open = self.flag("open");
        let multi_only = self.flag("multiOnly");
        let count = self.num("robloxCount");
        match self.page {
            Page::Accounts => {
                let back = self.back(self.t("tray.playAs"), cx);
                return body.child(back).child(div().id("tray-list").max_h(px(340.)).overflow_y_scroll().child(self.account_rows(true, cx)));
            }
            Page::Presets => {
                let back = self.back(self.t("tray.runPreset"), cx);
                let mut l = div().flex().flex_col().gap(px(2.));
                for (i, p) in self.list("presets").iter().enumerate() {
                    let id = p.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
                    let name = p.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
                    l = l.child(self.item(("tray-mp", i), "layers", name, None, false, false, false, false, cx, move |t, _| t.act(json!({ "what": "preset", "id": id }))));
                }
                return body.child(back).child(div().id("tray-list").max_h(px(340.)).overflow_y_scroll().child(l));
            }
            // Each macro: its panel's state as a dot, its name, "Shown" while its panel is; a click shows or hides it.
            Page::Macros => {
                let back = self.back(self.t("tray.macros"), cx);
                let mut l = div().flex().flex_col().gap(px(2.));
                for (i, m) in self.list("macros").iter().enumerate() {
                    let id = m.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
                    let name = m.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
                    let shown = m.get("shown").and_then(Value::as_bool).unwrap_or(false);
                    let state = m.get("state").and_then(Value::as_str).unwrap_or(if shown { "ready" } else { "off" });
                    let dot = match state {
                        "ready" | "running" => c(OK),
                        "paused" => c(WARN),
                        "stopped" | "recording" => c(BAD),
                        _ => c(DIM),
                    };
                    let tip: SharedString = self.t(if shown { "tray.panelHide" } else { "tray.panelShow" }).into();
                    l = l.child(
                        div()
                            .id(("tray-macro", i))
                            .flex()
                            .items_center()
                            .gap(px(11.))
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(9.))
                            .cursor_pointer()
                            .hover(|d| d.bg(ca(WHITE, 0.06)))
                            .child(div().mx(px(5.)).size(px(7.)).flex_none().rounded_full().bg(dot))
                            .child(div().flex_1().min_w_0().truncate().child(name))
                            .when(shown, |d| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(self.t("tray.shown"))))
                            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                            .on_click(cx.listener(move |t, _, _, _| t.act(json!({ "what": "panel", "id": id })))),
                    );
                }
                return body.child(back).child(div().id("tray-list").max_h(px(340.)).overflow_y_scroll().child(l));
            }
            Page::Multi => {
                let back = self.back("Multi-Roblox".into(), cx);
                let fix = (multi == "notReady" && open && !multi_only).then(|| self.item("tray-fix", "wrench", self.t("tray.fix"), None, false, false, false, false, cx, |t, _| t.act(json!({ "what": "open" }))));
                let switch = if self.can_switch(on) {
                    self.item("tray-switch", "power", self.t(if on { "tray.turnOff" } else { "tray.turnOn" }), None, false, false, false, false, cx, move |t, _| {
                        t.act(if !on && open && !multi_only { json!({ "what": "multiAsk" }) } else { json!({ "what": "multi", "on": on }) })
                    })
                } else {
                    self.item("tray-switch", if !open { "lock" } else { "external" }, self.t(if !open { "tray.unlockToChange" } else { "tray.openToTurnOn" }), None, false, false, false, false, cx, |t, _| t.act(json!({ "what": "open" })))
                };
                return body.child(back).children(fix).child(switch);
            }
            Page::Close => {
                let back = self.back(self.t("tray.closeAllRoblox"), cx);
                let close = self.item("tray-close-n", "x", self.tn("tray.closeN", count), None, false, false, false, true, cx, |t, _| t.act(json!({ "what": "closeAll" })));
                return body.child(back).child(close);
            }
            Page::Home => {}
        }

        let short = self.t(match multi.as_str() {
            "ready" => "tray.mOn",
            "notReady" => "tray.mNotReady",
            _ => "tray.mOff",
        });
        let line = if !open && !multi_only {
            format!("{} · {short}", self.t("tray.locked"))
        } else if count > 0 {
            format!("{short} · {}", self.tn("tray.windows", count))
        } else {
            short
        };
        let can_close = count > 0 && (open || multi_only);
        let has = |k: &str| open && !self.list(k).is_empty();
        let mini_open = self.flag("miniOpen");
        let head = self.head(Some(line), true, None).px(px(8.)).pt(px(8.)).pb(px(10.));
        let open_item = self.item("tray-i-open", "external", self.t("tray.open"), None, false, false, true, false, cx, |t, _| t.act(json!({ "what": "open" })));
        let mini_item = self.item("tray-i-mini", "screens", self.t("tray.mini"), None, false, mini_open, false, false, cx, |t, _| t.act(json!({ "what": "mini" })));
        let play = has("accounts").then(|| self.item("tray-i-play", "play", self.t("tray.playAs"), None, true, false, false, false, cx, |t, cx| t.go(Page::Accounts, cx)));
        let presets = has("presets").then(|| self.item("tray-i-presets", "layers", self.t("tray.runPreset"), None, true, false, false, false, cx, |t, cx| t.go(Page::Presets, cx)));
        let macros = has("macros").then(|| self.item("tray-i-macros", "keyboard", self.t("tray.macros"), None, true, false, false, false, cx, |t, cx| t.go(Page::Macros, cx)));
        let new_tip: SharedString = self.t("tray.newTip").into();
        let new_item = self.item("tray-i-new", "userPlus", self.t("tray.newWindow"), None, false, false, false, false, cx, |t, _| t.act(json!({ "what": "newRoblox" }))).tooltip(move |_, cx| cx.new(|_| Tip(new_tip.clone())).into());
        // Switched right here when it can be (a tick while on); otherwise, or to fix it, its own page.
        let multi_item = if self.can_switch(on) && !(multi == "notReady" && open && !multi_only) {
            self.item("tray-i-multi", "copy", "Multi-Roblox".into(), (multi == "notReady").then(|| self.t("tray.notReady")), false, on, false, false, cx, move |t, _| t.act(json!({ "what": "multi", "on": on })))
        } else {
            let v = self.t(match multi.as_str() {
                "ready" => "tray.on",
                "notReady" => "tray.notReady",
                _ => "tray.off",
            });
            self.item("tray-i-multi", "copy", "Multi-Roblox".into(), Some(v), true, false, false, false, cx, |t, cx| t.go(Page::Multi, cx))
        };
        let close = can_close.then(|| self.item("tray-i-close", "x", self.t("tray.closeAllRoblox"), Some(count.to_string()), true, false, false, false, cx, |t, cx| t.go(Page::Close, cx)));
        let quit = self.item("tray-i-quit", "power", self.t("tray.quit"), None, false, false, false, false, cx, |t, _| t.act(json!({ "what": "quit" })));
        body.child(head)
            .child(Self::sep())
            .child(open_item)
            .child(mini_item)
            .child(Self::sep())
            .children(play)
            .children(presets)
            .children(macros)
            .child(new_item)
            .child(multi_item)
            .child(Self::sep())
            .children(close)
            .child(quit)
    }
}

impl Render for Tray {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window;
        let body = if self.mode == "menu" { self.menu(cx) } else { self.panel(cx) };
        // (Solid: its window has no picture under it, whatever glass the main window shows.)
        let ground = crate::theme::solid_panel();
        // Its size and the window's, said to the core once either changes: the core gives the window that size (times
        // the screen's scale) and shows an opening once it is drawn at it.
        let said = self.said.clone();
        let opened = self.opened;
        let measure = canvas(
            move |b, window, _| {
                let (w, h) = (f32::from(b.size.width).ceil() as i32, f32::from(b.size.height).ceil() as i32);
                let v = window.viewport_size();
                let (vw, vh) = (f32::from(v.width).round() as i32, f32::from(v.height).round() as i32);
                if said.get() != (w, h, vw, vh, opened) {
                    said.set((w, h, vw, vh, opened));
                    #[cfg(feature = "core")]
                    CHECK.with(|c| {
                        if let Some(v) = c.borrow_mut().as_mut() {
                            v.push((w, h, vw, vh));
                        }
                    });
                    to_core(move |t| t.on_size(w, h, vw, vh, opened as i32));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        div()
            .id("tray")
            .track_focus(&self.focus)
            .key_context("Tray")
            .size_full()
            // (A row, its one child at its own size: the part is measured as wide and tall as it draws, never as the
            // window happens to be, which the window is then made.)
            .flex()
            .items_start()
            .bg(ground)
            .font(crate::theme::ui_font())
            .text_size(px(T_M))
            .text_color(c(TEXT))
            .on_key_down(cx.listener(|t, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    if t.page != Page::Home {
                        t.go(Page::Home, cx);
                    } else {
                        t.act(json!({ "what": "hide" }));
                    }
                    cx.stop_propagation();
                }
            }))
            .child(div().relative().flex_none().bg(ground).child(measure).child(body))
    }
}

/// Multi-Roblox's switch as it looks (the whole row is what is clicked).
fn switch_look(on: bool, warn: bool) -> Div {
    div()
        .relative()
        .w(px(38.))
        .h(px(22.))
        .flex_none()
        .rounded_full()
        .bg(if on { if warn { c(WARN) } else { c(FACE) } } else { c(RAISED_2) })
        .child(div().absolute().top(px(3.)).left(px(if on { 19. } else { 3. })).size(px(16.)).rounded_full().bg(c(WHITE)).shadow_sm())
}

/// RAM in MB or GB (Tray.tsx's ram).
fn ram(mb: i64) -> String {
    crate::lang::cur().mb(mb as f64)
}

// ── Shots ──────────────────────────────────────────────────────────────────────────────────────────────────────────
// BLOXKEEP_SHOT_TRAY=panel | panel-locked | panel-play | menu | menu-macros: the tray drawn inside the shot's window
// (a shot has one window), with made-up windows, presets and macros and the mock's accounts.
#[cfg(feature = "shot")]
thread_local! {
    static SHOT: std::cell::RefCell<Option<Entity<Tray>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(feature = "shot")]
pub fn shot_layer(r: &Root, window: &mut Window, cx: &mut Context<Root>) -> Option<AnyElement> {
    let kind = std::env::var("BLOXKEEP_SHOT_TRAY").ok()?;
    let tray = match SHOT.with(|s| s.borrow().clone()) {
        Some(t) => t,
        None => {
            let accounts = r.accounts();
            // (Once the mock's accounts are in.)
            if accounts.is_empty() {
                return None;
            }
            let name = |a: &Value| a.get("username").and_then(Value::as_str).unwrap_or("player").to_string();
            let acc: Vec<Value> = accounts.iter().take(6).enumerate().map(|(i, a)| json!({ "id": a.get("id"), "name": name(a), "running": i < 2, "avatar": a.get("avatarUrl") })).collect();
            let games = ["Blox Fruits", "Grow a Garden", "Blox Fruits"];
            let mbs = [1180, 960, 1420];
            let clients: Vec<Value> =
                accounts.iter().take(3).enumerate().map(|(i, a)| json!({ "id": a.get("id"), "name": name(a), "game": games[i], "pid": 4100 + i, "ramMb": mbs[i], "avatar": a.get("avatarUrl") })).collect();
            let locked = kind == "panel-locked";
            let state = json!({
                "open": !locked, "multi": "ready", "multiOnly": false, "robloxCount": clients.len(), "ramMb": 3560, "miniOpen": false,
                "presets": [{ "id": "p1", "name": "Farm squad" }, { "id": "p2", "name": "Raid team" }],
                "macros": [{ "id": "m1", "name": "Auto click", "shown": true, "state": "running" }, { "id": "m2", "name": "Fishing", "shown": false, "state": "off" }],
                "accounts": acc,
                "clients": if locked { json!([]) } else { json!(clients) },
            });
            let mode = if kind.starts_with("menu") { "menu" } else { "panel" };
            let page = match kind.as_str() {
                "panel-play" => Page::Accounts,
                "menu-macros" => Page::Macros,
                _ => Page::Home,
            };
            let i18n = r.i18n.clone();
            let t = cx.new(|cx| {
                let mut t = Tray::new(i18n, mode.into(), 1, state, window, cx);
                t.page = page;
                t
            });
            SHOT.with(|s| *s.borrow_mut() = Some(t.clone()));
            t
        }
    };
    Some(div().absolute().top(px(60.)).left(px(80.)).rounded(px(12.)).overflow_hidden().border_1().border_color(c(LINE)).shadow_lg().child(tray).into_any_element())
}

// ── The check on a real window (--tray-window-check OUT) ───────────────────────────────────────────────────────────
// The shots draw in GPUI's test window, with no Windows frame and no Windows drawing rules: they can't show whether the
// real window draws, or at what size (alpha.39's tray never opened: its window kept a frame, 2026-11-19). This opens
// the real tray window on this PC as the app does, kept under DWM's cloak all along (nothing is seen, it never takes
// the keyboard), and writes what Windows and GPUI did: its style, its size against its inside, whether GPUI drew it
// cloaked, and whether it drew again at the size it was then given. Exit 0 when all hold.
#[cfg(feature = "core")]
thread_local! {
    static CHECK: std::cell::RefCell<Option<Vec<(i32, i32, i32, i32)>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(feature = "core")]
pub fn window_check(out: std::path::PathBuf, assets: std::path::PathBuf) {
    use windows_sys::Win32::{
        Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
        UI::{
            HiDpi::GetDpiForWindow,
            WindowsAndMessaging::{GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetWindowLongPtrW, GetWindowRect, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SetWindowPos, ShowWindow},
        },
    };
    gpui_kit::application().with_assets(crate::assets::FsAssets(assets.clone())).run(move |cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(false);
        crate::theme::dress(cx);
        images::init(cx);
        let fonts = ["Regular", "Medium", "SemiBold", "Bold"]
            .iter()
            .filter_map(|w| std::fs::read(assets.join(format!("fonts/BeVietnamPro-{w}.ttf"))).ok())
            .map(std::borrow::Cow::Owned)
            .collect::<Vec<_>>();
        cx.text_system().add_fonts(fonts).ok();
        let words = I18n::from_assets(&assets);
        CHECK.with(|c| *c.borrow_mut() = Some(Vec::new()));
        let state = json!({
            "open": true, "multi": "ready", "multiOnly": false, "robloxCount": 2, "ramMb": 2140, "miniOpen": false,
            "presets": [{ "id": "p1", "name": "Farm squad" }], "macros": [], "accounts": [{ "id": "a1", "name": "player_one", "running": false }],
            "clients": [{ "id": "a1", "name": "player_one", "game": "A game", "pid": 1, "ramMb": 1070 }],
        });
        let made = cx.open_window(window_options(), move |window, cx| cx.new(|cx| Tray::new(words, "menu".into(), 1, state, window, cx)));
        let h = made.as_ref().ok().and_then(|w| w.update(cx, |_, window, _| crate::hwnd_of(window)).ok().flatten()).unwrap_or(0);
        let made_err = made.as_ref().err().map(|e| e.to_string());
        cx.spawn(async move |cx| {
            let mut lines: Vec<String> = Vec::new();
            let mut ok = true;
            let reports = || CHECK.with(|c| c.borrow().clone().unwrap_or_default());
            let rects = || {
                let (mut w, mut c) = (unsafe { std::mem::zeroed::<windows_sys::Win32::Foundation::RECT>() }, unsafe { std::mem::zeroed::<windows_sys::Win32::Foundation::RECT>() });
                // SAFETY: plain queries into RECTs on the stack.
                unsafe {
                    GetWindowRect(h as _, &mut w);
                    GetClientRect(h as _, &mut c);
                }
                ((w.right - w.left, w.bottom - w.top), (c.right - c.left, c.bottom - c.top))
            };
            if h == 0 {
                ok = false;
                lines.push(format!("FAIL the window wasn't made: {made_err:?}"));
            } else {
                // SAFETY: plain calls on our own window.
                let (style, ex) = unsafe { (GetWindowLongPtrW(h as _, GWL_STYLE) as u32, GetWindowLongPtrW(h as _, GWL_EXSTYLE) as u32) };
                lines.push(format!("style {style:#010x}, ex style {ex:#010x}"));
                if style & 0x8000_0000 == 0 || style & 0x00C0_0000 != 0 {
                    ok = false;
                    lines.push("FAIL not a borderless pop-up (WS_POPUP without WS_CAPTION)".into());
                }
                let cloak: i32 = 1;
                // As the core's `park`: cloaked, given its last size (344 × 300 before any drawing), shown without the keyboard.
                // SAFETY: plain calls on our own window; an int on the stack.
                unsafe {
                    DwmSetWindowAttribute(h as _, DWMWA_CLOAK as _, &cloak as *const i32 as *const _, 4);
                    let dpi = GetDpiForWindow(h as _).max(96) as f32 / 96.;
                    SetWindowPos(h as _, std::ptr::null_mut(), 0, 0, (344. * dpi).ceil() as i32, (300. * dpi).ceil() as i32, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
                    ShowWindow(h as _, SW_SHOWNOACTIVATE);
                }
                let mut ms = 0;
                while ms < 3000 && reports().is_empty() {
                    cx.background_executor().timer(std::time::Duration::from_millis(25)).await;
                    ms += 25;
                }
                let first = reports().first().copied();
                lines.push(format!("first drawing under the cloak after {ms} ms: {first:?} (drawn w, h, window w, h)"));
                let (win, client) = rects();
                lines.push(format!("window {win:?}, its inside {client:?}"));
                if win != client {
                    ok = false;
                    lines.push("FAIL the window has a frame (its inside is smaller than it)".into());
                }
                match first {
                    None => {
                        ok = false;
                        lines.push("FAIL GPUI never drew the window under the cloak".into());
                    }
                    Some((w, hh, _, _)) => {
                        // The menu is MENU_W wide whatever the window is; a part as tall as its rows.
                        if w != MENU_W as i32 || hh < 200 {
                            ok = false;
                            lines.push(format!("FAIL measured {w}x{hh}, not the menu's own {MENU_W} wide (the window's size leaked in)"));
                        }
                        // SAFETY: plain call.
                        let dpi = unsafe { GetDpiForWindow(h as _) }.max(96) as f32 / 96.;
                        let (pw, ph) = ((w as f32 * dpi).ceil() as i32, (hh as f32 * dpi).ceil() as i32);
                        // SAFETY: plain call on our own window.
                        unsafe { SetWindowPos(h as _, std::ptr::null_mut(), 0, 0, pw, ph, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) };
                        let fits = |r: &[(i32, i32, i32, i32)]| r.last().is_some_and(|&(w, hh, vw, vh)| (w - vw).abs() <= 1 && (hh - vh).abs() <= 1);
                        let mut ms = 0;
                        while ms < 2000 && !fits(&reports()) {
                            cx.background_executor().timer(std::time::Duration::from_millis(25)).await;
                            ms += 25;
                        }
                        lines.push(format!("given {pw}x{ph} (scale {dpi}); drawing after {ms} ms: {:?}", reports().last()));
                        if !fits(&reports()) {
                            ok = false;
                            lines.push("FAIL GPUI didn't draw again at the size the window was given".into());
                        }
                        let (win, client) = rects();
                        lines.push(format!("window {win:?}, its inside {client:?}"));
                        if win != client || win != (pw, ph) {
                            ok = false;
                            lines.push("FAIL the window's inside isn't the size it was given".into());
                        }
                    }
                }
                // SAFETY: plain call on our own window.
                unsafe { ShowWindow(h as _, SW_HIDE) };
            }
            lines.push(if ok { "ALL PASS".into() } else { "FAILED".into() });
            let _ = std::fs::write(&out, lines.join("\n") + "\n");
            cx.update(|cx| cx.quit());
            if !ok {
                std::process::exit(1);
            }
        })
        .detach();
    });
}
