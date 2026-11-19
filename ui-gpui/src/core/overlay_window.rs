// OverlayWindow.cs: drawn over a Roblox window's picture area, following it as it moves: overlay.html
// (ui/src/overlay/OverlayView.tsx) in a window of ours lying over the game's, so it never sends anything to Roblox: every
// click lands on it, not on the game. Until 2026-11 it was hand-drawn WinForms (GameOverlay).
// - Edit: the game dimmed under the macro's points (numbered dots, the step's number) and areas (labelled boxes with
//   corner handles); a dot is dragged, a box dragged by its inside and sized by its corners.
// - Areas: the same with the areas only (the panel's "Edit areas").
// - Pick: the window's picture as it was a moment ago, frozen, with a magnifier; a click takes the place and its colour.
// - Screen: Pick with no game, over the monitor under the pointer (`screen_picker`); the colour only.
// A bar at its top says what it is for, with Done (or Cancel); Esc or Enter ends it too. It ends by itself when the
// window goes. The dimming is the page's own see-through black over the game (the window has no surface of its own:
// `see_through`), not a see-through colour key, which would let clicks through to the game; until 2026-11-19 it was the
// whole window's opacity, which dimmed the dots, boxes and bar with it.
// The page only says where things went (parts of the picture area); the colour of a click is read here, from the
// picture itself.
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Duration,
};

use base64::Engine;
use serde_json::{Value, json};
use windows_sys::Win32::{
    Foundation::{POINT, RECT},
    Graphics::Gdi::ClientToScreen,
    UI::WindowsAndMessaging::GetClientRect,
};

use super::{
    l, log,
    macro_model::MacroRect,
    macro_panel::{area_geometry::Dot, macro_options},
    themes::{self, Color},
    web_tool_window::{WebTool, WebToolWindow, str_of},
    win_form::{self, Timer},
    win_kit::{Picture, Rect, hwnd},
    window_report,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Edit,
    Areas,
    Pick,
    Screen,
}

#[derive(Clone, Debug)]
pub struct Area {
    pub name: String,
    pub label: String,
    pub colour: Color,
    pub rect: MacroRect,
}

pub struct OverlayWindow {
    tool: WebToolWindow,
    game: isize,
    picture: Option<Picture>,
    picture_url: Option<String>,
    follow: Timer,
    dots: RefCell<Vec<Dot>>,
    areas: RefCell<Vec<Area>>,
    name: RefCell<String>,
    client: Cell<Rect>,
    done: Cell<bool>,
    opened: Cell<bool>,
    /// error.log: its first hide was written; later ones aren't.
    hidden_noted: Cell<bool>,
    pub kind: Mode,
    /// The custom theme's colours when that theme is on (AppController.CustomThemeDto), for the bar.
    pub custom_theme: RefCell<Option<Value>>,
    /// A dot let go: its step (0-based) and its new place (parts of the picture area).
    pub on_dot_moved: RefCell<Option<Box<dyn Fn(i32, f64, f64)>>>,
    /// An area let go: its name and its new place.
    pub on_area_changed: RefCell<Option<Box<dyn Fn(&str, MacroRect)>>>,
    /// Pick and Screen: the place clicked and its colour.
    pub on_picked: RefCell<Option<Box<dyn Fn(f64, f64, &str)>>>,
    /// Ended by the person (Done, Esc, Enter, the window gone), not by `end` or a pick.
    pub on_done: RefCell<Option<Box<dyn Fn()>>>,
    /// FormClosed += …
    pub on_closed: RefCell<Vec<Box<dyn Fn()>>>,
}

/// A picture as a PNG data address, for the page.
fn png_url(picture: &Picture) -> Option<String> {
    let img = image::RgbaImage::from_raw(picture.width as u32, picture.height as u32, picture.to_rgba())?;
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, image::ImageFormat::Png).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png.into_inner())))
}

impl OverlayWindow {
    /// `game`: the game's window (zero for Screen). `name`: Edit and Areas: the macro's name (empty for a new one).
    /// `picture`: Pick and Screen: the picture as it is now (this takes it). `screen`: Screen: the monitor it covers.
    pub fn new(game: isize, mode: Mode, name: &str, picture: Option<Picture>, screen: Option<Rect>) -> Rc<OverlayWindow> {
        let picture_url = picture.as_ref().and_then(png_url);
        let me = Rc::new(OverlayWindow {
            tool: WebToolWindow::new(),
            game,
            picture,
            picture_url,
            follow: Timer::new(100),
            dots: RefCell::new(Vec::new()),
            areas: RefCell::new(Vec::new()),
            name: RefCell::new(name.to_string()),
            client: Cell::new(Rect::default()),
            done: Cell::new(false),
            opened: Cell::new(false),
            hidden_noted: Cell::new(false),
            kind: mode,
            custom_theme: RefCell::new(None),
            on_dot_moved: RefCell::new(None),
            on_area_changed: RefCell::new(None),
            on_picked: RefCell::new(None),
            on_done: RefCell::new(None),
            on_closed: RefCell::new(Vec::new()),
        });
        me.tool.bind(Rc::downgrade(&me) as Weak<dyn WebTool>);
        me.tool.set_text(super::ui::TITLE);
        if let Some(s) = screen {
            me.client.set(s);
            me.tool.set_bounds(s);
        }
        super::ui::note_on_top(me.tool.form_ref());
        let w = Rc::downgrade(&me);
        me.follow.on_tick(move || {
            if let Some(o) = w.upgrade() {
                o.follow();
            }
        });
        me
    }

    pub fn tool(&self) -> &WebToolWindow {
        &self.tool
    }

    /// Starts the page, then shows over the game and takes the keyboard (Esc and Enter are its own, not the
    /// game's). A page that can't start ends it, as Esc would.
    pub async fn open(self: Rc<Self>) {
        if self.opened.get() || self.tool.is_disposed() {
            return;
        }
        self.opened.set(true);
        let dynme: Rc<dyn WebTool> = self.clone();
        if !WebToolWindow::ensure_browser(dynme).await || self.tool.is_disposed() {
            if !self.tool.is_disposed() {
                log::note(&format!("Overlay ({:?}): its page couldn't start; ended.", self.kind));
                // No window was made, so close() only disposes it: the ending is told here, or the panel behind a
                // screen pick stayed hidden and Edit/Pick never heard back.
                self.on_form_closed();
                self.tool.close();
            }
            return;
        }
        self.push();
        if self.kind == Mode::Screen {
            self.tool.show();
        } else {
            self.follow();
        }
        self.follow.set_enabled(self.kind != Mode::Screen);
        if self.tool.visible() {
            self.tool.activate();
            self.tool.focus_page();
        }
        let over = if self.kind == Mode::Screen { "the screen".to_string() } else { format!("Roblox window {}", self.game_pid()) };
        let state = if self.tool.visible() {
            format!("shown and activated, {}", window_report::describe(self.tool.handle()))
        } else {
            "not shown yet (the game is minimized or another app is in front)".to_string()
        };
        log::note(&format!("Overlay ({:?}): opened always on top over {over}, {state}.", self.kind));
    }

    /// The points and areas to draw (the page's draft again after each change); the page keeps its own while one is dragged.
    pub fn set(&self, dots: Vec<Dot>, areas: Vec<Area>, name: Option<&str>) {
        *self.dots.borrow_mut() = if self.kind == Mode::Edit { dots } else { Vec::new() };
        *self.areas.borrow_mut() = if matches!(self.kind, Mode::Edit | Mode::Areas) { areas } else { Vec::new() };
        if let Some(n) = name {
            *self.name.borrow_mut() = n.to_string();
        }
        self.push();
    }

    /// Closed by the app (the page ended it, the vault locked): on_done isn't raised.
    pub fn end(&self) {
        self.done.set(true);
        if !self.tool.is_disposed() {
            self.tool.close();
        }
    }

    fn push(&self) {
        if !self.tool.browser_ready() {
            return;
        }
        let mode = match self.kind {
            Mode::Edit => "edit",
            Mode::Areas => "areas",
            Mode::Pick => "pick",
            Mode::Screen => "screen",
        };
        let dots: Vec<Value> = self.dots.borrow().iter().map(|d| json!({ "step": d.step, "x": d.x, "y": d.y })).collect();
        let areas: Vec<Value> = self
            .areas
            .borrow()
            .iter()
            .map(|a| json!({ "name": a.name, "label": a.label, "colour": screen_picker::hex(a.colour), "rect": { "x": a.rect.x, "y": a.rect.y, "w": a.rect.w, "h": a.rect.h } }))
            .collect();
        let data = json!({
            "mode": mode,
            "name": *self.name.borrow(),
            "dots": dots,
            "areas": areas,
            "picture": self.picture_url,
            "theme": themes::current().id,
            "customTheme": self.custom_theme.borrow().clone(),
            "language": l::language(),
        });
        self.tool.send_event("overlay", data);
    }

    /// Over the game's picture area as it is now; hidden while it is minimised or another program is in front
    /// (it is on top of everything, so it would lie over that program), closed when the game is gone.
    fn follow(&self) {
        if self.tool.is_disposed() {
            return;
        }
        if !win_form::is_window(self.game) {
            log::note(&format!("Overlay ({:?}): its Roblox window is gone; ended.", self.kind));
            self.tool.close();
            return;
        }
        let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: plain query into a RECT on the stack.
        let got = unsafe { GetClientRect(hwnd(self.game), &mut rc) } != 0;
        if win_form::is_iconic(self.game) || self.elsewhere() || !got {
            if self.tool.visible() {
                // Once: alt-tabbing back and forth would write a line each time.
                if !self.hidden_noted.get() {
                    let why = if win_form::is_iconic(self.game) {
                        "the game was minimized"
                    } else if self.elsewhere() {
                        "another app came in front"
                    } else {
                        "the game's area couldn't be read"
                    };
                    log::note(&format!("Overlay ({:?}): hidden ({why}).", self.kind));
                }
                self.hidden_noted.set(true);
                self.tool.hide();
            }
            return;
        }
        let mut origin = POINT { x: 0, y: 0 };
        // SAFETY: plain conversion of a POINT on the stack.
        unsafe {
            ClientToScreen(hwnd(self.game), &mut origin);
        }
        let client = Rect::new(origin.x, origin.y, rc.right - rc.left, rc.bottom - rc.top);
        if client.width < 2 || client.height < 2 {
            return;
        }
        if client != self.client.get() || self.tool.bounds() != client {
            self.client.set(client);
            self.tool.set_bounds(client);
        }
        if !self.tool.visible() && self.tool.is_handle_created() {
            self.tool.show();
        }
    }

    /// The game's process id, for error.log.
    fn game_pid(&self) -> u32 {
        win_form::window_pid(self.game)
    }

    /// The window in front is neither the game nor one of Bloxkeep's (this overlay, the macro editor that opened it).
    fn elsewhere(&self) -> bool {
        let front = win_form::foreground_window();
        if front == 0 || front == self.game {
            return false;
        }
        win_form::window_pid(front) != std::process::id()
    }
}

/// Parts of the picture area as the page sent them: kept inside it, rounded to 4 places as the app keeps them.
fn part(o: &Value, name: &str) -> f64 {
    match o.get(name).and_then(Value::as_f64) {
        Some(d) if d.is_finite() => (d.clamp(0.0, 1.0) * 10_000.0).round() / 10_000.0,
        _ => f64::NAN,
    }
}

impl WebTool for OverlayWindow {
    fn tool(&self) -> &WebToolWindow {
        &self.tool
    }
    fn type_name(&self) -> &'static str {
        "OverlayWindow"
    }
    fn page_file(&self) -> &'static str {
        "overlay.html"
    }
    fn starts_top_most(&self) -> bool {
        true
    }
    fn rounded(&self) -> bool {
        false
    }
    fn see_through(&self) -> bool {
        true
    }
    fn shadow(&self) -> bool {
        false
    }
    fn backdrop(&self) -> Color {
        Color::BLACK
    }
    fn keep_browser(&self) -> Duration {
        Duration::from_secs(1)
    }

    fn on_page_ready(&self) {
        self.push();
    }

    fn on_activated(&self) {
        self.tool.focus_page();
    }

    fn on_act(&self, what: &str, args: &Value) {
        match what {
            "done" | "cancel" => self.tool.close(),
            "dot" if self.kind == Mode::Edit => {
                let step = args.get("step").and_then(Value::as_i64).and_then(|s| i32::try_from(s).ok());
                let Some(step) = step else { return };
                let i = self.dots.borrow().iter().position(|d| d.step == step);
                let (x, y) = (part(args, "x"), part(args, "y"));
                if let Some(i) = i {
                    if !x.is_nan() && !y.is_nan() {
                        {
                            let mut dots = self.dots.borrow_mut();
                            dots[i] = Dot { x, y, ..dots[i] };
                        }
                        if let Some(f) = self.on_dot_moved.borrow().as_ref() {
                            f(step, x, y);
                        }
                    }
                }
            }
            "area" if matches!(self.kind, Mode::Edit | Mode::Areas) => {
                let name = str_of(args, "name");
                let at = self.areas.borrow().iter().position(|a| a.name == name);
                if let Some(at) = at {
                    let asked = MacroRect { x: part(args, "x"), y: part(args, "y"), w: part(args, "w"), h: part(args, "h") };
                    if let Some(rect) = macro_options::rect(Some(asked)) {
                        self.areas.borrow_mut()[at].rect = rect;
                        if let Some(f) = self.on_area_changed.borrow().as_ref() {
                            f(&name, rect);
                        }
                    }
                }
            }
            "pick" if matches!(self.kind, Mode::Pick | Mode::Screen) => {
                let Some(p) = self.picture.as_ref() else { return };
                let (px, py) = (part(args, "x"), part(args, "y"));
                if px.is_nan() || py.is_nan() {
                    return;
                }
                // The pixel under the click, from the picture as it was shown (stretched over the window).
                let (r, g, b) = p.rgb(((px * p.width as f64) as i32).clamp(0, p.width - 1), ((py * p.height as f64) as i32).clamp(0, p.height - 1));
                self.done.set(true); // picked: not a cancel
                if let Some(f) = self.on_picked.borrow().as_ref() {
                    f(px, py, &screen_picker::hex(Color { r, g, b }));
                }
                self.tool.close();
            }
            _ => {}
        }
    }

    /// Never moved to the size the page asks (it asks none) or Windows' idea of a new monitor's size: it follows the game.
    fn on_dpi_changed(&self, _new_dpi: u32, _suggested: Rect) {
        let c = self.client.get();
        if !c.is_empty() {
            self.tool.set_bounds(c);
        }
    }

    fn on_form_closed(&self) {
        self.follow.stop();
        for f in self.on_closed.borrow().iter() {
            f();
        }
        if !self.done.get() {
            self.done.set(true);
            if let Some(f) = self.on_done.borrow().as_ref() {
                f();
            }
        }
    }

    fn on_disposing(&self) {
        self.follow.dispose();
    }
}

/// "Pick" with no game open: the monitor under the pointer as it was a moment ago, frozen, with a magnifier, in an
/// OverlayWindow; a click takes the colour under the pointer, Esc cancels. Only a picture of the screen: nothing is
/// clicked or sent anywhere. (The WinForms picker covered every monitor at once; a page has one scale, so this covers the
/// one the pointer is on.)
pub mod screen_picker {
    use std::rc::Rc;

    use serde_json::Value;
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SRCCOPY,
        SelectObject,
    };

    use super::{Mode, OverlayWindow};
    use crate::core::{
        log,
        themes::Color,
        web_tool_window::WebTool,
        win_form,
        win_kit::{Picture, Rect},
    };

    /// Graphics.CopyFromScreen of `screen` into a new picture (opaque).
    fn copy_from_screen(screen: Rect) -> Option<Picture> {
        // SAFETY: a 32-bit top-down DIB section in a memory DC; every GDI object is freed before returning.
        unsafe {
            let screen_dc = GetDC(std::ptr::null_mut());
            let dc = CreateCompatibleDC(screen_dc);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: screen.width,
                biHeight: -screen.height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            let mut out = None;
            if !dib.is_null() && !bits.is_null() {
                let old = SelectObject(dc, dib);
                if BitBlt(dc, 0, 0, screen.width, screen.height, screen_dc, screen.x, screen.y, SRCCOPY) != 0 {
                    let mut pic = Picture::new(screen.width, screen.height);
                    let src = std::slice::from_raw_parts(bits as *const u32, pic.pixels.len());
                    for (d, s) in pic.pixels.iter_mut().zip(src) {
                        *d = 0xFF00_0000 | (s & 0x00FF_FFFF);
                    }
                    out = Some(pic);
                }
                SelectObject(dc, old);
                DeleteObject(dib);
            }
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen_dc);
            out
        }
    }

    /// The owner steps aside while the screen is pictured, then comes back. `suspend`: a panel's Suspended (a panel
    /// stepping aside is not shown again by the panels' tick meanwhile, nor its page let go); None for other windows.
    pub fn pick(owner: Rc<dyn WebTool>, suspend: Option<Rc<dyn Fn(bool)>>, picked: impl Fn(&str) + 'static, custom_theme: Option<Value>) {
        win_form::spawn(async move {
            let was_shown = owner.tool().visible();
            let back = {
                let owner = owner.clone();
                let suspend = suspend.clone();
                Rc::new(move || {
                    if let Some(s) = suspend.as_ref() {
                        s(false);
                    }
                    if was_shown && !owner.tool().is_disposed() {
                        owner.tool().show();
                    }
                })
            };
            if let Some(s) = suspend.as_ref() {
                s(true);
            }
            log::note(&format!("Screen colour picker: {} hidden while the screen is pictured.", owner.type_name()));
            owner.tool().hide();
            win_form::delay(200).await; // the panel gone from the picture
            let screen = win_form::screen_from_point(win_form::cursor_position()).bounds;
            let Some(shot) = copy_from_screen(screen) else {
                log::note("Screen colour picker: the screen couldn't be pictured.");
                back();
                return;
            };
            let form = OverlayWindow::new(0, Mode::Screen, "", Some(shot), Some(screen));
            *form.custom_theme.borrow_mut() = custom_theme;
            *form.on_picked.borrow_mut() = Some(Box::new(move |_, _, colour| picked(colour)));
            let b = back.clone();
            form.on_closed.borrow_mut().push(Box::new(move || b()));
            form.open().await;
        });
    }

    /// "#rrggbb" of a colour.
    pub fn hex(c: Color) -> String {
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    }
}
