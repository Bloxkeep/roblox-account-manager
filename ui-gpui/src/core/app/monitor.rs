// AppController.Monitor.cs: the game monitor tab (monitor_tab.rs): every Roblox window on this PC with its account.
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

use serde_json::{Value, json};

use super::App;
use super::controller::{JsonKind, bool_of, has, str_of};
use super::state::log_connection;
use crate::core::{
    base::{Guid, Res, Stamp},
    close_plan::{Cause, Then},
    input_turn, l, log, roblox_processes, rt, ui, window_report,
};

/// The process check's snapshot is used while younger than this (its ticks come every 1.5 s, or 4 s with nothing running).
pub const MONITOR_SNAPSHOT_AGE: Duration = Duration::from_millis(4500);

pub const MONITOR_MAX: f64 = 100_000.0;

/// `account_id` is None for a window Bloxkeep didn't open. `status` as `connection` says it ("" when nothing is known);
/// `job_id` the server it is in (the page marks windows sharing one); `icon_url` the game's, once fetched.
#[derive(Clone, Debug)]
pub struct MonitorClient {
    pub hwnd: isize,
    pub pid: i32,
    pub account_id: Option<Guid>,
    pub label: String,
    pub game: String,
    pub ram_mb: i64,
    pub status: String,
    pub job_id: Option<String>,
    pub icon_url: Option<String>,
}

/// The monitor tab's area in CSS pixels, the page's pixel ratio, and where each tile's picture goes in it.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub pictures: Vec<MonitorPicture>,
}

/// A tile's picture: its window and its rectangle in CSS pixels (relative to the page, as the area is).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MonitorPicture {
    pub hwnd: isize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Default)]
pub struct MonitorState {
    /// The window lays the live pictures over the page's monitor tab (monitor_tab.rs), or takes them away (None).
    // PORT: C#'s `Action<MonitorArea?>?` init property. An Rc, not a Box: auto_lock.rs, state.rs and unlock.rs clone it
    // out of the cell before calling it (so the call never holds the borrow).
    pub set_monitor_area: RefCell<Option<Rc<dyn Fn(Option<MonitorArea>)>>>,
    /// The window asks its monitor tab for the windows again (after a click changed one).
    pub monitor_refresh: RefCell<Option<Rc<dyn Fn()>>>,
    /// Pictures taken away for a bad area from the page: logged once until a good one comes.
    pub monitor_area_bad_logged: Cell<bool>,
}

/// A window's id on the page: its handle as a number.
pub fn monitor_id(hwnd: isize) -> String {
    (hwnd as i64).to_string()
}

/// Written so that NaN (a value missing) fails every test.
fn sane(x: f64, y: f64, w: f64, h: f64) -> bool {
    w >= 1.0 && h >= 1.0 && w < MONITOR_MAX && h < MONITOR_MAX && x.abs() < MONITOR_MAX && y.abs() < MONITOR_MAX
}

fn num(a: &Value, name: &str) -> f64 {
    if has(a, name, JsonKind::Number) { a[name].as_f64().unwrap_or(f64::NAN) } else { f64::NAN }
}

/// Nothing absurd or missing from the page reaches the window's layout; a picture that is, is left out.
pub fn parse_monitor_area(a: &Value) -> Option<MonitorArea> {
    let (x, y, w, h, scale) = (num(a, "x"), num(a, "y"), num(a, "w"), num(a, "h"), num(a, "scale"));
    if !sane(x, y, w, h) || !(scale >= 0.25 && scale <= 8.0) {
        return None;
    }
    let mut pictures = Vec::new();
    if has(a, "pictures", JsonKind::Array) {
        for p in a["pictures"].as_array().into_iter().flatten().take(200) {
            if !p.is_object() {
                continue;
            }
            // long.TryParse(Integer, Invariant): optional sign and surrounding white space.
            let Ok(hwnd) = str_of(p, "id").trim().parse::<i64>() else { continue };
            if hwnd == 0 {
                continue;
            }
            let (px, py, pw, ph) = (num(p, "x"), num(p, "y"), num(p, "w"), num(p, "h"));
            if sane(px, py, pw, ph) {
                pictures.push(MonitorPicture { hwnd: hwnd as isize, x: px, y: py, width: pw, height: ph });
            }
        }
    }
    Some(MonitorArea { x, y, width: w, height: h, scale, pictures })
}

/// `MonitorRefresh?.Invoke()` from another thread (an input turn's action): handed to the core's thread first.
fn refresh_later() {
    rt::post(|| App::with(|app| app.monitor_refresh()));
}

impl App {
    pub fn monitor_refresh(&self) {
        let refresh = self.monitor.monitor_refresh.borrow().clone();
        if let Some(refresh) = refresh {
            refresh();
        }
    }

    fn set_monitor_area_now(&self, area: Option<MonitorArea>) {
        let set_area = self.monitor.set_monitor_area.borrow().clone();
        if let Some(set_area) = set_area {
            set_area(area);
        }
    }

    /// One window per client: tracked accounts in launch order, then other clients by pid. The snapshot may be seconds
    /// old: a client gone since has no window, so no tile.
    pub fn monitor_clients(&self) -> Vec<MonitorClient> {
        let now = Stamp::utc_now();
        let snapshot = self.processes.last_players.borrow().as_ref().and_then(|(players, at)| {
            let age = now.secs_since(*at);
            (age >= 0.0 && age < MONITOR_SNAPSHOT_AGE.as_secs_f64()).then(|| players.clone())
        });
        let players = snapshot.unwrap_or_else(roblox_processes::get_players);
        let pids: Vec<i32> = players.iter().map(|p| p.pid).collect();
        let windows = roblox_processes::visible_windows(&pids);
        // ToDictionary: a pid twice would have thrown; the last one wins here (a snapshot never lists a pid twice).
        let ram: HashMap<i32, i64> = players.iter().map(|p| (p.pid, p.ram() / super::controller::MB)).collect();
        let mut list = Vec::new();
        let mut taken: HashSet<i32> = HashSet::new();
        if let Some(vault) = self.open() {
            let mut accounts = {
                let d = vault.data();
                let order = &d.settings.launch_order;
                let mut accounts = d.accounts.clone();
                // OrderBy is stable, and so is sort_by_key.
                accounts.sort_by_key(|a| order.iter().position(|id| *id == a.id).map_or(i32::MAX, |i| i as i32));
                accounts
            }; // the guard goes before anything below asks the vault again
            for account in accounts.drain(..) {
                let run = self.controller.running.borrow().get(&account.id).map(|(pid, launch)| (*pid, launch.clone()));
                let Some((pid, launch)) = run else { continue };
                let Some(handles) = windows.get(&pid) else { continue };
                let hwnd = roblox_processes::main_window_of(handles);
                if hwnd == 0 || !taken.insert(pid) {
                    continue;
                }
                let status = self.connection(&account, pid, &launch).to_string();
                let log = self.controller.logs.borrow().state_of(pid);
                // A window cut off keeps its game: the page groups it with the others there, marked.
                let place = if status == "menu" {
                    None
                } else {
                    self.monitor_place(log.as_ref().and_then(|l| l.place_id), launch.target.as_ref().map(|t| t.place_id))
                };
                let job_id = if status == "ingame" { log.as_ref().and_then(|l| l.job_id.clone()) } else { None };
                list.push(MonitorClient {
                    hwnd,
                    pid,
                    account_id: Some(account.id),
                    label: self.name_text(&account),
                    game: self.monitor_game_name(place),
                    ram_mb: ram.get(&pid).copied().unwrap_or(0),
                    status,
                    job_id,
                    icon_url: self.monitor_icon(place),
                });
            }
        }
        let mut others: Vec<(&i32, &Vec<isize>)> = windows.iter().collect();
        others.sort_by_key(|w| *w.0);
        for (&pid, handles) in others {
            let hwnd = roblox_processes::main_window_of(handles);
            if hwnd == 0 || !taken.insert(pid) {
                continue;
            }
            // Opened outside Bloxkeep: its log still says where it is (the logs of every client are followed).
            let log = self.controller.logs.borrow().state_of(pid);
            let place = log.as_ref().and_then(|l| l.place_id);
            let status = match &log {
                None => String::new(),
                Some(l) => log_connection(&l.status).to_string(),
            };
            let job_id = if status == "ingame" { log.as_ref().and_then(|l| l.job_id.clone()) } else { None };
            // Not in a game, it is "No game" like the accounts' windows, so the page puts them in one lane.
            list.push(MonitorClient {
                hwnd,
                pid,
                account_id: None,
                label: "Roblox".to_string(),
                game: self.monitor_game_name(place),
                ram_mb: ram.get(&pid).copied().unwrap_or(0),
                status,
                job_id,
                icon_url: self.monitor_icon(place),
            });
        }
        list
    }

    /// As the page shows it (session_game_dto): the logged place once named, else the one opened.
    fn monitor_place(&self, logged: Option<i64>, opened: Option<i64>) -> Option<i64> {
        let logged_named = logged.is_some_and(|l| self.controller.games.borrow().get(&l).and_then(|g| g.as_ref()).is_some_and(|g| !g.name.is_empty()));
        if logged_named || opened.is_none() { logged.or(opened) } else { opened }
    }

    fn monitor_game_name(&self, place_id: Option<i64>) -> String {
        let Some(id) = place_id else { return l::t("No game") };
        match self.controller.games.borrow().get(&id).and_then(|g| g.as_ref()).map(|g| g.name.clone()) {
            Some(name) if !name.is_empty() => name,
            _ => l::f("Place {0}", &[&id]),
        }
    }

    fn monitor_icon(&self, place_id: Option<i64>) -> Option<String> {
        let id = place_id?;
        self.controller.games.borrow().get(&id).and_then(|g| g.as_ref()).and_then(|g| g.icon_url.clone())
    }

    pub fn close_monitor_client(&self, client: &MonitorClient) {
        if let Some(id) = client.account_id
            && self.controller.running.borrow().contains_key(&id)
        {
            self.close_accounts(&[id], false);
        } else {
            self.ask_close(None, client.pid, Cause::Untracked, Then::StayClosed, false);
        }
    }

    /// The monitor tab's windows to the page ("monitor"): name (Hide names holds), game, RAM, PID, minimised.
    pub fn emit_monitor(&self, clients: &[MonitorClient], minimized: &dyn Fn(isize) -> bool) {
        if self.open().is_none() {
            return;
        }
        let tiles: Vec<Value> = clients
            .iter()
            .map(|c| {
                json!({
                    "id": monitor_id(c.hwnd), "pid": c.pid, "name": c.label, "game": c.game, "ramMb": c.ram_mb,
                    "ours": c.account_id.is_some(), "minimized": minimized(c.hwnd),
                    "status": c.status, "jobId": c.job_id, "iconUrl": c.icon_url,
                })
            })
            .collect();
        self.emit("monitor", json!({ "tiles": tiles }));
    }

    /// A tile's click, key or menu item: show, minimize, restore, close {id}; copyPid returns the PID copied.
    pub fn monitor_act_command(&self, a: &Value) -> Res<Value> {
        self.vault(|_| ())?; // RequireVault
        let id = str_of(a, "id");
        let Some(client) = self.monitor_clients().into_iter().find(|c| monitor_id(c.hwnd) == id) else {
            log::note(&format!("Game monitor: {} did nothing, the window is gone.", str_of(a, "what")));
            return Ok(Value::Null); // gone meanwhile: every action survives that
        };
        Ok(self.monitor_act(&client, &str_of(a, "what")))
    }

    /// A game's heading: front (over every app) or back (behind every app) for its windows {ids}.
    pub fn monitor_lane_command(&self, a: &Value) -> Res<Value> {
        self.vault(|_| ())?; // RequireVault
        let what = str_of(a, "what");
        // `x.GetString()`: a non-string item would have thrown; here it is left out.
        let ids: HashSet<String> = match a.get("ids").and_then(Value::as_array) {
            Some(list) => list.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
            None => HashSet::new(),
        };
        let windows: Vec<isize> = self.monitor_clients().into_iter().filter(|c| ids.contains(&monitor_id(c.hwnd))).map(|c| c.hwnd).collect();
        if windows.is_empty() {
            log::note(&format!("Game monitor: {what} for a game did nothing, its windows are gone."));
            return Ok(Value::Null);
        }
        input_turn::run_when_free("monitor", move || {
            let before = window_report::states(&windows);
            if what == "front" {
                roblox_processes::bring_all_to_front(&windows);
            } else if what == "back" {
                roblox_processes::send_all_to_back(&windows);
            }
            log::note(&format!(
                "Game monitor: {} on {} Roblox window(s) (were {before}).",
                if what == "front" { "Bring all to front" } else { "Send all to back" },
                windows.len()
            ));
            refresh_later();
        });
        Ok(Value::Null)
    }

    /// A tile's action on its window.
    fn monitor_act(&self, client: &MonitorClient, what: &str) -> Value {
        match what {
            "show" => {
                log::note(&format!("Game monitor: Show on Roblox window {} ({}).", client.pid, window_report::describe(client.hwnd)));
                roblox_processes::focus_in_turn(client.pid);
            }
            // After a nudge, a picture or a macro's turn under way (as Show): the turn would put the window back as it
            // found it, undoing the change.
            "minimize" => {
                let (pid, hwnd) = (client.pid, client.hwnd);
                input_turn::run_when_free("monitor", move || {
                    log::note(&format!("Game monitor: Minimize on Roblox window {pid} (was {}).", window_report::describe(hwnd)));
                    roblox_processes::minimise(hwnd);
                    refresh_later();
                });
                return Value::Null;
            }
            "restore" => {
                let (pid, hwnd) = (client.pid, client.hwnd);
                input_turn::run_when_free("monitor", move || {
                    log::note(&format!("Game monitor: Restore on Roblox window {pid} (was {}).", window_report::describe(hwnd)));
                    roblox_processes::restore_quietly(hwnd);
                    refresh_later();
                });
                return Value::Null;
            }
            "close" => self.close_monitor_client(client),
            "copyPid" => {
                ui::copy_to_clipboard(&client.pid.to_string(), false);
                return json!(client.pid);
            }
            _ => {}
        }
        self.monitor_refresh();
        Value::Null
    }

    /// The monitor tab moved, or must not be covered ({ hidden: true }: another tab, a dialog, the window hidden).
    /// Locked: no tiles, the page shows the lock screen.
    pub fn monitor_area_command(&self, a: &Value) -> Res<Value> {
        let open = self.open().is_some();
        let hidden = bool_of(a, "hidden");
        let area = if !open || hidden { None } else { parse_monitor_area(a) };
        // Pictures taken away for a bad area from the page: logged once until a good one comes.
        let bad = area.is_none() && open && !hidden;
        if bad && !self.monitor.monitor_area_bad_logged.get() {
            log::note("Game monitor: the page sent an area out of range; live pictures taken away.");
        }
        self.monitor.monitor_area_bad_logged.set(bad);
        self.set_monitor_area_now(area);
        Ok(Value::Null)
    }
}
