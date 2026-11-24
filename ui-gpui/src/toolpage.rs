// The faces of the app's own small windows: the mini window, the dialogs, the macro panel beside a game, the overlay
// and colour picker over one. The core keeps each window (its place, size, frame, on top, the drag by its title row,
// the keyboard: core/web_tool_window.rs, core/forms/dialog.rs); what is drawn in it is a window of the interface's own,
// put inside the core's as its child and filling it (core/tool_page.rs). Until 2026-11-19 each was a WebView2 browser
// with a page of ui/ (MiniView.tsx, DialogView.tsx, PanelView.tsx, OverlayView.tsx), whose processes came and went
// beside Bloxkeep in Task Manager; the look and behaviour here are those pages', part for part.
//
// The core sends "toolPage" events: open (a page of a kind, inside a window), event (what it draws, by name), wake
// (its window was shown, unseen until this draws once more), close. The page answers as the web pages did: { cmd,
// args } with "size" (what it drew, the window it drew in, and for which showing), "act", "drag", "resize".
use std::{cell::Cell, collections::HashMap, rc::Rc};

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{i18n::I18n, theme::*};

#[cfg(feature = "core")]
mod check;
#[cfg(feature = "core")]
pub use check::window_check;
mod dialog;
mod mini;
mod overlay;
mod panel;

/// The pages, by the core's id.
#[derive(Default)]
pub struct Pages(HashMap<u64, WindowHandle<ToolPage>>);

/// What a page says to its window in the core.
#[derive(Clone, Copy)]
pub struct Link(u64);

impl Link {
    pub fn send(&self, cmd: &str, args: Value) {
        #[cfg(feature = "core")]
        crate::core::tool_page::from_ui(self.0, crate::core::tool_page::FromPage::Message(json!({ "cmd": cmd, "args": args })));
        #[cfg(not(feature = "core"))]
        let _ = (self.0, cmd, args);
    }
    /// Something clicked: `what`, with the rest in `args` (an object).
    pub fn act(&self, what: &str, args: Value) {
        let mut a = if args.is_object() { args } else { json!({}) };
        a["what"] = json!(what);
        self.send("act", a);
    }
    /// The title row dragged: the core's window moves with the pointer.
    pub fn drag(&self) {
        self.send("drag", Value::Null);
    }
    /// The bottom grip dragged: the core's window grows or shrinks with the pointer.
    pub fn resize(&self) {
        self.send("resize", Value::Null);
    }
}

/// What a page draws.
pub enum Body {
    Waiting,
    Mini(mini::Mini),
    Dialog(dialog::Dialog),
    Panel(panel::Panel),
    Overlay(overlay::Overlay),
}

pub struct ToolPage {
    pub link: Link,
    pub i18n: I18n,
    pub focus: FocusHandle,
    pub body: Body,
    kind: String,
    /// The showing to draw for (the core's wake), and how many events came: each is said back with the size.
    seq: u64,
    events: u64,
    said: Rc<Cell<(i32, i32, i32, i32, u64, u64)>>,
    _active: Subscription,
}

/// A "toolPage" event from the core.
pub fn on_event(pages: &mut Pages, i18n: &I18n, data: &Value, cx: &mut App) {
    let id = data.get("id").and_then(Value::as_u64).unwrap_or(0);
    if id == 0 {
        return;
    }
    match data.get("op").and_then(Value::as_str).unwrap_or("") {
        "open" => {
            let kind = data.get("kind").and_then(Value::as_str).unwrap_or("").to_string();
            let parent = data.get("parent").and_then(Value::as_i64).unwrap_or(0) as isize;
            open(pages, i18n, id, kind, parent, cx);
        }
        "event" => {
            let name = data.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let payload = data.get("data").cloned().unwrap_or(Value::Null);
            if let Some(h) = pages.0.get(&id).copied() {
                if h.update(cx, |t, window, cx| t.event(&name, payload, window, cx)).is_err() {
                    pages.0.remove(&id);
                    gone(id);
                }
            }
        }
        "wake" => {
            let seq = data.get("seq").and_then(Value::as_u64).unwrap_or(0);
            if let Some(h) = pages.0.get(&id).copied() {
                let _ = h.update(cx, |t, _, cx| {
                    t.seq = seq;
                    cx.notify();
                });
            }
        }
        "close" => {
            if let Some(h) = pages.0.remove(&id) {
                let _ = h.update(cx, |_, window, _| window.remove_window());
            }
        }
        _ => {}
    }
}

/// Its window went without being asked (the core's window was destroyed with it in): the core makes it again.
fn gone(id: u64) {
    #[cfg(feature = "core")]
    crate::core::tool_page::from_ui(id, crate::core::tool_page::FromPage::Gone);
    #[cfg(not(feature = "core"))]
    let _ = id;
}

fn open(pages: &mut Pages, i18n: &I18n, id: u64, kind: String, parent: isize, cx: &mut App) {
    let i18n = i18n.clone();
    let see_through = kind == "overlay";
    let options = WindowOptions {
        kind: WindowKind::PopUp,
        window_background: if see_through { WindowBackgroundAppearance::Transparent } else { WindowBackgroundAppearance::Opaque },
        titlebar: None,
        focus: false,
        show: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(120.), px(80.)) })),
        ..Default::default()
    };
    let made = cx.open_window(options, move |window, cx| cx.new(|cx| ToolPage::new(id, kind, i18n, window, cx)));
    let h = match made {
        Ok(h) => h,
        Err(e) => {
            eprintln!("tool page: its window couldn't be made: {e}");
            return;
        }
    };
    let child = h.update(cx, |_, window, _| crate::hwnd_of(window)).ok().flatten().unwrap_or(0);
    if child == 0 || !adopt(child, parent) {
        // (The core's window went meanwhile: nothing to draw in.)
        let _ = h.update(cx, |_, window, _| window.remove_window());
        gone(id);
        return;
    }
    pages.0.insert(id, h);
    #[cfg(feature = "core")]
    crate::core::tool_page::from_ui(id, crate::core::tool_page::FromPage::Attached(child));
}

/// Puts the page's window inside the core's: a child window (no frame, not on top of its own, not in Alt+Tab), filling
/// it, at its scale (a child gets no scale message of its own: it is told once here).
fn adopt(child: isize, parent: isize) -> bool {
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GWL_STYLE, GetClientRect, IsWindow, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SendMessageW, SetParent, SetWindowLongPtrW,
            SetWindowPos, WM_DPICHANGED_AFTERPARENT, WS_CHILD, WS_CLIPSIBLINGS,
        },
    };
    let (c, p) = (child as HWND, parent as HWND);
    // SAFETY: plain calls on the interface's own window and the core's (checked to be there); a RECT on the stack.
    unsafe {
        if parent == 0 || IsWindow(p) == 0 {
            return false;
        }
        // The style first: a pop-up made a child keeps acting as a pop-up otherwise (SetParent's own remark).
        SetWindowLongPtrW(c, GWL_STYLE, (WS_CHILD | WS_CLIPSIBLINGS) as isize);
        SetWindowLongPtrW(c, GWL_EXSTYLE, 0);
        if SetParent(c, p).is_null() {
            return false;
        }
        let mut r = std::mem::zeroed();
        GetClientRect(p, &mut r);
        let (w, h) = ((r.right - r.left).max(1), (r.bottom - r.top).max(1));
        SetWindowPos(c, std::ptr::null_mut(), 0, 0, w, h, SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
        SendMessageW(c, WM_DPICHANGED_AFTERPARENT, 0, 0);
    }
    true
}

impl ToolPage {
    fn new(id: u64, kind: String, i18n: I18n, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        // Its window took the keyboard (the core's window was activated): the keys come here (Esc, Enter, Tab).
        let active = cx.observe_window_activation(window, |t, window, cx| {
            if window.is_window_active() {
                window.focus(&t.focus, cx);
            } else {
                let link = t.link;
                if let Body::Panel(p) = &mut t.body {
                    p.blurred(link);
                    cx.notify();
                }
            }
        });
        ToolPage { link: Link(id), i18n, focus, body: Body::Waiting, kind, seq: 0, events: 0, said: Rc::new(Cell::new((0, 0, 0, 0, 0, 0))), _active: active }
    }

    /// What the core sent: drawn anew; the size is said again after it, as the pages did after each event.
    fn event(&mut self, name: &str, data: Value, window: &mut Window, cx: &mut Context<Self>) {
        // The keys (Esc, Enter, a panel's arrows) come to the page itself while nothing in it has them (a number typed
        // into keeps them): with nothing focused GPUI gives a key to no one.
        if window.focused(cx).is_none() {
            window.focus(&self.focus, cx);
        }
        if let Some(l) = data.get("language").and_then(Value::as_str) {
            self.i18n.set_language(l);
        }
        match (self.kind.as_str(), name) {
            ("mini", "mini") => match &mut self.body {
                Body::Mini(m) => m.set(data),
                _ => self.body = Body::Mini(mini::Mini::new(data)),
            },
            ("dialog", "dialog") => self.body = Body::Dialog(dialog::Dialog::new(data)),
            ("panel", "panel") => match &mut self.body {
                Body::Panel(p) if p.is_for(&data) => p.set(data, cx),
                _ => {
                    let mut p = panel::Panel::new(data.clone());
                    p.set(data, cx);
                    self.body = Body::Panel(p);
                }
            },
            ("panel", "panelError") => {
                if let Body::Panel(p) = &mut self.body {
                    p.error(data, cx);
                }
            }
            ("overlay", "overlay") => match &mut self.body {
                Body::Overlay(o) => o.set(data),
                _ => self.body = Body::Overlay(overlay::Overlay::new(data)),
            },
            _ => return,
        }
        self.events += 1;
        cx.notify();
    }

    pub fn t(&self, k: &str) -> String {
        self.i18n.t(k)
    }
    pub fn tn(&self, k: &str, n: i64) -> String {
        self.i18n.tn(k, n, &[])
    }

    /// Reports what was drawn, once per change (of its size, the window's, the showing or the event drawn).
    fn measure(&self, fill: bool) -> impl IntoElement {
        let (said, link, seq, events) = (self.said.clone(), self.link, self.seq, self.events);
        let (natural, pill) = match &self.body {
            Body::Panel(p) => (Some(p.natural.clone()), p.pill),
            _ => (None, false),
        };
        canvas(
            move |bounds, window, _| {
                let vp = window.viewport_size();
                let (w, mut h) = if fill { (f32::from(vp.width), f32::from(vp.height)) } else { (f32::from(bounds.size.width), f32::from(bounds.size.height)) };
                // (A panel: the height it would take with nothing cut, which the core caps at the screen.)
                if let Some(n) = natural.as_ref().map(|c| c.get()).filter(|n| *n > 0.) {
                    h = n;
                }
                let now = (w.ceil() as i32, h.ceil() as i32 + if pill { 100_000 } else { 0 }, f32::from(vp.width).round() as i32, f32::from(vp.height).round() as i32, seq, events);
                if now.0 > 0 && h > 0. && said.get() != now {
                    said.set(now);
                    link.send("size", json!({ "w": w.ceil(), "h": h.ceil(), "vw": f32::from(vp.width), "vh": f32::from(vp.height), "seq": seq, "pill": pill }));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }
}

impl Render for ToolPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ground = crate::theme::solid_panel();
        let (part, fill): (AnyElement, bool) = match &self.body {
            Body::Waiting => (div().into_any_element(), true),
            Body::Mini(_) => (mini::render(self, window, cx), true),
            Body::Dialog(_) => (dialog::render(self, window, cx), false),
            Body::Panel(_) => (panel::render(self, window, cx), false),
            Body::Overlay(_) => (overlay::render(self, window, cx), true),
        };
        let measure = self.measure(fill);
        let root = div()
            .id("tool-page")
            .track_focus(&self.focus)
            .key_context("ToolPage")
            .size_full()
            .font(crate::theme::ui_font())
            .text_size(px(T_M))
            .text_color(c(TEXT))
            .on_key_down(cx.listener(|t, e: &KeyDownEvent, window, cx| t.key(e, window, cx)));
        let root = match &self.body {
            Body::Overlay(_) => root,
            _ => root.bg(ground),
        };
        if fill {
            root.relative().child(div().size_full().relative().child(part).child(measure))
        } else {
            // (Measured at its own size, not the window's: the window is sized to it.)
            root.flex().items_start().child(div().relative().flex_none().bg(ground).child(part).child(measure))
        }
    }
}

impl ToolPage {
    fn key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = e.keystroke.key.clone();
        let key = key.as_str();
        let which = match &self.body {
            Body::Mini(_) => 1,
            Body::Dialog(_) => 2,
            Body::Panel(_) => 3,
            Body::Overlay(_) => 4,
            Body::Waiting => 0,
        };
        let handled = match which {
            1 => mini::key(self, key, cx),
            2 => dialog::key(self, key, window, cx),
            3 => panel::key(self, key, window, cx),
            4 => overlay::key(self, key, cx),
            _ => false,
        };
        if handled {
            cx.stop_propagation();
        }
    }
}

/// The title row's drag: moves the window once the pointer moved a few pixels with the button down (a click stays a
/// click). `from` keeps where the button went down.
pub fn drag_row(d: Stateful<Div>, link: Link, from: Rc<Cell<Option<Point<Pixels>>>>) -> Stateful<Div> {
    let down = from.clone();
    d.on_mouse_down(MouseButton::Left, move |e, _, _| down.set(Some(e.position)))
        .on_mouse_up(MouseButton::Left, {
            let from = from.clone();
            move |_, _, _| from.set(None)
        })
        .on_mouse_move(move |e: &MouseMoveEvent, _, _| {
            let Some(at) = from.get() else { return };
            if e.pressed_button != Some(MouseButton::Left) {
                from.set(None);
                return;
            }
            if (f32::from(e.position.x - at.x)).abs() + (f32::from(e.position.y - at.y)).abs() >= 3. {
                from.set(None);
                link.drag();
            }
        })
}

/// RAM in MB or GB (the pages' fmtMb).
pub fn mb(mb: i64) -> String {
    crate::lang::cur().mb(mb as f64)
}

/// A small round state light (the pages' .pn-dot).
pub fn dot(col: Hsla) -> Div {
    div().size(px(8.)).flex_none().rounded_full().bg(col)
}

/// The icon button of a title row (.pn-icon): 28 px, quiet until hovered.
pub fn title_icon(id: impl Into<ElementId>, name: &'static str, tip: String) -> Stateful<Div> {
    let tip: SharedString = tip.into();
    div()
        .id(id)
        .size(px(28.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(R_S))
        .cursor_pointer()
        .hover(|d| d.bg(ca(WHITE, 0.07)))
        .child(crate::kit::icon(name, 16., c(TEXT_2)))
        .tooltip(move |_, cx| cx.new(|_| crate::kit::Tip(tip.clone())).into())
}
