// AppController.Macros.cs: Macros (Beta): the Macros tab's commands, the runs' state for the page, what starts a macro by
// itself (Ctrl+Alt+1…9, a repeat, joining or rejoining a game) and recording one. Playing is macro_player.rs's, on the
// real desktop (macro_desk.rs): one window at a time, brought forward through `input_turn` like anti-AFK's nudge.
//
// PORT: the C# player ran on the UI thread between awaits and called AppController's delegates there. The Rust player
// plays on a thread of its own (macro_player.rs), so the hooks it is given (Changed, FindMacro, Played, Rejoin, Alert)
// and the desk's (the account's window, raw input's last person) reach the App on the core thread: fire-and-forget
// through `rt::post`, or, where an answer is needed, posted and waited for (`ask_core`).
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, MutexGuard, OnceLock, atomic::Ordering, mpsc},
    thread::ThreadId,
    time::Duration,
};

use base64::Engine;
use serde_json::{Value, json};

use super::App;
use super::controller::{JsonKind, has, parse_guid, str_of};
use super::settings::{open_file_dialog, save_file_dialog};
use crate::core::base::{Cancel, Fail, Guid, Res, Stamp, new_guid};
use crate::core::discord_webhook::{self, DiscordFile, DiscordMessage, DiscordPing};
use crate::core::macro_desk::{ClientFn, MacroDesk, PersonAtFn, RawWorksFn};
use crate::core::macro_model::{Macro, MacroTest, macro_rules, macro_rules::page_json, macro_rules::page_json::Shape, net};
use crate::core::macro_player::{self, IMacroDesk, MacroPlayer, MacroRun, RunRef};
use crate::core::macro_recorder::{MacroRecorder, Post, macro_recording};
use crate::core::win_kit::{Picture, Rect};
use crate::core::{input_turn, l, log, macro_ocr, macro_templates, macro_vision, multi_instance, roblox_processes, rt, ui, window_shot};

/// Ctrl+Alt+0 stops every macro and a recording; Ctrl+Alt+1…9 are ids 11…19; F8 (alone) ends a recording.
pub const HOTKEY_STOP_MACROS: i32 = 10;
pub const HOTKEY_MACRO_FIRST: i32 = 11;
pub const HOTKEY_END_RECORDING: i32 = 20;
const MAX_MACRO_IMPORT: u64 = 4 * 1024 * 1024;
const MACRO_FILE_TYPE: &str = ".bloxkeepmacro";
/// A macro that starts by itself waits this long after the join (or after Bloxkeep opened) for the game to settle.
const MACRO_SETTLE: Duration = Duration::from_secs(30);
const MACRO_PUSH_EVERY: Duration = Duration::from_millis(400);
/// How long the editor's picture waits for the turn.
const PICTURE_WAIT: Duration = Duration::from_secs(15);

/// The instance fields AppController.Macros.cs declares.
#[derive(Default)]
pub struct MacrosState {
    pub macro_player: RefCell<Option<Arc<MacroPlayer>>>,
    pub macro_desk: RefCell<Option<Arc<MacroDesk>>>,
    pub recorder: RefCell<Option<MacroRecorder>>,
    pub macro_sweep: Cell<Option<f64>>,
    /// The accounts each macro last ran on (Ctrl+Alt+digit of a macro for the selected accounts, with no window in front).
    pub macro_last_accounts: RefCell<HashMap<Guid, Vec<Guid>>>,
    pub macro_interval_at: RefCell<HashMap<Guid, Stamp>>,
    /// The join (its log's time) a join or rejoin macro already ran for, per macro and account.
    pub macro_join_ran: RefCell<HashMap<(Guid, Guid), Stamp>>,
    /// None: DateTime.MinValue (never pushed).
    pub macro_pushed_at: Cell<Option<Stamp>>,
    pub macro_push_waiting: Cell<bool>,
    /// error.log: what a macro's trigger did, written again only when it changes (a 10 s repeat would fill the log).
    pub macro_trigger_noted: RefCell<HashMap<Guid, String>>,
}

/// The core's thread, noted when the desk is first made (always from the core).
static CORE: OnceLock<ThreadId> = OnceLock::new();

/// `f` with the App on the core's thread, its answer waited for (from the player's or the recorder's thread); run at
/// once when already on the core. None when there is no App, or the core didn't answer within `wait` (it is closing,
/// or busy waiting on the caller).
fn ask_core<T: Send + 'static>(wait: Duration, f: impl FnOnce(&Rc<App>) -> T + Send + 'static) -> Option<T> {
    if CORE.get() == Some(&std::thread::current().id()) {
        let mut out = None;
        App::with(|app| out = Some(f(app)));
        return out;
    }
    let (tx, rx) = mpsc::channel();
    rt::post(move || {
        App::with(|app| {
            let _ = tx.send(f(app));
        })
    });
    rx.recv_timeout(wait).ok()
}

/// A run's lock, for a moment (never held across a call to the player).
pub(crate) fn lk(run: &RunRef) -> MutexGuard<'_, MacroRun> {
    run.lock().unwrap_or_else(|e| e.into_inner())
}

fn user(text: String) -> Fail {
    Fail::user(text)
}

/// The id cut short for error.log.
pub(crate) fn short(id: Guid) -> String {
    id.to_string()[..8].to_string()
}

/// MyDocuments (Environment.GetFolderPath), "" when Windows can't say.
fn documents() -> String {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_Documents, SHGetKnownFolderPath};
    let mut path: windows_sys::core::PWSTR = std::ptr::null_mut();
    // SAFETY: FOLDERID_Documents is a valid known-folder id; `path` receives a CoTaskMemAlloc'd string freed below.
    let hr = unsafe { SHGetKnownFolderPath(&FOLDERID_Documents, 0, std::ptr::null_mut(), &mut path) };
    if hr < 0 || path.is_null() {
        return String::new();
    }
    // SAFETY: `path` is a NUL-terminated wide string from the call above.
    let text = unsafe {
        let mut n = 0usize;
        while *path.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(path, n))
    };
    // SAFETY: freed once, as SHGetKnownFolderPath asks.
    unsafe { CoTaskMemFree(path as *const std::ffi::c_void) };
    text
}

/// string.Substring(0, n) in UTF-16 units, never cutting a character in two.
fn cut_units(text: &str, max: usize) -> String {
    let mut units = 0;
    let mut out = String::new();
    for ch in text.chars() {
        units += ch.len_utf16();
        if units > max {
            break;
        }
        out.push(ch);
    }
    out
}

impl App {
    // ───────────────────────── the desk and the player ─────────────────────────

    pub fn macro_desk_now(&self) -> Arc<MacroDesk> {
        let existing = self.macros.macro_desk.borrow().clone();
        if let Some(desk) = existing {
            return desk;
        }
        let _ = CORE.set(std::thread::current().id());
        let client: ClientFn = Arc::new(|account| ask_core(Duration::from_secs(2), move |app| app.macro_client(account)).flatten());
        // PORT: C# read _robloxInput straight from the player (all on the UI thread); here the App holds it on the core.
        let person_at: PersonAtFn =
            Arc::new(|| ask_core(Duration::from_secs(1), |app| app.anti_afk.roblox_input.borrow().as_ref().and_then(|i| i.person_at())).flatten());
        let raw_works: RawWorksFn =
            Arc::new(|| ask_core(Duration::from_secs(1), |app| app.anti_afk.roblox_input.borrow().as_ref().is_some_and(|i| i.working())).unwrap_or(false));
        let desk = Arc::new(MacroDesk::new(client, Some(person_at), Some(raw_works)));
        *self.macros.macro_desk.borrow_mut() = Some(desk.clone());
        desk
    }

    pub fn macro_player_now(&self) -> Arc<MacroPlayer> {
        let existing = self.macros.macro_player.borrow().clone();
        if let Some(player) = existing {
            return player;
        }
        let desk: Arc<dyn IMacroDesk> = self.macro_desk_now();
        let player = MacroPlayer::new(desk);
        self.wire_macro_player(&player);
        *self.macros.macro_player.borrow_mut() = Some(player.clone());
        player
    }

    fn macro_player(&self) -> Option<Arc<MacroPlayer>> {
        self.macros.macro_player.borrow().clone()
    }

    fn wire_macro_player(&self, player: &Arc<MacroPlayer>) {
        *player.changed.write().unwrap_or_else(|e| e.into_inner()) =Some(Arc::new(|| rt::post(|| App::with(|app| app.macros_changed()))));
        // A Run a macro step plays the saved macro as it is when the run starts.
        *player.find_macro.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(|id| {
            ask_core(Duration::from_secs(2), move |app| {
                if app.open().is_none() {
                    return None;
                }
                app.vault(|d| d.settings.macros.iter().find(|m| m.id == id).map(macro_rules::copy)).ok().flatten()
            })
            .flatten()
        }));
        // Anti-AFK counts the turn as the window's activity: a nudge right after it would be for nothing.
        *player.played.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(|_, pid| {
            rt::post(move || {
                App::with(|app| {
                    app.anti_afk.last_input.borrow_mut().insert(pid, Stamp::utc_now());
                    app.anti_afk.afk_held.borrow_mut().remove(&pid);
                })
            })
        }));
        // PORT: no answer from the core in 10 s (it is closing): said as a refusal rather than taken for "rejoining".
        *player.rejoin.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(|account| {
            ask_core(Duration::from_secs(10), move |app| app.macro_rejoin(account)).unwrap_or_else(|| Some(l::t("This account can't be opened again now.")))
        }));
        *player.alert.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(|account, m: &Macro, text: &str, png| {
            let (m, text) = (m.clone(), text.to_string());
            rt::post(move || App::with(|app| app.macro_alert(account, &m, &text, png)));
        }));
    }

    /// A macro's alert: the activity entry, the toast, and Discord (with the window's picture when asked).
    pub fn macro_alert(self: &Rc<Self>, account: Guid, _macro: &Macro, text: &str, png: Option<Vec<u8>>) {
        let Some(png) = png else {
            self.notify(text, "warn", "macro", account, true, None);
            return;
        };
        self.note_notified(text, "warn", "macro", account);
        self.toast(text, "warn");
        let (app, text) = (self.clone(), text.to_string());
        rt::spawn(async move { app.send_macro_picture(account, text, png).await });
    }

    async fn send_macro_picture(self: &Rc<Self>, account: Guid, text: String, png: Vec<u8>) {
        if self.controller.disposed.get() {
            return;
        }
        let Ok(Some((url, style, username, avatar_url))) = self.vault(|d| {
            let s = &d.discord;
            (s.enabled && !s.webhook_url.is_empty()).then(|| (s.webhook_url.clone(), s.style.clone(), s.username.clone(), s.avatar_url.clone()))
        }) else {
            return;
        };
        let mut message = DiscordMessage::new(self.dt("Macro alert"), discord_webhook::escape(Some(&text)), Vec::new());
        message.color = discord_webhook::YELLOW;
        message.timestamp = Some(Stamp::utc_now());
        message.author = self.find_account(account).map(|acc| self.discord_name(&acc));
        let mut payload = discord_webhook::build(&message, &DiscordPing::none(), style != "plain", Some(&username), Some(&avatar_url), Some("macro.png"));
        let file = DiscordFile { name: "macro.png".into(), data: png };
        let sent = rt::blocking(move || discord_webhook::send(&url, &mut payload, None, false, &Cancel::new(), Some(&file))).await;
        if let Err(e) = sent {
            log::write(&e);
        }
    }

    /// The account's Roblox as Bloxkeep knows it: its process, the frame rate limit it was given, whether its log says it
    /// is in a game.
    pub fn macro_client(&self, account_id: Guid) -> Option<(i32, i32, bool)> {
        let (pid, fps) = {
            let running = self.controller.running.borrow();
            let (pid, launch) = running.get(&account_id)?;
            (*pid, launch.fps_limit)
        };
        if self.rejoin.closing_for_rejoin.borrow().contains(&pid) {
            return None;
        }
        let in_game = self.controller.logs.borrow().state_of(pid).is_none_or(|s| s.status == "ingame");
        Some((pid, fps, in_game))
    }

    /// The rejoin step: the window closed and opened again into its server, the way a preset's restart does it.
    pub fn macro_rejoin(&self, account_id: Guid) -> Option<String> {
        let tag = format!("Macro rejoin on account {}", short(account_id));
        let run = self.controller.running.borrow().get(&account_id).cloned();
        let Some((pid, launch)) = run.filter(|(_, launch)| launch.target.is_some()) else {
            let has = self.controller.running.borrow().contains_key(&account_id);
            log::note(&format!("{tag}: refused, {}.", if has { "the window wasn't opened into a game" } else { "no window" }));
            return Some(l::t("Bloxkeep didn't open this window into a game, so it can't rejoin it."));
        };
        if self.rejoin.closing_for_rejoin.borrow().contains(&pid) {
            log::note(&format!("{tag}: window {pid} is already closing for a rejoin."));
            return None;
        }
        match self.find_account(account_id) {
            Some(account) if account.cookie_valid != Some(false) && account.banned != Some(true) => {}
            _ => {
                log::note(&format!("{tag}: refused, the account is gone, signed out or banned."));
                return Some(l::t("This account can't be opened again now."));
            }
        }
        let count = self.controller.running.borrow().len();
        if !multi_instance::armed() && count > 1 {
            log::note(&format!("{tag}: refused, Multi-Roblox is off with {count} windows open."));
            return Some(l::t("Turn on Multi-Roblox to rejoin while other windows are open."));
        }
        log::note(&format!("{tag}: closing window {pid} to open it again into its server."));
        let target = self.restart_target(pid, &launch);
        self.presets.restarts.borrow_mut().insert(account_id, (target, Stamp::utc_now()));
        self.rejoin.closing_for_rejoin.borrow_mut().insert(pid); // Bloxkeep's own close: neither a drop nor closed by Roblox
        roblox_processes::close(pid, self.in_game_now(pid));
        None
    }

    /// Runs changed: the page hears it at most every 400 ms (a run's step moves many times a second).
    pub fn macros_changed(&self) {
        if self.controller.disposed.get() || self.macros.macro_push_waiting.get() {
            return;
        }
        let now = Stamp::utc_now();
        let wait = match self.macros.macro_pushed_at.get() {
            Some(at) => at.plus_secs(MACRO_PUSH_EVERY.as_secs_f64()).secs_since(now),
            None => -1.0,
        };
        if wait <= 0.0 {
            self.macros.macro_pushed_at.set(Some(now));
            self.push_state_if_changed();
            return;
        }
        self.macros.macro_push_waiting.set(true);
        rt::spawn(async move {
            rt::sleep(Duration::from_secs_f64(wait)).await;
            App::with(|app| {
                app.macros.macro_push_waiting.set(false);
                app.macros.macro_pushed_at.set(Some(Stamp::utc_now()));
                app.push_state_if_changed();
            });
        });
    }

    // ───────────────────────── saved macros ─────────────────────────

    /// RequireVault().Data.Settings.Macros, a copy (changes go through `vault_mut`).
    pub fn macro_list(&self) -> Res<Vec<Macro>> {
        self.vault(|d| d.settings.macros.clone())
    }

    pub fn macro_by_id(&self, id: Guid) -> Res<Macro> {
        self.vault(|d| d.settings.macros.iter().find(|m| m.id == id).cloned())?.ok_or_else(|| user(l::t("That macro isn't there any more.")))
    }

    pub fn save_macro(&self, a: &Value) -> Res<Value> {
        let known: HashSet<Guid> = self.vault(|d| d.accounts.iter().map(|acc| acc.id).collect())?;
        if !has(a, "macro", JsonKind::Object) {
            return Err(user(l::t("That isn't a macro.")));
        }
        let mut m = App::read_macro(&a["macro"])?;
        let macros = self.macro_list()?;
        let index = if m.id.is_nil() { None } else { macros.iter().position(|x| x.id == m.id) };
        if index.is_none() && macros.len() >= macro_rules::MAX_MACROS as usize {
            return Err(user(l::f("You can save up to {0} macros.", &[&macro_rules::MAX_MACROS])));
        }
        macro_rules::check(&mut m, &known, &macros)?;
        macro_rules::check_calls(&m, &macros)?;
        if index.is_none() {
            // A new one keeps an id it came with (a draft tried with "Try it" before its first save).
            if m.id.is_nil() {
                m.id = new_guid();
            }
        }
        let saved = m.clone();
        self.vault_mut(move |d| {
            let list = &mut d.settings.macros;
            match list.iter().position(|x| x.id == saved.id) {
                // Runs going on keep the steps they started with; the next run takes these.
                Some(i) => list[i] = saved,
                None => list.push(saved),
            }
        })?;
        self.macros.macro_interval_at.borrow_mut().remove(&m.id);
        log::note(&format!(
            "Macro {}: saved{}, {} steps, {} features, trigger {}{}, targets {}, hotkey {}.",
            short(m.id),
            if index.is_none() { " (new)" } else { "" },
            m.steps.len(),
            m.features.len(),
            m.trigger.kind,
            if m.enabled { " on" } else { " off" },
            m.targets,
            match m.hotkey {
                Some(digit) => format!("Ctrl+Alt+{digit}"),
                None => "none".into(),
            }
        ));
        self.persist();
        self.hotkeys_changed();
        self.refresh_panel(m.id);
        self.push_state();
        Ok(page_json::write(&m, Shape::Macro))
    }

    /// HotkeysChanged?.Invoke().
    pub(crate) fn hotkeys_changed(&self) {
        let changed = self.hotkeys.hotkeys_changed.borrow().clone();
        if let Some(f) = changed {
            f();
        }
    }

    /// MacroRules.Read, a JsonException said as "That isn't a macro." (a UserError as it is).
    pub fn read_macro(json: &Value) -> Res<Macro> {
        match macro_rules::read(json) {
            Ok(m) => Ok(m),
            Err(Fail::User(text)) => Err(Fail::User(text)),
            Err(_) => Err(user(l::t("That isn't a macro."))),
        }
    }

    pub fn delete_macro(&self, id: Guid) -> Res<Value> {
        let removed = self.vault_mut(|d| {
            let before = d.settings.macros.len();
            d.settings.macros.retain(|m| m.id != id);
            before - d.settings.macros.len()
        })?;
        if removed == 0 {
            return Ok(Value::Null);
        }
        log::note(&format!("Macro {}: deleted.", short(id)));
        if let Some(player) = self.macro_player() {
            player.stop(Some(id), "byUser", None, false);
        }
        self.close_panel_window(id);
        self.macros.macro_last_accounts.borrow_mut().remove(&id);
        self.macros.macro_interval_at.borrow_mut().remove(&id);
        self.macros.macro_join_ran.borrow_mut().retain(|k, _| k.0 != id);
        self.persist();
        self.hotkeys_changed();
        self.push_state();
        Ok(Value::Null)
    }

    /// A copy right after it, named "… (copy)", its hotkey left free and its trigger off.
    pub fn duplicate_macro(&self, id: Guid) -> Res<Value> {
        let macros = self.macro_list()?;
        let source = self.macro_by_id(id)?;
        if macros.len() >= macro_rules::MAX_MACROS as usize {
            return Err(user(l::f("You can save up to {0} macros.", &[&macro_rules::MAX_MACROS])));
        }
        let mut copy = macro_rules::copy(&source);
        copy.id = new_guid();
        copy.hotkey = None;
        copy.enabled = false;
        copy.panel.auto_show = false; // one panel by itself per game
        copy.name = App::copy_name(&source.name, macros.iter().map(|m| m.name.as_str()), macro_rules::MAX_NAME);
        let inserted = copy.clone();
        self.vault_mut(move |d| {
            let list = &mut d.settings.macros;
            let at = list.iter().position(|m| m.id == id).map(|i| i + 1).unwrap_or(list.len());
            list.insert(at, inserted);
        })?;
        self.persist();
        self.push_state();
        Ok(page_json::write(&copy, Shape::Macro))
    }

    /// "Name (copy)", or "(copy 2)" and on when that is taken, shortened so it fits 60 characters.
    pub fn copy_name<'a>(name: &str, taken: impl Iterator<Item = &'a str>, max: i32) -> String {
        // StringComparer.OrdinalIgnoreCase.
        let names: HashSet<String> = taken.map(str::to_uppercase).collect();
        let mut n = 1;
        loop {
            let suffix = if n == 1 { l::t(" (copy)") } else { l::f(" (copy {0})", &[&n]) };
            let room = (max as usize).saturating_sub(net::len16(&suffix));
            let candidate = format!("{}{suffix}", App::cut_text(name, room).trim_end());
            if !names.contains(&candidate.to_uppercase()) {
                return candidate;
            }
            n += 1;
        }
    }

    pub fn move_macro(&self, id: Guid, to: i32) -> Res<Value> {
        let m = self.macro_by_id(id)?;
        self.vault_mut(move |d| {
            let list = &mut d.settings.macros;
            list.retain(|x| x.id != id);
            let at = to.clamp(0, list.len() as i32) as usize;
            list.insert(at, m);
        })?;
        self.persist();
        self.push_state();
        Ok(Value::Null)
    }

    /// The accounts a run goes to: those sent (the page's selection), else the macro's own.
    pub fn macro_accounts(&self, m: &Macro, chosen: Option<&[Guid]>) -> Res<Vec<Guid>> {
        let accounts: Vec<Guid> = self.vault(|d| d.accounts.iter().map(|acc| acc.id).collect())?;
        let ids: Vec<Guid> = match chosen {
            Some(c) if !c.is_empty() => c.to_vec(),
            _ => match m.targets.as_str() {
                "all" => {
                    let running = self.controller.running.borrow();
                    accounts.iter().copied().filter(|id| running.contains_key(id)).collect()
                }
                "accounts" => m.account_ids.clone(),
                _ => Vec::new(),
            },
        };
        let mut out: Vec<Guid> = Vec::new();
        for id in ids {
            if accounts.contains(&id) && !out.contains(&id) {
                out.push(id);
            }
        }
        Ok(out)
    }

    pub fn run_macro(&self, id: Guid, account_ids: Option<&[Guid]>) -> Res<Value> {
        let m = self.macro_by_id(id)?;
        // Accounts sent, but none (nothing selected): not the macro's own instead. On the selected accounts with none sent
        // (the Macros page with nothing selected): where its panel works, the accounts in its game or the window in front.
        let mut accounts = if account_ids.is_some_and(|a| a.is_empty()) { Vec::new() } else { self.macro_accounts(&m, account_ids)? };
        if accounts.is_empty() && account_ids.is_none() && m.targets == "selected" {
            accounts = self.panel_accounts(&m);
        }
        let tag = format!("Macro {}: Run", short(m.id));
        if accounts.is_empty() {
            log::note(&format!(
                "{tag} did nothing: no accounts to run on (targets {}, {} sent).",
                m.targets,
                account_ids.map(|a| a.len().to_string()).unwrap_or_else(|| "none".into())
            ));
            return Err(user(if account_ids.is_some_and(|a| a.is_empty()) || (m.targets == "selected" && account_ids.is_none()) {
                l::t("Select the accounts to run the macro on.")
            } else {
                l::t("None of the macro's accounts is there.")
            }));
        }
        let open: Vec<Guid> = {
            let running = self.controller.running.borrow();
            accounts.iter().copied().filter(|a| running.contains_key(a)).collect()
        };
        if open.is_empty() {
            log::note(&format!("{tag} did nothing: none of its {} accounts has Roblox open.", accounts.len()));
            return Err(user(if accounts.len() == 1 { l::t("This account's Roblox isn't open.") } else { l::t("None of these accounts has Roblox open.") }));
        }
        // One macro on a window at a time: said when it is another's on every one of them.
        let player = self.macro_player_now();
        let busy: Vec<Option<Arc<Macro>>> = open.iter().map(|a| player.busy_with(*a, m.id)).collect();
        if busy.iter().all(Option::is_some) {
            log::note(&format!("{tag} did nothing: another macro runs on all {} open accounts.", open.len()));
            return Err(user(if open.len() == 1 {
                l::f("{0} is running on this account. Stop it first.", &[&busy[0].as_ref().map(|o| o.name.clone()).unwrap_or_default()])
            } else {
                l::t("Another macro is running on these accounts. Stop it first.")
            }));
        }
        log::note(&format!("{tag} pressed for {} open accounts.", open.len()));
        let queued = self.queue_macro(&m, &open)?;
        Ok(json!({ "queued": queued }))
    }

    pub fn queue_macro(&self, m: &Macro, accounts: &[Guid]) -> Res<i32> {
        self.macros.macro_last_accounts.borrow_mut().insert(m.id, accounts.to_vec());
        // The steps as saved now: an edit while it runs doesn't change a run halfway.
        let player = self.macro_player_now();
        let queued = player.queue(macro_rules::copy(m), accounts, false);
        if let Ok(q) = queued
            && q > 0
        {
            let fps = {
                let running = self.controller.running.borrow();
                accounts.iter().map(|a| running.get(a).map(|(_, launch)| launch.fps_limit).unwrap_or(0)).max().unwrap_or(0)
            };
            let windows = player.runs().iter().filter(|r| !lk(r).finished()).count() as i32;
            self.macros.macro_sweep.set(Some(macro_player::sweep_seconds(m, windows, fps)));
        }
        self.push_state();
        queued
    }

    /// `account_id`: only this account's run (the editor's "Stop trying"). `trial`: only Try it's run: the saved
    /// macro's own runs, same id, go on.
    pub fn stop_macros(self: &Rc<Self>, id: Option<Guid>, account_id: Option<Guid>, trial: bool) -> Res<Value> {
        let stopped = self.macro_player().map(|p| p.stop(id, "byUser", account_id, trial)).unwrap_or(0);
        let all = id.is_none() && account_id.is_none() && !trial;
        log::note(&format!(
            "Macro {}: stop asked{}{}, {stopped} runs stopped{}.",
            id.map(short).unwrap_or_else(|| "(all)".into()),
            account_id.map(|acc| format!(" on account {}", short(acc))).unwrap_or_default(),
            if trial { " (Try it)" } else { "" },
            if all && self.macros.recorder.borrow().is_some() { ", recording ends" } else { "" }
        ));
        if all {
            let app = self.clone();
            rt::spawn(async move {
                let _ = app.stop_recording().await;
            });
        }
        self.push_state();
        Ok(Value::Null)
    }

    // ───────────────────────── the editor's tools ─────────────────────────

    /// The account's window as it is now, for the editor to cut pictures and pick places from.
    pub async fn macro_picture(self: &Rc<Self>, account_id: Guid) -> Res<Value> {
        self.vault(|_| ())?;
        let picture = self.macro_picture_of(account_id).await?;
        let png = macro_player::png_of(&picture)?;
        Ok(json!({
            "image": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)),
            "w": picture.width,
            "h": picture.height,
        }))
    }

    /// The window's client area, taken in its turn (a minimised or hidden one is shown unseen for it).
    pub async fn macro_picture_of(&self, account_id: Guid) -> Res<Picture> {
        let Some(window) = self.macro_desk_now().window(account_id) else {
            return Err(user(l::t("This account's Roblox isn't open.")));
        };
        // A macro's turn can last minutes: the editor says so rather than look hung.
        let wait = Cancel::new();
        {
            let wait = wait.clone();
            rt::spawn(async move {
                rt::sleep(PICTURE_WAIT).await;
                wait.cancel();
            });
        }
        let turn = rt::blocking(move || input_turn::enter("picture", Some(&wait))).await;
        let turn = match turn {
            Ok(t) => t,
            Err(_) => {
                log::note(&format!(
                    "Macro editor picture: window {} wasn't free in {:.0} s ({} has it).",
                    window.pid,
                    PICTURE_WAIT.as_secs_f64(),
                    input_turn::holder().unwrap_or_else(|| "?".into())
                ));
                return Err(user(l::t("A macro is using the window. Try again in a moment.")));
            }
        };
        let h = window.handle;
        let picture = rt::blocking(move || {
            let p = window_shot::client(h, false);
            drop(turn);
            p
        })
        .await;
        if picture.is_none() {
            log::note(&format!("Macro editor picture: window {} couldn't be pictured.", window.pid));
        }
        picture.ok_or_else(|| user(l::t("Couldn't take a picture of this window. Try again.")))
    }

    /// The editor's Try: one check on the window as it is now.
    pub async fn macro_test(self: &Rc<Self>, a: &Value) -> Res<Value> {
        self.vault(|_| ())?;
        let mut test: Option<MacroTest> = if has(a, "test", JsonKind::Object) { page_json::read::<MacroTest>(&a["test"], Shape::Test).ok() } else { None };
        macro_rules::check_test(test.as_mut())?;
        let Some(test) = test else {
            return Err(user(l::t("That isn't a macro.")));
        };
        let account_id = parse_guid(&str_of(a, "accountId")).unwrap_or(Guid::nil());
        if test.kind == "var" {
            let (texts, numbers) = match self.macro_player() {
                Some(p) => {
                    let st = p.state();
                    (st.texts.get(&account_id).cloned(), st.vars.get(&account_id).cloned())
                }
                None => (None, None),
            };
            let mut of = |name: &str| -> Option<Value> {
                if let Some(t) = texts.as_ref().and_then(|t| t.get(name)) {
                    return Some(Value::String(t.clone()));
                }
                numbers.as_ref().and_then(|n| n.get(name)).map(|d| json!(d))
            };
            return Ok(match crate::core::macro_values::var_test(&test, &mut of, true) {
                Ok(ok) => json!({ "ok": ok }),
                Err(_) => json!({ "ok": false }),
            });
        }
        let picture = self.macro_picture_of(account_id).await?;
        let (pw, ph) = (picture.width, picture.height);
        // Cut from a window of another width (the macro's recordedSize, when the page sends it): scaled to this one's.
        let width = a.get("recordedSize").filter(|r| r.is_object()).and_then(|r| r.get("w")).and_then(Value::as_i64).and_then(|w| i32::try_from(w).ok());
        let scale = match width {
            Some(w) if w > 0 => pw as f64 / w as f64,
            _ => 1.0,
        };
        let not = test.not == Some(true);
        match test.kind.as_str() {
            "image" => {
                let gray = macro_vision::to_gray(&picture, None);
                let (image, region) = (test.image.clone().unwrap_or_default(), test.region);
                let (gray, result) = rt::blocking(move || {
                    let found = macro_vision::template(&image, scale).and_then(|t| macro_vision::find(&gray, &t, macro_vision::area(region.as_ref(), gray.w, gray.h)));
                    (gray, found)
                })
                .await;
                let found = result.as_ref().is_some_and(|(score, _)| *score >= test.threshold.unwrap_or(0.9));
                Ok(json!({
                    "ok": found != not,
                    "found": if found { result.map(|(_, at)| page_json::write(&macro_vision::fraction(at, gray.w, gray.h), Shape::Rect)) } else { None },
                    "score": result.map(|(score, _)| macro_player::round_to(score, 3)),
                }))
            }
            "text" => {
                let area = macro_vision::area(test.region.as_ref(), pw, ph);
                let desk = self.macro_desk_now();
                let read = rt::blocking(move || desk.read(&picture, area)).await?.ok_or_else(|| {
                    user(l::t("Windows can't read text on this PC: add a language with text recognition in Settings › Time & language."))
                })?;
                let mut at = None;
                for lines in &read {
                    at = match &test.words {
                        Some(words) => macro_ocr::locate_any(lines, words, words).map(|(b, _)| b),
                        None => macro_ocr::locate(lines, test.text.as_deref().unwrap_or("")),
                    };
                    if at.is_some() {
                        break;
                    }
                }
                let text = read.first().map(|lines| lines.iter().map(|l| l.text()).collect::<Vec<_>>().join("\n")).unwrap_or_default();
                let found = at.map(|b| {
                    // Rectangle.Round(box).
                    let r = Rect::new(b.x.round() as i32, b.y.round() as i32, b.width.round() as i32, b.height.round() as i32);
                    page_json::write(&macro_vision::fraction(r, pw, ph), Shape::Rect)
                });
                Ok(json!({
                    "ok": at.is_some() != not,
                    "found": found,
                    "text": if net::len16(&text) > 2000 { cut_units(&text, 2000) } else { text },
                }))
            }
            _ => {
                // PORT: C#'s X!.Value threw on a test without a place; check_test refuses one, 0 stands in all the same.
                let ok = macro_vision::colour_at(&picture, test.x.unwrap_or(0.0), test.y.unwrap_or(0.0), test.colour.as_deref().unwrap_or(""), test.tolerance.unwrap_or(24));
                Ok(json!({ "ok": ok != not }))
            }
        }
    }

    // ───────────────────────── recording ─────────────────────────

    /// Records the account's window: it comes forward, and what the person does in it after a 3-second countdown becomes
    /// steps.
    pub fn record_macro(self: &Rc<Self>, account_id: Guid) -> Res<Value> {
        self.vault(|_| ())?;
        if self.macros.recorder.borrow().is_some() {
            return Err(user(l::t("A recording is already going on.")));
        }
        let Some(window) = self.macro_desk_now().window(account_id) else {
            log::note(&format!("Macro recording: refused on account {}, no Roblox window.", short(account_id)));
            return Err(user(l::t("This account's Roblox isn't open.")));
        };
        if window.elevated {
            log::note(&format!("Macro recording: refused, window {} runs as administrator.", window.pid));
            return Err(user(l::t("This Roblox runs as administrator, so Bloxkeep can't record or play into it.")));
        }
        // Runs wait while recording: their input would be recorded too, and would fight the person's.
        // The recorder first: when Windows refuses it, the runs go on.
        let post: Post = Arc::new(|job| rt::post(job));
        // PORT: C# handed the recorder AppController's RobloxInput; here the App owns its RobloxInput (not shared), so the
        // recorder gets none and guesses the camera's turn from the pointer, as it does when raw input isn't working.
        let recorder = MacroRecorder::new(account_id, window.pid, window.handle, None, Some(post))?;
        if let Some(player) = self.macro_player() {
            player.stop(None, "recording", None, false);
        }
        recorder.set_stop_asked(Some(Arc::new(|| {
            rt::post(|| {
                App::with(|app| {
                    let app = app.clone();
                    rt::spawn(async move {
                        let _ = app.stop_recording().await;
                    });
                })
            })
        })));
        recorder.set_changed(Some(Arc::new(|| rt::post(|| App::with(|app| app.macros_changed())))));
        *self.macros.recorder.borrow_mut() = Some(recorder);
        self.macro_player_now().recording.store(true, Ordering::SeqCst);
        self.hotkeys_changed(); // F8
        roblox_processes::focus_in_turn(window.pid);
        self.push_state();
        Ok(Value::Null)
    }

    /// Ends the recording; the page gets its steps as the event macroRecorded.
    pub async fn stop_recording(self: &Rc<Self>) -> Res<Value> {
        let Some(mut recorder) = self.macros.recorder.borrow_mut().take() else {
            return Ok(Value::Null);
        };
        log::note(&format!("Macro recording: ending after {:.0} s (counted from the end of the countdown).", recorder.starts_at.age_secs()));
        if let Some(player) = self.macro_player() {
            player.recording.store(false, Ordering::SeqCst);
        }
        self.hotkeys_changed();
        let (steps, recorder) = rt::blocking(move || {
            let steps = recorder.finish();
            (steps, recorder)
        })
        .await;
        self.push_state();
        if self.controller.vault.borrow().is_some() {
            let steps: Vec<Value> = steps.iter().map(|s| page_json::write(s, Shape::Step)).collect();
            self.emit("macroRecorded", json!({ "steps": steps, "w": recorder.client.width, "h": recorder.client.height }));
        }
        Ok(Value::Null)
    }

    /// The recording dropped without its steps (the vault locked).
    pub fn drop_recording(&self) {
        let recorder = self.macros.recorder.borrow_mut().take();
        if let Some(mut r) = recorder {
            log::note("Macro recording: dropped without its steps (the vault locked).");
            r.dispose();
        }
        if let Some(player) = self.macro_player() {
            player.recording.store(false, Ordering::SeqCst);
        }
    }

    // ───────────────────────── files ─────────────────────────

    pub fn import_macro(&self) -> Res<Value> {
        self.vault(|_| ())?;
        let t = MACRO_FILE_TYPE;
        let filter = format!(
            "{} (*{t}; *.rec)|*{t};*.rec|{} (*{t})|*{t}|{} (*.rec)|*.rec",
            l::t("Macros"),
            l::t("Bloxkeep macro"),
            l::t("TinyTask recording")
        );
        let owner = self.controller.owner.get();
        let title = l::t("Add a macro from a file");
        let dir = documents();
        let Some(path) = ui::modal(|| open_file_dialog(owner, &title, &filter, &dir)) else {
            return Ok(Value::Null);
        };
        Ok(App::for_page(&App::read_macro_file(&path)?))
    }

    /// A macro not saved yet, for the editor: its id empty ("").
    pub fn for_page(m: &Macro) -> Value {
        let mut node = page_json::write(m, Shape::Macro);
        if let Some(o) = node.as_object_mut() {
            o.insert("id".into(), Value::String(String::new()));
        }
        node
    }

    /// A macro from a file, not saved: the editor opens it, and Save checks it.
    pub fn read_macro_file(path: &str) -> Res<Macro> {
        let size = std::fs::metadata(path).ok().filter(|m| m.is_file()).map(|m| m.len());
        if size.is_none_or(|len| len > MAX_MACRO_IMPORT) {
            return Err(user(l::t("That file is too big to be a macro.")));
        }
        let bytes = std::fs::read(path)?;
        let stem = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let name = App::cut_text(&stem, macro_rules::MAX_NAME as usize);
        if path.to_lowercase().ends_with(".rec") {
            return macro_recording::from_tiny_task(&bytes, if name.is_empty() { "TinyTask" } else { &name });
        }
        // JsonDocument.Parse skips a UTF-8 byte order mark.
        let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
        let json: Value = serde_json::from_slice(body).map_err(|_| user(l::t("That isn't a macro.")))?;
        let mut m = App::read_macro(&json)?;
        m.id = Guid::nil();
        m.hotkey = None; // a hotkey may be another macro's here
        m.enabled = false;
        m.account_ids = Vec::new(); // the accounts of someone else's vault
        if m.targets == "accounts" {
            m.targets = "selected".into();
        }
        if m.trigger.kind != "manual" && m.targets == "selected" {
            m.targets = "all".into();
        }
        Ok(m)
    }

    pub fn export_macro(&self, id: Guid) -> Res<Value> {
        let m = self.macro_by_id(id)?;
        // Path.GetInvalidFileNameChars() on Windows.
        let invalid = |ch: char| (ch as u32) < 32 || matches!(ch, '"' | '<' | '>' | '|' | ':' | '*' | '?' | '\\' | '/');
        let file_name: String = m.name.chars().map(|ch| if invalid(ch) { '-' } else { ch }).collect::<String>() + MACRO_FILE_TYPE;
        let filter = format!("{} (*{MACRO_FILE_TYPE})|*{MACRO_FILE_TYPE}", l::t("Bloxkeep macro"));
        let owner = self.controller.owner.get();
        let title = l::t("Save the macro as a file");
        let dir = documents();
        let Some(path) = ui::modal(|| save_file_dialog(owner, &title, &filter, &file_name, &dir)) else {
            return Ok(Value::Null);
        };
        std::fs::write(&path, App::macro_file_text(&m))?;
        let saved = std::path::Path::new(&path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(json!({ "saved": saved }))
    }

    /// A macro as its file has it: the page's shape, without this vault's accounts.
    pub fn macro_file_text(m: &Macro) -> String {
        let mut copy = macro_rules::copy(m);
        copy.account_ids = Vec::new();
        copy.hotkey = None;
        serde_json::to_string_pretty(&page_json::write(&copy, Shape::Macro)).unwrap_or_default()
    }

    // ───────────────────────── what starts a macro by itself ─────────────────────────

    /// The open vault's macros (`Open?.Data.Settings.Macros`), a copy; None while locked.
    pub(crate) fn open_macros(&self) -> Option<Vec<Macro>> {
        let vault = self.open()?;
        let macros = vault.data().settings.macros.clone();
        Some(macros)
    }

    /// Ctrl+Alt+digit: the macro holding it runs on its accounts; one for the selected accounts runs on the window in
    /// front when that is an account's, else on those it last ran on.
    pub fn run_macro_hotkey(&self, digit: i32) {
        let open = self.open_macros();
        let Some(m) = open.as_ref().and_then(|list| list.iter().find(|m| m.hotkey == Some(digit)).cloned()) else {
            log::note(&format!("Macro hotkey Ctrl+Alt+{digit}: {}.", if open.is_none() { "the vault is locked" } else { "no macro holds it" }));
            return;
        };
        let tag = format!("Macro {}: hotkey Ctrl+Alt+{digit}", short(m.id));
        let mut accounts: Option<Vec<Guid>> = None;
        if m.targets == "selected" {
            let front = roblox_processes::foreground_pid();
            let in_front: Vec<Guid> = self.controller.running.borrow().iter().filter(|(_, (pid, _))| *pid == front).map(|(id, _)| *id).collect();
            accounts = if !in_front.is_empty() { Some(in_front) } else { self.macros.macro_last_accounts.borrow().get(&m.id).cloned() };
            if accounts.as_ref().is_none_or(Vec::is_empty) {
                log::note(&format!("{tag} did nothing: no Roblox window in front (process {front}) and no accounts it ran on before."));
                self.toast(&l::f("Run {0} from Bloxkeep once first: it doesn't know which accounts to use.", &[&m.name]), "warn");
                return;
            }
        }
        let open: Vec<Guid> = match self.macro_accounts(&m, accounts.as_deref()) {
            Ok(list) => {
                let running = self.controller.running.borrow();
                list.into_iter().filter(|a| running.contains_key(a)).collect()
            }
            Err(e) => {
                log::write(&e);
                return;
            }
        };
        if open.is_empty() {
            log::note(&format!("{tag} did nothing: none of its accounts has Roblox open (targets {}).", m.targets));
            self.toast(&l::f("{0}: none of its accounts has Roblox open.", &[&m.name]), "warn");
            return;
        }
        log::note(&format!("{tag} pressed for {} open accounts.", open.len()));
        if let Err(e) = self.queue_macro(&m, &open) {
            log::write(&e);
        }
    }

    /// The digits macros hold, for the window to register their Ctrl+Alt hotkeys.
    pub fn macro_hotkeys(&self) -> Vec<i32> {
        let mut digits: Vec<i32> =
            self.open_macros().unwrap_or_default().iter().filter_map(|m| m.hotkey).filter(|d| (1..=9).contains(d)).collect();
        digits.sort_unstable();
        digits.dedup();
        digits
    }

    pub fn recording_macro(&self) -> bool {
        self.macros.recorder.borrow().is_some()
    }

    /// Every process tick: macros that start by themselves, a recording past its 10 minutes.
    pub fn tick_macros(self: &Rc<Self>, now: Stamp) {
        let past = self
            .macros
            .recorder
            .borrow()
            .as_ref()
            .is_some_and(|r| now.secs_since(r.starts_at) > crate::core::macro_recorder::MAX_LENGTH.as_secs_f64());
        if past {
            log::note("Macro recording: reached its 10 minutes.");
            let app = self.clone();
            rt::spawn(async move {
                let _ = app.stop_recording().await;
            });
        }
        if self.controller.vault.borrow().is_none() || self.macros.recorder.borrow().is_some() {
            return;
        }
        let settle = MACRO_SETTLE.as_secs_f64();
        // Not before Bloxkeep has watched the windows a while: the windows already in a game when it opened count as just joined.
        if now.secs_since(self.relaunch.started.get()) < settle {
            return;
        }
        let Ok(macros) = self.vault(|d| d.settings.macros.clone()) else { return };
        for m in &macros {
            if !m.enabled || m.trigger.kind == "manual" {
                continue;
            }
            let accounts = self.macro_accounts(m, None).unwrap_or_default();
            if m.trigger.kind == "interval" {
                let every = m.trigger.every_seconds.unwrap_or(60) as f64;
                let last = self.macros.macro_interval_at.borrow().get(&m.id).copied();
                let Some(last) = last.filter(|l| *l <= now) else {
                    // The first run a settle after it was turned on (or Bloxkeep opened), then every period.
                    self.macros.macro_interval_at.borrow_mut().insert(m.id, now.plus_secs(-every + settle));
                    continue;
                };
                if now.secs_since(last) < every {
                    continue;
                }
                self.macros.macro_interval_at.borrow_mut().insert(m.id, now);
                let ready: Vec<Guid> = accounts.iter().copied().filter(|id| self.macro_client(*id).is_some_and(|c| c.2)).collect();
                if !ready.is_empty() {
                    let queued = self.queue_macro(m, &ready).unwrap_or_else(|e| {
                        log::write(&e);
                        0
                    });
                    self.note_trigger(m, &format!("interval trigger fired, {} of {} accounts in a game, {queued} runs started", ready.len(), accounts.len()), false);
                } else {
                    self.note_trigger(m, &format!("interval trigger due, but none of its {} accounts is in a game", accounts.len()), false);
                }
                continue;
            }
            let mut due: Vec<Guid> = Vec::new();
            for id in &accounts {
                let run = self.controller.running.borrow().get(id).cloned();
                let Some((pid, launch)) = run else { continue };
                let state = self.controller.logs.borrow().state_of(pid);
                let Some(state) = state.filter(|s| s.status == "ingame") else { continue };
                if now.secs_since(state.at) < settle {
                    continue;
                }
                if m.trigger.kind == "rejoin" && !launch.rejoin {
                    continue;
                }
                if self.macros.macro_join_ran.borrow().get(&(m.id, *id)) == Some(&state.at) {
                    continue;
                }
                self.macros.macro_join_ran.borrow_mut().insert((m.id, *id), state.at);
                due.push(*id);
            }
            if !due.is_empty() {
                let queued = self.queue_macro(m, &due).unwrap_or_else(|e| {
                    log::write(&e);
                    0
                });
                self.note_trigger(m, &format!("{} trigger fired on {} accounts, {queued} runs started", m.trigger.kind, due.len()), true);
            }
        }
    }

    /// error.log: what a macro's trigger did, written again only when it changes (a 10 s repeat would fill the log).
    fn note_trigger(&self, m: &Macro, text: &str, always: bool) {
        if !always && self.macros.macro_trigger_noted.borrow().get(&m.id).map(String::as_str) == Some(text) {
            return;
        }
        self.macros.macro_trigger_noted.borrow_mut().insert(m.id, text.to_string());
        log::note(&format!("Macro {}: {text}.", short(m.id)));
    }

    /// The vault locked: runs stop, the recording is dropped, variables go.
    pub fn stop_macros_for_lock(&self) {
        self.drop_recording();
        if let Some(player) = self.macro_player() {
            player.clear("vaultLocked");
        }
        self.macros.macro_last_accounts.borrow_mut().clear();
        self.macros.macro_interval_at.borrow_mut().clear();
        self.macros.macro_join_ran.borrow_mut().clear();
        self.macros.macro_trigger_noted.borrow_mut().clear();
        self.macros.macro_sweep.set(None);
        self.hotkeys_changed();
    }

    /// The PC going to sleep: what plays stops (the game won't be where it was at wake).
    pub fn stop_macros_for_sleep(self: &Rc<Self>) {
        let app = self.clone();
        rt::spawn(async move {
            let _ = app.stop_recording().await;
        });
        if let Some(player) = self.macro_player() {
            player.stop(None, "locked", None, false);
        }
    }

    /// A macro run is going on or waits: an update's restart waits for it.
    pub fn macros_playing(&self) -> bool {
        self.macro_player().is_some_and(|p| p.active()) || self.macros.recorder.borrow().is_some()
    }

    // ───────────────────────── the page's state ─────────────────────────

    pub fn macro_runs_dto(&self) -> Value {
        let player = self.macro_player();
        let (Some(_), Some(player)) = (self.open(), player) else {
            return json!([]);
        };
        let runs: Vec<Value> = player
            .runs()
            .iter()
            .map(|r| {
                let r = lk(r);
                let finished = r.finished();
                json!({
                    "macroId": r.r#macro.id,
                    "accountId": r.account_id,
                    "status": r.status,
                    "reason": r.reason,
                    "step": if finished { None } else { Some(r.shown_pc()) },
                    // The feature playing (or where an ended run was): its id; none for the main steps.
                    "section": r.section,
                    // Where an ended run was: the editor marks the step a failed run stopped at.
                    "lastStep": if finished { Some(r.shown_pc()) } else { None },
                    "trial": if r.trial { Some(true) } else { None },
                    "loops": if r.loops > 0 { Some(r.loops) } else { None },
                    "startedAt": r.started_at,
                    "endsAt": r.ends_at,
                })
            })
            .collect();
        Value::Array(runs)
    }

    /// The macros on the selected accounts that Run still has somewhere to go with none selected: the accounts in their
    /// game, the Roblox window in front, or those it last ran on (as their panel works on).
    pub fn macro_can_run_dto(&self) -> Value {
        if self.open().is_none() {
            return json!([]);
        }
        let ids: Vec<Guid> =
            self.macro_list().unwrap_or_default().iter().filter(|m| m.targets == "selected" && !self.panel_accounts(m).is_empty()).map(|m| m.id).collect();
        json!(ids)
    }

    pub fn macro_recording_dto(&self) -> Value {
        if self.open().is_none() {
            return Value::Null;
        }
        match self.macros.recorder.borrow().as_ref() {
            Some(r) => json!({ "accountId": r.account_id, "startsAt": r.starts_at, "steps": r.steps() }),
            None => Value::Null,
        }
    }

    pub fn macro_vars_dto(&self) -> Value {
        match (self.open(), self.macro_player()) {
            (Some(_), Some(player)) => {
                let st = player.state();
                let map: serde_json::Map<String, Value> = st.vars.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
                Value::Object(map)
            }
            _ => json!({}),
        }
    }

    pub fn macro_sweep_dto(&self) -> Value {
        match (self.open(), self.macros.macro_sweep.get()) {
            (Some(_), Some(seconds)) if self.macro_player().is_some_and(|p| p.active()) => json!({ "seconds": seconds }),
            _ => Value::Null,
        }
    }
}
