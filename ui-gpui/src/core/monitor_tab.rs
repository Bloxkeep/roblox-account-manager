// MonitorTab.cs: the "Game monitor" tab's live pictures. The interface draws the tab (tile frames, names, buttons,
// menus) from the windows this sends it (the "monitor" event, every 1.5 s while the tab shows and at once when a window
// is minimised or opened again), and reports where each tile's picture goes; it can't show other windows, so
// window_thumbnails lays each one there. The interface withdraws a picture its menu would lie under, and all of them
// ({ hidden }) whenever it must show there instead (another tab, a dialog, the lock screen): Windows draws the pictures
// above the whole window.
//
// On the windows thread; the App's part (the windows open now, the event) runs on the core's thread through rt::post,
// and the windows it listed come back here with win_form::invoke.
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
};

use super::{
    app::{App, monitor::MonitorArea},
    log, rt,
    win_form::{self, Timer},
    win_kit::{Point, Rect},
    window_thumbnails::WindowThumbnails,
};

thread_local! {
    static CURRENT: RefCell<Weak<MonitorTab>> = RefCell::new(Weak::new());
}

pub struct MonitorTab {
    host: isize,
    /// The window the page's pixels are counted in; None while there is none.
    page: Box<dyn Fn() -> Option<isize>>,
    pictures: RefCell<Option<WindowThumbnails>>,
    timer: Timer,
    area: RefCell<Option<MonitorArea>>,
    /// The windows the page was last sent: only theirs are pictured, whatever handle a report names.
    known: RefCell<HashSet<isize>>,
    shown: Cell<bool>,
    failed: Cell<bool>,
}

impl MonitorTab {
    pub fn new(host: isize, page: impl Fn() -> Option<isize> + 'static) -> Rc<MonitorTab> {
        let pictures = WindowThumbnails::new(host);
        let me = Rc::new(MonitorTab {
            host,
            page: Box::new(page),
            pictures: RefCell::new(None),
            timer: Timer::new(1500),
            area: RefCell::new(None),
            known: RefCell::new(HashSet::new()),
            shown: Cell::new(false),
            failed: Cell::new(false),
        });
        // Changed comes on the pictures' watch thread: refreshed on the windows thread.
        if let Ok(mut changed) = pictures.on_changed.lock() {
            *changed = Some(Box::new(|| {
                win_form::invoke(|| {
                    if let Some(m) = MonitorTab::current() {
                        m.refresh();
                    }
                })
            }));
        }
        *me.pictures.borrow_mut() = Some(pictures);
        let w = Rc::downgrade(&me);
        me.timer.on_tick(move || {
            if let Some(m) = w.upgrade() {
                m.refresh();
            }
        });
        CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&me));
        me
    }

    /// The monitor tab, on the windows thread.
    pub fn current() -> Option<Rc<MonitorTab>> {
        CURRENT.with(|c| c.borrow().upgrade())
    }

    /// The page's monitor tab and its pictures are here (CSS pixels and its pixel ratio), or must not be covered (None).
    pub fn set_area(&self, area: Option<MonitorArea>) {
        *self.area.borrow_mut() = area;
        self.place();
    }

    /// Puts the pictures where the page last said, or takes them away; also called when the window is minimised,
    /// hidden or back (before the page says so).
    pub fn place(&self) {
        let area = self.area.borrow().clone();
        let page = (self.page)();
        let host_ok = win_form::is_window(self.host) && win_form::is_visible(self.host) && !win_form::is_iconic(self.host);
        let (Some(a), true, Some(page)) = (area, host_ok, page.filter(|&p| win_form::is_window(p) && win_form::is_visible(p))) else {
            self.withdraw();
            return;
        };
        // CSS pixels times the page's pixel ratio are the browser's device pixels, from its top-left corner. Edges are
        // rounded (halves up, as the browser snaps them: 90 px at 125 % starts at 113, not 112), not the size.
        let px = |css: f64, scale: f64| (css * scale).round() as i32;
        let page_origin = win_form::client_origin(page);
        let host_origin = win_form::client_origin(self.host);
        let origin = Point::new(page_origin.x - host_origin.x, page_origin.y - host_origin.y);
        let to_host = |x: f64, y: f64, w: f64, h: f64| {
            let r = Rect::from_ltrb(px(x, a.scale), px(y, a.scale), px(x + w, a.scale), px(y + h, a.scale));
            Rect::new(r.x + origin.x, r.y + origin.y, r.width, r.height)
        };
        // Never past the page: a report from before a resize may be larger than the page is now.
        let clip = Rect::intersect(to_host(a.x, a.y, a.width, a.height), Rect::at(origin, win_form::client_size(page)));
        let mut boxes: HashMap<isize, Rect> = HashMap::new();
        {
            let known = self.known.borrow();
            for p in a.pictures.iter().filter(|p| known.contains(&p.hwnd)) {
                boxes.insert(p.hwnd, to_host(p.x, p.y, p.width, p.height));
            }
        }
        if let Some(pictures) = self.pictures.borrow().as_ref() {
            pictures.place(&boxes, clip);
        }
        // Back on the tab, the pictures go on the tiles it still shows (the windows last sent) first, and the windows are
        // looked up after: looking first kept the tiles empty for that long, and the pictures popped in late (a flash).
        if !self.shown.get() {
            self.shown.set(true);
            self.refresh();
            self.timer.start();
        }
    }

    /// Unregistered before hidden: the pictures are Windows' own and would stay on screen without the page.
    fn withdraw(&self) {
        self.timer.stop();
        self.shown.set(false);
        if let Some(pictures) = self.pictures.borrow().as_ref() {
            pictures.clear();
        }
    }

    /// The windows open now, to the page (a refresh that fails is logged once, not shown: the next tick tries again).
    pub fn refresh(&self) {
        if !self.shown.get() {
            return;
        }
        // PORT: the App's EmitMonitor asked the pictures (on this thread) whether each window is minimised; it gets
        // their answers for the windows pictured now, and asks Windows for any other (what is_minimized does for those).
        let minimized: HashMap<isize, bool> = match self.pictures.borrow().as_ref() {
            Some(p) => self.known.borrow().iter().map(|&h| (h, p.is_minimized(h))).collect(),
            None => HashMap::new(),
        };
        rt::post(move || {
            App::with(|app| {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let clients = app.monitor_clients();
                    let windows: Vec<isize> = clients.iter().map(|c| c.hwnd).collect();
                    app.emit_monitor(&clients, &|h: isize| minimized.get(&h).copied().unwrap_or_else(|| win_form::is_iconic(h) || !win_form::is_visible(h)));
                    windows
                }));
                win_form::invoke(move || {
                    let Some(me) = MonitorTab::current() else { return };
                    match result {
                        Ok(windows) => {
                            *me.known.borrow_mut() = windows.into_iter().collect();
                            me.failed.set(false);
                        }
                        Err(_) => {
                            if !me.failed.get() {
                                log::note("Game monitor: a refresh of its windows failed.");
                            }
                            me.failed.set(true);
                        }
                    }
                });
            })
        });
    }

    pub fn dispose(&self) {
        self.withdraw();
        self.timer.dispose();
        if let Some(mut pictures) = self.pictures.borrow_mut().take() {
            pictures.dispose();
        }
    }
}
