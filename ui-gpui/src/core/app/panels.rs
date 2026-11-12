// AppController.Panels.cs: Macros (Beta): each macro's panel (panel_window.rs, a page beside the game), shown by itself
// while an account plays its game; the settings the player changes there ("$o1"); the on-game overlay
// (overlay_window.rs) that edits the macro's points and areas on the Roblox window itself and picks a place or colour
// there; trying a macro not saved yet; "What it sees" (macro_watch.rs).
//
// PORT: the panels' and the overlay's windows live on the windows thread (win_form), the App on the core thread. The App
// keeps a `PanelHandle` per panel (panel_window.rs) and, for the overlay, only what it is for (its serial number, its
// kind, `overlay_for`); the overlay itself is held on the windows thread (`OVERLAY`) and reached through
// `win_form::invoke`. Its events come back to the App with `rt::post`. "Is it disposed" is therefore known a moment late:
// the core forgets the overlay when its closing reaches it (`overlay_gone`).
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::Engine;
use futures::channel::oneshot;
use serde_json::{Value, json};

use super::App;
use super::controller::{JsonKind, has, id_of, parse_guid, str_of};
use super::launching::require_vault;
use super::macros::{lk, short};
use crate::core::base::{Cancel, Fail, Guid, Json, Res, Stamp, new_guid};
use crate::core::macro_model::{Macro, MacroRect, MacroTrigger, macro_rules, macro_rules::page_json, macro_rules::page_json::Shape};
use crate::core::macro_panel::{MacroOption, area_geometry, macro_options};
use crate::core::macro_player::{self, IMacroDesk, MacroRun, RunRef};
use crate::core::overlay_window::{Area, Mode, OverlayWindow};
use crate::core::panel_window::PanelHandle;
use crate::core::themes::{self, Color};
use crate::core::win_kit::Rect;
use crate::core::{anti_afk, input_turn, l, log, macro_templates, macro_watch, prefs, roblox_processes, rt, shell_form, ui, win_form, window_shot};

const MAX_AUTO_PANELS: i32 = 3;
const STOPPED_SHOWN: Duration = Duration::from_secs(10 * 60);
const UNDO_FOR: Duration = Duration::from_secs(6);

/// What the overlay is on: the macro (nil for a pick), the account, and whether the page asked for it.
pub type OverlayFor = (Guid, Guid, bool);
type Picked = Option<(f64, f64, String)>;

/// The instance fields AppController.Panels.cs declares.
#[derive(Default)]
pub struct PanelsState {
    pub panels: RefCell<HashMap<Guid, PanelHandle>>,
    /// Panels shown by themselves (their game is played), and those the person closed meanwhile (left closed until no
    /// account plays it).
    pub panels_auto: RefCell<HashSet<Guid>>,
    pub panels_closed: RefCell<HashSet<Guid>>,
    /// Each macro's last run that ended with an error, and when the panel first saw it ended ("Stopped: …" for 10 minutes).
    pub last_run: RefCell<HashMap<Guid, (RunRef, Stamp)>>,
    /// The values before "Reset all", for its Undo (6 s).
    pub undo_values: RefCell<HashMap<Guid, (HashMap<String, Option<Json>>, Stamp)>>,
    /// The last window in front that wasn't a panel: where the keyboard goes back to.
    pub last_foreground: Cell<isize>,
    /// _panelStatePush: the serial of the 250 ms push waiting (0: none).
    pub state_push_waiting: Cell<u64>,
    /// _panelFollowLater: restarted by each call; only the newest one runs.
    pub follow_later: Cell<u64>,
    /// Serial numbers for the timers above and the overlays.
    pub serials: Cell<u64>,
    /// _overlay: the overlay showing (its serial, 0: none) and its kind.
    pub overlay: Cell<u64>,
    pub overlay_kind: Cell<Option<Mode>>,
    pub overlay_for: Cell<Option<OverlayFor>>,
    /// A pick waiting for its answer, by the overlay's serial.
    pub pick_wait: RefCell<Option<(u64, oneshot::Sender<Picked>)>>,
    /// The editor's "Try it" going on: its run's macro id and account.
    pub try_run: Cell<Option<(Guid, Guid)>>,
    /// The panels' show/hide by their game error.log was last told.
    pub panels_decided_logged: RefCell<Option<String>>,
    // What it sees.
    pub watch_timer_on: Cell<bool>,
    pub watching: Cell<Option<(Guid, Guid, bool)>>,
    /// The editor's macro as it is now (not saved yet), looked at in place of the saved one.
    pub watch_macro: RefCell<Option<Macro>>,
    pub watch_busy: Cell<bool>,
    /// None: DateTime.MinValue.
    pub watch_read_at: Cell<Option<Stamp>>,
    pub watch_text: RefCell<Vec<macro_watch::Box>>,
}

thread_local! {
    /// The overlay's window, on the windows thread, with its serial.
    static OVERLAY: RefCell<Option<(u64, Rc<OverlayWindow>)>> = const { RefCell::new(None) };
}

/// Work for the overlay with this serial, on the windows thread (dropped when it is gone or replaced).
fn on_overlay(serial: u64, f: impl FnOnce(&Rc<OverlayWindow>) + Send + 'static) {
    win_form::invoke(move || {
        let overlay = OVERLAY.with(|o| o.borrow().as_ref().filter(|(s, _)| *s == serial).map(|(_, w)| w.clone()));
        if let Some(o) = overlay {
            f(&o);
        }
    });
}

/// What an overlay is for: which of its events the App hears, and how.
#[derive(Clone, Copy)]
enum Purpose {
    /// The editor's "Edit on game".
    Edit,
    /// The panel's "Move areas on game", for this macro.
    Areas(Guid),
    /// "Pick on game".
    Pick,
}

fn user(text: String) -> Fail {
    Fail::user(text)
}

/// ColorTranslator.FromHtml of a "#rrggbb" already checked.
fn colour_of(hex: &str) -> Color {
    let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    Color { r: ((n >> 16) & 0xff) as u8, g: ((n >> 8) & 0xff) as u8, b: (n & 0xff) as u8 }
}

/// SameValue: two values as their JSON text.
fn same_value(a: &Option<Json>, b: &Option<Json>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x.to_string() == y.to_string(),
        _ => false,
    }
}

/// A counter's name as the panel shows it: the maker's label when there is one (panel.counterLabels).
fn counter_label(m: &Macro, name: &str) -> String {
    m.panel
        .extra
        .get("counterLabels")
        .and_then(Value::as_object)
        .and_then(|labels| labels.get(name))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| name.to_string())
}

fn panel_option(o: &MacroOption) -> Value {
    let value = if o.kind == "heading" { None } else { Some(macro_options::value_of(o)) };
    let number = o.kind == "number";
    json!({
        "name": o.name,
        "label": o.label,
        "kind": o.kind,
        "tab": o.tab,
        "value": value,
        "choices": o.choices,
        "shown": value.as_ref().map(|v| macro_options::show(o, v)).unwrap_or_default(),
        // A number: its range, step and unit as the page shows them while − and + are held (the host's text replaces it after).
        "min": if number { o.min } else { None },
        "max": if number { o.max } else { None },
        "step": if number { Some(macro_options::step(o)) } else { None },
        "scale": if number { Some(macro_options::shown_scale(o)) } else { None },
        "suffix": if number { Some(macro_options::suffix(o)) } else { None },
        "minShown": if number { o.min.map(|min| macro_options::show(o, &json!(min))) } else { None },
        "maxShown": if number { o.max.map(|max| macro_options::show(o, &json!(max))) } else { None },
    })
}

/// A run's reason as the panel says it (the page has its own wording for these).
fn reason_text(reason: Option<&str>) -> String {
    l::t(match reason {
        Some("personActive") => "you're using the PC",
        Some("locked") => "the PC is locked",
        Some("fullscreen") => "a full-screen app is in front",
        Some("focusRefused") => "couldn't bring the window forward",
        Some("notInGame") => "not in a game",
        Some("covered") => "another window covers the game",
        Some("busy") => "another window's turn",
        Some("recording") => "recording",
        Some("imeOn") => "Vietnamese typing is on",
        _ => "waiting",
    })
}

fn fail_text(reason: Option<&str>) -> String {
    match reason {
        Some("timeout") => l::t("a wait ran out of time"),
        Some("notFound") => l::t("what it looks for wasn't there"),
        Some("missingValue") => l::t("a setting or area is missing"),
        Some("badFormula") => l::t("some math went wrong"),
        Some("missingMacro") => l::t("a macro it runs is gone"),
        Some("windowGone") => l::t("the window closed"),
        Some("maxTime") => l::t("it ran its longest time"),
        Some("stuck") => l::t("it got stuck"),
        _ => reason_text(reason),
    }
}

fn overlay_areas(m: &Macro) -> Vec<Area> {
    m.areas
        .iter()
        .map(|a| {
            let c = a.colour.as_str();
            let ok = c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
            Area {
                name: a.name.clone(),
                label: if a.label.is_empty() { a.name.clone() } else { a.label.clone() },
                colour: colour_of(if ok { c } else { macro_options::AREA_COLOUR }),
                rect: macro_options::rect(Some(a.rect)).unwrap_or(MacroRect { x: 0.25, y: 0.25, w: 0.5, h: 0.5 }),
            }
        })
        .collect()
}

/// The steps' own areas (where a picture, words or a colour are looked for, written on the step rather than named in
/// Areas), named "step:N" or "test:N" for the step's own or its If's; amber, to tell them from named areas.
fn step_areas(m: &Macro) -> Vec<Area> {
    let mut list = Vec::new();
    for (i, s) in m.steps.iter().enumerate() {
        if let Some(own) = macro_options::rect(s.region) {
            list.push(Area { name: format!("step:{i}"), label: l::f("Step {0}", &[&(i + 1)]), colour: colour_of("#f59e0b"), rect: own });
        }
        if let Some(test) = macro_options::rect(s.test.as_ref().and_then(|t| t.region)) {
            list.push(Area { name: format!("test:{i}"), label: l::f("Step {0}", &[&(i + 1)]), colour: colour_of("#f59e0b"), rect: test });
        }
    }
    list
}

/// "step:N" / "test:N" of `step_areas`: the step and whether it is its If's area.
fn step_area_of(name: &str) -> Option<(i32, bool)> {
    let mut parts = name.split(':');
    let (kind, n) = (parts.next()?, parts.next()?);
    if parts.next().is_some() || !(kind == "step" || kind == "test") {
        return None;
    }
    n.parse::<i32>().ok().map(|i| (i, kind == "test"))
}

/// The custom theme for a window's bar, None when it isn't on.
fn custom_theme() -> Option<Value> {
    Some(App::custom_theme_dto()).filter(|v| !v.is_null())
}

/// GetForegroundWindow and the window questions the panels ask.
pub mod panel_native {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible};

    pub fn get_foreground_window() -> isize {
        // SAFETY: no arguments; returns a window handle or null.
        unsafe { GetForegroundWindow() as isize }
    }

    pub fn is_window(h: isize) -> bool {
        // SAFETY: IsWindow accepts any value, a stale or null handle gives false.
        unsafe { IsWindow(h as HWND) != 0 }
    }

    pub fn is_window_visible(h: isize) -> bool {
        // SAFETY: as above, any handle value is accepted.
        unsafe { IsWindowVisible(h as HWND) != 0 }
    }

    pub fn is_iconic(h: isize) -> bool {
        // SAFETY: as above, any handle value is accepted.
        unsafe { IsIconic(h as HWND) != 0 }
    }

    pub fn pid_of(h: isize) -> i32 {
        if h == 0 {
            return 0;
        }
        let mut pid = 0u32;
        // SAFETY: `pid` is a valid out pointer; a stale handle leaves it 0.
        unsafe { GetWindowThreadProcessId(h as HWND, &mut pid) };
        pid as i32
    }
}

impl App {
    fn next_serial(&self) -> u64 {
        let n = self.panels.serials.get() + 1;
        self.panels.serials.set(n);
        n
    }

    /// The vault open now, for the panels (OpenVault).
    pub fn open_vault(&self) -> Option<Arc<crate::core::vault::Vault>> {
        self.open()
    }

    /// Open?.Data.Settings.Macros.FirstOrDefault(id), a copy.
    pub(crate) fn open_macro(&self, id: Guid) -> Option<Macro> {
        let vault = self.open()?;
        let m = vault.data().settings.macros.iter().find(|m| m.id == id).cloned();
        m
    }

    fn shown_panel(&self, id: Guid) -> Option<PanelHandle> {
        self.panels.panels.borrow().get(&id).filter(|p| !p.is_disposed() && p.visible()).cloned()
    }

    // ───────────────────────── commands ─────────────────────────

    pub fn show_panel(&self, id: Guid, show: bool) -> Res<Value> {
        let m = self.macro_by_id(id)?;
        if show {
            self.panels.panels_closed.borrow_mut().remove(&id);
            self.show_panel_window(&m, true);
        } else {
            self.hide_panel_window(id, true);
        }
        self.push_state();
        Ok(Value::Null)
    }

    /// A setting's value from the page or the panel, saved at once; runs going on read it from their next step.
    /// `from_panel`: from the panel beside the game: saved and told to the main page a moment later, once for a burst.
    pub fn set_option(&self, id: Guid, name: &str, value: &Value, from_panel: bool) -> Res<Value> {
        let value = self.vault_mut(|d| -> Res<Json> {
            let m = d.settings.macros.iter_mut().find(|m| m.id == id).ok_or_else(|| user(l::t("That macro isn't there any more.")))?;
            let o = m.options.iter_mut().find(|o| o.name == name).ok_or_else(|| user(l::t("That setting isn't there any more.")))?;
            let fits = macro_options::normalize(o, value).ok_or_else(|| user(l::t("That value doesn't fit the setting.")))?;
            o.value = Some(fits.clone());
            Ok(fits)
        })??;
        self.live(id, |copy| {
            if let Some(o) = copy.options.iter_mut().find(|o| o.name == name) {
                o.value = Some(value.clone());
            }
        });
        self.options_changed(id, from_panel);
        Ok(Value::Null)
    }

    /// Saved and shown: at once from the page; from the panel, saved within seconds (on lock and exit at the latest) and
    /// the main page told once a quarter second, so holding + doesn't write the vault and redraw the page forty times a second.
    fn options_changed(&self, id: Guid, from_panel: bool) {
        self.refresh_panel(id);
        if !from_panel {
            self.persist();
            self.push_state();
            return;
        }
        self.persist_soon();
        if self.panels.state_push_waiting.get() == 0 {
            let serial = self.next_serial();
            self.panels.state_push_waiting.set(serial);
            rt::spawn(async move {
                rt::sleep(Duration::from_millis(250)).await;
                App::with(|app| {
                    if app.panels.state_push_waiting.get() == serial {
                        app.panels.state_push_waiting.set(0);
                        app.push_state();
                    }
                });
            });
        }
    }

    /// "Reset all": every setting back to how the macro was made (the values before kept 6 s for Undo).
    pub fn reset_options(&self, id: Guid, from_panel: bool) -> Res<()> {
        let m = self.macro_by_id(id)?;
        if from_panel {
            let values: HashMap<String, Option<Json>> = m.options.iter().map(|o| (o.name.clone(), o.value.clone())).collect();
            self.panels.undo_values.borrow_mut().insert(id, (values, Stamp::utc_now().plus_secs(UNDO_FOR.as_secs_f64())));
        }
        self.vault_mut(|d| {
            if let Some(m) = d.settings.macros.iter_mut().find(|m| m.id == id) {
                for o in &mut m.options {
                    o.value = o.default.clone();
                }
            }
        })?;
        self.live(id, |copy| {
            for o in &mut copy.options {
                o.value = o.default.clone();
            }
        });
        self.options_changed(id, from_panel);
        Ok(())
    }

    /// "Undo" after "Reset all": the values as they were, if it is still within its 6 s.
    fn undo_reset(&self, id: Guid) {
        let Some((values, until)) = self.panels.undo_values.borrow_mut().remove(&id) else { return };
        if Stamp::utc_now() > until || self.open_macro(id).is_none() {
            return;
        }
        let Ok(now) = self.vault_mut(|d| {
            let mut now: HashMap<String, Option<Json>> = HashMap::new();
            if let Some(m) = d.settings.macros.iter_mut().find(|m| m.id == id) {
                for o in &mut m.options {
                    if let Some(v) = values.get(&o.name) {
                        o.value = match v.as_ref().and_then(|was| macro_options::normalize(o, was)) {
                            Some(fits) => Some(fits),
                            None => o.default.clone(),
                        };
                    }
                    now.insert(o.name.clone(), o.value.clone());
                }
            }
            now
        }) else {
            return;
        };
        self.live(id, |copy| {
            for o in &mut copy.options {
                // `?.Value ?? o.Value`: a setting without a value keeps the run's.
                if let Some(Some(v)) = now.get(&o.name) {
                    o.value = Some(v.clone());
                }
            }
        });
        self.options_changed(id, true);
    }

    /// An area moved on the game from the panel ("Move areas on game"), saved at once.
    pub fn set_area(&self, id: Guid, name: &str, rect: MacroRect) -> Res<()> {
        self.macro_by_id(id)?;
        let Some(fits) = macro_options::rect(Some(rect)) else { return Ok(()) };
        let found = self.vault_mut(|d| {
            let area = d.settings.macros.iter_mut().find(|m| m.id == id).and_then(|m| m.areas.iter_mut().find(|a| a.name == name));
            match area {
                Some(a) => {
                    a.rect = fits;
                    true
                }
                None => false,
            }
        })?;
        if !found {
            return Ok(());
        }
        self.live(id, |copy| {
            if let Some(c) = copy.areas.iter_mut().find(|a| a.name == name) {
                c.rect = fits;
            }
        });
        self.persist_soon();
        self.push_state();
        Ok(())
    }

    /// The copies runs going on play (a run keeps the steps it started with, but follows its settings).
    /// PORT: a run's macro is an Arc<Macro> here (shared with the player's thread), so it is replaced by a changed copy
    /// under the run's lock instead of changed in place.
    fn live(&self, id: Guid, change: impl Fn(&mut Macro)) {
        let Some(player) = self.macros.macro_player.borrow().clone() else { return };
        for run in player.runs() {
            let mut r = lk(&run);
            if !r.finished() && !r.trial && r.r#macro.id == id {
                let mut copy = (*r.r#macro).clone();
                change(&mut copy);
                r.r#macro = Arc::new(copy);
            }
        }
    }

    pub fn macro_templates_dto(&self) -> Res<Json> {
        macro_templates::all()
    }

    // ───────────────────────── running from a panel ─────────────────────────

    /// The accounts a panel's Run plays on: the macro's own, or for the selected accounts those in its panel's game,
    /// else the Roblox window that was in front before the panel, else those it last ran on, else the one window open.
    pub fn panel_accounts(&self, m: &Macro) -> Vec<Guid> {
        let running = self.controller.running.borrow();
        if m.targets == "accounts" {
            return m.account_ids.iter().copied().filter(|id| running.contains_key(id)).collect();
        }
        if m.targets == "all" {
            return running.keys().copied().collect();
        }
        let in_game: Vec<Guid> = match m.panel.place_id {
            Some(place) => {
                let logs = self.controller.logs.borrow();
                running
                    .iter()
                    .filter(|(_, (pid, _))| logs.state_of(*pid).is_some_and(|st| st.status == "ingame" && st.place_id == Some(place)))
                    .map(|(id, _)| *id)
                    .collect()
            }
            None => Vec::new(),
        };
        if !in_game.is_empty() {
            return in_game;
        }
        let mut front = roblox_processes::foreground_pid();
        let fg = panel_native::get_foreground_window();
        if self.panels.panels.borrow().values().any(|p| !p.is_disposed() && p.is_handle_created() && p.handle() == fg) {
            front = panel_native::pid_of(self.panels.last_foreground.get()); // the panel is in front: the game before it
        }
        let in_front: Vec<Guid> = running.iter().filter(|(_, (pid, _))| *pid == front).map(|(id, _)| *id).collect();
        if !in_front.is_empty() {
            return in_front;
        }
        let last: Vec<Guid> =
            self.macros.macro_last_accounts.borrow().get(&m.id).map(|l| l.iter().copied().filter(|id| running.contains_key(id)).collect()).unwrap_or_default();
        if !last.is_empty() {
            return last;
        }
        if running.len() == 1 { running.keys().copied().collect() } else { Vec::new() }
    }

    /// The account the panel works on the game with (pick, edit areas): its first, else any window open; nil for none.
    fn panel_account(&self, m: &Macro) -> Guid {
        let first = self.panel_accounts(m).first().copied();
        first.or_else(|| self.controller.running.borrow().keys().next().copied()).unwrap_or(Guid::nil())
    }

    /// A panel's message for its row (a setting's name) or its status line (None); a toast when no panel shows.
    pub fn panel_error(&self, id: Guid, name: Option<&str>, text: &str) {
        match self.shown_panel(id) {
            Some(panel) => panel.error(name, text),
            None => self.toast(text, "warn"),
        }
    }

    /// The panel's Run: plays the macro, or stops it when it runs.
    pub fn panel_run(&self, id: Guid) -> Res<()> {
        let Some(m) = self.open_macro(id) else { return Ok(()) };
        let player = self.macros.macro_player.borrow().clone();
        if let Some(player) = player
            && player.runs().iter().any(|r| {
                let r = lk(r);
                !r.finished() && !r.trial && r.r#macro.id == id
            })
        {
            player.stop(Some(id), "byUser", None, false);
            self.push_state();
            self.refresh_panel(id);
            return Ok(());
        }
        if self.macros.recorder.borrow().is_some() {
            self.panel_error(id, None, &l::t("Runs wait while recording."));
            return Ok(());
        }
        let accounts = self.panel_accounts(&m);
        if accounts.is_empty() {
            self.panel_error(id, None, &l::f("{0}: no Roblox window to run it in.", &[&m.name]));
            return Ok(());
        }
        self.panels.last_run.borrow_mut().remove(&id);
        self.queue_macro(&m, &accounts)?;
        self.refresh_panel(id);
        Ok(())
    }

    /// What the panel shows now (the page puts it in words): its state, who runs, the counters, since when, the last
    /// error, and what can be done.
    pub fn panel_now(&self, m: &Macro) -> (String, Value) {
        let id = m.id;
        let player = self.macros.macro_player.borrow().clone();
        let all: Vec<RunRef> = player.as_ref().map(|p| p.runs()).unwrap_or_default();
        let runs: Vec<RunRef> = all
            .iter()
            .filter(|r| {
                let r = lk(r);
                !r.finished() && !r.trial && r.r#macro.id == id
            })
            .cloned()
            .collect();
        // The last run that stopped on an error, until the next run or 10 minutes.
        let ended = all
            .iter()
            .rev()
            .find(|r| {
                let r = lk(r);
                r.finished() && !r.trial && r.r#macro.id == id
            })
            .cloned();
        {
            let mut last_run = self.panels.last_run.borrow_mut();
            if let Some(ended) = &ended {
                let failed = lk(ended).status == "failed";
                let seen = last_run.get(&id).map(|(run, _)| Arc::ptr_eq(run, ended));
                if failed && seen != Some(true) {
                    last_run.insert(id, (ended.clone(), Stamp::utc_now()));
                } else if !failed && seen == Some(false) {
                    last_run.remove(&id);
                }
            }
            if !runs.is_empty() {
                last_run.remove(&id);
            }
        }
        let failed: Option<RunRef> = self
            .panels
            .last_run
            .borrow()
            .get(&id)
            .filter(|(_, seen_at)| Stamp::utc_now().secs_since(*seen_at) < STOPPED_SHOWN.as_secs_f64())
            .map(|(run, _)| run.clone());
        let has_game = !self.panel_accounts(m).is_empty();
        let recording = self.macros.recorder.borrow().as_ref().map(|r| r.account_id);
        let statuses: Vec<String> = runs.iter().map(|r| lk(r).status.clone()).collect();
        let state = if recording.is_some() {
            "recording"
        } else if !runs.is_empty() {
            if statuses.iter().any(|s| s == "running") || statuses.iter().all(|s| s == "waiting") { "running" } else { "paused" }
        } else if failed.is_some() {
            "stopped"
        } else if has_game {
            "ready"
        } else {
            "noGame"
        };
        let vars = player.as_ref().map(|p| p.state().vars.clone()).unwrap_or_default();
        let counters: serde_json::Map<String, Value> = macro_options::counters(m)
            .into_iter()
            .map(|c| {
                let sum: f64 = vars.values().map(|d| d.get(&c).copied().unwrap_or(0.0)).sum();
                (c, json!(sum))
            })
            .collect();
        let name = |account: Guid| self.find_account(account).map(|acc| self.name_text(&acc)).unwrap_or_else(|| "?".into());
        let changed = m.options.iter().any(|o| o.kind != "heading" && !same_value(&o.value, &o.default));
        let runs_dto: Vec<Value> = runs
            .iter()
            .map(|r| {
                let r = lk(r);
                json!({
                    "name": name(r.account_id),
                    "status": if r.status == "running" && r.reason.as_deref() == Some("recovering") { "recovering".to_string() } else { r.status.clone() },
                    "reason": if r.status == "paused" { Some(reason_text(r.reason.as_deref())) } else { None },
                })
            })
            .collect();
        let started_at = runs.iter().map(|r| lk(r).started_at).min();
        let last_error = failed.as_ref().map(|f| {
            let f = lk(f);
            json!({ "text": fail_text(f.reason.as_deref()), "step": f.shown_pc(), "section": f.section })
        });
        let undoable = self.panels.undo_values.borrow().get(&id).is_some_and(|(_, until)| Stamp::utc_now() < *until);
        let dto = json!({
            "state": state,
            "runs": runs_dto,
            "counters": counters,
            "startedAt": started_at,
            "lastError": last_error,
            "editingAreas": self.editing_areas_of(id),
            "hotkeysOn": prefs::lock().global_hotkeys,
            "hasGame": has_game,
            "hasWindow": !self.controller.running.borrow().is_empty(),
            "recording": recording.map(|acc| json!({ "account": name(acc) })),
            "undoable": undoable,
            "changed": changed,
            "features": self.features_now(m, &runs),
        });
        (state.to_string(), dto)
    }

    /// While it runs: each feature switched on, by its switch, as the panel says it under the switch: playing (at its
    /// step), due at this safe point, stopped last time (why), its next time (it plays every so often), or at the end of
    /// a round. The run playing it is the one shown, else the first.
    fn features_now(&self, m: &Macro, runs: &[RunRef]) -> Option<Value> {
        let player = self.macros.macro_player.borrow().clone()?;
        if runs.is_empty() || m.features.is_empty() {
            return None;
        }
        let mut now = serde_json::Map::new();
        for f in m.features.iter().filter(|f| macro_player::switch_on(m, f)) {
            let run = runs
                .iter()
                .find(|r| lk(r).section.as_deref() == Some(f.id.as_str()))
                .or_else(|| runs.iter().find(|r| lk(r).due.contains(&f.id)))
                .unwrap_or(&runs[0]);
            // The player's state is read with no run locked (the player's thread takes them the other way round).
            let account = lk(run).account_id;
            let record = player.state().features.get(&(m.id, account, f.id.clone())).cloned();
            let r = lk(run);
            let value = if r.section.as_deref() == Some(f.id.as_str()) {
                let of = match r.origins.get(&f.id) {
                    Some(o) if !o.is_empty() => o[o.len() - 1] + 1,
                    _ => f.steps.len() as i32,
                };
                json!({ "state": "playing", "step": r.shown_pc() + 1, "of": of })
            } else if r.due.contains(&f.id) {
                json!({ "state": "due" })
            } else if let Some(failed) = record.as_ref().and_then(|rec| rec.failed.clone()) {
                json!({ "state": "failed", "why": fail_text(Some(&failed)) })
            } else if let (Some(every), Some(last)) = (f.every_ms, record.as_ref().and_then(|rec| rec.last_ran))
                && Stamp::utc_now().secs_since(last) < every as f64 / 1000.0
            {
                json!({ "state": "next", "at": last.plus_secs(every as f64 / 1000.0) })
            } else {
                json!({ "state": "round" })
            };
            now.insert(f.option.clone(), value);
        }
        Some(Value::Object(now))
    }

    /// The page of a macro's panel: the macro (its settings with their values as shown), what it does now, the theme and
    /// the language.
    pub fn panel_message(&self, id: Guid, pill: bool) -> Value {
        let m = self.open_macro(id);
        let dto = m.as_ref().map(|m| {
            json!({
                "id": m.id,
                "name": m.name,
                "hotkey": m.hotkey,
                "tabs": m.panel.tabs,
                "areasCount": m.areas.len(),
                "counters": macro_options::counters(m).iter().map(|c| json!({ "name": c, "label": counter_label(m, c) })).collect::<Vec<_>>(),
                "options": m.options.iter().map(panel_option).collect::<Vec<_>>(),
            })
        });
        json!({
            "macro": dto,
            "now": m.as_ref().map(|m| self.panel_now(m).1),
            "theme": themes::current().id,
            "customTheme": App::custom_theme_dto(),
            "language": l::language(),
            "pill": pill,
        })
    }

    // ───────────────────────── what the panel's page does ─────────────────────────

    /// Something clicked on a panel (PanelWindow's page).
    pub fn panel_act(self: &Rc<Self>, panel: &PanelHandle, what: &str, a: &Value) {
        let id = panel.macro_id();
        let name = str_of(a, "name");
        if let Err(e) = self.panel_act_inner(panel, what, a, &name) {
            match e {
                Fail::User(text) => {
                    self.refresh_panel(id);
                    self.panel_error(id, if name.is_empty() { None } else { Some(&name) }, &text);
                }
                // PORT: only a UserError was caught in C#; anything else reached the page's message handler, which logged it.
                other => log::write(&other),
            }
        }
    }

    fn panel_act_inner(self: &Rc<Self>, panel: &PanelHandle, what: &str, a: &Value, name: &str) -> Res<()> {
        let id = panel.macro_id();
        let number_option = || self.open_macro(id).and_then(|m| m.options.into_iter().find(|o| o.name == name && o.kind == "number"));
        match what {
            "run" | "stop" => {
                self.panel_run(id)?;
                self.hand_back(panel);
            }
            "set" if a.get("value").is_some() => {
                self.set_option(id, name, &a["value"], true)?;
            }
            "set" if a.get("key").is_some() => {
                let key = &a["key"];
                let refused = self
                    .open_macro(id)
                    .and_then(|m| m.options.into_iter().find(|o| o.name == name))
                    .is_some_and(|ko| macro_options::normalize(&ko, key).is_none());
                if refused {
                    self.panel_error(id, Some(name), &l::t("Can't use this key"));
                } else {
                    self.set_option(id, name, key, true)?;
                }
            }
            "nudge" if number_option().is_some() => {
                let o = number_option().unwrap_or_default();
                // (int)Math.Clamp(by, -100000, 100000): the fraction cut off.
                let by = a.get("by").and_then(Value::as_f64).map(|b| b.clamp(-100000.0, 100000.0) as i32).unwrap_or(0);
                if by != 0 {
                    self.set_option(id, name, &macro_options::nudge(&o, by), true)?;
                } else {
                    self.refresh_panel(id);
                }
            }
            "typed" if number_option().is_some() => {
                let o = number_option().unwrap_or_default();
                let text = str_of(a, "text");
                let Some(raw) = macro_options::typed_number(&o, &text) else {
                    self.refresh_panel(id);
                    self.panel_error(id, Some(name), &l::t("That isn't a number."));
                    return Ok(());
                };
                self.set_option(id, name, &json!(raw), true)?;
                // Kept in its range: said why it isn't what was typed.
                if let Some(max) = o.max
                    && raw > max
                {
                    self.panel_error(id, Some(name), &l::f("Max is {0}", &[&macro_options::show(&o, &json!(max))]));
                } else if let Some(min) = o.min
                    && raw < min
                {
                    self.panel_error(id, Some(name), &l::f("Min is {0}", &[&macro_options::show(&o, &json!(min))]));
                }
            }
            "reset" => self.reset_options(id, true)?,
            "undoReset" => self.undo_reset(id),
            "pickColour" => {
                let (app, name, panel) = (self.clone(), name.to_string(), panel.clone());
                rt::spawn(async move { app.panel_pick_colour(id, name, panel).await });
            }
            "pickPoint" => {
                let (app, name) = (self.clone(), name.to_string());
                rt::spawn(async move { app.panel_pick_point(id, name).await });
            }
            "areas" => self.toggle_areas_on_game(id),
            "hide" => panel.dismiss(),
            "snap" => panel.snap(self),
            "away" => self.hand_back(panel),
            "edit" => {
                let step = a.get("step").and_then(Value::as_i64).map(|s| s as i32);
                // The feature whose steps it is (none: the main steps).
                let section = a.get("section").and_then(Value::as_str).map(str::to_string);
                if shell_form::is_attached() {
                    log::note(&format!("Macro panel {}: main window brought to the front to edit the macro.", short(id)));
                    shell_form::with(|shell| shell.show_in_front("macro panel"));
                }
                self.emit("editMacro", json!({ "id": id, "step": step, "section": section }));
            }
            _ => {}
        }
        Ok(())
    }

    // ───────────────────────── panel windows ─────────────────────────

    fn show_panel_window(&self, m: &Macro, activate: bool) {
        if self.controller.disposed.get() || self.open().is_none() {
            return;
        }
        let existing = self.panels.panels.borrow().get(&m.id).filter(|p| !p.is_disposed()).cloned();
        let panel = match existing {
            Some(p) => p,
            None => {
                // Its Dismissed is `panel_dismissed`, called by the window.
                let p = PanelHandle::new(m.id);
                self.panels.panels.borrow_mut().insert(m.id, p.clone());
                p
            }
        };
        // Shown by hand counts as by hand from then on (an update waits for it).
        panel.set_auto_shown(!activate && (panel.auto_shown() || !panel.visible()));
        panel.open(activate);
    }

    /// panel.Dismissed: the person closed it.
    pub fn panel_dismissed(&self, id: Guid) {
        self.panels.panels_auto.borrow_mut().remove(&id);
        if self.open_macro(id).is_some_and(|m| m.panel.auto_show) {
            self.panels.panels_closed.borrow_mut().insert(id);
        }
        if let Some((m, _, false)) = self.panels.overlay_for.get()
            && m == id
        {
            self.close_overlay(true);
        }
        self.push_state();
    }

    fn hide_panel_window(&self, id: Guid, by_person: bool) {
        if let Some(panel) = self.shown_panel(id) {
            log::note(&format!("Macro panel {}: hidden ({}).", short(id), if by_person { "by the person" } else { "its game ended" }));
            panel.hide_quietly();
        }
        self.panels.panels_auto.borrow_mut().remove(&id);
        if by_person && self.open_macro(id).is_some_and(|m| m.panel.auto_show) {
            self.panels.panels_closed.borrow_mut().insert(id);
        }
        if let Some((m, _, false)) = self.panels.overlay_for.get()
            && m == id
        {
            self.close_overlay(true);
        }
    }

    /// The macro deleted: its panel goes (disposed: a Close would only hide it and keep its browser), with its place.
    pub fn close_panel_window(&self, id: Guid) {
        let removed = self.panels.panels.borrow_mut().remove(&id);
        if let Some(panel) = removed
            && !panel.is_disposed()
        {
            panel.dispose();
        }
        self.panels.panels_auto.borrow_mut().remove(&id);
        self.panels.panels_closed.borrow_mut().remove(&id);
        self.panels.last_run.borrow_mut().remove(&id);
        self.panels.undo_values.borrow_mut().remove(&id);
        if let Some((m, _, false)) = self.panels.overlay_for.get()
            && m == id
        {
            self.close_overlay(true);
        }
        let removed = prefs::lock().panel_windows.shift_remove(&id.to_string()).is_some();
        if removed {
            prefs::save();
        }
    }

    /// The panel shows what is true now (values patched on its page, never drawn anew).
    pub fn refresh_panel(&self, id: Guid) {
        if let Some(panel) = self.shown_panel(id) {
            panel.push(self);
        }
    }

    pub fn panels_shown_dto(&self) -> Value {
        if self.open().is_none() {
            return json!([]);
        }
        let ids: Vec<Guid> = self.panels.panels.borrow().iter().filter(|(_, p)| !p.is_disposed() && p.visible()).map(|(id, _)| *id).collect();
        json!(ids)
    }

    /// The game window a panel goes beside: its account's, in screen pixels.
    pub fn panel_game_rect(&self, id: Guid) -> Option<Rect> {
        let m = self.open_macro(id)?;
        let account = self.panel_account(&m);
        if account.is_nil() {
            return None;
        }
        self.macro_desk_now().window(account).map(|w| w.client).filter(|c| c.width > 0)
    }

    /// The other panels showing, where they are (a new one goes below them).
    pub fn panels_shown_besides(&self, me: &PanelHandle) -> Vec<Rect> {
        self.panels.panels.borrow().values().filter(|p| *p != me && !p.is_disposed() && p.visible()).map(|p| p.bounds()).collect()
    }

    /// A panel just showed: on top as the window in front says; shown by itself, the keyboard stays where it was.
    pub fn panel_shown(&self, panel: &PanelHandle) {
        self.panels_follow_foreground();
        if panel.auto_shown() {
            // The browser starting can take the activation with it (TrayWindow saw it): given straight back.
            self.give_back_if_taken(panel);
            let later = panel.clone();
            rt::spawn(async move {
                rt::sleep(Duration::from_millis(400)).await;
                if !later.is_disposed() {
                    App::with(|app| app.give_back_if_taken(&later));
                }
            });
        }
        self.push_state();
    }

    fn give_back_if_taken(&self, panel: &PanelHandle) {
        let last = self.panels.last_foreground.get();
        if panel.before() == 0 && last != 0 {
            panel.set_before(last);
        }
        self.hand_back(panel);
    }

    /// A panel took the keyboard: which window had it before (lParam is another program's only within this one).
    pub fn panel_activated(&self, panel: &PanelHandle, previous: isize) {
        let before = if previous != 0 && !self.is_panel(previous) { previous } else { self.panels.last_foreground.get() };
        if before != 0 && !self.is_panel(before) {
            panel.set_before(before);
        }
    }

    fn is_panel(&self, hwnd: isize) -> bool {
        hwnd != 0 && self.panels.panels.borrow().values().any(|p| !p.is_disposed() && p.is_handle_created() && p.handle() == hwnd)
    }

    /// A Roblox window of an account Bloxkeep runs, shown and not minimised.
    fn is_running_game(&self, hwnd: isize) -> bool {
        if hwnd == 0 || !panel_native::is_window(hwnd) {
            return false;
        }
        let pid = panel_native::pid_of(hwnd);
        pid != 0 && self.controller.running.borrow().values().any(|(p, _)| *p == pid)
    }

    /// The keyboard back to the game after Run or Stop, or once the pointer left the panel after a change: only while
    /// the panel is still in front, only to the running account's Roblox window that was in front before it, never
    /// unhiding or restoring it, and only between a macro's turns (a turn busy: nothing, never queued). Typing a number
    /// and "Press a key…" keep it (the page doesn't ask then).
    fn hand_back(&self, panel: &PanelHandle) {
        if panel.is_disposed() || !panel.is_handle_created() || panel_native::get_foreground_window() != panel.handle() {
            return;
        }
        let target = panel.before();
        if !self.is_running_game(target) || !panel_native::is_window_visible(target) || panel_native::is_iconic(target) || roblox_processes::is_hidden(target) {
            return;
        }
        let Some(turn) = input_turn::try_enter("focus") else {
            log::note(&format!(
                "Macro panel {}: keyboard not handed back to Roblox window {}, a nudge, picture or macro turn is under way.",
                short(panel.macro_id()),
                panel_native::pid_of(target)
            ));
            return;
        };
        anti_afk::give_back(target);
        drop(turn);
    }

    /// The panels stay on top while the window in front is a running account's Roblox, Bloxkeep's own (the main window,
    /// the overlay, a panel); when another program comes forward they go just below it, never to the bottom. Called by
    /// the foreground hook (cores.rs); a window brought forward for a moment by a macro's turn waits 0.7 s.
    pub fn panels_follow_foreground(&self) {
        let front = panel_native::get_foreground_window();
        if front != 0 && !self.is_panel(front) {
            self.panels.last_foreground.set(front);
        }
        let shown: Vec<PanelHandle> = self.panels.panels.borrow().values().filter(|p| !p.is_disposed() && p.visible()).cloned().collect();
        if shown.is_empty() {
            return;
        }
        if self.windows_in_turn() {
            let serial = self.next_serial();
            self.panels.follow_later.set(serial);
            rt::spawn(async move {
                rt::sleep(Duration::from_millis(700)).await;
                App::with(|app| {
                    if app.panels.follow_later.get() == serial {
                        app.panels.follow_later.set(0);
                        app.panels_follow_foreground();
                    }
                });
            });
            return;
        }
        // A message box of Bloxkeep's open: the panels stay lowered under it (Ui.Modal), never raised over it again.
        if ui::modals_open() > 0 {
            if front != 0 && !self.is_panel(front) {
                for panel in &shown {
                    panel.layer(false, front);
                }
            }
            return;
        }
        let on_top = front == 0 || self.is_panel(front) || panel_native::pid_of(front) as u32 == std::process::id() || self.is_running_game(front);
        for panel in &shown {
            panel.layer(on_top, front);
        }
    }

    /// Every process tick: panels shown and hidden by their game, and what they show.
    pub fn tick_panels(&self) {
        let Some(vault) = self.open() else { return };
        let macros = vault.data().settings.macros.clone();
        // No macro and no panel: nothing to show, hide or refresh (the usual case), so no lists built every tick.
        if macros.is_empty() && self.panels.panels.borrow().is_empty() && self.panels.panels_auto.borrow().is_empty() {
            return;
        }
        let in_game: Vec<(Guid, i64)> = {
            let running = self.controller.running.borrow();
            let logs = self.controller.logs.borrow();
            running
                .iter()
                .filter_map(|(id, (pid, _))| logs.state_of(*pid).filter(|st| st.status == "ingame").and_then(|st| st.place_id).filter(|p| *p > 0).map(|p| (*id, p)))
                .collect()
        };
        let places: HashSet<i64> = in_game.iter().map(|(_, p)| *p).collect();
        let playing: HashSet<Guid> = in_game.iter().map(|(id, _)| *id).collect();
        let (show, hide) = {
            let shown = |id: Guid| self.shown_panel(id).is_some();
            let any_game = |m: &Macro| self.panel_accounts(m).iter().any(|a| playing.contains(a));
            let mut auto = self.panels.panels_auto.borrow_mut();
            let mut closed = self.panels.panels_closed.borrow_mut();
            macro_options::auto_show(&macros, &places, &mut auto, &mut closed, &shown, Some(&any_game), MAX_AUTO_PANELS)
        };
        // Shown and hidden by their game: logged once per change (a panel still opening is asked again each tick).
        let decided = if show.len() + hide.len() == 0 {
            String::new()
        } else {
            format!(
                "show {}; hide {}",
                show.iter().map(|i| short(*i)).collect::<Vec<_>>().join(", "),
                hide.iter().map(|i| short(*i)).collect::<Vec<_>>().join(", ")
            )
        };
        if self.panels.panels_decided_logged.borrow().as_deref() != Some(decided.as_str()) && !decided.is_empty() {
            log::note(&format!("Macro panels by their game: {decided}."));
        }
        *self.panels.panels_decided_logged.borrow_mut() = Some(decided);
        for id in &hide {
            self.hide_panel_window(*id, false);
        }
        for id in &show {
            let suspended = self.panels.panels.borrow().get(id).is_some_and(|p| p.suspended());
            if let Some(m) = self.open_macro(*id)
                && !suspended
            {
                self.show_panel_window(&m, false);
            }
        }
        if !show.is_empty() || !hide.is_empty() {
            self.push_state();
        }
        let shown: Vec<PanelHandle> = self.panels.panels.borrow().values().filter(|p| !p.is_disposed() && p.visible()).cloned().collect();
        for panel in shown {
            panel.push(self);
        }
    }

    /// Settings › Appearance changed: every panel in the new theme at once.
    pub fn apply_theme_to_panels(&self) {
        let all: Vec<PanelHandle> = self.panels.panels.borrow().values().filter(|p| !p.is_disposed()).cloned().collect();
        for panel in all {
            panel.push(self);
        }
    }

    /// The vault locked: every panel goes (disposed, with its browser), with the overlay, the try and the watch.
    pub fn close_panels_for_lock(&self) {
        let all: Vec<PanelHandle> = self.panels.panels.borrow().values().filter(|p| !p.is_disposed()).cloned().collect();
        for panel in all {
            panel.dispose();
        }
        self.panels.panels.borrow_mut().clear();
        self.panels.panels_auto.borrow_mut().clear();
        self.panels.panels_closed.borrow_mut().clear();
        self.panels.last_run.borrow_mut().clear();
        self.panels.undo_values.borrow_mut().clear();
        self.panels.state_push_waiting.set(0);
        self.panels.try_run.set(None);
        self.close_overlay(false);
        self.stop_watch();
    }

    // ───────────────────────── on the game ─────────────────────────

    /// A new overlay on the windows thread, its events wired to the App by `purpose`; returns its serial (the App's
    /// `_overlay` from now on).
    fn new_overlay(&self, game: isize, mode: Mode, name: &str, picture: Option<crate::core::win_kit::Picture>, purpose: Purpose) -> u64 {
        let serial = self.next_serial();
        let name = name.to_string();
        let theme = custom_theme();
        win_form::invoke(move || {
            let overlay = OverlayWindow::new(game, mode, &name, picture, None);
            *overlay.custom_theme.borrow_mut() = theme;
            overlay.on_closed.borrow_mut().push(Box::new(move || {
                OVERLAY.with(|o| {
                    let mut o = o.borrow_mut();
                    if o.as_ref().is_some_and(|(s, _)| *s == serial) {
                        *o = None;
                    }
                });
                rt::post(move || App::with(|app| app.overlay_gone(serial)));
            }));
            match purpose {
                Purpose::Edit => {
                    *overlay.on_dot_moved.borrow_mut() = Some(Box::new(|step, x, y| {
                        rt::post(move || App::with(|app| app.emit("onGameEdit", json!({ "step": step, "x": x, "y": y }))));
                    }));
                    *overlay.on_area_changed.borrow_mut() = Some(Box::new(|area: &str, rect: MacroRect| {
                        let area = area.to_string();
                        rt::post(move || {
                            App::with(|app| {
                                let rect = page_json::write(&rect, Shape::Rect);
                                match step_area_of(&area) {
                                    Some((step, test)) => app.emit("onGameEdit", json!({ "region": step, "test": test, "rect": rect })),
                                    None => app.emit("onGameEdit", json!({ "area": area, "rect": rect })),
                                }
                            })
                        });
                    }));
                    *overlay.on_done.borrow_mut() = Some(Box::new(move || {
                        rt::post(move || {
                            App::with(|app| {
                                app.overlay_done(serial);
                                app.emit("onGameEdit", json!({ "done": true }));
                            })
                        });
                    }));
                }
                Purpose::Areas(id) => {
                    *overlay.on_area_changed.borrow_mut() = Some(Box::new(move |area: &str, rect: MacroRect| {
                        let area = area.to_string();
                        rt::post(move || {
                            App::with(|app| {
                                if let Err(e) = app.set_area(id, &area, rect) {
                                    log::write(&e);
                                }
                            })
                        });
                    }));
                    *overlay.on_done.borrow_mut() = Some(Box::new(move || {
                        rt::post(move || {
                            App::with(|app| {
                                app.overlay_done(serial);
                                app.refresh_panel(id);
                            })
                        });
                    }));
                }
                Purpose::Pick => {
                    *overlay.on_picked.borrow_mut() = Some(Box::new(move |x, y, colour: &str| {
                        let colour = colour.to_string();
                        rt::post(move || App::with(|app| app.overlay_picked(serial, x, y, colour)));
                    }));
                }
            }
            OVERLAY.with(|o| *o.borrow_mut() = Some((serial, overlay)));
        });
        self.panels.overlay.set(serial);
        self.panels.overlay_kind.set(Some(mode));
        serial
    }

    /// Done (Edit, Areas): `if (_overlay == overlay) (_overlay, _overlayFor) = (null, null)`.
    fn overlay_done(&self, serial: u64) {
        if self.panels.overlay.get() == serial {
            self.forget_overlay();
        }
    }

    /// FormClosed / Disposed: the core forgets it, and a pick waiting for it hears null.
    fn overlay_gone(&self, serial: u64) {
        self.overlay_done(serial);
        let waiting = self.panels.pick_wait.borrow_mut().take_if(|(s, _)| *s == serial);
        if let Some((_, tx)) = waiting {
            let _ = tx.send(None);
        }
    }

    fn overlay_picked(&self, serial: u64, x: f64, y: f64, colour: String) {
        let waiting = self.panels.pick_wait.borrow_mut().take_if(|(s, _)| *s == serial);
        if let Some((_, tx)) = waiting {
            let _ = tx.send(Some((x, y, colour)));
        }
    }

    fn forget_overlay(&self) {
        self.panels.overlay.set(0);
        self.panels.overlay_kind.set(None);
        self.panels.overlay_for.set(None);
    }

    /// The editor's "Edit on game": the draft's points and areas over the account's window, the page told of each drag
    /// (onGameEdit) and calling again with the new draft; accountId null ends it.
    pub fn edit_on_game_command(&self, a: &Value) -> Res<Value> {
        require_vault(self)?;
        let account = parse_guid(&str_of(a, "accountId")).unwrap_or(Guid::nil());
        if account.is_nil() {
            if matches!(self.panels.overlay_for.get(), Some((_, _, true))) {
                self.close_overlay(false);
            }
            return Ok(Value::Null);
        }
        let draft = if has(a, "macro", JsonKind::Object) { App::read_macro(&a["macro"])? } else { self.macro_by_id(id_of(a))? };
        let Some(window) = self.macro_desk_now().window(account) else {
            return Err(user(l::t("This account's Roblox isn't open.")));
        };
        let title = draft.name.clone(); // the page says "Editing …" (or "New macro")
        let dots = area_geometry::dots(&draft);
        let mut areas = overlay_areas(&draft);
        areas.extend(step_areas(&draft));
        let serial = self.panels.overlay.get();
        if serial != 0
            && self.panels.overlay_kind.get() == Some(Mode::Edit)
            && let Some((_, on, true)) = self.panels.overlay_for.get()
            && on == account
        {
            on_overlay(serial, move |o| o.set(dots, areas, Some(&title)));
            return Ok(Value::Null);
        }
        self.close_overlay(true);
        let serial = self.new_overlay(window.handle, Mode::Edit, &title, None, Purpose::Edit);
        on_overlay(serial, move |o| o.set(dots, areas, None));
        self.panels.overlay_for.set(Some((draft.id, account, true)));
        self.show_overlay(serial, window.pid); // the game in view under the overlay
        Ok(Value::Null)
    }

    /// "Pick on game": the next click on the account's window (caught by the overlay, nothing reaches the game) gives its
    /// place and colour; Esc gives null.
    pub async fn pick_on_game_command(self: &Rc<Self>, a: &Value) -> Res<Value> {
        require_vault(self)?;
        let account = parse_guid(&str_of(a, "accountId")).unwrap_or(Guid::nil());
        if account.is_nil() {
            return Err(user(l::t("Choose the account whose game to pick on.")));
        }
        Ok(match self.pick_on_game(account).await? {
            Some((x, y, colour)) => json!({ "x": x, "y": y, "colour": colour }),
            None => Value::Null,
        })
    }

    pub async fn pick_on_game(self: &Rc<Self>, account: Guid) -> Res<Picked> {
        let Some(window) = self.macro_desk_now().window(account) else {
            return Err(user(l::t("This account's Roblox isn't open.")));
        };
        let picture = self.macro_picture_of(account).await?;
        self.close_overlay(true);
        let (tx, rx) = oneshot::channel();
        let serial = self.new_overlay(window.handle, Mode::Pick, "", Some(picture), Purpose::Pick);
        // An older pick still waiting (replaced): it hears null, as its overlay's Disposed said in C#.
        let old = self.panels.pick_wait.borrow_mut().replace((serial, tx));
        if let Some((_, old)) = old {
            let _ = old.send(None);
        }
        self.panels.overlay_for.set(Some((Guid::nil(), account, false)));
        self.show_overlay(serial, window.pid);
        Ok(rx.await.unwrap_or(None))
    }

    /// A panel's colour "Pick": on the game of its account, or on the screen when no game is open.
    async fn panel_pick_colour(self: &Rc<Self>, id: Guid, name: String, panel: PanelHandle) {
        let result: Res<()> = async {
            let Some(m) = self.open_macro(id) else { return Ok(()) };
            let account = self.panel_account(&m);
            if account.is_nil() {
                let name = name.clone();
                panel.pick_screen_colour(custom_theme(), move |app, colour| {
                    if let Err(e) = app.set_option(id, &name, &json!(colour), true) {
                        match e {
                            Fail::User(text) => app.panel_error(id, Some(&name), &text),
                            other => log::write(&other),
                        }
                    }
                });
                return Ok(());
            }
            if let Some((_, _, colour)) = self.pick_on_game(account).await? {
                self.set_option(id, &name, &json!(colour), true)?;
            }
            Ok(())
        }
        .await;
        match result {
            Ok(()) => {}
            Err(Fail::User(text)) => self.panel_error(id, Some(&name), &text),
            Err(e) => log::write(&e),
        }
    }

    /// A panel's "Pick spot": a click on the game of its account (a picture of it held still meanwhile).
    async fn panel_pick_point(self: &Rc<Self>, id: Guid, name: String) {
        let result: Res<()> = async {
            let Some(m) = self.open_macro(id) else { return Ok(()) };
            let account = self.panel_account(&m);
            if account.is_nil() || self.macro_desk_now().window(account).is_none() {
                self.panel_error(id, Some(&name), &l::t("Open the game in an account first"));
                return Ok(());
            }
            if let Some((x, y, _)) = self.pick_on_game(account).await? {
                self.set_option(id, &name, &macro_options::point(x, y), true)?;
            }
            Ok(())
        }
        .await;
        match result {
            Ok(()) => {}
            Err(Fail::User(text)) => self.panel_error(id, Some(&name), &text),
            Err(e) => log::write(&e),
        }
    }

    pub fn editing_areas_of(&self, id: Guid) -> bool {
        self.panels.overlay.get() != 0
            && self.panels.overlay_kind.get() == Some(Mode::Areas)
            && self.panels.overlay_for.get().is_some_and(|(m, _, _)| m == id)
    }

    /// The panel's "Move areas on game": the macro's areas over its account's window to move and size (saved as they
    /// change); again, Done or Esc ends it.
    pub fn toggle_areas_on_game(&self, id: Guid) {
        if self.editing_areas_of(id) {
            self.close_overlay(true);
            self.refresh_panel(id);
            return;
        }
        let Some(m) = self.open_macro(id).filter(|m| !m.areas.is_empty()) else {
            log::note(&format!("Macro panel {}: Move areas on game did nothing (no such macro or no areas).", short(id)));
            return;
        };
        let account = self.panel_account(&m);
        let window = if account.is_nil() { None } else { self.macro_desk_now().window(account) };
        let Some(window) = window else {
            self.panel_error(id, None, &l::t("Open the game in an account first"));
            return;
        };
        self.close_overlay(true);
        let serial = self.new_overlay(window.handle, Mode::Areas, &m.name, None, Purpose::Areas(id));
        let areas = overlay_areas(&m);
        on_overlay(serial, move |o| o.set(Vec::new(), areas, None));
        self.panels.overlay_for.set(Some((id, account, false)));
        self.show_overlay(serial, window.pid);
        self.refresh_panel(id);
    }

    /// The game brought forward in its turn (between a macro's turns), then the overlay over it once its page has
    /// started, taking the keyboard so Esc and Enter are its own, not the game's.
    /// PORT: the turn is waited for off the core (rt::blocking) and the overlay opened on the windows thread.
    fn show_overlay(&self, serial: u64, pid: i32) {
        rt::spawn(async move {
            let entered = rt::blocking(move || {
                let turn = input_turn::enter("focus", None)?;
                roblox_processes::focus(pid);
                drop(turn);
                Ok::<(), Fail>(())
            })
            .await;
            if let Err(e) = entered {
                log::write(&e);
                return;
            }
            let mut current = false;
            App::with(|app| current = app.panels.overlay.get() == serial);
            if !current {
                return;
            }
            on_overlay(serial, |o| win_form::spawn(o.clone().open()));
        });
    }

    /// The overlay goes; the page hears its editing ended unless it asked for that itself.
    fn close_overlay(&self, tell_page: bool) {
        let serial = self.panels.overlay.get();
        let kind = self.panels.overlay_kind.get();
        let was = self.panels.overlay_for.get();
        self.forget_overlay();
        if serial == 0 {
            return;
        }
        log::note(&format!(
            "Overlay ({}): ended by the app (toggled off, replaced, its panel hidden or the vault locked).",
            kind.map(|k| format!("{k:?}")).unwrap_or_default()
        ));
        win_form::invoke(move || {
            let overlay = OVERLAY.with(|o| {
                let mut o = o.borrow_mut();
                if o.as_ref().is_some_and(|(s, _)| *s == serial) { o.take().map(|(_, w)| w) } else { None }
            });
            if let Some(o) = overlay {
                o.end();
            }
        });
        // Its pick (if any) hears null now, as its FormClosed (or Disposed) said in C#.
        let waiting = self.panels.pick_wait.borrow_mut().take_if(|(s, _)| *s == serial);
        if let Some((_, tx)) = waiting {
            let _ = tx.send(None);
        }
        if tell_page && matches!(was, Some((_, _, true))) {
            self.emit("onGameEdit", json!({ "done": true }));
        }
        if let Some((m, _, false)) = was
            && !m.is_nil()
        {
            self.refresh_panel(m);
        }
    }

    // ───────────────────────── trying a draft ─────────────────────────

    /// The editor's "Try it": the macro as it is in the editor, once, on one account; "What it sees" follows it
    /// (macroSeen) until it ends, its last frame saying how.
    pub fn macro_try_run_command(self: &Rc<Self>, a: &Value) -> Res<Value> {
        require_vault(self)?;
        if !has(a, "macro", JsonKind::Object) {
            return Err(user(l::t("That isn't a macro.")));
        }
        let account = parse_guid(&str_of(a, "accountId")).unwrap_or(Guid::nil());
        if account.is_nil() {
            return Err(user(l::t("Choose the account to try it on.")));
        }
        if !self.controller.running.borrow().contains_key(&account) {
            return Err(user(l::t("This account's Roblox isn't open.")));
        }
        let mut draft = App::read_macro(&a["macro"])?;
        // Once, by hand, on this account: nothing of how it starts by itself.
        draft.hotkey = None;
        draft.enabled = false;
        draft.trigger = MacroTrigger::default();
        draft.targets = "selected".into();
        draft.account_ids = Vec::new();
        if draft.name.trim().is_empty() {
            draft.name = l::t("New macro");
        }
        let (known, saved): (HashSet<Guid>, Vec<Macro>) = self.vault(|d| (d.accounts.iter().map(|acc| acc.id).collect(), d.settings.macros.clone()))?;
        macro_rules::check(&mut draft, &known, &[])?;
        macro_rules::check_calls(&draft, &saved)?;
        if draft.id.is_nil() {
            draft.id = match self.panels.try_run.get() {
                Some((last, _)) if self.open_macro(last).is_none() => last,
                _ => new_guid(),
            };
        }
        let player = self.macros.macro_player.borrow().clone();
        if let (Some((m, acc)), Some(p)) = (self.panels.try_run.get(), player.as_ref()) {
            p.stop(Some(m), "byUser", Some(acc), true);
        }
        if let Some(p) = player.as_ref() {
            if p.runs().iter().any(|r| {
                let r = lk(r);
                !r.finished() && !r.trial && r.r#macro.id == draft.id && r.account_id == account
            }) {
                return Err(user(l::t("It's running on this account already. Stop it first.")));
            }
            if let Some(other) = p.busy_with(account, draft.id) {
                return Err(user(l::f("{0} is running on this account. Stop it first.", &[&other.name])));
            }
        }
        let queued = self.macro_player_now().queue(draft.clone(), &[account], true)?;
        self.panels.try_run.set(Some((draft.id, account)));
        let draft_id = draft.id;
        *self.panels.watch_macro.borrow_mut() = Some(draft);
        self.start_watch(account, draft_id, true);
        self.push_state();
        Ok(json!({ "queued": queued, "macroId": draft_id }))
    }

    /// How the try ended, in words; None while it goes on.
    fn try_ended(&self) -> Option<String> {
        let (m, account) = self.panels.try_run.get()?;
        let player = self.macros.macro_player.borrow().clone();
        let run = player.and_then(|p| {
            p.runs().into_iter().rev().find(|r| {
                let r = lk(r);
                r.trial && r.r#macro.id == m && r.account_id == account
            })
        });
        let (status, reason) = match &run {
            Some(r) => {
                let r = lk(r);
                if !r.finished() {
                    return None;
                }
                (Some(r.status.clone()), r.reason.clone())
            }
            None => (None, None),
        };
        Some(match status.as_deref() {
            Some("done") => l::t("Tried: it ran to the end."),
            Some("failed") => l::f("Tried: it stopped ({0}).", &[&fail_text(reason.as_deref())]),
            _ => l::t("Tried: stopped."),
        })
    }

    // ───────────────────────── what it sees ─────────────────────────

    /// The editor's Watch panel: the page event macroSeen about 5 times a second, on the macro being edited (macro) or
    /// the saved one (macroId). accountId null (or empty) stops it.
    pub fn macro_watch_command(self: &Rc<Self>, a: &Value) -> Res<Value> {
        require_vault(self)?;
        let account = parse_guid(&str_of(a, "accountId")).unwrap_or(Guid::nil());
        let macro_id = parse_guid(&str_of(a, "macroId")).unwrap_or(Guid::nil());
        let mut edited: Option<Macro> = None;
        if !account.is_nil() && has(a, "macro", JsonKind::Object) {
            edited = macro_rules::read(&a["macro"]).ok(); // half-edited: the saved one, if any
        }
        if account.is_nil() || (edited.is_none() && macro_id.is_nil()) {
            self.stop_watch();
            return Ok(Value::Null);
        }
        if edited.is_none() {
            self.macro_by_id(macro_id)?;
        }
        self.panels.try_run.set(None);
        let watched = match edited.as_ref().map(|e| e.id) {
            Some(e) if !e.is_nil() => e,
            _ => macro_id,
        };
        *self.panels.watch_macro.borrow_mut() = edited;
        self.start_watch(account, watched, true);
        Ok(Value::Null)
    }

    fn start_watch(self: &Rc<Self>, account: Guid, macro_id: Guid, page: bool) {
        self.panels.watching.set(Some((account, macro_id, page)));
        self.panels.watch_text.borrow_mut().clear();
        self.panels.watch_read_at.set(None);
        if self.panels.watch_timer_on.get() {
            return;
        }
        self.panels.watch_timer_on.set(true);
        let app = self.clone();
        // The 200 ms timer: a tick while the last one is still busy does nothing (watch_busy).
        rt::spawn(async move {
            loop {
                rt::sleep(Duration::from_millis(200)).await;
                if app.panels.watching.get().is_none() || app.controller.disposed.get() {
                    app.panels.watch_timer_on.set(false);
                    break;
                }
                let tick = app.clone();
                rt::spawn(async move { tick.watch_tick().await });
            }
        });
    }

    fn stop_watch(&self) {
        self.panels.watching.set(None);
        *self.panels.watch_macro.borrow_mut() = None;
    }

    async fn watch_tick(self: &Rc<Self>) {
        let Some(w) = self.panels.watching.get() else { return };
        if self.panels.watch_busy.get() || self.open().is_none() {
            return;
        }
        let (account, macro_id, page) = w;
        let watched = self.panels.watch_macro.borrow().clone();
        let m = watched.or_else(|| self.open_macro(macro_id));
        let window = self.macro_desk_now().window(account);
        let (Some(window), Some(m)) = (window, m.clone()) else {
            if page {
                let note = if m.is_none() { l::t("That macro isn't there any more.") } else { l::t("This account's Roblox isn't open.") };
                self.emit(
                    "macroSeen",
                    json!({ "accountId": account, "image": Value::Null, "w": 0, "h": 0, "boxes": [], "numbers": [], "note": note }),
                );
            }
            self.stop_watch();
            self.panels.try_run.set(None);
            return;
        };
        let ended = self.try_ended();
        // Between a macro's turns only: a picture shows a minimised window for a moment, as a turn does.
        let Some(lease) = input_turn::try_enter("picture") else { return };
        self.panels.watch_busy.set(true);
        if let Err(e) = self.watch_look(w, window.handle, m, lease, ended).await {
            log::write(&e);
        }
        self.panels.watch_busy.set(false);
    }

    async fn watch_look(self: &Rc<Self>, w: (Guid, Guid, bool), handle: isize, m: Macro, lease: input_turn::Lease, ended: Option<String>) -> Res<()> {
        let (account, _, _) = w;
        let picture = rt::blocking(move || {
            let p = window_shot::client(handle, false);
            drop(lease);
            p
        })
        .await;
        let Some(picture) = picture else { return Ok(()) };
        let player = self.macro_player_now();
        let run: RunRef = player
            .runs()
            .into_iter()
            .rev()
            .find(|r| {
                let r = lk(r);
                r.r#macro.id == m.id && r.account_id == account
            })
            .unwrap_or_else(|| watch_run(&m, account));
        let picture = Arc::new(picture);
        let seen = {
            let (picture, m, player) = (picture.clone(), m.clone(), player.clone());
            rt::blocking(move || macro_watch::look(&picture, &m, &|name| player.value(&run, name))).await
        };
        let mut seen = seen?;
        // Text is read at most once a second (it takes a while).
        let due = self.panels.watch_read_at.get().is_none_or(|at| Stamp::utc_now().secs_since(at) > 1.0);
        if due && !macro_watch::text_steps(&m).is_empty() {
            self.panels.watch_read_at.set(Some(Stamp::utc_now()));
            let desk = self.macro_desk_now();
            let (picture, m) = (picture.clone(), m.clone());
            let text = rt::blocking(move || macro_watch::read_text(&picture, &m, desk.as_ref() as &dyn IMacroDesk)).await?;
            *self.panels.watch_text.borrow_mut() = text;
        }
        seen.boxes.extend(self.panels.watch_text.borrow().iter().cloned());
        if self.panels.watching.get() != Some(w) {
            return Ok(());
        }
        let (pw, ph) = (picture.width, picture.height);
        let jpeg = {
            let picture = picture.clone();
            rt::blocking(move || jpeg_of(&macro_watch::scaled(&picture, 640))).await
        }?;
        let note = ended.clone().or_else(|| macro_watch::note(&m, &seen));
        self.emit(
            "macroSeen",
            json!({
                "accountId": account,
                "image": format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpeg)),
                "w": pw,
                "h": ph,
                "boxes": seen.boxes,
                "numbers": seen.numbers,
                "note": note,
            }),
        );
        if ended.is_some() {
            self.stop_watch();
            self.panels.try_run.set(None);
        }
        Ok(())
    }

    // ───────────────────────── the tray ─────────────────────────

    pub fn tray_macros(&self) -> Vec<(Guid, String, bool, String)> {
        if self.auto_lock.screen_locked.get() {
            return Vec::new();
        }
        let Some(vault) = self.open() else { return Vec::new() };
        let macros = vault.data().settings.macros.clone();
        macros.iter().map(|m| (m.id, m.name.clone(), self.shown_panel(m.id).is_some(), self.panel_now(m).0)).collect()
    }

    /// "Macros ›": a macro's panel shown or hidden.
    pub fn tray_toggle_panel(&self, id: Guid) {
        let Some(m) = self.open_macro(id) else { return };
        if self.shown_panel(id).is_some() {
            self.hide_panel_window(id, true);
        } else {
            self.panels.panels_closed.borrow_mut().remove(&id);
            self.show_panel_window(&m, true);
        }
        self.push_state();
    }
}

/// `new MacroRun { Macro = macro, AccountId = account, Jumps = [] }`: a run never queued, for "What it sees" to read
/// the settings through.
fn watch_run(m: &Macro, account: Guid) -> RunRef {
    let now = Stamp::utc_now();
    Arc::new(Mutex::new(MacroRun {
        r#macro: Arc::new(m.clone()),
        account_id: account,
        jumps: Vec::new(),
        trial: false,
        status: String::new(),
        reason: None,
        pc: 0,
        loops: 0,
        started_at: now,
        ends_at: None,
        resume_at: now,
        deadline: None,
        last_frame: None,
        still_since: None,
        passes: HashMap::new(),
        again: 0,
        held: Vec::new(),
        held_as: HashMap::new(),
        wait_left: None,
        pid: 0,
        focus_misses: 0,
        recover_at: 0,
        recovering: false,
        miss_since: None,
        paused_at: None,
        restarts: 0,
        section: None,
        feature_jumps: HashMap::new(),
        safe_ends: HashSet::new(),
        due: VecDeque::new(),
        return_to: 0,
        main_passes: None,
        resumed: false,
        origins: HashMap::new(),
        owner: None,
        last_web_at: super::launching::min_stamp(),
        cancel: Cancel::new(),
        steps_run: 0,
        logged_pause: None,
        quiet: false,
        noted: HashSet::new(),
        last_score: None,
    }))
}

/// Bitmap.Save(stream, ImageFormat.Jpeg): the `image` crate's encoder (default quality), no alpha.
fn jpeg_of(picture: &crate::core::win_kit::Picture) -> Res<Vec<u8>> {
    let rgba = image::RgbaImage::from_raw(picture.width.max(0) as u32, picture.height.max(0) as u32, picture.to_rgba())
        .ok_or_else(|| Fail::other("The picture's size doesn't fit its pixels."))?;
    let rgb = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
    let mut out = std::io::Cursor::new(Vec::new());
    rgb.write_to(&mut out, image::ImageFormat::Jpeg).map_err(Fail::other)?;
    Ok(out.into_inner())
}
