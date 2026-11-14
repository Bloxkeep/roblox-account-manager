//! Updating without a "Restart to update" click: once an update is downloaded and Bloxkeep is idle, the run writes what
//! the next one needs, Velopack installs and restarts, and the new run carries on unlocked, with the window as it was
//! and the Roblox windows followed again (AdoptOpenSessions). The vault key travels in a DPAPI file for this Windows
//! user, mixed with a random value only the new process's command line carries; the file is deleted as it is read and
//! refused once a couple of minutes old.
use serde::{Deserialize, Serialize};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom, CRYPT_INTEGER_BLOB, CryptProtectData, CryptUnprotectData},
};

use super::{
    base::{Fail, Res, Stamp},
    log,
};

const ARG: &str = "--resume";
/// Started again after an update to this version (with or without a hand-over): see `after_update`.
pub const AFTER_UPDATE_ARG: &str = "--after-update";
/// TimeSpan.FromMinutes(3), in seconds.
const LIFE_SECS: f64 = 3.0 * 60.0;

fn file_path() -> std::path::PathBuf {
    crate::core::vault::data_dir().join("resume.bin")
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct State {
    pub written_at: Stamp,
    pub from: Option<String>,
    /// The version being installed: a run that comes back as anything else knows the install didn't take.
    pub to: Option<String>,
    /// The vault's key, salt and iterations (base64), or null when it was locked.
    pub key: Option<String>,
    pub salt: Option<String>,
    pub iterations: i32,
    pub weak_password: bool,
    /// hidden (in the tray), minimized, or shown.
    pub window: String,
    /// The window was the one in front: it comes back in front, else behind what is.
    pub foreground: bool,
    /// What the page had on screen (see AppController.KeepView).
    pub view: Option<String>,
    /// The lock screen was up (a while without use): the next run comes back behind it, not unlocked.
    pub screen_locked: bool,
}

/// `key` left out: it opens the vault.
impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("written_at", &self.written_at)
            .field("from", &self.from)
            .field("to", &self.to)
            .field("key", &self.key.as_ref().map(|_| crate::core::base::Redacted))
            .field("salt", &self.salt)
            .field("iterations", &self.iterations)
            .field("weak_password", &self.weak_password)
            .field("window", &self.window)
            .field("foreground", &self.foreground)
            .field("view", &self.view)
            .field("screen_locked", &self.screen_locked)
            .finish()
    }
}

impl Default for State {
    fn default() -> State {
        State {
            // default(DateTime): 0001-01-01T00:00:00.
            written_at: Stamp::from_unix_secs(-62_135_596_800),
            from: None,
            to: None,
            key: None,
            salt: None,
            iterations: 0,
            weak_password: false,
            window: "shown".into(),
            foreground: false,
            view: None,
            screen_locked: false,
        }
    }
}

/// RandomNumberGenerator.GetBytes.
fn random_bytes(n: usize) -> Res<Vec<u8>> {
    let mut bytes = vec![0u8; n];
    // SAFETY: the system's preferred generator fills `bytes`, of the length given.
    let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), n as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status != 0 { Err(Fail::other(format!("BCryptGenRandom: 0x{status:08X}"))) } else { Ok(bytes) }
}

const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

/// ProtectedData.Protect / Unprotect with DataProtectionScope.CurrentUser (CRYPTPROTECT_UI_FORBIDDEN, as .NET passes).
fn dpapi(data: &[u8], entropy: &[u8], protect: bool) -> Res<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let extra = CRYPT_INTEGER_BLOB { cbData: entropy.len() as u32, pbData: entropy.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: the blobs point at live slices; the output is LocalAlloc'd by Windows and freed here after copying.
    unsafe {
        let ok = if protect {
            CryptProtectData(&input, std::ptr::null(), &extra, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), &extra, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        };
        if ok == 0 || output.pbData.is_null() {
            return Err(Fail::other(format!("CryptographicException: DPAPI error {}", windows_sys::Win32::Foundation::GetLastError())));
        }
        let out = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
        LocalFree(output.pbData as _);
        Ok(out)
    }
}

/// CryptographicOperations.ZeroMemory.
fn zero(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        // SAFETY: a write through a valid &mut, volatile so it isn't left out.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
}

/// Convert.ToHexString: upper case.
fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// Convert.FromHexString: None where it would throw FormatException.
fn from_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 || !text.is_ascii() {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok()).collect()
}

/// Writes `state` for the next run and returns the arguments it must be started with.
pub fn write(state: &State) -> Res<Vec<String>> {
    let entropy = random_bytes(32)?;
    let mut json = serde_json::to_vec(state)?;
    let result = (|| -> Res<()> {
        let dir = crate::core::vault::data_dir();
        std::fs::create_dir_all(&dir)?;
        std::fs::write(file_path(), dpapi(&json, &entropy, true)?)?;
        Ok(())
    })();
    zero(&mut json);
    result?;
    Ok(vec![ARG.to_string(), to_hex(&entropy)])
}

/// An update is installing now (a fresh hand-over not taken yet); a start by hand waits for that run instead
/// of taking its place (Program).
pub fn hand_over_waiting() -> bool {
    let path = file_path();
    let Ok(meta) = std::fs::metadata(&path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    match meta.modified() {
        Ok(at) => std::time::SystemTime::now().duration_since(at).map(|d| d.as_secs_f64() < LIFE_SECS).unwrap_or(true),
        Err(_) => false,
    }
}

/// This run is the one an update started (with its hand-over, or by the watch over it).
pub fn is_update_run(args: &[String]) -> bool {
    args.iter().any(|a| a == ARG || a == AFTER_UPDATE_ARG)
}

/// Removes what `write` wrote when the quit it was for didn't happen after all.
pub fn discard() {
    if let Err(ex) = std::fs::remove_file(file_path()) {
        // File.Delete of a file that isn't there is no error.
        if ex.kind() != std::io::ErrorKind::NotFound {
            log::write(&ex);
        }
    }
}

/// What the previous run left, when it started this one in time; None otherwise. The file goes either way, so a
/// key never stays on disk.
pub fn take(args: &[String]) -> Option<State> {
    read(args).or_else(|| after_update(args))
}

/// Started after an update without a usable hand-over: back in the tray, locked, saying so, rather than a lock
/// screen popping up over what the person is doing.
fn after_update(args: &[String]) -> Option<State> {
    let at = args.iter().position(|a| a == AFTER_UPDATE_ARG)?;
    Some(State { window: "hidden".into(), to: args.get(at + 1).cloned(), ..State::default() })
}

fn read(args: &[String]) -> Option<State> {
    let at = args.iter().position(|a| a == ARG);
    let entropy = match at {
        Some(i) if i + 1 < args.len() => from_hex(&args[i + 1]),
        _ => None,
    };
    let path = file_path();
    let mut sealed: Option<Vec<u8>> = None;
    if path.is_file() {
        let done = std::fs::read(&path).and_then(|bytes| {
            sealed = Some(bytes);
            std::fs::remove_file(&path)
        });
        if let Err(ex) = done {
            log::write(&ex);
        }
    }
    let (entropy, sealed) = (entropy?, sealed?);
    let mut json = match dpapi(&sealed, &entropy, false) {
        Ok(json) => json,
        Err(ex) => {
            log::write(&ex);
            return None;
        }
    };
    let result = match serde_json::from_slice::<State>(&json) {
        Ok(state) => {
            let now = Stamp::utc_now();
            if now.secs_since(state.written_at) > LIFE_SECS || state.written_at > now.plus_secs(60.0) { None } else { Some(state) }
        }
        Err(ex) => {
            log::write(&ex);
            None
        }
    };
    zero(&mut json);
    result
}
