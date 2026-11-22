//! Vault.cs: the encrypted account store kept only on this machine.
//!
//! File layout: "RMV1" | PBKDF2 iterations (int32 LE) | salt (16) | nonce (12) | GCM tag (16) | ciphertext.
//! Key = PBKDF2-HMAC-SHA256(master password, salt); cipher = AES-256-GCM with the header bound as associated data,
//! so any tampering with the file (or a wrong password) is detected on open.
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard, RwLock, mpsc};
use std::time::{Duration, Instant};

use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce, Tag};
use regex::Regex;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use crate::core::base::{Fail, Res, Stamp};
use crate::core::l;
use crate::core::log;
use crate::core::models::{VaultData, to_net_json};
use crate::core::roblox_home_session::RobloxHomeSession;
use crate::core::themes;

/// InvalidPasswordException: "Wrong master password." in the app's language.
pub fn invalid_password() -> Fail {
    Fail::user(l::t("Wrong master password."))
}

/// The Fail is `invalid_password()` (C#'s `catch (InvalidPasswordException)`), in either language.
pub fn is_invalid_password(fail: &Fail) -> bool {
    matches!(fail, Fail::User(m) if *m == l::t_in("Wrong master password.", "en") || *m == l::t_in("Wrong master password.", "vi"))
}

const SALT_SIZE: usize = 16;
const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;
const KEY_SIZE: usize = 32;
const HEADER_SIZE: usize = 4 + 4 + SALT_SIZE;
const DEFAULT_ITERATIONS: i32 = 600_000;
const MAGIC: &[u8; 4] = b"RMV1";

/// A lock that a panic elsewhere doesn't leave unusable (C#'s `lock` has no poisoning).
fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn starts_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.is_char_boundary(prefix.len()) && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn ends_ci(s: &str, suffix: &str) -> bool {
    s.len() >= suffix.len() && s.is_char_boundary(s.len() - suffix.len()) && s[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

/// `path + suffix` as C# joins strings ("vault.rmv" + ".bak").
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The name of an error's kind, where C# logs `ex.GetType().Name`.
fn kind_name(e: &std::io::Error) -> String {
    format!("{:?}", e.kind())
}

static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(resolve_data_dir);

/// %LOCALAPPDATA%\Bloxkeep by default, apart from the program (%LOCALAPPDATA%\BloxkeepApp, which uninstalling
/// deletes), so accounts survive an uninstall. The portable zip, or a "data" folder next to the exe, keeps data beside
/// the app; BLOXKEEP_DATA overrides all of these.
pub fn data_dir() -> &'static Path {
    &DATA_DIR
}

/// Old copies of vaults: one taken off the list (vault-removed-…), the one a restore replaced (vault-before-restore-…).
pub fn old_vaults_dir() -> PathBuf {
    data_dir().join("old-vaults")
}
/// The custom theme's picture and the vaults' pictures (`themes::is_picture_name`).
pub fn pictures_dir() -> PathBuf {
    data_dir().join("pictures")
}
/// What Bloxkeep rebuilds when it's gone: the browsers' profiles, the sign-in windows' client paths.
pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

/// A browser profile's folder in `cache_dir`, with cache made first (best effort: WebView2 reports its own error).
pub fn cache_folder(name: &str) -> PathBuf {
    let _ = fs::create_dir_all(cache_dir());
    cache_dir().join(name)
}

const CACHE_FOLDERS: [&str; 4] = ["webview", "webview-safe", "captcha", "roblox-client-paths"];

/// Moves what builds before 1.0.11 kept at the top of the data folder into old-vaults, pictures and cache. Run
/// at start, before anything opens them; what won't move (held by another program) is tried next start. The vaults,
/// their .bak and .tmp copies, settings and the log stay at the top. Never stops the start: errors are only logged.
pub fn tidy() {
    if data_dir().is_dir() {
        if let Err(e) = tidy_folder() {
            log::write(&e);
        }
    }
}

fn tidy_folder() -> Res<()> {
    let files: Vec<PathBuf> = fs::read_dir(data_dir())?.filter_map(|e| e.ok()).filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false)).map(|e| e.path()).collect();
    for file in files {
        let name = file_name(&file);
        let to = if is_old_vault(&name) {
            Some(old_vaults_dir())
        } else if themes::is_picture_name(&name) {
            Some(pictures_dir())
        } else {
            None
        };
        let Some(to) = to else { continue };
        let moved = (|| -> std::io::Result<()> {
            fs::create_dir_all(&to)?;
            if !to.join(&name).exists() {
                fs::rename(&file, to.join(&name))?;
            }
            Ok(())
        })();
        if let Err(e) = moved {
            log::write(&e);
        }
    }
    for name in CACHE_FOLDERS {
        let (from, to) = (data_dir().join(name), cache_dir().join(name));
        // Right after an update's restart the last run's browser may hold its profile a moment longer; moved
        // later, the interface would start on an empty one (the list's layout and sort forgotten).
        let mut attempt = 1;
        while attempt <= 6 && from.is_dir() {
            let done = (|| -> std::io::Result<()> {
                fs::create_dir_all(cache_dir())?;
                if !to.is_dir() {
                    fs::rename(&from, &to)?;
                } else if name == "roblox-client-paths" {
                    // Junctions to Roblox's own folder: each removed as a link, never walked into.
                    RobloxHomeSession::cleanup_aliases(&from.to_string_lossy());
                    fs::remove_dir(&from)?;
                } else {
                    // A cache: with a new one already there the old one is only in the way.
                    fs::remove_dir_all(&from)?;
                }
                Ok(())
            })();
            if let Err(e) = done {
                // Waited for only to move one: an old copy beside a new one, or junctions in use (until Roblox
                // closes), wait for the next start instead of slowing every start.
                if attempt == 6 || to.is_dir() {
                    log::write(&e);
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            attempt += 1;
        }
    }
    Ok(())
}

fn is_old_vault(name: &str) -> bool {
    (starts_ci(name, "vault-before-restore-") || starts_ci(name, "vault-removed-")) && ends_ci(name, ".rmv")
}

/// Removes the interface's browser caches (rebuilt when needed). Accounts and settings stay.
pub fn delete_caches() {
    // The top of the data folder too: where builds before 1.0.11 kept them. Not in a folder BLOXKEEP_DATA points at: it
    // may be the person's own, with a "webview" or "captcha" folder that isn't ours (review, 2026-10-30).
    let own_folder = data_dir_variable().is_none();
    let mut dirs: Vec<PathBuf> = ["webview", "webview-safe", "captcha"]
        .iter()
        .flat_map(|n| if own_folder { vec![cache_dir().join(n), data_dir().join(n)] } else { vec![cache_dir().join(n)] })
        .collect();
    // The pictures the window keeps (images.rs) and their addresses (roblox.rs).
    dirs.push(cache_dir().join("pictures"));
    let _ = fs::remove_file(cache_dir().join("picture-addresses.json"));
    for dir in dirs {
        // The browser processes may need a moment to let go of their files.
        let mut attempt = 1;
        while attempt <= 4 && dir.is_dir() {
            if fs::remove_dir_all(&dir).is_err() {
                std::thread::sleep(Duration::from_millis(1000));
            }
            attempt += 1;
        }
    }
}

/// Every file Bloxkeep itself makes at the top of the data folder, by exact name (the one the code that
/// writes it joins to `data_dir`). Each new one is added here, or "Delete my accounts and settings" leaves it behind and
/// the folder stays: DataFolderSuite reads the source for the names and checks every one is covered.
const OWN_FILES: [&str; 14] = [
    "prefs.json", "error.log", "error.log.1", "error.log.2", "roblox-settings.json", "roblox-flags.json", "game-hours.json", "hidden-windows.txt",
    "window-icons.txt", "starting.flag", "resume.bin", "capacity.json", "running.mark", "heap-sites.txt",
];

// .NET's `$` also matches before a final "\n": `\n?\z` here, in every pattern ported with one.
static OWN_VAULT_FILE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^vault(-[0-9a-f]{8})?\.rmv(\.bak|\.tmp|\.damaged-.*|\.spare-.*)?\n?\z").unwrap());

/// Whether a file in the data folder (or old-vaults, pictures) is Bloxkeep's to delete. By exact name or pattern: in a
/// folder of the person's own, "prefs.json5" or "error.logs" aren't ours.
pub fn is_own_file(name: &str) -> bool {
    OWN_FILES.iter().any(|own| name.eq_ignore_ascii_case(own) || name.eq_ignore_ascii_case(&format!("{own}.tmp")))
        // Every vault (vault.rmv, vault-<id>.rmv) with its spare, unfinished and damaged copies.
        || OWN_VAULT_FILE.is_match(name)
        || starts_ci(name, "prefs.json.damaged-")
        || starts_ci(name, "roblox-flags.json.damaged-")
        || starts_ci(name, "roblox-settings.json.damaged-")
        || is_old_vault(name)
        || themes::is_picture_name(name) // the custom theme's picture, the vaults' pictures
}

/// An uninstall that takes everything: vaults with their spare copies, settings, logs and caches. Only Bloxkeep's
/// own files (DataDir may be a folder BLOXKEEP_DATA points at), then the folder once empty. Backups stay.
pub fn delete_all_data() {
    if !data_dir().is_dir() {
        return;
    }
    delete_caches();
    // The Wiki tab's game looks (wiki_look), or the cache folder and the data folder were never removed.
    for name in ["game-looks.json", "game-looks.json.tmp"] {
        let _ = fs::remove_file(cache_dir().join(name));
    }
    let folders: Vec<PathBuf> = [data_dir().to_path_buf(), old_vaults_dir(), pictures_dir()].into_iter().filter(|d| d.is_dir()).collect();
    let files: Vec<PathBuf> = folders
        .iter()
        .filter_map(|d| fs::read_dir(d).ok())
        .flat_map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false)).map(|e| e.path()).collect::<Vec<_>>())
        .filter(|f| is_own_file(&file_name(f)))
        .collect();
    for file in files {
        // Left behind when it won't go; the folder then stays too.
        let _ = fs::remove_file(&file);
    }
    let _ = (|| -> std::io::Result<()> {
        // Never a recursive delete here: cache\roblox-client-paths may hold junctions to Roblox's own folder.
        let aliases = cache_dir().join("roblox-client-paths");
        RobloxHomeSession::cleanup_aliases(&aliases.to_string_lossy());
        for dir in [aliases, old_vaults_dir(), pictures_dir(), cache_dir()] {
            if dir.is_dir() && fs::read_dir(&dir)?.next().is_none() {
                fs::remove_dir(&dir)?;
            }
        }
        if fs::read_dir(data_dir())?.next().is_none() {
            fs::remove_dir(data_dir())?;
        }
        Ok(())
    })();
}

static ACTIVE_FILE: LazyLock<RwLock<String>> = LazyLock::new(|| RwLock::new("vault.rmv".into()));

/// The vault file the lock screen opens or sets up (`vaults`): vault.rmv for the first, vault-<id>.rmv for each added
/// since. An open vault keeps writing to its own file whatever this becomes.
pub fn active_file() -> String {
    ACTIVE_FILE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn set_active_file(file: &str) {
    *ACTIVE_FILE.write().unwrap_or_else(|e| e.into_inner()) = file.to_string();
}

pub fn vault_path() -> PathBuf {
    data_dir().join(active_file())
}
fn backup_path() -> PathBuf {
    with_suffix(&vault_path(), ".bak")
}
fn temp_path() -> PathBuf {
    with_suffix(&vault_path(), ".tmp")
}

/// Files checked by `exists` this run, lower-cased (OrdinalIgnoreCase).
static RESTORE_CHECKED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Decides between "set up" and "unlock", so a vault lost mid-save is put back first (once per run and file).
pub fn exists() -> bool {
    if guard(&RESTORE_CHECKED).insert(active_file().to_lowercase()) {
        restore_if_missing();
    }
    vault_path().is_file()
}

/// The data folder set with BLOXKEEP_DATA (or its old name), or None.
fn data_dir_variable() -> Option<String> {
    let custom = std::env::var("BLOXKEEP_DATA").ok().or_else(|| std::env::var("ROBLOXMANAGER_DATA").ok())?;
    if custom.is_empty() { None } else { Some(custom) }
}

fn resolve_data_dir() -> PathBuf {
    if let Some(custom) = data_dir_variable() {
        return PathBuf::from(custom);
    }
    // The portable zip keeps the app in "<folder>\current", which every update replaces; data sits beside it.
    // (AppContext.BaseDirectory: the program's own folder.)
    let app_dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)).unwrap_or_default();
    if file_name(&app_dir).eq_ignore_ascii_case("current") {
        if let Some(root) = app_dir.parent() {
            if root.join(".portable").is_file() {
                return root.join("data");
            }
        }
    }
    let portable = app_dir.join("data");
    if portable.is_dir() {
        return portable;
    }
    let local = crate::core::base::local_app_data();
    let dir = local.join("Bloxkeep");
    copy_from_old_name(&local.join("RobloxManager"), &dir);
    dir
}

/// Early builds were called "Roblox Manager"; carry their vault over once (the old copy is left in place).
fn copy_from_old_name(old_dir: &Path, new_dir: &Path) {
    // Start with an empty vault rather than failing to start.
    let _ = (|| -> std::io::Result<()> {
        if new_dir.join("vault.rmv").is_file() || !old_dir.join("vault.rmv").is_file() {
            return Ok(());
        }
        fs::create_dir_all(new_dir)?;
        for name in ["vault.rmv", "vault.rmv.bak", "prefs.json"] {
            let source = old_dir.join(name);
            let target = new_dir.join(name);
            if source.is_file() {
                // File.Copy(…, overwrite: false): an existing file is an error, not overwritten.
                if target.exists() {
                    return Err(std::io::Error::from_raw_os_error(windows_sys::Win32::Foundation::ERROR_FILE_EXISTS as i32));
                }
                fs::copy(&source, &target)?;
            }
        }
        Ok(())
    })();
}

fn derive_key(password: &str, salt: &[u8], iterations: i32) -> Vec<u8> {
    // Rfc2898DeriveBytes.Pbkdf2(string, …) takes the password's UTF-8 bytes.
    let mut key = vec![0u8; KEY_SIZE];
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations as u32, &mut key);
    key
}

/// RandomNumberGenerator: the system's random source.
fn random_bytes(n: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; n];
    getrandom::fill(&mut bytes).expect("the system's random source");
    bytes
}

/// CryptographicOperations.FixedTimeEquals: time independent of where the two differ.
fn fixed_time_equals(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    std::hint::black_box(a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y))) == 0
}

fn disposed() -> Fail {
    Fail::other("The vault is closed (ObjectDisposedException).")
}

fn newer() -> Fail {
    Fail::user(l::t("This data file was made by a newer Bloxkeep. Update Bloxkeep to open it."))
}

/// What the key, salt and iteration count are, and how many saves have taken the vault (`_taken`); C#'s `_sync`.
struct Keys {
    key: Vec<u8>,
    salt: Vec<u8>,
    iterations: i32,
    taken: i64,
}

/// A background save waiting for the worker: the vault as it was then, and the key it goes under. Wiped when dropped
/// (C#'s `Forget`).
struct Pending {
    number: i64,
    plain: Vec<u8>,
    key: Vec<u8>,
    salt: Vec<u8>,
    iterations: i32,
    path: PathBuf,
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.plain.zeroize();
        self.key.zeroize();
    }
}

struct QueueState {
    queued: Option<Pending>,
    /// Background saves asked for and not finished: `_background` not completed while above 0.
    running: usize,
}

/// What the background worker shares with the vault: C#'s `_queueGate`, `_queued`, `_background` and `_written`.
struct Shared {
    queue: Mutex<QueueState>,
    idle: Condvar,
    /// Saves are numbered as they take the vault (under `sync`); the file gate keeps the newest written. A background
    /// save that took the vault before a save made now but reaches the file after it is left out, so the file never goes
    /// back to an older vault, nor to an old password.
    written: AtomicI64,
}

/// One background save's end: Ok once this vault (or newer) is on disk, Err when that write failed (it is then tried
/// again with the next save). C#'s Task from SaveInBackground.
pub struct SaveTask(mpsc::Receiver<Res<()>>);

impl SaveTask {
    /// Waits for it.
    pub fn wait(&self) -> Res<()> {
        self.0.recv().unwrap_or_else(|_| Err(Fail::other("The vault's save worker stopped.")))
    }
}

/// One write to the vault file at a time: a save in the background and one made now never interleave.
static FILE_GATE: Mutex<()> = Mutex::new(());

type Hook = Arc<dyn Fn() + Send + Sync>;
/// Test hook (Tests/Bloxkeep.Tests): a background save calls it between taking the vault and reaching the file, as a
/// busy PC may pause there. The app never sets it.
static BEFORE_BACKGROUND_WRITE: Mutex<Option<Hook>> = Mutex::new(None);

pub fn set_before_background_write(hook: Option<Hook>) {
    *guard(&BEFORE_BACKGROUND_WRITE) = hook;
}

pub struct Vault {
    /// The file it was read from (or made at), taken before the key's slow derivation: the vault chosen may change
    /// meanwhile (the picker during an unlock), and this one must never save over another's file.
    path: PathBuf,
    sync: Mutex<Keys>,
    data: Mutex<VaultData>,
    shared: Arc<Shared>,
    worker: Mutex<Option<mpsc::Sender<mpsc::SyncSender<Res<()>>>>>,
}

/// What `read_other` found in a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OtherStatus {
    Opened,
    TooLarge,
    Unreadable,
    NotAVault,
    WrongPassword,
    Newer,
}

/// Far above any real vault (a thousand accounts are well under 5 MB); a bigger file isn't read into memory.
pub const MAX_OTHER_SIZE: u64 = 50 * 1024 * 1024;

/// Below any vault ever written (an empty one is about 1.7 KB, its settings alone hundreds of bytes): a file this
/// small with the right first bytes was cut short, and restoring it made every unlock fail (review, 2026-10-30).
const MIN_VAULT_SIZE: usize = 256;

/// The newest `VaultData::version` this build understands; a newer one must not be re-saved by it.
const SUPPORTED_VERSION: i32 = 1;

impl Vault {
    fn new(path: PathBuf, key: Vec<u8>, salt: Vec<u8>, iterations: i32, data: VaultData) -> Vault {
        let shared = Arc::new(Shared { queue: Mutex::new(QueueState { queued: None, running: 0 }), idle: Condvar::new(), written: AtomicI64::new(0) });
        let (tx, rx) = mpsc::channel::<mpsc::SyncSender<Res<()>>>();
        let worker_shared = shared.clone();
        // The background saves' worker (C#'s ContinueWith chain): one at a time, in the order asked.
        // Its own thread: it waits for saves for as long as the vault lives.
        crate::threads::own("vault-save", move || {
            for done in rx {
                let result = write_queued(&worker_shared);
                {
                    let mut q = guard(&worker_shared.queue);
                    q.running -= 1;
                    worker_shared.idle.notify_all();
                }
                let _ = done.send(result);
            }
        })
        .expect("a thread for the vault's saves");
        Vault { path, sync: Mutex::new(Keys { key, salt, iterations, taken: 0 }), data: Mutex::new(data), shared, worker: Mutex::new(Some(tx)) }
    }

    /// The file this vault was opened from or created in, which every save writes.
    pub fn file_path(&self) -> &Path {
        &self.path
    }

    /// The vault's data. Let go of it before `save`, `save_in_background` or `change_password`, which read it.
    pub fn data(&self) -> MutexGuard<'_, VaultData> {
        guard(&self.data)
    }

    /// `path`: the file to make; the chosen vault's when not given (take it before going to a worker).
    pub fn create(password: &str, path: Option<PathBuf>) -> Res<Vault> {
        let path = path.unwrap_or_else(vault_path);
        let salt = random_bytes(SALT_SIZE);
        let vault = Vault::new(path, derive_key(password, &salt, DEFAULT_ITERATIONS), salt, DEFAULT_ITERATIONS, VaultData::default());
        vault.save()?;
        Ok(vault)
    }

    /// Opens vault.rmv, or the previous save (vault.rmv.bak) when the main file is damaged (a power cut, an
    /// antivirus). The damaged file is kept beside the vault, never deleted.
    /// Err `invalid_password()`: the password does not decrypt the file.
    /// `path`: the file to open; the chosen vault's when not given (take it before going to a worker).
    pub fn open(password: &str, path: Option<PathBuf>) -> Res<Vault> {
        let path = path.unwrap_or_else(vault_path);
        let backup_path = with_suffix(&path, ".bak");
        let file = fs::read(&path)?;
        let backup = if backup_path.is_file() { Some(fs::read(&backup_path)?) } else { None };
        if let Some((iterations, salt)) = header_ok(&file) {
            let mut key = derive_key(password, &salt, iterations);
            if let Some(data) = decrypt(&file, &key)? {
                return Ok(Vault::new(path, key, salt, iterations, data));
            }
            // Wrong password, or a damaged file? The previous save was made with the same key (the salt only changes
            // with the password), so if it opens with this key the main file is what's broken.
            if let Some(backup) = &backup {
                if let Some((backup_iterations, backup_salt)) = header_ok(backup) {
                    let same_key = backup_iterations == iterations && backup_salt == salt;
                    if same_key {
                        if let Some(previous) = decrypt(backup, &key)? {
                            return Ok(recovered(path, key, salt, iterations, previous));
                        }
                    }
                    // A main file whose salt or iteration count was damaged still looks like a vault, but no password opens
                    // it: the previous save is tried with its own (review, 2026-10-30). Rare, since a password change drops
                    // the .bak: a wrong password pays a second derivation only then.
                    if !same_key {
                        let mut backup_key = derive_key(password, &backup_salt, backup_iterations);
                        if let Some(older) = decrypt(backup, &backup_key)? {
                            key.zeroize();
                            return Ok(recovered(path, backup_key, backup_salt, backup_iterations, older));
                        }
                        backup_key.zeroize();
                    }
                }
            }
            key.zeroize();
            return Err(invalid_password());
        }
        // Not a vault any more (cut short, or overwritten): the previous save is all there is.
        if let Some(backup) = &backup {
            if let Some((iterations, salt)) = header_ok(backup) {
                let mut key = derive_key(password, &salt, iterations);
                if let Some(previous) = decrypt(backup, &key)? {
                    return Ok(recovered(path, key, salt, iterations, previous));
                }
                key.zeroize();
                return Err(invalid_password());
            }
        }
        Err(Fail::user(l::t("The data file is not in the right format or is damaged.")))
    }

    /// Opens with the key the previous run handed over for an update's restart (Relaunch). Only the main file, and only
    /// while it still has that key's salt; otherwise the lock screen asks as usual.
    /// Err `invalid_password()`: the key doesn't open the file.
    pub fn open_with_key(mut key: Vec<u8>, salt: Vec<u8>, iterations: i32) -> Res<Vault> {
        let path = vault_path();
        let file = fs::read(&path)?;
        if let Some((file_iterations, file_salt)) = header_ok(&file) {
            if file_iterations == iterations && file_salt == salt {
                if let Some(data) = decrypt(&file, &key)? {
                    return Ok(Vault::new(path, key, salt, iterations, data));
                }
            }
        }
        key.zeroize();
        Err(invalid_password())
    }

    /// A copy of the key, salt and iteration count, for the run that follows an update (see Relaunch).
    pub fn export_key(&self) -> Res<(Vec<u8>, Vec<u8>, i32)> {
        let keys = guard(&self.sync);
        if keys.key.is_empty() {
            return Err(disposed());
        }
        Ok((keys.key.clone(), keys.salt.clone(), keys.iterations))
    }

    /// Writes the vault now (lock, exit, a changed password: callers that need it on disk before going on).
    pub fn save(&self) -> Res<()> {
        let mut keys = guard(&self.sync);
        if keys.key.is_empty() {
            return Err(disposed());
        }
        let (key, salt, iterations) = (keys.key.clone(), keys.salt.clone(), keys.iterations);
        let key = Zeroizing::new(key);
        self.write(&mut keys, &key, &salt, iterations)
    }

    /// Takes the vault now (on the caller's thread, so nothing changes under it) and encrypts, writes and flushes it on
    /// a worker. One write at a time; a newer save replaces one still waiting. The task ends once this vault (or newer)
    /// is on disk, failed when that write failed (it is then tried again with the next save).
    pub fn save_in_background(&self) -> Res<SaveTask> {
        let next = {
            let mut keys = guard(&self.sync);
            if keys.key.is_empty() {
                return Err(disposed());
            }
            keys.taken += 1;
            let plain = to_net_json(&*self.data())?;
            Pending { number: keys.taken, plain, key: keys.key.clone(), salt: keys.salt.clone(), iterations: keys.iterations, path: self.path.clone() }
        };
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let mut q = guard(&self.shared.queue);
        q.queued = Some(next); // the one waiting before (if any) is wiped as it is dropped
        q.running += 1;
        let sent = guard(&self.worker).as_ref().map(|w| w.send(done_tx).is_ok()).unwrap_or(false);
        if !sent {
            q.running -= 1;
            q.queued = None;
            return Err(disposed());
        }
        Ok(SaveTask(done_rx))
    }

    /// A save waiting in the background goes: what is written now is newer (and may be under a new password).
    fn drop_queued(&self) {
        guard(&self.shared.queue).queued = None;
    }

    /// A background save is waiting or being written.
    pub fn saving_in_background(&self) -> bool {
        guard(&self.shared.queue).running > 0
    }

    /// Waits for a background save still being written (the vault is about to close: lock, exit, an update).
    pub fn wait_for_saves(&self, timeout: Duration) {
        let until = Instant::now() + timeout;
        let mut q = guard(&self.shared.queue);
        // Its failure was reported to whoever asked for that save.
        while q.running > 0 {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            q = self.shared.idle.wait_timeout(q, left).unwrap_or_else(|e| e.into_inner()).0;
        }
    }

    /// Callers hold `sync` (C#: "callers hold _sync").
    fn write(&self, keys: &mut Keys, key: &[u8], salt: &[u8], iterations: i32) -> Res<()> {
        let plain = Zeroizing::new(to_net_json(&*self.data())?);
        keys.taken += 1;
        let number = keys.taken;
        let _gate = guard(&FILE_GATE);
        self.drop_queued(); // older than this, and possibly under the old password (ChangePassword)
        write_encrypted(&plain, key, salt, iterations, &self.path)?;
        self.shared.written.store(number, Ordering::SeqCst);
        Ok(())
    }

    pub fn verify_password(&self, password: &str) -> bool {
        let (salt, key, iterations) = {
            let keys = guard(&self.sync);
            (keys.salt.clone(), Zeroizing::new(keys.key.clone()), keys.iterations)
        };
        let candidate = Zeroizing::new(derive_key(password, &salt, iterations));
        fixed_time_equals(&candidate, &key)
    }

    /// The slow part of a password change (key stretching), safe to run off the UI thread: (key, salt).
    pub fn prepare_key(new_password: &str) -> (Vec<u8>, Vec<u8>) {
        let salt = random_bytes(SALT_SIZE);
        (derive_key(new_password, &salt, DEFAULT_ITERATIONS), salt)
    }

    /// Writes the vault under the new password and only then switches to it: if the write fails (antivirus lock,
    /// disk full) the old password keeps working, on disk and for later saves.
    pub fn change_password(&self, prepared: (Vec<u8>, Vec<u8>)) -> Res<()> {
        {
            let mut keys = guard(&self.sync);
            if keys.key.is_empty() {
                return Err(disposed());
            }
            self.write(&mut keys, &prepared.0, &prepared.1, DEFAULT_ITERATIONS)?;
            keys.key.zeroize();
            keys.key = prepared.0;
            keys.salt = prepared.1;
            keys.iterations = DEFAULT_ITERATIONS;
        }
        // The previous save is still encrypted with the old password; drop it so it doesn't open with that one.
        // (Older copies — backups in the backup folder, .spare/.damaged/before-restore files — still do; they are
        // the person's to delete.) The change itself has succeeded by now, so a file that won't go is only logged.
        let bak = with_suffix(&self.path, ".bak"); // this vault's, whichever is chosen now
        if let Err(e) = fs::remove_file(&bak) {
            // File.Delete of a missing file is no error.
            if e.kind() != std::io::ErrorKind::NotFound {
                log::write(&e);
            }
        }
        Ok(())
    }

    /// This vault's own id, as its backups carry it; it changes with the password (a new salt).
    pub fn id(&self) -> String {
        id_of_salt(&guard(&self.sync).salt)
    }

    /// Closing: a save still on its way is finished first, then the key is wiped. Saves fail after this.
    pub fn dispose(&self) {
        // A save still on its way to disk is finished first: closing (lock, exit, an update's restart) mustn't lose it.
        self.wait_for_saves(Duration::from_secs(10));
        self.drop_queued();
        let mut keys = guard(&self.sync);
        keys.key.zeroize();
        keys.key = Vec::new();
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        self.dispose();
        // The worker ends once its queue is closed.
        guard(&self.worker).take();
    }
}

fn write_queued(shared: &Shared) -> Res<()> {
    let p = guard(&shared.queue).queued.take();
    // None: written already, by the save before this one or by one made now.
    let Some(p) = p else { return Ok(()) };
    let hook = guard(&BEFORE_BACKGROUND_WRITE).clone();
    if let Some(hook) = hook {
        hook();
    }
    let result = {
        let _gate = guard(&FILE_GATE);
        if p.number < shared.written.load(Ordering::SeqCst) {
            return Ok(()); // a newer save took the file first: one made now, while this was on its way
        }
        write_encrypted(&p.plain, &p.key, &p.salt, p.iterations, &p.path).map(|_| shared.written.store(p.number, Ordering::SeqCst))
    };
    if result.is_err() {
        // Kept for the next save to try again, unless a newer one is already waiting.
        let mut q = guard(&shared.queue);
        if q.queued.is_none() {
            q.queued = Some(p);
        }
    }
    result
}

/// The file is big enough to hold a vault, starts with the magic and has a sane iteration count: (iterations, salt).
fn header_ok(file: &[u8]) -> Option<(i32, Vec<u8>)> {
    if file.len() < HEADER_SIZE + NONCE_SIZE + TAG_SIZE || &file[0..4] != MAGIC {
        return None;
    }
    let iterations = i32::from_le_bytes([file[4], file[5], file[6], file[7]]);
    if !(10_000..=50_000_000).contains(&iterations) {
        return None;
    }
    Some((iterations, file[8..8 + SALT_SIZE].to_vec()))
}

/// Ok(None) when the key does not open the file (wrong password, or the file was changed); Err (newer) when the file
/// opens but was written by a newer Bloxkeep.
fn decrypt(file: &[u8], key: &[u8]) -> Res<Option<VaultData>> {
    let cipher_text = &file[HEADER_SIZE + NONCE_SIZE + TAG_SIZE..];
    let mut plain = Zeroizing::new(cipher_text.to_vec());
    let aes = Aes256Gcm::new_from_slice(key).map_err(Fail::other)?;
    let nonce = Nonce::from_slice(&file[HEADER_SIZE..HEADER_SIZE + NONCE_SIZE]);
    let tag = Tag::from_slice(&file[HEADER_SIZE + NONCE_SIZE..HEADER_SIZE + NONCE_SIZE + TAG_SIZE]);
    if aes.decrypt_in_place_detached(nonce, &file[..HEADER_SIZE], &mut plain, tag).is_err() {
        return Ok(None); // AuthenticationTagMismatchException
    }
    // The password was right (the tag matched): a field that won't read changed shape in a newer version.
    let data: Option<VaultData> = match serde_json::from_slice(&plain) {
        Ok(data) => data,
        // System.Text.Json read null into a string, list, bool or object and the C# app may have written one;
        // serde won't. Only on this path, so a clean file's unknown fields round-trip exactly.
        Err(_) => {
            let mut value: serde_json::Value = serde_json::from_slice(&plain).map_err(|_| newer())?;
            drop_nulls(&mut value);
            serde_json::from_value(value).map_err(|_| newer())?
        }
    };
    let data = data.unwrap_or_default();
    if data.version > SUPPORTED_VERSION {
        return Err(newer());
    }
    Ok(Some(data))
}

/// Removes null members and null array elements at every depth: a missing field takes the model's default, which is
/// what C# did with null in practice (and the same as null for an Option field: the models default those to None).
fn drop_nulls(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            for (key, v) in map.iter_mut() {
                // A macro's steps, features and words keep their nulls: their places count (loop and break targets
                // are step numbers) and a null step shows as "Step N is empty." (macro_model's steps_de).
                if let (true, serde_json::Value::Array(items)) = (matches!(key.as_str(), "Steps" | "Features" | "Words"), &mut *v) {
                    items.iter_mut().for_each(drop_nulls);
                } else {
                    drop_nulls(v);
                }
            }
        }
        serde_json::Value::Array(items) => {
            items.retain(|v| !v.is_null());
            items.iter_mut().for_each(drop_nulls);
        }
        _ => {}
    }
}

/// Puts the previous save back in place of the damaged main file, which is kept as vault.rmv.damaged-….
fn recovered(path: PathBuf, key: Vec<u8>, salt: Vec<u8>, iterations: i32, data: VaultData) -> Vault {
    let put_back = (|| -> std::io::Result<()> {
        rename_new(&path, &with_suffix(&path, &format!(".damaged-{}", stamp())))?;
        copy_new(&with_suffix(&path, ".bak"), &path)?;
        log::note("vault.rmv was damaged; opened from vault.rmv.bak and kept the damaged file beside it.");
        Ok(())
    })();
    if let Err(e) = put_back {
        log::write(&e); // the next save writes vault.rmv afresh
    }
    Vault::new(path, key, salt, iterations, data)
}

/// File.Move(from, to): fails when `to` exists (std's rename would replace it).
fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
    if to.exists() {
        return Err(exists_error(windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS, to));
    }
    fs::rename(from, to)
}

/// File.Copy(from, to) without overwrite.
fn copy_new(from: &Path, to: &Path) -> std::io::Result<()> {
    if to.exists() {
        return Err(exists_error(windows_sys::Win32::Foundation::ERROR_FILE_EXISTS, to));
    }
    fs::copy(from, to).map(|_| ())
}

/// The IOException .NET throws there, as Windows' own error: its "(os error N)" makes it an I/O failure to
/// `is_io_error` and `error_text`, which a custom io::Error's text wouldn't. The path goes to the log only.
pub(crate) fn exists_error(code: u32, path: &Path) -> std::io::Error {
    log::note(&format!("{} exists", path.display()));
    std::io::Error::from_raw_os_error(code as i32)
}

/// A save writes vault.rmv.tmp, then swaps it in and keeps the replaced file as vault.rmv.bak. A swap cut short
/// (antivirus, power cut) can leave no vault.rmv but .bak/.tmp, and the app would offer a new vault over them.
/// The .tmp, written in full before the swap, goes back with .bak kept as its fallback (see `open`);
/// without a usable .tmp the previous save goes back.
fn restore_if_missing() {
    let done = (|| -> Res<()> {
        if vault_path().is_file() {
            return Ok(());
        }
        let restore = if looks_like_vault(&temp_path())? {
            temp_path()
        } else if looks_like_vault(&backup_path())? {
            backup_path()
        } else {
            return Ok(());
        };
        rename_new(&restore, &vault_path())?;
        // Only what can't serve as the fallback is set aside (a .tmp that isn't a vault, a .bak when .bak went back).
        for other in [backup_path(), temp_path()] {
            if other.is_file() && (restore == backup_path() || other != backup_path()) {
                rename_new(&other, &with_suffix(&vault_path(), &format!(".spare-{}", stamp())))?;
            }
        }
        log::note(&format!("vault.rmv was missing; put back {}.", file_name(&restore)));
        Ok(())
    })();
    if let Err(e) = done {
        log::write(&e);
    }
}

/// Starts like a vault file; whether it opens is only known with its password (see backup). Err when it is there but
/// can't be read (C#'s File.ReadAllBytes throws).
pub fn looks_like_vault(path: &Path) -> Res<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let bytes = fs::read(path)?;
    Ok(bytes.len() >= MIN_VAULT_SIZE && header_ok(&bytes).is_some())
}

/// Opens another vault file (from another PC, or a backup) with its own master password, to bring accounts over.
/// Read-only: neither that file nor this vault's key and files are touched. The derived key and the decrypted JSON
/// are wiped before returning (see `decrypt`). Slow (key stretching): call it off the UI thread.
/// The data when `OtherStatus::Opened`; None otherwise.
pub fn read_other(path: &Path, password: &str) -> (OtherStatus, Option<VaultData>) {
    let file = match fs::metadata(path) {
        Ok(m) if m.len() > MAX_OTHER_SIZE => return (OtherStatus::TooLarge, None),
        Ok(_) => match fs::read(path) {
            Ok(f) => f,
            Err(e) => {
                log::write(&e);
                return (OtherStatus::Unreadable, None);
            }
        },
        Err(e) => {
            log::write(&e);
            return (OtherStatus::Unreadable, None);
        }
    };
    let Some((iterations, salt)) = header_ok(&file) else { return (OtherStatus::NotAVault, None) };
    let key = Zeroizing::new(derive_key(password, &salt, iterations));
    match decrypt(&file, &key) {
        Ok(Some(data)) => (OtherStatus::Opened, Some(data)),
        Ok(None) => (OtherStatus::WrongPassword, None),
        Err(_) => (OtherStatus::Newer, None),
    }
}

/// Puts other vault bytes (a backup) in place of vault.rmv, with the same safe swap as a save.
pub fn replace_file(bytes: &[u8]) -> Res<()> {
    let _gate = guard(&FILE_GATE);
    write_atomic(&vault_path(), bytes)?;
    // The replaced vault, left as the spare by the write: with its password (the usual one) Open would take the
    // restored file for damaged and put this one back. A copy of it is in old-vaults.
    if let Err(e) = fs::remove_file(with_suffix(&vault_path(), ".bak")) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::note(&format!("Restore: the replaced vault's spare copy couldn't be removed ({}).", kind_name(&e)));
        }
    }
    Ok(())
}

/// A vault taken off this PC's list (the lock screen's picker): its file and spare copies are moved aside as
/// old-vaults\vault-removed-<time>.rmv, never deleted (it is restored like a backup), so nothing brings it back.
/// Returns the file the vault now is.
pub fn set_aside(file: &str) -> Res<PathBuf> {
    let _gate = guard(&FILE_GATE);
    fs::create_dir_all(old_vaults_dir())?;
    let path = data_dir().join(file);
    let stamp = stamp();
    let kept = old_vaults_dir().join(format!("vault-removed-{stamp}.rmv"));
    rename_new(&path, &kept)?;
    for (from, what) in [(with_suffix(&path, ".bak"), "spare"), (with_suffix(&path, ".tmp"), "unsaved")] {
        if from.is_file() {
            // The main file is already moved: a spare copy that won't move is deleted rather than left to be found
            // again at the next start as a vault of its own.
            if let Err(e) = rename_new(&from, &old_vaults_dir().join(format!("vault-removed-{stamp}-{what}.rmv"))) {
                log::write(&e);
                if let Err(e) = fs::remove_file(&from) {
                    log::write(&e);
                }
            }
        }
    }
    Ok(kept)
}

/// DateTime.Now as "yyyyMMdd-HHmmss".
fn stamp() -> String {
    let c = Stamp::now().local();
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", c.year, c.month, c.day, c.hour, c.minute, c.second)
}

fn write_encrypted(plain: &[u8], key: &[u8], salt: &[u8], iterations: i32, path: &Path) -> Res<()> {
    let mut file = vec![0u8; HEADER_SIZE + NONCE_SIZE + TAG_SIZE + plain.len()];
    file[0..4].copy_from_slice(MAGIC);
    file[4..8].copy_from_slice(&iterations.to_le_bytes());
    file[8..8 + SALT_SIZE].copy_from_slice(salt);
    let nonce = random_bytes(NONCE_SIZE);
    file[HEADER_SIZE..HEADER_SIZE + NONCE_SIZE].copy_from_slice(&nonce);
    let body = HEADER_SIZE + NONCE_SIZE + TAG_SIZE;
    file[body..].copy_from_slice(plain);
    let aes = Aes256Gcm::new_from_slice(key).map_err(Fail::other)?;
    let (header, rest) = file.split_at_mut(HEADER_SIZE);
    let tag = aes.encrypt_in_place_detached(Nonce::from_slice(&nonce), header, &mut rest[NONCE_SIZE + TAG_SIZE..]).map_err(Fail::other)?;
    rest[NONCE_SIZE..NONCE_SIZE + TAG_SIZE].copy_from_slice(&tag);
    write_atomic(path, &file)
}

/// Writes to a temp file first so a crash mid-write never leaves a half-written vault.
fn write_atomic(path: &Path, bytes: &[u8]) -> Res<()> {
    use std::os::windows::fs::OpenOptionsExt;
    fs::create_dir_all(data_dir())?;
    let tmp = with_suffix(path, ".tmp");
    {
        // FileMode.Create, FileAccess.Write, FileShare.None.
        let mut fs_ = fs::OpenOptions::new().write(true).create(true).truncate(true).share_mode(0).open(&tmp)?;
        fs_.write_all(bytes)?;
        fs_.sync_all()?; // Flush(flushToDisk: true)
    }
    if path.exists() {
        replace_file_keeping_backup(&tmp, path, &with_suffix(path, ".bak"))?;
    } else {
        fs::rename(&tmp, path)?;
    }
    Ok(())
}

fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// File.Replace(tmp, path, bak, ignoreMetadataErrors: true): ReplaceFileW with metadata errors ignored. (No
/// REPLACEFILE_WRITE_THROUGH: documented as not supported, and .NET doesn't pass it; the .tmp was fsynced already.)
fn replace_file_keeping_backup(replacement: &Path, replaced: &Path, backup: &Path) -> std::io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{REPLACEFILE_IGNORE_MERGE_ERRORS, ReplaceFileW};
    let (replaced_w, replacement_w, backup_w) = (wide(replaced), wide(replacement), wide(backup));
    // SAFETY: three NUL-terminated UTF-16 paths that live across the call; the two reserved pointers are null.
    let ok = unsafe {
        ReplaceFileW(
            replaced_w.as_ptr(),
            replacement_w.as_ptr(),
            backup_w.as_ptr(),
            REPLACEFILE_IGNORE_MERGE_ERRORS,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok == 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }
}

/// A short id from the header's salt (no password needed); None when not a vault. Backups carry it in their
/// name, so vaults sharing a folder don't prune each other's.
pub fn id_of(file: &[u8]) -> Option<String> {
    header_ok(file).map(|(_, salt)| id_of_salt(&salt))
}

fn id_of_salt(salt: &[u8]) -> String {
    // Convert.ToHexString(SHA256(salt))[..8], lower case: the first four bytes.
    Sha256::digest(salt)[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// The same id from a file on disk, reading its header only; None when unreadable or not a vault.
pub fn id_of_path(path: &Path) -> Option<String> {
    // std opens with FileShare.ReadWrite | Delete, as the C# does.
    let mut stream = fs::File::open(path).ok()?;
    let mut header = vec![0u8; HEADER_SIZE + NONCE_SIZE + TAG_SIZE];
    let mut read = 0;
    while read < header.len() {
        match stream.read(&mut header[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
    if read == header.len() { id_of(&header) } else { None }
}
