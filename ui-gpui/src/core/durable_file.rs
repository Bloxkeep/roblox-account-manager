//! Small files written so that a crash or a power cut in the middle leaves the old file or the new one, never an
//! empty one: the settings (prefs.json), backups, the person's own Roblox settings kept while Bloxkeep's are in place.
//! (The vault has its own, which also keeps the previous save: Vault.)
use std::{
    collections::HashMap,
    io::Write,
    path::Path,
    sync::{Arc, LazyLock, Mutex},
};

use super::base::Res;

/// One writer at a time per file: two saves at once (the tick's and a page's) shared the one temp file, and
/// the second threw "being used by another process".
static LOCKS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn lock_of(path: &Path) -> Arc<Mutex<()>> {
    // Path.GetFullPath, compared without regard to case (StringComparer.OrdinalIgnoreCase).
    let full = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()).to_string_lossy().to_lowercase();
    let mut locks = LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    // Backups are named by the time, so each adds one: the ones no writer holds (only this map) go. Clones are taken
    // under this same lock, so none can be taken while they are looked at.
    if locks.len() > 64 {
        locks.retain(|_, l| Arc::strong_count(l) > 1);
    }
    locks.entry(full).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
}

/// Writes a temp file beside it, has it flushed to the disk, then puts it in place of the old one.
pub fn write(path: impl AsRef<Path>, bytes: &[u8]) -> Res<()> {
    let path = path.as_ref();
    let gate = lock_of(path);
    let _held = gate.lock().unwrap_or_else(|e| e.into_inner());
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    {
        // FileMode.Create, FileShare.None: a sync client or a scanner can't open it half written.
        use std::os::windows::fs::OpenOptionsExt;
        let mut fs = std::fs::OpenOptions::new().write(true).create(true).truncate(true).share_mode(0).open(&tmp)?;
        fs.write_all(bytes)?;
        fs.flush()?;
        fs.sync_all()?; // Flush(flushToDisk: true)
    }
    // File.Move(tmp, path, overwrite: true): MoveFileEx with MOVEFILE_REPLACE_EXISTING.
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// The same for text, as UTF-8 without a byte order mark (as File.WriteAllText writes it).
pub fn write_text(path: impl AsRef<Path>, text: &str) -> Res<()> {
    write(path, text.as_bytes())
}
