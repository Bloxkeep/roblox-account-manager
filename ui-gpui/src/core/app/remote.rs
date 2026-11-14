// AppController.Remote.cs.
//! Settings › Discord › Control from Discord (off until turned on): Bloxkeep's Discord bot hands this PC the person's /pc
//! commands (RemoteLink, services/DiscordBot/worker/remote.js), once they typed the code shown here into /pc link. What a
//! command can do is only this: say how the accounts are doing (the status board's card), run a preset, close windows,
//! switch accounts to another server, picture their windows. Nothing is asked of the person on the PC: a preset that would ask first
//! (Multi-Roblox off…) isn't run. Locked, Bloxkeep answers only that it is locked, and names nothing.
//! Turned off: the bot is told to forget the link, the secret is dropped (a new PC for the bot next time), and the
//! connection closes.
//! Each vault has its own switch and secret (prefs remote_control): vaults are for different people, so the link is
//! the vault's, a PC of its own for the bot, connected only while that vault is the one offered or open. Another vault
//! unlocked, it is closed (Discord says that PC isn't connected), and nothing of the other vault reaches it.
use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use futures::future::{Either, select};
use serde_json::{Map, Value, json};

use crate::core::app::App;
use crate::core::app::presets::LaunchResult;
use crate::core::base::{Fail, Guid, Res, Stamp};
use crate::core::discord_webhook::{self as webhook, DiscordPing};
use crate::core::models::{Account, LaunchPreset, stamp_min};
use crate::core::remote_link::{self, Asking, CommandFn, RemoteCommand, RemoteLink};
use crate::core::{l, log, prefs, rt, vaults};

const REMOTE_ENTROPY: &[u8] = b"bloxkeep-remote-key";

/// "All accounts", as Discord's suggestion for /pc close says it.
pub const ALL_ACCOUNTS: &str = "All accounts";

/// A preset that's still opening after this long is answered "opening…", and its outcome follows up later. Within
/// the 1.5 s the bot waits before Discord's "thinking…" (remote.js ANSWER_WAIT).
pub const REMOTE_QUICK_ANSWER: Duration = Duration::from_secs(1);
pub const REMOTE_FORGET_EVERY: Duration = Duration::from_secs(10 * 60);
pub const REMOTE_FORGET_AFTER_OFF: Duration = Duration::from_secs(30);
/// Discord shows 10 pictures in a message at most.
pub const MAX_SHOTS: usize = 10;
/// Between two screenshots of this PC: picturing shows minimized windows for a moment and holds anti-AFK back, so
/// asked again and again (by the person, or by whoever got into their Discord) the windows still get their nudges.
pub const SHOT_GAP: Duration = Duration::from_secs(15);

pub struct RemoteState {
    pub remote: RefCell<Option<Arc<RemoteLink>>>,
    /// The vault (`VaultEntry::id`) whose link `remote` is.
    pub remote_vault: RefCell<Option<String>>,
    /// A hash of what the last hello was made from (`tick_remote`).
    pub remote_said: Cell<Option<u64>>,
    pub next_remote_forget: Cell<Stamp>,
    pub remote_forgetting: Cell<bool>,
    /// The bot couldn't be told to forget an old link last time: written once until it goes through.
    pub remote_forget_failing: Cell<bool>,
    /// The Discord account last said to be asking: a new one is told with a Windows notification too.
    pub remote_ask_told: RefCell<Option<String>>,
    /// A /pc switch waiting for Roblox's server list: commands run side by side, and Hopping only knows of it after.
    pub remote_switching: Cell<bool>,
    /// A window being pictured: anti-AFK waits (both make a window unseen for a moment, and would undo each other).
    pub picturing: Cell<bool>,
    /// When the last screenshot was done.
    pub last_shot: Cell<Stamp>,
}

impl Default for RemoteState {
    fn default() -> RemoteState {
        RemoteState {
            remote: RefCell::new(None),
            remote_vault: RefCell::new(None),
            remote_said: Cell::new(None),
            next_remote_forget: Cell::new(stamp_min()),
            remote_forgetting: Cell::new(false),
            remote_forget_failing: Cell::new(false),
            remote_ask_told: RefCell::new(None),
            remote_switching: Cell::new(false),
            picturing: Cell::new(false),
            last_shot: Cell::new(stamp_min()),
        }
    }
}

/// The bot's address; the checks point it at a stand-in.
pub fn remote_test_url() -> Option<String> {
    std::env::var("BLOXKEEP_REMOTE_URL").ok().filter(|u| !u.is_empty())
}

/// Environment.MachineName: this PC's NetBIOS name.
pub fn machine_name() -> String {
    use windows_sys::Win32::System::WindowsProgramming::GetComputerNameW;
    let mut buffer = [0u16; 64];
    let mut size = buffer.len() as u32;
    // SAFETY: `buffer` holds `size` u16s, and Windows writes at most that many (with the size it wrote back).
    let ok = unsafe { GetComputerNameW(buffer.as_mut_ptr(), &mut size) } != 0;
    if ok { String::from_utf16_lossy(&buffer[..size as usize]) } else { String::new() }
}

/// ProtectedData.Protect / Unprotect for this Windows user, with Bloxkeep's entropy; None when Windows refused
/// (CryptographicException).
fn dpapi(data: &[u8], protect: bool) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let entropy = CRYPT_INTEGER_BLOB { cbData: REMOTE_ENTROPY.len() as u32, pbData: REMOTE_ENTROPY.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: the blobs point at slices that live through the call; Windows only reads `input` and `entropy` and
    // fills `output`, which is freed with LocalFree below as the docs ask.
    let ok = unsafe {
        if protect {
            CryptProtectData(&input, std::ptr::null(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        }
    } != 0;
    if !ok || output.pbData.is_null() {
        return None;
    }
    // SAFETY: Windows gave `cbData` bytes at `pbData`; they are copied out before being freed.
    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: `pbData` was allocated by Windows with LocalAlloc for this call.
    unsafe { LocalFree(output.pbData as _) };
    Some(bytes)
}

/// RandomNumberGenerator.GetBytes(n).
fn random_bytes(n: usize) -> Option<Vec<u8>> {
    use windows_sys::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};
    let mut bytes = vec![0u8; n];
    // SAFETY: `bytes` is n long; the system's RNG needs no algorithm handle with BCRYPT_USE_SYSTEM_PREFERRED_RNG.
    let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), n as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status == 0 { Some(bytes) } else { None }
}

/// A saved key read back: Err(the C# exception's name) when it isn't base64 (FormatException) or this Windows user
/// can't decrypt it (CryptographicException).
fn read_key(key: &str) -> Result<String, &'static str> {
    let bytes = STANDARD.decode(key).map_err(|_| "FormatException")?;
    let plain = dpapi(&bytes, false).ok_or("CryptographicException")?;
    String::from_utf8(plain).map_err(|_| "CryptographicException")
}

/// This vault's secret for the bot, made when it's turned on (random, kept encrypted for this Windows user).
pub fn remote_secret() -> Option<String> {
    let saved = prefs::lock().remote_key();
    if let Some(saved) = saved.filter(|s| !s.is_empty()) {
        match read_key(&saved) {
            Ok(secret) => return Some(secret),
            // Unreadable (copied from another Windows user): a new one, and the PC is linked again with a new code.
            Err(kind) => log::note(&format!("Remote: the saved key can't be read ({kind}); a new one is made, so the PC must be linked again.")),
        }
    }
    // Base64 without '=' and with '-' and '_' for '+' and '/'.
    let secret = URL_SAFE_NO_PAD.encode(random_bytes(32)?);
    // PORT: C#'s ProtectedData.Protect threw when Windows refused; here nothing is made and the link waits.
    let protected = dpapi(secret.as_bytes(), true)?;
    prefs::lock().set_remote_key(Some(STANDARD.encode(protected)));
    prefs::save();
    Some(secret)
}

pub fn is_all_accounts(name: &str) -> bool {
    let name = name.trim();
    // PORT: CurrentCultureIgnoreCase: compared here by lower case.
    name.eq_ignore_ascii_case(ALL_ACCOUNTS) || name.to_lowercase() == l::t(ALL_ACCOUNTS).to_lowercase()
}

impl App {
    /// The link's own vault is open and unlocked: only then does a command see anything.
    pub fn remote_open(&self) -> bool {
        self.has_open_vault() && !self.auto_lock.screen_locked.get() && *self.remote.remote_vault.borrow() == vaults::current().map(|v| v.id)
    }

    /// Connected while the vault offered or open has it on, closed when it's off or another vault is offered; the names
    /// Discord suggests kept up to date.
    pub fn tick_remote(self: &Rc<Self>) {
        self.forget_remote_links_if_due();
        let vault = vaults::current().map(|v| v.id);
        let remote_control = prefs::lock().remote_control();
        if self.remote.remote.borrow().is_some() && (!remote_control || *self.remote.remote_vault.borrow() != vault) {
            self.stop_remote();
        }
        if remote_control && self.remote.remote.borrow().is_none() {
            if let Some(secret) = remote_secret() {
                let command: CommandFn = Box::new(|c: RemoteCommand| -> Option<Map<String, Value>> {
                    // A command from the link's thread: answered on the core's, and waited for here.
                    let (tx, rx) = std::sync::mpsc::channel::<Option<Value>>();
                    rt::post(move || {
                        App::with(|app| {
                            let app = app.clone();
                            rt::spawn(async move {
                                let answer = app.remote_command(c).await;
                                let _ = tx.send(answer);
                            });
                        })
                    });
                    rx.recv().ok().flatten().and_then(|v| v.as_object().cloned())
                });
                let changed = Box::new(|| rt::post(|| App::with(|app| app.on_remote_changed())));
                *self.remote.remote.borrow_mut() = Some(Arc::new(RemoteLink::new(&secret, command, changed, remote_test_url().as_deref())));
                *self.remote.remote_vault.borrow_mut() = vault;
                self.remote.remote_said.set(None);
            }
        }
        let Some(link) = self.remote.remote.borrow().clone() else { return };
        let open = self.remote_open();
        let machine = machine_name();
        let language = l::language();
        // What it would say, hashed from what its names are made of (read in place: no copies, no names built every
        // tick): the hello (names cut to fit, then JSON) is made only when that changed.
        let mut said = DefaultHasher::new();
        open.hash(&mut said);
        language.hash(&mut said);
        machine.hash(&mut said);
        if open {
            let _ = self.vault(|d| {
                d.discord.hide_names.hash(&mut said);
                d.discord.language.hash(&mut said);
                for a in &d.accounts {
                    a.id.hash(&mut said);
                    a.alias.hash(&mut said);
                    a.username.hash(&mut said);
                }
                '|'.hash(&mut said);
                for preset in &d.settings.presets {
                    preset.name.hash(&mut said);
                }
            });
        }
        let key = said.finish();
        if self.remote.remote_said.get() == Some(key) {
            return;
        }
        self.remote.remote_said.set(Some(key));
        let (accounts, presets): (Vec<Account>, Vec<LaunchPreset>) =
            if open { self.vault(|d| (d.accounts.clone(), d.settings.presets.clone())).unwrap_or_default() } else { (Vec::new(), Vec::new()) };
        let names: Vec<String> = accounts.iter().map(|a| self.discord_name(a)).collect();
        // PORT: Distinct(StringComparer.CurrentCultureIgnoreCase): compared here by lower case.
        let distinct = |list: Vec<String>| {
            let mut out: Vec<String> = Vec::new();
            for n in list {
                if !out.iter().any(|o| o.to_lowercase() == n.to_lowercase()) {
                    out.push(n);
                }
            }
            out
        };
        let account_names = if open { distinct(names) } else { Vec::new() };
        let preset_names = if open { distinct(presets.iter().map(|p| p.name.clone()).filter(|n| !n.is_empty()).collect()) } else { Vec::new() };
        link.hello(&machine, &account_names, &preset_names, &language);
    }

    /// Links turned off or of vaults gone, told to the bot until it has said so (prefs remote_gone).
    /// A key this Windows user can't read any more can't be told: dropped.
    pub fn forget_remote_links_if_due(self: &Rc<Self>) {
        if self.remote.remote_forgetting.get() || prefs::lock().remote_gone.is_empty() || Stamp::utc_now() < self.remote.next_remote_forget.get() {
            return;
        }
        self.remote.next_remote_forget.set(Stamp::utc_now().plus_secs(REMOTE_FORGET_EVERY.as_secs_f64()));
        self.remote.remote_forgetting.set(true);
        let app = self.clone();
        rt::spawn(async move { app.forget_remote_links().await });
    }

    async fn forget_remote_links(self: &Rc<Self>) {
        // Set before the task started; cleared however this ends.
        let _forgetting = crate::core::base::Busy::set(&self.remote.remote_forgetting);
        let keys: Vec<String> = prefs::lock().remote_gone.clone();
        for key in keys {
            let secret = match read_key(&key) {
                Ok(secret) => Some(secret),
                Err(kind) => {
                    log::note(&format!("Remote: an old link's key can't be read by this Windows user ({kind}); dropped without telling the bot."));
                    None
                }
            };
            let told = match &secret {
                None => true,
                Some(secret) => {
                    let (secret, url) = (secret.clone(), remote_test_url());
                    rt::blocking(move || remote_link::forget(&secret, url.as_deref())).await
                }
            };
            if told {
                if secret.is_some() && self.remote.remote_forget_failing.get() {
                    self.remote.remote_forget_failing.set(false);
                    log::note("Remote: the bot was told to forget an old link (recovered).");
                }
                prefs::lock().remote_gone.retain(|k| *k != key);
                prefs::save();
            } else if !self.remote.remote_forget_failing.get() {
                self.remote.remote_forget_failing.set(true);
                log::note("Remote: the bot couldn't be told to forget an old link; asked again in 10 minutes.");
            }
        }
    }

    pub fn stop_remote(&self) {
        if let Some(link) = self.remote.remote.borrow_mut().take() {
            link.dispose();
        }
        *self.remote.remote_vault.borrow_mut() = None;
    }

    pub fn set_remote(self: &Rc<Self>, on: bool) -> Res<Value> {
        if on == prefs::lock().remote_control() {
            return Ok(Value::Null);
        }
        prefs::lock().set_remote_control(on);
        log::note(&format!("Remote: control from Discord turned {}.", if on { "on" } else { "off" }));
        // Off, the secret goes: turned on again, this is a new PC for the bot, linked to nobody until a new code is
        // typed and allowed. The unlink below doesn't reach a bot that can't be reached right now, and with the same
        // secret the PC came back still linked to that Discord account, without asking (2026-10-30 review).
        if !on {
            let mut p = prefs::lock();
            let key = p.remote_key();
            p.forget_remote_key(key.as_deref());
            p.set_remote_key(None);
            drop(p);
            // after the unlink below, if that one got through
            self.remote.next_remote_forget.set(Stamp::utc_now().plus_secs(REMOTE_FORGET_AFTER_OFF.as_secs_f64()));
        }
        prefs::save();
        if on {
            self.tick_remote();
            if let Some(link) = self.remote.remote.borrow().clone() {
                link.ask_code();
            }
        } else if let Some(link) = self.remote.remote.borrow_mut().take() {
            // The bot forgets the link before the connection closes.
            std::thread::spawn(move || {
                link.unlink();
                link.dispose();
            });
            *self.remote.remote_vault.borrow_mut() = None;
        }
        self.push_state();
        Ok(Value::Null)
    }

    pub fn on_remote_changed(&self) {
        let asking = self.remote.remote.borrow().as_ref().and_then(|r| r.asking());
        match asking {
            Some(Asking { user, id, .. }) if *self.remote.remote_ask_told.borrow() != Some(id.clone().unwrap_or_else(|| user.clone())) => {
                *self.remote.remote_ask_told.borrow_mut() = Some(id.unwrap_or_else(|| user.clone()));
                let text = l::f("{0} wants to control this PC from Discord. Allow it in Bloxkeep only if that's you.", &[&user]);
                self.notify(&text, "warn", "remote", Guid::nil(), false, None);
            }
            Some(_) => {}
            None => *self.remote.remote_ask_told.borrow_mut() = None,
        }
        self.push_state();
    }

    /// The answer to someone linking this PC (the page's Allow / Don't allow).
    pub fn remote_allow(&self, allow: bool, id: Option<&str>) -> Res<Value> {
        let link = self.remote.remote.borrow().clone();
        log::note(&match &link {
            None => "Remote: a link request was answered, but there is no connection to the bot; nothing sent.".to_string(),
            Some(_) => format!("Remote: a link request was {}.", if allow { "allowed" } else { "refused" }),
        });
        if let Some(link) = link {
            link.answer(allow, id);
        }
        self.push_state();
        Ok(Value::Null)
    }

    /// A new code for /pc link (the old one stops working).
    pub fn remote_new_code(&self) -> Res<Value> {
        if let Some(link) = self.remote.remote.borrow().clone() {
            link.ask_code();
        }
        self.push_state();
        Ok(Value::Null)
    }

    pub fn remote_dto(&self) -> Value {
        let link = self.remote.remote.borrow().clone();
        let open = self.remote_open();
        json!({
            "on": prefs::lock().remote_control(),
            "connected": link.as_ref().is_some_and(|r| r.connected()),
            "linked": link.as_ref().and_then(|r| r.linked()),
            "user": link.as_ref().and_then(|r| r.user()),
            "code": link.as_ref().and_then(|r| r.code()),
            "codeUntil": link.as_ref().and_then(|r| r.code_until()).map(|until| until.unix_ms()),
            // Asked only while unlocked: the lock screen says nothing of it, and an answer waits for the unlock.
            "ask": match link.as_ref().and_then(|r| r.asking()) {
                Some(Asking { user, replacing, id }) if open => json!({ "user": user, "replacing": replacing, "id": id }),
                _ => Value::Null,
            },
            "busy": link.as_ref().is_some_and(|r| r.busy()),
            "pc": machine_name(),
        })
    }

    /// An answer's text, with names hidden as Discord's own switch says (Settings › Discord): what the app wrote for itself
    /// (a launch error, a question a preset would ask) follows the app's switch, and showed real names in Discord while
    /// Discord's was on (2026-10-30 review).
    pub fn remote_say(&self, text: &str) -> Value {
        // What Discord was told, in error.log too (names as [account]): a "couldn't" said there must be findable here.
        let said: String = self.scrub(text).replace('\n', " / ").chars().take(300).collect();
        log::note_file(&format!("Remote: answered: {said}"));
        let hide = self.vault(|d| d.discord.hide_names).unwrap_or(false);
        json!({ "content": if hide { self.hide_names_in(text) } else { text.to_string() } })
    }

    /// A command from Discord, on the core's thread: its answer (an error is said, never thrown); None when it was
    /// answered already (the pictures of /pc screenshot go up on their own).
    pub async fn remote_command(self: &Rc<Self>, c: RemoteCommand) -> Option<Value> {
        let cmd = if matches!(c.name.as_str(), "status" | "preset" | "close" | "switch" | "screenshot") { c.name.as_str() } else { "unknown" };
        log::note(&format!("Remote: /pc {cmd} received."));
        // Another vault open (the link's own is closed a moment later): as locked, nothing of it said.
        if !self.remote_open() {
            let why = if !self.has_open_vault() || self.auto_lock.screen_locked.get() { "Bloxkeep locked" } else { "another vault open" };
            log::note(&format!("Remote: /pc {cmd} answered locked ({why})."));
            return Some(self.remote_say(&l::t("Bloxkeep is locked on this PC. Unlock it there first.")));
        }
        let answer: Res<Option<Value>> = match c.name.as_str() {
            "status" => self.remote_status().map(Some),
            "preset" => self.remote_preset(&c).await.map(Some),
            "close" => self.remote_close(c.value.as_deref().unwrap_or("")).map(Some),
            "switch" => self.remote_switch(c.value.as_deref().unwrap_or("")).await.map(Some),
            "screenshot" => self.remote_screenshot(&c).await,
            _ => Ok(Some(self.remote_say("?"))),
        };
        match answer {
            Ok(a) => a,
            Err(ex) if matches!(ex, Fail::User(_)) || crate::core::roblox_api::is_expected_error(&ex) => {
                let why = if matches!(ex, Fail::User(_)) {
                    "no such preset or account, or not possible now".to_string()
                } else {
                    App::log_kind_of(&ex)
                };
                log::note(&format!("Remote: /pc {cmd} failed ({why})."));
                Some(self.remote_say(&crate::core::app::controller::error_text(&ex)))
            }
            // Anything else (a picture Windows wouldn't make, memory running out): answered and logged, never let out, where it
            // would end the link to the bot until Bloxkeep restarts (2026-10-30 review).
            Err(ex) => {
                log::write(&ex);
                Some(self.remote_say(&l::t("Something went wrong on this PC. Try again.")))
            }
        }
    }

    /// The status board's card, as it is now, named after this PC.
    pub fn remote_status(&self) -> Res<Value> {
        let d = self.vault(|v| v.discord.clone())?;
        let players = crate::core::roblox_processes::get_players();
        let mut card = self.board_message(&d, &players);
        card.footer = Some(format!("Bloxkeep · {}", machine_name()));
        log::note("Remote: /pc status answered with the status card.");
        let payload = webhook::build(&card, &DiscordPing::none(), true, None, None, None);
        Ok(json!({ "embeds": payload.get("embeds").cloned() }))
    }

    pub async fn remote_preset(self: &Rc<Self>, c: &RemoteCommand) -> Res<Value> {
        let presets = self.vault(|d| d.settings.presets.clone())?;
        let wanted = c.value.as_ref().map(|v| v.trim().to_lowercase());
        // PORT: CurrentCultureIgnoreCase: compared here by lower case.
        let Some(preset) = presets.into_iter().find(|p| wanted.as_ref().is_some_and(|w| p.name.to_lowercase() == *w)) else {
            return Err(Fail::user(l::f("No preset is called {0}.", &[&c.value.clone().unwrap_or_default()])));
        };
        let (tx, rx) = futures::channel::oneshot::channel::<Res<LaunchResult>>();
        let app = self.clone();
        let id = preset.id;
        rt::spawn(async move {
            let _ = tx.send(app.start_preset(id, false).await);
        });
        let timer = rt::sleep(REMOTE_QUICK_ANSWER);
        futures::pin_mut!(timer);
        match select(rx, timer).await {
            Either::Left((run, _)) => {
                let run = run.map_err(|_| Fail::other("the preset's run ended without an answer"))??;
                Ok(self.preset_outcome(&preset, &run))
            }
            Either::Right((_, rx)) => {
                log::note("Remote: /pc preset still opening after a second; its outcome follows up.");
                let app = self.clone();
                let (id, name) = (c.id.clone(), preset.name.clone());
                rt::spawn(async move { app.follow_up_preset(id, preset, rx).await });
                Ok(self.remote_say(&l::f("Opening {0}…", &[&name])))
            }
        }
    }

    async fn follow_up_preset(self: &Rc<Self>, id: String, preset: LaunchPreset, run: futures::channel::oneshot::Receiver<Res<LaunchResult>>) {
        let outcome = match run.await.map_err(|_| Fail::other("the preset's run ended without an answer")).and_then(|r| r) {
            Ok(r) => self.preset_outcome(&preset, &r),
            Err(ex) if matches!(ex, Fail::User(_)) || crate::core::roblox_api::is_expected_error(&ex) => {
                log::note(&format!("Remote: /pc preset failed while opening ({}).", App::log_kind_of(&ex)));
                self.remote_say(&crate::core::app::controller::error_text(&ex))
            }
            Err(ex) => {
                log::write(&ex);
                self.remote_say(&l::t("Something went wrong on this PC. Try again."))
            }
        };
        let link = self.remote.remote.borrow().clone();
        match link {
            None => log::note("Remote: /pc preset finished, but the link to the bot is gone; its outcome isn't sent."),
            Some(link) => link.later(&id, outcome.as_object().cloned().unwrap_or_default()),
        }
    }

    /// How a preset's run went: opened how many, what went wrong, or the question it would have asked.
    pub fn preset_outcome(&self, preset: &LaunchPreset, r: &LaunchResult) -> Value {
        let errors = r.errors.as_ref().map_or(0, |e| e.len());
        log::note(&if r.needs_confirm {
            "Remote: /pc preset not run: it would ask a question on the PC first.".to_string()
        } else {
            format!("Remote: /pc preset {}: opened {} account(s), {errors} error(s).", if r.cancelled { "stopped" } else { "done" }, r.launched)
        });
        if r.needs_confirm {
            return self.remote_say(&format!("{}\n\n{}", r.message.clone().unwrap_or_default(), l::t("Nothing was opened: answer this in Bloxkeep on the PC.")));
        }
        let mut ids: Vec<Guid> = Vec::new();
        for id in &preset.account_ids {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        let total = ids.iter().filter(|id| self.find_account(**id).is_some()).count();
        let mut lines = vec![if r.cancelled {
            l::f("{0}: stopped after {1} of {2} accounts.", &[&preset.name, &r.launched, &total])
        } else {
            l::f("{0}: opened {1} of {2} accounts.", &[&preset.name, &r.launched, &total])
        }];
        lines.extend(r.errors.iter().flatten().take(10).map(|e| format!("- {e}")));
        self.remote_say(&lines.join("\n"))
    }

    /// An account by the name Discord suggested (its Discord name), else its label or username.
    pub fn find_remote_account(&self, name: &str) -> Res<Account> {
        let name = name.trim();
        let lower = name.to_lowercase();
        let accounts = self.vault(|d| d.accounts.clone())?;
        // PORT: CurrentCultureIgnoreCase: compared here by lower case.
        if let Some(a) = accounts.iter().find(|a| self.discord_name(a).to_lowercase() == lower) {
            return Ok(a.clone());
        }
        accounts
            .into_iter()
            .find(|a| a.label().to_lowercase() == lower || a.username.to_lowercase() == lower)
            .ok_or_else(|| Fail::user(l::f("No account is called {0}.", &[&name])))
    }

    /// The accounts with a Roblox window on this PC, in the list's order.
    pub fn playing_accounts(&self) -> Vec<Account> {
        let accounts = self.vault(|d| d.accounts.clone()).unwrap_or_default();
        let running = self.controller.running.borrow();
        accounts.into_iter().filter(|a| running.contains_key(&a.id)).collect()
    }

    pub fn remote_close(&self, name: &str) -> Res<Value> {
        if is_all_accounts(name) {
            // Also a window still to come (late) and a rejoin under way: Close all stops them too.
            let open = self.controller.running.borrow().len()
                + self.controller.pending.borrow().len()
                + self.launching.late.borrow().len()
                + if self.rejoin.rejoin_in_flight.borrow().is_none() { 0 } else { 1 };
            log::note(&format!("Remote: /pc close all: {open} window(s) to close."));
            if open == 0 {
                return Ok(self.remote_say(&l::t("No Roblox window is open on this PC.")));
            }
            self.close_all(false);
            return Ok(self.remote_say(&l::f("Closing {0} Roblox window(s)…", &[&open])));
        }
        let account = self.find_remote_account(name)?;
        let short = account.id.to_string()[..8].to_string();
        // As Close on the account: a window still to come (late), a rejoin waiting or under way count too, and are stopped.
        let has_window = self.controller.running.borrow().contains_key(&account.id)
            || self.controller.pending.borrow().iter().any(|p| p.account_id == account.id)
            || self.launching.late.borrow().iter().any(|p| p.account_id == account.id)
            || self.controller.rejoin_queue.borrow().iter().any(|r| r.account_id == account.id)
            || self.rejoin.rejoin_in_flight.borrow().as_ref().is_some_and(|r| r.account_id == account.id);
        if !has_window {
            log::note(&format!("Remote: /pc close: account {short} has no window; nothing closed."));
            return Ok(self.remote_say(&l::f("{0} has no Roblox window open.", &[&self.discord_name(&account)])));
        }
        log::note(&format!("Remote: /pc close: account {short} closing."));
        self.close_accounts(&[account.id], false);
        Ok(self.remote_say(&l::f("Closing {0}…", &[&self.discord_name(&account)])))
    }

    pub async fn remote_switch(self: &Rc<Self>, name: &str) -> Res<Value> {
        // Asked again before the last switch landed (Discord's /pc sent over and over): not moved again meanwhile.
        if self.hopping() || self.remote.remote_switching.get() {
            log::note("Remote: /pc switch refused: accounts are still moving to another server.");
            return Ok(self.remote_say(&l::t("Accounts are still moving to another server. Try again once they're in.")));
        }
        let _switching = crate::core::base::Busy::set(&self.remote.remote_switching);
        self.remote_switch_now(name).await
    }

    async fn remote_switch_now(self: &Rc<Self>, name: &str) -> Res<Value> {
        if is_all_accounts(name) {
            let playing = self.playing_accounts();
            if playing.is_empty() {
                log::note("Remote: /pc switch all: no window open; nothing moved.");
                return Ok(self.remote_say(&l::t("No Roblox window is open on this PC.")));
            }
            // Those in one game go to one server together, as Switch server does for several selected.
            let all = self.switch_server(&playing.iter().map(|a| a.id).collect::<Vec<_>>()).await?.as_i64().unwrap_or(0);
            log::note(&format!("Remote: /pc switch all: {all} of {} account(s) moving.", playing.len()));
            return Ok(self.remote_say(&if all > 0 {
                l::f("Moving {0} account(s) to another server…", &[&all])
            } else {
                l::t("Didn't switch: no other server of that game has room right now.")
            }));
        }
        let account = self.find_remote_account(name)?;
        let moved = self.switch_server(&[account.id]).await?.as_i64().unwrap_or(0);
        log::note(&format!(
            "Remote: /pc switch: account {} {}.",
            &account.id.to_string()[..8],
            if moved > 0 { "moving" } else { "not moved (no other server with room)" }
        ));
        Ok(self.remote_say(&if moved > 0 {
            l::f("{0} is moving to another server…", &[&self.discord_name(&account)])
        } else {
            l::t("Didn't switch: no other server of that game has room right now.")
        }))
    }

    /// /pc screenshot: a picture of one account's Roblox window, or of each (10 at most), sent to Discord by this PC
    /// (RemoteLink's shot), seen there only by who asked. What can't be pictured is said under them. One at a time,
    /// `SHOT_GAP` apart; the person at the PC is told each time.
    pub async fn remote_screenshot(self: &Rc<Self>, c: &RemoteCommand) -> Res<Option<Value>> {
        let now = Stamp::utc_now();
        let picturing = self.remote.picturing.get();
        let since_last = now.secs_since(self.remote.last_shot.get());
        let too_soon = since_last < SHOT_GAP.as_secs_f64();
        if picturing || too_soon {
            log::note(&format!("Remote: /pc screenshot refused: {}.", if picturing { "one is under way" } else { "asked again within 15 s" }));
        }
        if picturing {
            return Ok(Some(self.remote_say(&l::t("Already taking a screenshot. Try again in a moment."))));
        }
        if too_soon {
            let wait = (SHOT_GAP.as_secs_f64() - since_last).ceil() as i32;
            return Ok(Some(self.remote_say(&l::f("Wait {0} s, then try again.", &[&wait]))));
        }
        let name = c.value.clone().unwrap_or_default();
        let accounts: Vec<Account> = if is_all_accounts(&name) {
            let playing = self.playing_accounts();
            if playing.is_empty() {
                log::note("Remote: /pc screenshot: no window open; nothing pictured.");
                return Ok(Some(self.remote_say(&l::t("No Roblox window is open on this PC."))));
            }
            playing
        } else {
            let account = self.find_remote_account(&name)?;
            if !self.controller.running.borrow().contains_key(&account.id) {
                log::note(&format!("Remote: /pc screenshot: account {} has no window; nothing pictured.", &account.id.to_string()[..8]));
                return Ok(Some(self.remote_say(&l::f("{0} has no Roblox window open.", &[&self.discord_name(&account)]))));
            }
            vec![account]
        };
        let Some(link) = self.remote.remote.borrow().clone() else {
            log::note("Remote: /pc screenshot dropped: the link to the bot is gone.");
            return Ok(None);
        };
        let mut pictures: Vec<Vec<u8>> = Vec::new();
        let mut named: Vec<String> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        let picturing = crate::core::base::Busy::set(&self.remote.picturing);
        let mut turn: Option<crate::core::input_turn::Lease> = None;
        let body: Res<bool> = async {
            // A nudge or a macro's turn under way finishes first (a few seconds at most).
            turn = Some(rt::blocking(|| crate::core::input_turn::enter("picture", None)).await?);
            let who = link.user().unwrap_or_else(|| "Discord".to_string());
            self.notify(&l::f("{0} took a screenshot of this PC's Roblox windows from Discord.", &[&who]), "info", "remote", Guid::nil(), true, None);
            for account in accounts.iter().take(MAX_SHOTS) {
                // Locked, or turned off, since it was asked: no pictures go out.
                if !self.remote_open() {
                    log::note("Remote: /pc screenshot stopped: locked meanwhile, no pictures sent.");
                    return Ok(false);
                }
                let Some(pid) = self.controller.running.borrow().get(&account.id).map(|(pid, _)| *pid) else { continue };
                let windows = crate::core::roblox_processes::visible_windows(&[pid]);
                let window = crate::core::roblox_processes::main_window_of(&windows.get(&pid).cloned().unwrap_or_default());
                let width = if accounts.len() == 1 { 1920 } else { 1280 };
                let (shot, how) = rt::blocking(move || crate::core::window_shot::capture(window, width)).await;
                // Every picture's story in error.log: a white one can't be explained without it (a user's second
                // report of white pictures, 2026-11-14, had nothing to go by).
                log::note_file(&format!("Remote: /pc screenshot: account {} (Roblox {pid}): {how}.", &account.id.to_string()[..8]));
                let Some(shot) = shot else {
                    notes.push(l::f("{0}: no picture (its window isn't answering, or Roblox is still opening).", &[&self.discord_name(account)]));
                    continue;
                };
                // Nothing drawn: said in words, not sent as a white square.
                if shot.blank && !shot.black {
                    notes.push(l::f("{0}: no picture (Roblox drew nothing in its window; try again in a moment).", &[&self.discord_name(account)]));
                    continue;
                }
                let (jpeg, black) = (shot.jpeg, shot.black);
                pictures.push(jpeg);
                named.push(self.discord_name(account));
                if black {
                    notes.push(l::f("{0}: the picture came out black (Windows may be locked, or the game isn't drawing).", &[&self.discord_name(account)]));
                }
            }
            Ok(true)
        }
        .await;
        // finally
        if let Some(turn) = turn {
            turn.dispose();
        }
        drop(picturing);
        self.remote.last_shot.set(Stamp::utc_now());
        if !body? {
            return Ok(None);
        }
        if accounts.len() > MAX_SHOTS {
            notes.push(l::f("{0} more not pictured: Discord shows 10 at most.", &[&(accounts.len() - MAX_SHOTS)]));
        }
        log::note(&format!("Remote: /pc screenshot: {} of {} window(s) pictured, {} note(s).", pictures.len(), accounts.len(), notes.len()));
        for note in &notes {
            log::note_file(&format!("Remote: /pc screenshot said: {}", self.scrub(note)));
        }
        if pictures.is_empty() {
            return Ok(Some(self.remote_say(&notes.join("\n"))));
        }
        let content = std::iter::once(named.join(" · ")).chain(notes).collect::<Vec<_>>().join("\n");
        let command = c.clone();
        let sent = rt::blocking(move || link.shot(&command, &content, &pictures)).await;
        if !sent {
            log::note("Remote: /pc screenshot: the pictures couldn't be sent to Discord.");
        }
        Ok(if sent { None } else { Some(self.remote_say(&l::t("Couldn't send the picture to Discord. Try again in a moment."))) })
    }
}
