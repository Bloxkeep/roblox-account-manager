// Bloxkeep as the program Windows opens Roblox's roblox-player: links with (the website's Play button, a friend's
// "Join"), a switch in Settings › Launching. Taken in the person's own registry (HKCU, no administrator), the command
// it replaced kept beside it ("BloxkeepBefore") and put back when the switch goes off or Bloxkeep is uninstalled.
// Roblox's installer takes the link back at every update: while the switch is on, Bloxkeep takes it again (`reclaim`),
// unless a Bloxstrap-style launcher took it, which is the person moving on. A link that comes in starts Roblox at once,
// the way the program before would have (`roblox_launcher::open_link`), and never shows Bloxkeep's window. The link
// holds a sign-in ticket: never logged.
use crate::core::base::{Fail, Res};
use crate::core::roblox_installs::{self as installs, file_name};
use crate::core::win_kit::{self, Hive, RegKey};
use crate::core::{l, log, roblox_launcher, ui};

/// The argument Bloxkeep is started with for a link; also how a command is known to be Bloxkeep's.
pub const ARG: &str = "--roblox-link";
/// Beside the command: the one Bloxkeep replaced.
const BEFORE: &str = "BloxkeepBefore";
/// The link's command in the person's own registry (Roblox's per-user installer writes the same key).
const COMMAND: &str = r"Software\Classes\roblox-player\shell\open\command";
const SCHEME_KEY: &str = r"Software\Classes\roblox-player";
/// The switch: one for the PC (the links are), in the registry beside them, never in a vault (each vault had its
/// own, and turning it off in one gave the links back while another's was on). The key holds nothing else: removed
/// with the switch off.
const WANT_KEY: &str = r"Software\Bloxkeep";
const WANT: &str = "OwnRobloxLinks";

/// What Settings shows: the switch on and Bloxkeep really holding the links (a launcher that took them since shows
/// it off). 0 off, 1 on, 2 not read yet.
static SHOWN: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);

fn wanted() -> bool {
    RegKey::open(Hive::CurrentUser, WANT_KEY, false).ok().flatten().is_some_and(|k| k.get_string(WANT).is_some_and(|v| v == "1"))
}

fn refresh() -> bool {
    let on = wanted() && held();
    SHOWN.store(on as u8, std::sync::atomic::Ordering::Relaxed);
    on
}

/// The switch as Settings shows it (read again at every scan, `watch`).
pub fn shown() -> bool {
    match SHOWN.load(std::sync::atomic::Ordering::Relaxed) {
        2 => refresh(),
        v => v == 1,
    }
}

/// A copy run on a data folder of its own (BLOXKEEP_DATA: a test build tried on an empty folder) beside the installed
/// Bloxkeep: the links are the machine's one setting, and the switch (read from the registry, not the data) says on
/// for the installed one; this copy would take them over at its first scan.
fn test_copy() -> bool {
    std::env::var("BLOXKEEP_DATA").is_ok_and(|d| !d.is_empty())
}

/// The switch turned on: Bloxkeep takes the links (from whatever has them, a launcher included: the person asked).
pub fn turn_on() -> Res<()> {
    if test_copy() {
        return Err(Fail::other("a test copy on its own data folder (BLOXKEEP_DATA) leaves the Roblox links to the installed Bloxkeep".to_string()));
    }
    let done = take().and_then(|()| RegKey::create(Hive::CurrentUser, WANT_KEY)?.set_string(WANT, "1"));
    refresh();
    done
}

/// The switch turned off, or Bloxkeep uninstalled: the links back to what had them.
pub fn turn_off() {
    if test_copy() {
        return;
    }
    release();
    if let Err(e) = win_kit::delete_tree(Hive::CurrentUser, WANT_KEY) {
        log::note(&format!("Roblox links: the switch couldn't be cleared ({e})."));
    }
    refresh();
}

/// At every scan: with the switch on, the links taken back from Roblox's installer (`reclaim`); the switch as shown
/// read again.
pub fn watch() {
    if wanted() && !test_copy() {
        reclaim();
    }
    refresh();
}

/// On the scheme key when Bloxkeep made it (Roblox installed for every user: its registration is the machine's, none
/// here). Then giving the links back is removing the key: a copy of the machine's command left here would hide
/// Roblox's next update of it.
const MADE: &str = "BloxkeepMade";

fn made_here() -> bool {
    RegKey::open(Hive::CurrentUser, SCHEME_KEY, false).ok().flatten().is_some_and(|k| k.get_string(MADE).is_some())
}

/// What reclaim says goes to error.log once until it changes (it runs at every scan).
fn say_once(said: Option<String>) {
    static SAID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    let mut last = SAID.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = said.as_deref()
        && last.as_deref() != Some(s)
    {
        log::note(s);
    }
    *last = said;
}

/// A command that starts Bloxkeep for a link.
pub fn is_ours(command: &str) -> bool {
    command.to_ascii_lowercase().contains(ARG)
}

/// The command Windows runs for a link now (the per-user one over the machine's).
fn command_now() -> Option<String> {
    installs::registry::classes_root(r"roblox-player\shell\open\command")?.string(None).filter(|c| !c.trim().is_empty())
}

/// Bloxkeep opens the links now.
pub fn held() -> bool {
    command_now().is_some_and(|c| is_ours(&c))
}

/// The command Bloxkeep replaced, when it holds the links (never one of its own).
pub fn before() -> Option<String> {
    // (The scheme key made by Bloxkeep: the machine's command as it is now, Roblox may have updated it since.)
    if made_here()
        && let Some(machine) = RegKey::open(Hive::LocalMachine, COMMAND, false).ok().flatten().and_then(|k| k.get_string("")).filter(|c| !c.trim().is_empty() && !is_ours(c))
    {
        return Some(machine);
    }
    let key = RegKey::open(Hive::CurrentUser, COMMAND, false).ok().flatten()?;
    key.get_string(BEFORE).filter(|c| !c.trim().is_empty() && !is_ours(c))
}

fn own_command() -> Res<String> {
    let exe = std::env::current_exe().map_err(|e| Fail::other(format!("Bloxkeep's own file: {e}")))?;
    Ok(format!("\"{}\" {ARG} \"%1\"", exe.display()))
}

/// Names only in error.log, never a path (it holds the Windows user's name).
fn named(command: &str) -> String {
    let exe = installs::exe_of_command(command).unwrap_or_default();
    let name = file_name(&exe);
    if name.is_empty() { "nothing".into() } else { name }
}

/// Makes Bloxkeep open the links, keeping the command it replaces.
fn take() -> Res<()> {
    let ours = own_command()?;
    let now = command_now();
    if now.as_deref() == Some(ours.as_str()) {
        return Ok(());
    }
    // (Roblox installed for every user leaves no key here: the scheme's own values come along, or Windows would read
    // this key alone and the link would have no "URL Protocol".)
    let fresh = RegKey::open(Hive::CurrentUser, SCHEME_KEY, false)?.is_none();
    if fresh {
        let scheme = RegKey::create(Hive::CurrentUser, SCHEME_KEY)?;
        scheme.set_string("", "URL: Roblox Protocol")?;
        scheme.set_string("URL Protocol", "")?;
        scheme.set_string(MADE, "1")?;
    } else if RegKey::open(Hive::CurrentUser, COMMAND, false).ok().flatten().and_then(|k| k.get_string("")).is_some_and(|c| !c.trim().is_empty() && !is_ours(&c))
        && let Ok(Some(scheme)) = RegKey::open(Hive::CurrentUser, SCHEME_KEY, true)
    {
        // A Roblox (or launcher) installed for this user wrote its own registration here: the key isn't Bloxkeep's.
        let _ = scheme.delete_value(MADE);
    }
    let build = installs::registered_build();
    let key = RegKey::create(Hive::CurrentUser, COMMAND)?;
    if let Some(now) = now.as_deref().filter(|c| !is_ours(c)) {
        key.set_string(BEFORE, now)?;
        if fresh && let Some(build) = build {
            key.set_string("version", &build)?;
        }
    }
    key.set_string("", &ours)?;
    log::note(&format!("Roblox links: Bloxkeep opens them now (before: {}).", now.as_deref().map(named).unwrap_or_else(|| "nothing".into())));
    Ok(())
}

/// Gives the links back to the command Bloxkeep replaced (nothing when Bloxkeep doesn't hold them).
fn release() {
    let Ok(Some(key)) = RegKey::open(Hive::CurrentUser, COMMAND, true) else { return };
    let now = key.get_string("").unwrap_or_default();
    if !is_ours(&now) {
        let _ = key.delete_value(BEFORE);
        if let Ok(Some(scheme)) = RegKey::open(Hive::CurrentUser, SCHEME_KEY, true) {
            let _ = scheme.delete_value(MADE);
        }
        return;
    }
    if made_here() {
        drop(key);
        match win_kit::delete_tree(Hive::CurrentUser, SCHEME_KEY) {
            Ok(()) => log::note("Roblox links: given back to Roblox (installed for every user)."),
            Err(e) => log::note(&format!("Roblox links: couldn't give them back ({e}).")),
        }
        return;
    }
    let before = key.get_string(BEFORE).filter(|c| !c.trim().is_empty() && !is_ours(c));
    let done = match &before {
        Some(b) => key.set_string("", b),
        None => key.delete_value(""),
    };
    match done {
        Ok(()) => {
            let _ = key.delete_value(BEFORE);
            log::note(&format!("Roblox links: given back to {}.", before.as_deref().map(named).unwrap_or_else(|| "nothing".into())));
        }
        Err(e) => log::note(&format!("Roblox links: couldn't give them back ({e}).")),
    }
}

/// While the switch is on: Roblox's installer took the links back at an update (Roblox's own client): Bloxkeep takes
/// them again. A Bloxstrap-style launcher taking them is the person's choice, and nothing holding them is Roblox gone
/// (a registration made then would point at nothing): both left alone, said once in error.log. Runs at every scan, so
/// a failure is said once too.
pub fn reclaim() {
    let Some(now) = command_now() else {
        say_once(Some("Roblox links: nothing opens them (Roblox not installed?); left as they are.".into()));
        return;
    };
    if is_ours(&now) {
        // Bloxkeep moved (another folder): the command follows it.
        if own_command().is_ok_and(|ours| ours != now) {
            match take() {
                Ok(()) => say_once(None),
                Err(e) => say_once(Some(format!("Roblox links: couldn't point them at this Bloxkeep ({e})."))),
            }
        }
        return;
    }
    let exe = installs::exe_of_command(&now).unwrap_or_default();
    let roblox = ["robloxplayerbeta.exe", "robloxplayerlauncher.exe", "robloxplayerinstaller.exe"].contains(&file_name(&exe).to_ascii_lowercase().as_str());
    if roblox {
        match take() {
            Ok(()) => {
                say_once(None);
                log::note("Roblox links: Roblox's installer had taken them back; Bloxkeep took them again.");
            }
            Err(e) => say_once(Some(format!("Roblox links: couldn't take them back from Roblox ({e})."))),
        }
    } else {
        say_once(Some(format!("Roblox links: {} opens them now; left to it.", named(&now))));
    }
}

/// A link's form as the website hands it over: "roblox-player:", printable ASCII, no spaces or quotes, at most 8 KB
/// (it goes on Roblox's command line as it is).
pub fn valid(link: &str) -> bool {
    link.len() <= 8192
        && link.get(..14).is_some_and(|p| p.eq_ignore_ascii_case("roblox-player:"))
        && link.bytes().all(|b| (0x21..0x7f).contains(&b) && b != b'"' && b != b'\'' && b != b'`')
}

/// Bloxkeep started for a link: Roblox opens for it, as the program before would have opened it; no window of
/// Bloxkeep's. Any failure is said to the person (they pressed Play and would see nothing) and goes to error.log.
pub fn open(link: Option<&str>) {
    let link = link.unwrap_or_default().trim();
    if !valid(link) {
        log::note(&format!("Roblox links: a link Bloxkeep doesn't take was handed over ({} characters); nothing opened.", link.len()));
        ui::error(None, &l::t("Bloxkeep couldn't read this Roblox link. Press Play again on the website."));
        return;
    }
    match roblox_launcher::open_link(link) {
        Ok(with) => log::note(&format!("Roblox links: a link from the website opened Roblox with {with}.")),
        Err(e) => {
            log::note(&format!("Roblox links: a link from the website couldn't open Roblox: {}", roblox_launcher::hide_ticket(&e.to_string())));
            ui::error(None, &e.to_string());
        }
    }
}
