//! Bloxkeep's own entry in Windows' startup apps (HKCU Run, named "Bloxkeep"): started with ARG, it opens in
//! the tray, locked, rather than a lock screen over what the person is starting. The registry is the setting itself, so
//! switching it off in Task Manager › Startup apps shows here as off too.
use super::{
    base::Res,
    log,
    win_kit::{Hive, RegKey},
};

pub const ARG: &str = "--startup";
const NAME: &str = "Bloxkeep";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// `"<Environment.ProcessPath>" --startup`.
fn command() -> String {
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    format!("\"{exe}\" {ARG}")
}

/// On when the entry is there and Task Manager hasn't switched it off.
pub fn is_on() -> bool {
    let read = || -> Res<bool> {
        let Some(run) = RegKey::open(Hive::CurrentUser, RUN_KEY, false)? else {
            return Ok(false);
        };
        if run.get_string(NAME).is_none() {
            return Ok(false);
        }
        let approved = RegKey::open(Hive::CurrentUser, APPROVED_KEY, false)?;
        // Task Manager marks a switched-off entry with an odd first byte; none means on.
        Ok(match approved.and_then(|k| k.get_binary(NAME)) {
            Some(flags) if !flags.is_empty() => flags[0] & 1 == 0,
            _ => true,
        })
    };
    match read() {
        Ok(on) => on,
        Err(ex) => {
            log::write(&ex);
            false
        }
    }
}

pub fn set(on: bool) -> Res<()> {
    {
        let run = RegKey::create(Hive::CurrentUser, RUN_KEY)?;
        if on {
            run.set_string(NAME, &command())?;
        } else {
            run.delete_value(NAME)?;
        }
    }
    // Switched back on here after Task Manager switched it off: Task Manager's mark goes too.
    if let Some(approved) = RegKey::open(Hive::CurrentUser, APPROVED_KEY, true)? {
        approved.delete_value(NAME)?;
    }
    Ok(())
}

/// At each start: an entry left pointing at a copy that isn't there any more (a moved portable copy) points
/// at this one. One pointing at another copy that is there stays as it is.
pub fn keep_path_current() {
    let fix = || -> Res<()> {
        let Some(run) = RegKey::open(Hive::CurrentUser, RUN_KEY, true)? else {
            return Ok(());
        };
        let Some(had) = run.get_string(NAME) else {
            return Ok(());
        };
        let cmd = command();
        if had == cmd {
            return Ok(());
        }
        let at = match had.strip_prefix('"').and_then(|rest| rest.find('"')) {
            // had.IndexOf('"', 1) is > 1: the closing quote after at least one character.
            Some(i) if i + 1 > 1 => had[1..i + 1].to_string(),
            _ => had.clone(),
        };
        if !std::path::Path::new(&at).is_file() {
            run.set_string(NAME, &cmd)?;
        }
        Ok(())
    };
    if let Err(ex) = fix() {
        log::write(&ex);
    }
}
