// RobloxInstalls.cs: finds the Roblox clients and launchers (Bloxstrap, Fishstrap, Voidstrap, …) installed for this
// Windows user.
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::sync::{LazyLock, Mutex, RwLock};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::core::log;
use crate::core::roblox_api::{eq_ci, lock};
use crate::core::roblox_launcher;

const PROTOCOL_COMMAND_KEY: &str = r"roblox-player\shell\open\command";

/// %LOCALAPPDATA%; a field the checks point at a folder of their own (`set_local`).
static LOCAL: RwLock<Option<String>> = RwLock::new(None);

fn local() -> String {
    if let Some(l) = LOCAL.read().unwrap_or_else(|e| e.into_inner()).clone() {
        return l;
    }
    std::env::var("LOCALAPPDATA").unwrap_or_default()
}

/// Tests: %LOCALAPPDATA% in place of the real one.
pub(crate) fn set_local(path: Option<String>) {
    *LOCAL.write().unwrap_or_else(|e| e.into_inner()) = path;
}

/// Environment.SpecialFolder.ProgramFilesX86.
pub(crate) fn program_files_x86() -> String {
    std::env::var("ProgramFiles(x86)").or_else(|_| std::env::var("ProgramFiles")).unwrap_or_default()
}

// ---- Path's ways ----

/// Path.Combine.
pub(crate) fn combine(a: &str, b: &str) -> String {
    Path::new(a).join(b).to_string_lossy().into_owned()
}

/// Path.GetDirectoryName: None for a root, "" for a bare name.
pub(crate) fn dir_name(p: &str) -> Option<String> {
    if p.is_empty() {
        return None;
    }
    Path::new(p).parent().map(|d| d.to_string_lossy().into_owned())
}

/// Path.GetFileName.
pub(crate) fn file_name(p: &str) -> String {
    Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

pub(crate) fn file_name_without_extension(p: &str) -> String {
    Path::new(p).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

pub(crate) fn file_exists(p: &str) -> bool {
    !p.is_empty() && Path::new(p).is_file()
}

pub(crate) fn dir_exists(p: &str) -> bool {
    !p.is_empty() && Path::new(p).is_dir()
}

/// File.GetLastWriteTimeUtc (None: no such file, the earliest).
fn last_write(p: &str) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// Path.GetInvalidFileNameChars on Windows.
pub(crate) fn has_invalid_file_name_chars(name: &str) -> bool {
    name.chars().any(|c| (c as u32) < 32 || matches!(c, '"' | '<' | '>' | '|' | ':' | '*' | '?' | '\\' | '/'))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---- the registry, read only ----

pub(crate) mod registry {
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, KEY_READ, REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
    };

    use super::wide;

    /// An open key, closed when dropped.
    pub struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the key was opened by RegOpenKeyExW and is closed once.
            unsafe { RegCloseKey(self.0) };
        }
    }

    fn open_under(root: HKEY, sub: &str) -> Option<Key> {
        let mut key: HKEY = std::ptr::null_mut();
        let name = wide(sub);
        // SAFETY: `name` is NUL-terminated and outlives the call; `key` receives the handle.
        let r = unsafe { RegOpenKeyExW(root, name.as_ptr(), 0, KEY_READ, &mut key) };
        (r == ERROR_SUCCESS).then_some(Key(key))
    }

    /// Registry.ClassesRoot.OpenSubKey.
    pub fn classes_root(sub: &str) -> Option<Key> {
        open_under(HKEY_CLASSES_ROOT, sub)
    }

    /// Registry.CurrentUser.OpenSubKey.
    pub fn current_user(sub: &str) -> Option<Key> {
        open_under(HKEY_CURRENT_USER, sub)
    }

    impl Key {
        pub fn open(&self, sub: &str) -> Option<Key> {
            open_under(self.0, sub)
        }

        /// GetValue(name) when it is a string (REG_SZ, or REG_EXPAND_SZ expanded as .NET does); None for another kind.
        pub fn string(&self, name: Option<&str>) -> Option<String> {
            let name = name.map(wide);
            let name_ptr = name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
            let (mut kind, mut size) = (0u32, 0u32);
            // SAFETY: asks only the kind and size; all pointers are valid or null.
            let r = unsafe { RegQueryValueExW(self.0, name_ptr, std::ptr::null(), &mut kind, std::ptr::null_mut(), &mut size) };
            if r != ERROR_SUCCESS || (kind != REG_SZ && kind != REG_EXPAND_SZ) {
                return None;
            }
            let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
            let mut bytes = (buf.len() * 2) as u32;
            // SAFETY: `buf` holds `bytes` bytes.
            let r = unsafe { RegQueryValueExW(self.0, name_ptr, std::ptr::null(), &mut kind, buf.as_mut_ptr().cast(), &mut bytes) };
            if r != ERROR_SUCCESS {
                return None;
            }
            let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            let text = String::from_utf16_lossy(&buf[..len]);
            Some(if kind == REG_EXPAND_SZ { expand(&text) } else { text })
        }

        /// GetSubKeyNames.
        pub fn subkey_names(&self) -> Vec<String> {
            let mut names = Vec::new();
            let mut index = 0;
            loop {
                let mut buf = vec![0u16; 256];
                let mut len = buf.len() as u32;
                // SAFETY: `buf` holds `len` characters; the other outputs are not asked for.
                let r = unsafe {
                    RegEnumKeyExW(self.0, index, buf.as_mut_ptr(), &mut len, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
                };
                if r == ERROR_MORE_DATA {
                    index += 1;
                    continue;
                }
                if r != ERROR_SUCCESS {
                    break;
                }
                names.push(String::from_utf16_lossy(&buf[..len as usize]));
                index += 1;
            }
            names
        }
    }

    fn expand(text: &str) -> String {
        let src = wide(text);
        // SAFETY: a null buffer of size 0 asks only the size needed.
        let need = unsafe { ExpandEnvironmentStringsW(src.as_ptr(), std::ptr::null_mut(), 0) };
        if need == 0 {
            return text.to_string();
        }
        let mut buf = vec![0u16; need as usize];
        // SAFETY: `buf` holds `need` characters.
        let got = unsafe { ExpandEnvironmentStringsW(src.as_ptr(), buf.as_mut_ptr(), need) };
        if got == 0 || got > need {
            return text.to_string();
        }
        String::from_utf16_lossy(&buf[..got as usize - 1])
    }
}

/// Executable registered for roblox-player: links (the official client or a bootstrapper).
pub fn protocol_handler_path() -> Option<String> {
    let key = registry::classes_root(PROTOCOL_COMMAND_KEY)?;
    let command = key.string(None)?;
    let command = command.trim();
    if let Some(rest) = command.strip_prefix('"') {
        let end = rest.find('"')?;
        return if end > 0 { Some(rest[..end].to_string()) } else { None };
    }
    // Unquoted: up to ".exe", so a folder with a space ("C:\Program Files\Fishstrap\Fishstrap.exe -player %1") isn't cut
    // at "C:\Program" (RoALT reads it the same way).
    if let Some(exe) = command.to_ascii_lowercase().find(".exe").filter(|i| *i > 0) {
        return Some(command[..exe + 4].to_string());
    }
    match command.find(' ') {
        Some(space) if space > 0 => Some(command[..space].to_string()),
        Some(_) => Some(command.to_string()),
        None => Some(command.to_string()),
    }
}

/// Roblox VNG's client (%LOCALAPPDATA%\RobloxPCVNG). Its installer takes the roblox-player: links (seen
/// 2026-10-19); Bloxkeep's Default never follows them there, not even for a VNG account.
pub fn is_vng_client(path: &str) -> bool {
    crate::core::roblox_api::contains_ci(path, r"\RobloxPCVNG\")
}

/// Roblox VNG's newest client: the "vng" option, and Default's and "direct"'s last resort when no other Roblox
/// is installed (never ahead of one; the owner's rule, 2026-10-28). It opens global accounts too (seen 2026-10-19).
pub fn find_vng_client() -> Option<String> {
    newest(clients_in(Some(&combine(&local(), "RobloxPCVNG"))))
}

/// The program roblox-player: links open, when it is still there and isn't Roblox VNG's client.
pub fn usable_handler_path() -> Option<String> {
    protocol_handler_path().filter(|h| file_exists(h) && !is_vng_client(h))
}

/// A bootstrapper by name, from its uninstall entry, its default folder, or the protocol handler.
pub fn find_bootstrapper(name: &str) -> Option<String> {
    let exe_name = format!("{name}.exe");
    if let Some(key) = registry::current_user(&format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{name}"))
        && let Some(dir) = key.string(Some("InstallLocation"))
        && file_exists(&combine(&dir, &exe_name))
    {
        return Some(combine(&dir, &exe_name));
    }
    let default_path = combine(&combine(&local(), name), &exe_name);
    if file_exists(&default_path) {
        return Some(default_path);
    }
    protocol_handler_path().filter(|h| file_exists(h) && eq_ci(&file_name(h), &exe_name))
}

/// Launchers with their own option in Settings; others of the same family are found by `find_other_bootstrappers`.
const KNOWN_BOOTSTRAPPERS: [&str; 3] = ["Bloxstrap", "Fishstrap", "Voidstrap"];

/// Where a launcher keeps its copies of Roblox, one "version-…" folder each: Bloxstrap and Fishstrap "Versions",
/// Voidstrap "RblxVersions" (its Paths.cs, KloBraticc/Voidstrap 1.1.3.7, 2026-11-07).
const VERSION_FOLDERS: [&str; 2] = ["Versions", "RblxVersions"];

/// Other Bloxstrap forks: an uninstall entry named after the launcher, whose folder holds "<name>.exe" and
/// a Versions folder of Roblox clients.
pub fn find_other_bootstrappers() -> Vec<String> {
    let mut found = Vec::new();
    // PORT: the C# caught IOException/SecurityException here and noted it once; the registry reads below fail quietly
    // (an entry that can't be read is left out).
    if let Some(uninstall) = registry::current_user(r"Software\Microsoft\Windows\CurrentVersion\Uninstall") {
        for name in uninstall.subkey_names() {
            if KNOWN_BOOTSTRAPPERS.iter().any(|k| eq_ci(k, &name)) || has_invalid_file_name_chars(&name) {
                continue;
            }
            if let Some(key) = uninstall.open(&name)
                && let Some(dir) = key.string(Some("InstallLocation"))
                && file_exists(&combine(&dir, &format!("{name}.exe")))
                && VERSION_FOLDERS.iter().any(|v| dir_exists(&combine(&dir, v)))
            {
                found.push(name);
            }
        }
    }
    found.sort_by_key(|n| n.to_uppercase());
    found
}

fn clients_in(root: Option<&str>) -> Vec<String> {
    let Some(root) = root else { return Vec::new() };
    let mut found = Vec::new();
    for folder in VERSION_FOLDERS {
        let versions = combine(root, folder);
        if !dir_exists(&versions) {
            continue;
        }
        match std::fs::read_dir(&versions) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(e) if e.path().is_dir() => {
                            let exe = combine(&e.path().to_string_lossy(), "RobloxPlayerBeta.exe");
                            if file_exists(&exe) && !is_broken(&exe) {
                                found.push(exe);
                            }
                        }
                        Ok(_) => {}
                        Err(ex) => note_once("listing a Versions folder", &ex),
                    }
                }
            }
            Err(ex) => note_once("listing a Versions folder", &ex),
        }
    }
    found
}

const CLIENT_DLL: &str = "RobloxPlayerBeta.dll";
static NEEDS_DLL: LazyLock<Mutex<HashMap<(String, Option<SystemTime>), bool>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// A client that can't start: the DLL it names isn't beside it (a user's report on a rival, 2026-10-30), and Windows
/// would only show its own error. Builds from before that DLL don't name it, so they stay usable. The exe (about 137 MB)
/// is read only when the DLL is gone.
pub fn is_broken(exe: &str) -> bool {
    let Some(dir) = dir_name(exe) else { return false };
    if file_exists(&combine(&dir, CLIENT_DLL)) || !file_exists(exe) {
        return false;
    }
    let key = (exe.to_string(), last_write(exe));
    if let Some(known) = lock(&NEEDS_DLL).get(&key) {
        return *known;
    }
    match file_mentions(exe, CLIENT_DLL) {
        Ok(broken) => {
            if broken {
                log::note(&format!("Roblox installs: build {} is missing {CLIENT_DLL}; it isn't used.", file_name(&dir)));
            }
            lock(&NEEDS_DLL).entry(key).or_insert(broken);
            broken
        }
        Err(ex) => {
            note_once("checking a Roblox build", &ex);
            false
        }
    }
}

fn file_mentions(file: &str, text: &str) -> std::io::Result<bool> {
    let wanted = text.as_bytes();
    let mut buffer = vec![0u8; 1 << 20];
    let mut stream = std::fs::File::open(file)?;
    let mut kept = 0usize;
    loop {
        let read = stream.read(&mut buffer[kept..])?;
        if read == 0 {
            return Ok(false);
        }
        let have = kept + read;
        if buffer[..have].windows(wanted.len()).any(|w| w == wanted) {
            return Ok(true);
        }
        // The name may lie across two reads.
        kept = (wanted.len() - 1).min(have);
        buffer.copy_within(have - kept..have, 0);
    }
}

/// The links open a client that can't start.
pub fn handler_broken() -> bool {
    usable_handler_path().is_some_and(|h| eq_ci(&file_name(&h), "RobloxPlayerBeta.exe") && is_broken(&h))
}

/// Roblox's own install has a build that can't start: said in place of "not installed" when no other is left.
pub fn own_build_broken() -> bool {
    for root in [combine(&local(), "Roblox"), combine(&program_files_x86(), "Roblox")] {
        let versions = combine(&root, "Versions");
        if !dir_exists(&versions) {
            continue;
        }
        match std::fs::read_dir(&versions) {
            Ok(entries) => {
                for e in entries.flatten() {
                    if e.path().is_dir() && is_broken(&combine(&e.path().to_string_lossy(), "RobloxPlayerBeta.exe")) {
                        return true;
                    }
                }
            }
            Err(ex) => note_once("looking for a broken build of Roblox's own install", &ex),
        }
    }
    false
}

/// Whether a client behind Roblox's newest build gets updated when it opens: Roblox's own install runs its updater
/// (the owner's logs, 2026-10-30), a launcher updates its copy first. Started by itself, a client in a launcher's folder
/// finds no updater ("Updater not found. Skipping update") and goes on into the game on the old build.
pub fn updates_on_open(exe: Option<&str>) -> bool {
    match exe {
        None => true,
        Some(exe) => is_roblox_own(exe) || updater_for("direct", "").is_some_and(|u| !is_own_installer(Some(&u))),
    }
}

/// "Directly" with a client behind the newest build, in a launcher's folder: this once through that launcher, which
/// updates it first. An old build isn't played on (accounts were reported banned for "a disallowed version of Roblox",
/// 2026-10-31, unconfirmed by Roblox). Anything else, or the newest build not known, is left as chosen.
pub fn updating_option(launch_via: &str, up_to_date: Option<bool>) -> String {
    // Roblox's own copy updates itself as it opens: nothing to go through.
    if launch_via != "direct" || up_to_date != Some(false) {
        return launch_via.to_string();
    }
    let Some(updater) = updater_for(launch_via, "") else { return launch_via.to_string() };
    if is_own_installer(Some(&updater)) {
        return launch_via.to_string();
    }
    let name = file_name_without_extension(&updater);
    if name.eq_ignore_ascii_case("Bloxstrap") {
        "bloxstrap".into()
    } else if name.eq_ignore_ascii_case("Fishstrap") {
        "fishstrap".into()
    } else if name.eq_ignore_ascii_case("Voidstrap") {
        "voidstrap".into()
    } else {
        format!("{}{name}", roblox_launcher::OTHER_PREFIX)
    }
}

/// .NET's Version: 2 to 4 parts, a missing one -1, compared part by part.
pub(crate) fn parse_version(text: &str) -> Option<[i64; 4]> {
    let parts: Vec<&str> = text.split('.').collect();
    if !(2..=4).contains(&parts.len()) {
        return None;
    }
    let mut v = [-1i64; 4];
    for (i, p) in parts.iter().enumerate() {
        let n: i32 = p.trim().parse().ok()?;
        if n < 0 {
            return None;
        }
        v[i] = n as i64;
    }
    Some(v)
}

static VERSION_PREFIX: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"^\d+(\.\d+){1,3}").unwrap());

/// The client to use of several: the build Roblox's installer registered last (`registered_build`), then the
/// highest version, then the newest file. By file time alone, 2026-10-30 picked a test channel's 0.744.478 (its file
/// from 10-29) over 0.745 (10-28), the build Roblox wanted: each window it opened started Roblox's updater, which
/// closed it and opened another. The registered build counts only among the builds of Roblox's own install, whose
/// installer wrote it: between installs (Roblox's and Bloxstrap's, say, for "directly"), each one's pick goes by version,
/// so Roblox's stale registered build never wins over a launcher's newer one (review, 2026-10-30).
pub(crate) fn newest(paths: Vec<String>) -> Option<String> {
    let registered = registered_build();
    let mut version: HashMap<String, [i64; 4]> = HashMap::new();
    let mut version_number = |p: &str| -> [i64; 4] {
        let key = p.to_lowercase();
        if let Some(known) = version.get(&key) {
            return *known;
        }
        // The version alone: version_of's source_of lists the Uninstall keys and every launcher, for each client.
        let text = version_text(p).unwrap_or_default();
        let v = VERSION_PREFIX.find(&text).and_then(|m| parse_version(m.as_str())).unwrap_or([0, 0, -1, -1]);
        version.insert(key, v);
        v
    };
    let is_registered =
        |p: &str| registered.as_deref().is_some_and(|r| is_roblox_own(p) && dir_name(p).is_some_and(|d| eq_ci(&file_name(&d), r)));
    // …\<install>\Versions\version-…\RobloxPlayerBeta.exe: one pick per Versions folder. Roblox's live build first (LIVE_BUILD).
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for p in paths {
        let install = dir_name(&dir_name(&p).unwrap_or_default()).unwrap_or_default();
        match groups.iter_mut().find(|(k, _)| eq_ci(k, &install)) {
            Some((_, members)) => members.push(p),
            None => groups.push((install, vec![p])),
        }
    }
    let mut picks: Vec<(String, bool, [i64; 4], Option<SystemTime>)> = Vec::new();
    for (_, members) in groups {
        let mut best: Option<(bool, bool, [i64; 4], Option<SystemTime>, String)> = None;
        for p in members {
            let key = (is_live(&p), is_registered(&p), version_number(&p), last_write(&p));
            if best.as_ref().is_none_or(|b| (key.0, key.1, key.2, key.3) > (b.0, b.1, b.2, b.3)) {
                best = Some((key.0, key.1, key.2, key.3, p));
            }
        }
        if let Some((live, _, v, t, p)) = best {
            picks.push((p, live, v, t));
        }
    }
    let mut best: Option<(String, bool, [i64; 4], Option<SystemTime>)> = None;
    for pick in picks {
        if best.as_ref().is_none_or(|b| (pick.1, pick.2, pick.3) > (b.1, b.2, b.3)) {
            best = Some(pick);
        }
    }
    best.map(|b| b.0)
}

/// The build folder ("version-…") Roblox serves everyone now (clientsettingscdn's WindowsPlayer, no channel); None until
/// Roblox answered. Roblox puts some accounts on a test channel and installs that channel's build next to the live one,
/// the roblox-player links then pointing at it. Bloxkeep's links ask for no channel, so a client on the test build finds
/// itself off the live one and runs Roblox's installer, which closes every Roblox window open: on a user's PC (2026-11-13,
/// 0.747.649 installed beside the live 0.747.0) each window opened with others running ended them all within seconds.
/// RoALT met it too and starts the live build's file; Bloxkeep does the same when that build is installed.
static LIVE_BUILD: RwLock<Option<String>> = RwLock::new(None);

pub fn set_live_build(folder: Option<String>) {
    *LIVE_BUILD.write().unwrap_or_else(|e| e.into_inner()) = folder;
}

pub fn live_build() -> Option<String> {
    LIVE_BUILD.read().unwrap_or_else(|e| e.into_inner()).clone().filter(|b| !b.is_empty())
}

/// The client is in Roblox's live build's folder (LIVE_BUILD).
fn is_live(exe: &str) -> bool {
    live_build().is_some_and(|live| dir_name(exe).is_some_and(|d| eq_ci(&file_name(&d), &live)))
}

/// The live build's client in the same Versions folder as this one when it is there and this is another build;
/// otherwise the client as given (LIVE_BUILD).
pub fn live_beside(exe: Option<String>) -> Option<String> {
    let exe = exe?;
    let Some(live) = live_build() else { return Some(exe) };
    if is_live(&exe) || !has_build(Some(&exe), Some(&live)) {
        return Some(exe);
    }
    let versions = dir_name(&dir_name(&exe).unwrap_or_default()).unwrap_or_default();
    Some(combine(&combine(&versions, &live), "RobloxPlayerBeta.exe"))
}

/// In Roblox's own install (its installer's, per user or for all), not a launcher's copy.
pub fn is_roblox_own(exe: &str) -> bool {
    under(exe, &combine(&local(), "Roblox")) || under(exe, &combine(&program_files_x86(), "Roblox"))
}

/// Tests: the registered build in place of the registry's.
pub(crate) static REGISTERED_BUILD_OVERRIDE: RwLock<Option<Box<dyn Fn() -> Option<String> + Send + Sync>>> = RwLock::new(None);

/// The build folder ("version-…") Roblox's installer set the roblox-player link up for, written beside its command.
pub(crate) fn registered_build() -> Option<String> {
    if let Some(stand) = REGISTERED_BUILD_OVERRIDE.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return stand();
    }
    registry::classes_root(PROTOCOL_COMMAND_KEY)?.string(Some("version")).filter(|b| !b.is_empty())
}

/// Whether the build folder ("version-…") is in this client's Versions folder. Roblox keeps a build per channel
/// side by side and starts the account's channel's: on 2026-10-29 the links pointed at a test channel's 0.744.478
/// while a main-channel account started 0.745 from the folder next to it.
pub fn has_build(exe: Option<&str>, folder: Option<&str>) -> bool {
    let (Some(exe), Some(folder)) = (exe, folder) else { return false };
    if !crate::core::roblox_api::starts_with_ci(folder, "version-") {
        return false;
    }
    let Some(versions) = dir_name(exe).and_then(|d| dir_name(&d)) else { return false };
    let client = combine(&combine(&versions, folder), "RobloxPlayerBeta.exe");
    file_exists(&client) && !is_broken(&client)
}

/// Newest client installed by Roblox's own installer.
pub fn find_official_client() -> Option<String> {
    let mut all = clients_in(Some(&combine(&local(), "Roblox")));
    all.extend(clients_in(Some(&combine(&program_files_x86(), "Roblox"))));
    newest(all)
}

/// Newest client from any install, bootstrappers included.
pub fn find_any_client() -> Option<String> {
    let mut clients = clients_in(Some(&combine(&local(), "Roblox")));
    let names: Vec<String> = KNOWN_BOOTSTRAPPERS.iter().map(|s| s.to_string()).chain(find_other_bootstrappers()).collect();
    for name in names {
        if let Some(exe) = find_bootstrapper(&name) {
            clients.extend(clients_in(dir_name(&exe).as_deref()));
        }
    }
    if let Some(handler) = usable_handler_path() {
        if !eq_ci(&file_name(&handler), "RobloxPlayerBeta.exe") {
            clients.extend(clients_in(dir_name(&handler).as_deref()));
        } else if !is_broken(&handler) {
            clients.push(handler);
        }
    }
    let mut seen = HashSet::new();
    clients.retain(|c| seen.insert(c.to_lowercase()));
    newest(clients)
}

/// `version`: e.g. "0.742.0.7421053". `folder`: the version folder, e.g. "version-cec3ad5889b447cf" (Roblox's id for
/// that build). `source`: whose copy it is ("Roblox", "Bloxstrap", …); only for a client found on this PC. `exe`: the
/// client file itself; never sent to the page.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ClientVersion {
    pub version: String,
    pub folder: String,
    #[serde(skip)]
    pub source: Option<String>,
    #[serde(skip)]
    pub exe: Option<String>,
}

fn under(path: &str, root: &str) -> bool {
    crate::core::roblox_api::starts_with_ci(path, &format!("{}\\", root.trim_end_matches(['\\', '/'])))
}

/// Which install a client belongs to: Roblox's own, a known launcher's, or the folder its Versions folder is in.
pub fn source_of(exe: &str) -> String {
    if is_store_client(exe) {
        return "Microsoft Store".into();
    }
    if under(exe, &combine(&local(), "Roblox")) || under(exe, &combine(&program_files_x86(), "Roblox")) {
        return "Roblox".into();
    }
    let names: Vec<String> = KNOWN_BOOTSTRAPPERS.iter().map(|s| s.to_string()).chain(find_other_bootstrappers()).collect();
    for name in names {
        if let Some(launcher) = find_bootstrapper(&name)
            && let Some(root) = dir_name(&launcher).filter(|r| !r.is_empty())
            && under(exe, &root)
        {
            return name;
        }
    }
    // …\<install>\Versions\version-…\RobloxPlayerBeta.exe
    let install = dir_name(exe).and_then(|d| dir_name(&d)).and_then(|d| dir_name(&d)).unwrap_or_default();
    file_name(&install)
}

/// The client a launch option would start: bootstrappers keep their own copy of Roblox.
pub fn client_for(launch_via: &str, custom_launcher: &str) -> Option<String> {
    let bootstrapped = |name: &str| find_bootstrapper(name).and_then(|exe| newest(clients_in(dir_name(&exe).as_deref())));
    match launch_via {
        "bloxstrap" => bootstrapped("Bloxstrap"),
        "fishstrap" => bootstrapped("Fishstrap"),
        "voidstrap" => bootstrapped("Voidstrap"),
        _ if roblox_launcher::other_name(launch_via).is_some() => bootstrapped(&roblox_launcher::other_name(launch_via).unwrap_or_default()),
        // A launcher picked by hand: its own copy of Roblox if it keeps one next to it, else the newest anywhere.
        "custom" => (if roblox_launcher::custom_launcher_ready(custom_launcher) { newest(clients_in(dir_name(custom_launcher).as_deref())) } else { None })
            .or_else(find_any_client),
        "roblox" => find_official_client(),
        "direct" => find_any_client().or_else(find_vng_client), // VNG's only when nothing else is there
        "store" => find_store_client(),
        "vng" => find_vng_client(),
        // Whatever opens roblox-player: links (not VNG's client: see is_vng_client).
        _ => match usable_handler_path() {
            Some(handler) => {
                if eq_ci(&file_name(&handler), "RobloxPlayerBeta.exe") {
                    // What roblox_launcher starts in the links' place (LIVE_BUILD).
                    live_beside(Some(handler))
                } else {
                    newest(clients_in(dir_name(&handler).as_deref())).or_else(find_any_client)
                }
            }
            None => find_any_client().or_else(find_vng_client),
        },
    }
}

/// What updates the client this option starts: the Bloxstrap-style launcher whose copy it is, which with "-player
/// -nolaunch" installs the newest Roblox without opening it (Bloxstrap 2.11.4 and Fishstrap source, 2026-10-17), or for
/// Roblox's own copy its installer beside the client (`OWN_INSTALLER`), which installs the newest build and
/// then opens the Roblox app (the owner's PC, 2026-11-06: no switch it keeps stops that, so the app's window is closed
/// after). None for the Store edition, Roblox VNG and a hand-picked launcher.
pub fn updater_for(launch_via: &str, custom_launcher: &str) -> Option<String> {
    if matches!(launch_via, "custom" | "store" | "vng") {
        return None;
    }
    let client = client_for(launch_via, custom_launcher)?;
    let names: Vec<String> = KNOWN_BOOTSTRAPPERS.iter().map(|s| s.to_string()).chain(find_other_bootstrappers()).collect();
    for name in names {
        if let Some(exe) = find_bootstrapper(&name)
            && let Some(dir) = dir_name(&exe)
            && under(&client, &dir)
        {
            return Some(exe);
        }
    }
    let own = combine(&dir_name(&client).unwrap_or_default(), OWN_INSTALLER);
    if is_roblox_own(&client) && !is_vng_client(&client) && file_exists(&own) { Some(own) } else { None }
}

/// Whether a Bloxstrap-style launcher bringing this client up to the live build leaves the Roblox windows open. Its usual
/// update ends every RobloxPlayerBeta first (Bloxstrap's Bootstrapper.cs, read 2026-11-14: UpgradeRoblox calls
/// KillRobloxInstances); only its background update doesn't, and only when turned on (BackgroundUpdatesEnabled in its
/// Settings.json), at most one version behind and with 5 GB free. Unknown counts as closing them.
pub fn updates_in_background(launcher: &str, client: &ClientVersion, live: &ClientVersion) -> bool {
    let dir = dir_name(launcher).unwrap_or_default();
    let Ok(text) = std::fs::read_to_string(combine(&dir, "Settings.json")) else { return false };
    let Ok(serde_json::Value::Object(root)) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')) else { return false };
    if root.get("BackgroundUpdatesEnabled") != Some(&serde_json::Value::Bool(true)) {
        return false;
    }
    let minor = |v: &str| v.split('.').nth(1).and_then(|m| m.parse::<i64>().ok());
    let (Some(have), Some(newest)) = (parse_version(&client.version).and(minor(&client.version)), parse_version(&live.version).and(minor(&live.version))) else {
        return false;
    };
    if !matches!(newest - have, 0 | 1) {
        return false;
    }
    free_bytes(&dir).is_some_and(|free| free >= 5_000_000_000)
}

/// Free space for this user on the drive holding `dir`.
fn free_bytes(dir: &str) -> Option<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let d = crate::core::win_kit::wide(dir);
    let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: plain call with three out values on the stack.
    let ok = unsafe { GetDiskFreeSpaceExW(d.as_ptr(), &mut available, &mut total, &mut free) };
    if ok == 0 { None } else { Some(available) }
}

/// Where `kept_copy` keeps the build the windows open run on (Settings › Roblox version › When: "keep").
pub fn kept_root() -> String {
    combine(&combine(&local(), "Bloxkeep"), "Roblox kept")
}

const CLIENT_SETTINGS: &str = "ClientSettings";

/// A copy of Roblox's own build `exe` is in, without its updater: started from there, Roblox opens on that build instead of
/// updating, which would close every Roblox window open. Roblox looks for the updater beside itself only (the owner's logs,
/// 2026-11-13: "Checking if updater exists at …\version-…\RobloxPlayerInstaller.exe"), and without it says "Updater not
/// found. Skipping update" and plays on, as a launcher's copy does. Files are hard links where they can be (no space
/// taken, even after Roblox deletes its old build), its Fast Flags (ClientSettings) copied fresh each time. A launcher's
/// copy (no updater beside it) is started as it is.
pub fn kept_copy(exe: &str) -> std::io::Result<String> {
    use std::path::{Path, PathBuf};
    if !is_roblox_own(exe) {
        return Ok(exe.to_string());
    }
    let from = PathBuf::from(dir_name(exe).unwrap_or_default());
    let to = Path::new(&kept_root()).join(from.file_name().unwrap_or_default());
    if !to.join(".complete").exists() {
        let part = PathBuf::from(format!("{}.part", to.display()));
        for old in [&to, &part] {
            if old.is_dir() {
                std::fs::remove_dir_all(old)?;
            }
        }
        copy_tree(&from, &from, &part)?;
        std::fs::write(part.join(".complete"), "")?;
        std::fs::rename(&part, &to)?;
    }
    let flags = to.join(CLIENT_SETTINGS);
    if flags.is_dir() {
        std::fs::remove_dir_all(&flags)?;
    }
    let source = from.join(CLIENT_SETTINGS);
    if source.is_dir() {
        std::fs::create_dir_all(&flags)?;
        for e in std::fs::read_dir(&source)? {
            let e = e?;
            if e.file_type()?.is_file() {
                std::fs::copy(e.path(), flags.join(e.file_name()))?;
            }
        }
    }
    Ok(to.join(Path::new(exe).file_name().unwrap_or_default()).to_string_lossy().into_owned())
}

/// Every file under `dir` into `part` at its place under `root`, a hard link where it can be, without the updater and
/// the Fast Flags folder at the top.
fn copy_tree(root: &std::path::Path, dir: &std::path::Path, part: &std::path::Path) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let path = e.path();
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let rel_text = rel.to_string_lossy();
        if e.file_type()?.is_dir() {
            if !rel_text.eq_ignore_ascii_case(CLIENT_SETTINGS) {
                copy_tree(root, &path, part)?;
            }
            continue;
        }
        if rel_text.eq_ignore_ascii_case(OWN_INSTALLER) {
            continue;
        }
        let target = part.join(&rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if std::fs::hard_link(&path, &target).is_err() {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

/// Removes the kept builds; only called with no Roblox open (a running one's files would go from under it).
pub fn forget_kept_copies() -> std::io::Result<()> {
    let root = kept_root();
    if std::path::Path::new(&root).is_dir() {
        std::fs::remove_dir_all(&root)?;
    }
    Ok(())
}

/// Roblox's installer, in each build folder of its own install: run with no switches it installs the newest build
/// for this PC (as Roblox serves it) in seconds, then opens the Roblox app (its window: "-isInstallerLaunch").
pub const OWN_INSTALLER: &str = "RobloxPlayerInstaller.exe";

pub fn is_own_installer(exe: Option<&str>) -> bool {
    exe.is_some_and(|e| eq_ci(&file_name(e), OWN_INSTALLER))
}

/// A client's build is behind Roblox's newest. Only builds in "version-<hash>" folders can tell (the Store's has
/// none); a newer number than Roblox serves is another channel's.
pub fn is_behind(client: Option<&ClientVersion>, latest: Option<&ClientVersion>) -> bool {
    let (Some(client), Some(latest)) = (client, latest) else { return false };
    use crate::core::roblox_api::starts_with_ci;
    client.folder != latest.folder
        && starts_with_ci(&client.folder, "version-")
        && starts_with_ci(&latest.folder, "version-")
        && !matches!((parse_version(&client.version), parse_version(&latest.version)), (Some(have), Some(newest)) if have > newest)
}

/// Reads the version stamped in a client's file (the file is only read, never run).
pub fn version_of(exe: Option<&str>) -> Option<ClientVersion> {
    let exe = exe?;
    let version = version_text(exe)?;
    let folder = if is_store_client(exe) { String::new() } else { file_name(&dir_name(exe).unwrap_or_default()) };
    Some(ClientVersion { version, folder, source: Some(source_of(exe)), exe: Some(exe.to_string()) })
}

/// `version_of`'s version without the rest (no registry or launcher lookups): None when the file isn't there, or is a
/// Store client whose package folder doesn't say.
fn version_text(exe: &str) -> Option<String> {
    if !file_exists(exe) {
        return None;
    }
    // The Store's exe reads 0.0.0.0: its package folder's name has the version (and no Roblox build id).
    if is_store_client(exe) {
        return store_version(&file_name(&dir_name(exe).unwrap_or_default()));
    }
    let (text, parts) = file_version(exe);
    // The text form ("0, 742, 0, 7421053"): the numeric parts are 16-bit, and Roblox's build number is larger.
    let version = text.unwrap_or_default().split([',', '.']).map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(".");
    Some(if version.is_empty() { format!("{}.{}.{}.{}", parts[0], parts[1], parts[2], parts[3]) } else { version })
}

/// FileVersionInfo: the FileVersion text (first translation, then the usual English code pages) and the fixed
/// numbers (major, minor, build, private). A file without a version resource gives none and zeros.
fn file_version(exe: &str) -> (Option<String>, [u16; 4]) {
    use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW};
    let path = wide(exe);
    let mut handle = 0u32;
    // SAFETY: `path` is NUL-terminated.
    let size = unsafe { GetFileVersionInfoSizeW(path.as_ptr(), &mut handle) };
    if size == 0 {
        return (None, [0; 4]);
    }
    let mut data = vec![0u8; size as usize];
    // SAFETY: `data` holds `size` bytes.
    if unsafe { GetFileVersionInfoW(path.as_ptr(), 0, size, data.as_mut_ptr().cast()) } == 0 {
        return (None, [0; 4]);
    }
    let query = |sub: &str| -> Option<(*const u8, u32)> {
        let sub = wide(sub);
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        // SAFETY: `data` is the block GetFileVersionInfoW filled; `ptr` points into it on success.
        let ok = unsafe { VerQueryValueW(data.as_ptr().cast(), sub.as_ptr(), &mut ptr, &mut len) };
        (ok != 0 && !ptr.is_null()).then_some((ptr as *const u8, len))
    };
    let mut parts = [0u16; 4];
    if let Some((ptr, len)) = query("\\")
        && len as usize >= std::mem::size_of::<VS_FIXEDFILEINFO>()
    {
        // SAFETY: VerQueryValueW returned a VS_FIXEDFILEINFO of at least that size inside `data`.
        let fixed = unsafe { std::ptr::read_unaligned(ptr as *const VS_FIXEDFILEINFO) };
        parts = [(fixed.dwFileVersionMS >> 16) as u16, fixed.dwFileVersionMS as u16, (fixed.dwFileVersionLS >> 16) as u16, fixed.dwFileVersionLS as u16];
    }
    let string_at = |codepage: &str| -> Option<String> {
        let (ptr, len) = query(&format!("\\StringFileInfo\\{codepage}\\FileVersion"))?;
        if len == 0 {
            return None;
        }
        // SAFETY: the value is `len` UTF-16 characters (its NUL included) inside `data`.
        let chars = unsafe { std::slice::from_raw_parts(ptr as *const u16, len as usize) };
        let end = chars.iter().position(|c| *c == 0).unwrap_or(chars.len());
        let text = String::from_utf16_lossy(&chars[..end]);
        (!text.is_empty()).then_some(text)
    };
    let mut codepages = Vec::new();
    if let Some((ptr, len)) = query("\\VarFileInfo\\Translation")
        && len >= 4
    {
        // SAFETY: the translation table holds at least one pair of u16 inside `data`.
        let (lang, cp) = unsafe { (std::ptr::read_unaligned(ptr as *const u16), std::ptr::read_unaligned((ptr as *const u16).add(1))) };
        codepages.push(format!("{lang:04x}{cp:04x}"));
    }
    codepages.extend(["040904B0".to_string(), "040904E4".to_string(), "04090000".to_string()]);
    (codepages.iter().find_map(|cp| string_at(cp)), parts)
}

/// Roblox from the Microsoft Store (GDK build since September 2025, replacing ROBLOXCORPORATION.ROBLOX). Its
/// Start entry runs GameLaunchHelper.exe, which only brings an open Roblox forward, so Bloxkeep starts the package's
/// RobloxPlayerBeta.exe (RobloxLauncher, "store").
pub const STORE_FAMILY: &str = "ROBLOXCorporation.RobloxGDK_55nm5eh3cm0pr";
pub const STORE_APP_ID: &str = "Game";
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

/// The Microsoft Store edition's RobloxPlayerBeta.exe, or None when it isn't installed for this user.
pub fn find_store_client() -> Option<String> {
    use windows_sys::Win32::Storage::Packaging::Appx::{GetPackagePathByFullName, GetPackagesByPackageFamily};
    let family = wide(STORE_FAMILY);
    let (mut count, mut length) = (0u32, 0u32);
    // SAFETY: a first call with no buffers asks only the sizes.
    let r = unsafe { GetPackagesByPackageFamily(family.as_ptr(), &mut count, std::ptr::null_mut(), &mut length, std::ptr::null_mut()) };
    if r != ERROR_INSUFFICIENT_BUFFER || count == 0 {
        return None;
    }
    let mut names: Vec<windows_sys::core::PWSTR> = vec![std::ptr::null_mut(); count as usize];
    let mut buffer = vec![0u16; length as usize];
    // SAFETY: `names` holds `count` pointers and `buffer` `length` characters, as the first call asked.
    let r = unsafe { GetPackagesByPackageFamily(family.as_ptr(), &mut count, names.as_mut_ptr(), &mut length, buffer.as_mut_ptr()) };
    if r != 0 || count == 0 || names[0].is_null() {
        return None;
    }
    // SAFETY: names[0] points at a NUL-terminated string inside `buffer`.
    let full_name: Vec<u16> = unsafe {
        let start = names[0];
        let mut len = 0;
        while *start.add(len) != 0 {
            len += 1;
        }
        std::slice::from_raw_parts(start, len + 1).to_vec()
    };
    if full_name.len() <= 1 {
        return None;
    }
    let mut size = 0u32;
    // SAFETY: a null path asks only the size.
    if unsafe { GetPackagePathByFullName(full_name.as_ptr(), &mut size, std::ptr::null_mut()) } != ERROR_INSUFFICIENT_BUFFER || size == 0 {
        return None;
    }
    let mut path = vec![0u16; size as usize];
    // SAFETY: `path` holds `size` characters.
    if unsafe { GetPackagePathByFullName(full_name.as_ptr(), &mut size, path.as_mut_ptr()) } != 0 {
        return None;
    }
    let exe = combine(&String::from_utf16_lossy(&path[..(size as usize).saturating_sub(1)]), "RobloxPlayerBeta.exe");
    file_exists(&exe).then_some(exe)
}

pub fn store_edition_installed() -> bool {
    find_store_client().is_some()
}

/// What failed to be read, by kind, already in error.log: these reads repeat, so each is logged once a run.
static NOTED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// One line per kind of failure (no path: it may hold the Windows user's name).
fn note_once(what: &str, ex: &std::io::Error) {
    let kind = format!("{:?}", ex.kind());
    if lock(&NOTED).insert(format!("{what}|{kind}")) {
        let hresult = ex.raw_os_error().map(|c| 0x8007_0000u32 | (c as u32 & 0xFFFF)).unwrap_or(0x8013_1620);
        log::note(&format!("Roblox installs: {what} failed ({kind} 0x{hresult:08X}); left out."));
    }
}

/// A client in the Store edition's package folder (…\WindowsApps\ROBLOXCorporation.RobloxGDK_…).
pub fn is_store_client(exe: &str) -> bool {
    let family = STORE_FAMILY.split('_').next().unwrap_or("");
    crate::core::roblox_api::starts_with_ci(&file_name(&dir_name(exe).unwrap_or_default()), &format!("{family}_"))
}

/// The Store client's version in Roblox's form, from its package folder: "…RobloxGDK_2.743.1134.0_x64__…" gives
/// 0.743.0.7431134, comparable with what Roblox serves. None when the name isn't of that form.
pub fn store_version(package_folder: &str) -> Option<String> {
    let name: Vec<&str> = package_folder.split('_').collect();
    let parts: Vec<&str> = if name.len() > 1 { name[1].split('.').collect() } else { Vec::new() };
    if parts.len() != 4 {
        return None;
    }
    let release: i32 = parts[1].trim().parse().ok()?;
    let build: i32 = parts[2].trim().parse().ok()?;
    Some(format!("0.{release}.0.{release}{build:04}"))
}
