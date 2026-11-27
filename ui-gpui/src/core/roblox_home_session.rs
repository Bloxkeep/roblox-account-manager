// RobloxHomeSession.cs: ticket-free Roblox home screens, each in its own junction.
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::base::{Fail, Res};
use crate::core::roblox_api::{eq_ci, fail_of, starts_with_ci};
use crate::core::roblox_processes::{self, PlayerProcess};

/// Ticket-free Roblox home screens. A directory junction gives each start its own path-based .shm/.mtx names,
/// without copying or modifying the client. Exclusive handles keep both shared sign-in files unreadable and
/// unwritable: each new client signs in in memory instead of adopting or overwriting another client's session.
/// Keep this alive while clients using these aliases run, including while Bloxkeep is locked or Multi-Roblox is switched off.
/// Verified with Roblox 0.743.0.7431134: signing in and out in the new window leaves the old window signed in.
pub struct RobloxHomeSession {
    files: Vec<File>,
    aliases: String,
    disposed: bool,
    client_paths: HashMap<(i32, i64), bool>,
}

const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

fn io_fail(ex: std::io::Error) -> Fail {
    let kind = match ex.kind() {
        ErrorKind::NotFound => "FileNotFoundException",
        ErrorKind::PermissionDenied => "UnauthorizedAccessException",
        _ => "IOException",
    };
    fail_of(kind, ex)
}

/// Path.GetFullPath.
fn full_path(p: &str) -> Res<String> {
    std::path::absolute(p).map(|a| a.to_string_lossy().into_owned()).map_err(|e| fail_of("ArgumentException", e))
}

/// Path.TrimEndingDirectorySeparator: a root keeps its separator.
fn trim_ending_separator(p: &str) -> String {
    let trimmed = p.trim_end_matches(['\\', '/']);
    if trimmed.len() == p.len() || trimmed.is_empty() || trimmed.ends_with(':') {
        return p.to_string();
    }
    trimmed.to_string()
}

/// "client-" and a Guid written without dashes ("N").
fn is_alias_name(name: &str) -> bool {
    name.len() == 39 && starts_with_ci(name, "client-") && name[7..].chars().all(|c| c.is_ascii_hexdigit())
}

impl RobloxHomeSession {
    pub fn new(storage: &str, aliases: &str) -> Res<Self> {
        let mut session = RobloxHomeSession { files: Vec::new(), aliases: trim_ending_separator(&full_path(aliases)?), disposed: false, client_paths: HashMap::new() };
        std::fs::create_dir_all(storage).map_err(io_fail)?;
        for name in ["RobloxCookies.dat", "appStorage.json"] {
            match lock_file(&Path::new(storage).join(name)) {
                Ok(f) => session.files.push(f),
                Err(ex) => {
                    session.dispose(); // If the second file is busy, never leave just the first one locked.
                    return Err(ex);
                }
            }
        }
        Ok(session)
    }

    /// Only a client executable directly inside one of our GUID-named junctions belongs to this session.
    pub fn owns_client_path(&self, path: &str) -> bool {
        let p = Path::new(path);
        if !p.is_absolute() || !p.file_name().is_some_and(|n| eq_ci(&n.to_string_lossy(), "RobloxPlayerBeta.exe")) {
            return false;
        }
        let Ok(full) = std::path::absolute(p) else { return false };
        let Some(parent) = full.parent() else { return false };
        let name = parent.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        parent.parent().is_some_and(|pp| eq_ci(&pp.to_string_lossy(), &self.aliases)) && is_alias_name(&name)
    }

    /// Each process identity (a reused PID is a new one) is queried once; an unknown client that can't be inspected
    /// keeps the locks held until confirmed or gone, rather than risking an early unlock.
    /// `query`: the launch path of (pid, created); roblox_processes::launch_path when None.
    pub fn find_clients(&mut self, players: &[PlayerProcess], query: Option<&dyn Fn(i32, i64) -> Option<String>>) -> (Vec<i32>, bool) {
        let default_query = |pid: i32, created: i64| roblox_processes::launch_path(pid, created);
        let query: &dyn Fn(i32, i64) -> Option<String> = query.unwrap_or(&default_query);
        let alive: HashSet<(i32, i64)> = players.iter().map(|p| (p.pid, p.created)).collect();
        self.client_paths.retain(|k, _| alive.contains(k));
        let mut ours = Vec::new();
        let mut uncertain = false;
        for player in players {
            let key = (player.pid, player.created);
            let own = match self.client_paths.get(&key) {
                Some(own) if player.created != 0 => *own,
                _ => {
                    let Some(path) = query(player.pid, player.created) else {
                        uncertain = true;
                        continue;
                    };
                    let own = self.owns_client_path(&path);
                    if player.created != 0 {
                        self.client_paths.insert(key, own);
                    }
                    own
                }
            };
            if own {
                ours.push(player.pid);
            }
        }
        (ours, uncertain)
    }

    /// The start of a home screen for `client`, in a new junction; returns the command and the junction (its working folder).
    pub fn create_start_info(&self, client: &str) -> Res<(Command, String)> {
        if self.disposed {
            return Err(fail_of("ObjectDisposedException", "Cannot access a disposed object. Object name: 'RobloxHomeSession'."));
        }
        let client = full_path(client)?;
        let p = Path::new(&client);
        if !p.is_file() || !p.file_name().is_some_and(|n| eq_ci(&n.to_string_lossy(), "RobloxPlayerBeta.exe")) {
            return Err(fail_of("FileNotFoundException", "RobloxPlayerBeta.exe was not found."));
        }
        let target = p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        if target.starts_with(r"\\") {
            return Err(fail_of("IOException", "A Roblox home screen needs a client installed on a local drive."));
        }
        std::fs::create_dir_all(&self.aliases).map_err(io_fail)?;
        let alias = Path::new(&self.aliases).join(format!("client-{}", uuid::Uuid::new_v4().simple())).to_string_lossy().into_owned();
        create_junction(&alias, &target)?;
        let mut cmd = Command::new(Path::new(&alias).join("RobloxPlayerBeta.exe"));
        cmd.current_dir(&alias);
        Ok((cmd, alias)) // No ticket, game link, or environment override; Roblox handles the sign-in itself.
    }

    pub fn start(&mut self, client: &str) -> Res<()> {
        let (mut cmd, alias) = self.create_start_info(client)?;
        // (Under the desktop's process, as roblox_launcher's start: its own app in Task Manager, not inside Bloxkeep.)
        let exe = Path::new(&alias).join("RobloxPlayerBeta.exe").to_string_lossy().into_owned();
        if let Some(started) = crate::core::win_kit::start_beside_shell(&exe, &[], &alias) {
            return match started {
                Ok((pid, created)) => {
                    self.client_paths.insert((pid as i32, created), true);
                    Ok(())
                }
                Err(ex) => {
                    delete_alias(Path::new(&alias));
                    Err(fail_of("Win32Exception", ex))
                }
            };
        }
        match cmd.spawn() {
            Ok(child) => {
                // The snapshot and launch-path query will identify it if it is still running.
                if let Some(created) = creation_time(&child) {
                    self.client_paths.insert((child.id() as i32, created), true);
                }
                Ok(())
            }
            Err(ex) => {
                delete_alias(Path::new(&alias));
                Err(fail_of("Win32Exception", ex))
            }
        }
    }

    /// Call only when no clients using these aliases are running. Delete junctions themselves, never their targets.
    pub fn cleanup_aliases(root: &str) {
        let Ok(entries) = std::fs::read_dir(root) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            // (Not is_dir(): it follows the junction, and one whose Roblox folder is gone, after an update, was kept
            // for good; delete_alias looks at the entry itself, as EnumerateDirectories did.)
            if is_alias_name(&name) {
                delete_alias(&e.path());
            }
        }
    }

    pub fn dispose(&mut self) {
        if self.disposed {
            return;
        }
        self.disposed = true;
        self.files.clear();
        self.client_paths.clear();
        // Paths stay usable if someone explicitly quits Bloxkeep before closing their clients.
    }
}

impl Drop for RobloxHomeSession {
    fn drop(&mut self) {
        self.dispose();
    }
}

fn lock_file(path: &PathBuf) -> Res<File> {
    let mut attempt = 0;
    loop {
        match OpenOptions::new().read(true).share_mode(0).open(path) {
            Ok(f) => return Ok(f),
            Err(e) if e.kind() == ErrorKind::NotFound => {
                // Only our own empty placeholder disappears, atomically when its handle closes (even on a crash).
                match OpenOptions::new().read(true).write(true).create_new(true).share_mode(0).custom_flags(FILE_FLAG_DELETE_ON_CLOSE).open(path) {
                    Ok(f) => return Ok(f),
                    // Someone created the file between Open and CreateNew: open it normally, never truncate it.
                    Err(_) if attempt < 2 && path.is_file() => {}
                    Err(e) => return Err(io_fail(e)),
                }
            }
            Err(e) => return Err(io_fail(e)),
        }
        attempt += 1;
    }
}

fn delete_alias(dir: &Path) {
    // An alias still in use can wait for the next time no clients are open.
    if let Ok(meta) = std::fs::symlink_metadata(dir)
        && meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        let _ = std::fs::remove_dir(dir); // Non-recursive: removing a junction must never walk the Roblox installation.
    }
}

/// Process.StartTime.ToFileTimeUtc of a child just started.
fn creation_time(child: &std::process::Child) -> Option<i64> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetProcessTimes;
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: the child's handle is open while `child` lives; the four outputs are valid.
    let ok = unsafe { GetProcessTimes(child.as_raw_handle() as _, &mut created, &mut exited, &mut kernel, &mut user) };
    (ok != 0).then(|| ((created.dwHighDateTime as i64) << 32) | created.dwLowDateTime as i64)
}

fn create_junction(alias: &str, target: &str) -> Res<()> {
    let utf16 = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(u16::to_le_bytes).collect() };
    let substitute = utf16(&format!(r"\??\{target}"));
    let print = utf16(target);
    let data_length = 8 + substitute.len() + 2 + print.len() + 2;
    if data_length > u16::MAX as usize {
        return Err(fail_of("PathTooLongException", "The specified path, file name, or both are too long."));
    }
    let mut data = vec![0u8; 8 + data_length];
    data[0..4].copy_from_slice(&0xA000_0003u32.to_le_bytes()); // IO_REPARSE_TAG_MOUNT_POINT
    data[4..6].copy_from_slice(&(data_length as u16).to_le_bytes());
    data[10..12].copy_from_slice(&(substitute.len() as u16).to_le_bytes());
    data[12..14].copy_from_slice(&(substitute.len() as u16 + 2).to_le_bytes());
    data[14..16].copy_from_slice(&(print.len() as u16).to_le_bytes());
    data[16..16 + substitute.len()].copy_from_slice(&substitute);
    let at = 16 + substitute.len() + 2;
    data[at..at + print.len()].copy_from_slice(&print);
    std::fs::create_dir_all(alias).map_err(io_fail)?;
    if let Err(ex) = set_reparse(alias, &data) {
        let _ = std::fs::remove_dir(alias); // A fresh, empty directory or the junction itself; never recurse.
        return Err(ex);
    }
    Ok(())
}

fn set_reparse(alias: &str, data: &[u8]) -> Res<()> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::CreateFileW;
    use windows_sys::Win32::System::IO::DeviceIoControl;
    let path: Vec<u16> = alias.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `path` is NUL-terminated; GENERIC_WRITE, share all, OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS.
    let handle = unsafe { CreateFileW(path.as_ptr(), 0x4000_0000, 7, std::ptr::null(), 3, 0x0220_0000, std::ptr::null_mut()) };
    if handle == INVALID_HANDLE_VALUE {
        // SAFETY: reads this thread's last error.
        return Err(fail_of("Win32Exception", std::io::Error::from_raw_os_error(unsafe { GetLastError() } as i32)));
    }
    let mut returned = 0u32;
    // SAFETY: `data` is a whole REPARSE_DATA_BUFFER of `data.len()` bytes; no output buffer, no overlapped.
    let ok = unsafe {
        DeviceIoControl(handle, 0x0009_00A4, data.as_ptr().cast(), data.len() as u32, std::ptr::null_mut(), 0, &mut returned, std::ptr::null_mut())
    };
    // SAFETY: reads this thread's last error before CloseHandle changes it.
    let error = unsafe { GetLastError() };
    // SAFETY: `handle` was opened above and is closed once.
    unsafe { CloseHandle(handle) };
    if ok == 0 {
        return Err(fail_of("Win32Exception", std::io::Error::from_raw_os_error(error as i32)));
    }
    Ok(())
}
