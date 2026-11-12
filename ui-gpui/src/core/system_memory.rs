//! What Windows has of RAM and of its commit limit, and how far the page files may still grow.
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::Mutex,
};

use windows_sys::Win32::{
    Storage::FileSystem::GetDiskFreeSpaceExW,
    System::SystemInformation::{GetSystemInfo, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO},
};

use super::{
    base::Stamp,
    win_kit::{Hive, RegKey, wide},
};

/// `commit_limit_mb`: Windows' commit limit (RAM plus page files); 0 when not known. Past it allocations fail
/// even with RAM free: Roblox crashes as it opens (Hyperion's "Memory dump captured"; seen on a 32 GB PC with a 4 GB
/// page file).
/// `commit_available_mb`: what of `commit_limit_mb` programs don't hold yet.
/// `page_file_room_mb`: how much the commit limit may still grow with the page files (`page_file_room_mb`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryStatus {
    pub total_mb: i64,
    pub available_mb: i64,
    pub load_percent: i32,
    pub commit_limit_mb: i64,
    pub commit_available_mb: i64,
    pub page_file_room_mb: i64,
}

impl MemoryStatus {
    /// What programs may still take before one fails: what is left of the commit limit, and the room it may still grow.
    pub fn commit_room_mb(&self) -> i64 {
        self.commit_available_mb + self.page_file_room_mb
    }
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQuerySystemInformation(info_class: i32, info: *mut c_void, length: i32, returned: *mut i32) -> i32;
}

const SYSTEM_PAGE_FILE_INFORMATION: i32 = 18;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

const MB: i64 = 1024 * 1024;

pub fn read() -> MemoryStatus {
    // SAFETY: dwLength is set as Windows requires; the struct is plain data.
    let status = unsafe {
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut status) == 0 {
            return MemoryStatus::default();
        }
        status
    };
    // For a 64-bit process the "page file" fields are the system's commit limit and what is left of it; the limit is
    // RAM and the page files' size now together.
    let total = (status.ullTotalPhys / MB as u64) as i64;
    let limit = (status.ullTotalPageFile / MB as u64) as i64;
    let p = paging_files();
    let free = |root: &str| p.drives.get(&root.to_uppercase()).map(|d| d.0).unwrap_or(0);
    let size = |root: &str| p.drives.get(&root.to_uppercase()).map(|d| d.1).unwrap_or(0);
    MemoryStatus {
        total_mb: total,
        available_mb: (status.ullAvailPhys / MB as u64) as i64,
        load_percent: status.dwMemoryLoad as i32,
        commit_limit_mb: limit,
        commit_available_mb: (status.ullAvailPageFile / MB as u64) as i64,
        page_file_room_mb: page_file_room_mb(&p.files, p.made.as_ref(), total, &free, &size),
    }
}

/// Path.GetPathRoot: "c:\" of "c:\pagefile.sys", "" for a relative path.
fn path_root(path: &str) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() >= 2 && chars[1] == ':' {
        if chars.len() >= 3 && (chars[2] == '\\' || chars[2] == '/') {
            return chars[..3].iter().collect();
        }
        return chars[..2].iter().collect();
    }
    if chars.first().is_some_and(|c| *c == '\\' || *c == '/') {
        return chars[..1].iter().collect();
    }
    String::new()
}

/// How much the page files may still grow, in MB: Windows grows them as the commit limit is reached, so a nearly used
/// limit isn't full while that room is left (a PC with a 16–32 GB page file grown to 17.5 GB was wrongly told virtual
/// memory was almost full). Each existing page file counts from its size now up to its largest: the one set, or for a
/// system-managed one ("?:" for every drive) Microsoft's 3 × RAM or 4 GB, whichever is larger, capped at its drive's
/// size ÷ 8 (learn.microsoft.com, page file size for 64-bit Windows); each within its drive's free space less 1 GB.
/// A page file set but never made counts for nothing: on 2026-10-22 the owner's E: one (16–32 GB) didn't exist
/// (BitLocker's PagefileOnOsVolume = 1 makes page files only on the system drive at boot), and counting it kept the
/// warning quiet while 6 Roblox windows filled the commit limit (99.7 %, two crashed on ERROR_COMMITMENT_LIMIT).
///
/// `paging_files`: Windows' PagingFiles: "c:\pagefile.sys 1024 4096" (initial and largest MB); "0 0" or no
/// sizes for system-managed; "?:\pagefile.sys" for system-managed ones on drives Windows picks.
/// `made`: the page files Windows has now, by drive ("C:\"), with their size now in MB (`page_files_now`); None when not
/// known: none counts. Its drives are compared without regard to case.
pub fn page_file_room_mb(paging_files: &[String], made: Option<&Vec<(String, i64)>>, ram_mb: i64, free_mb: &dyn Fn(&str) -> i64, size_mb: &dyn Fn(&str) -> i64) -> i64 {
    let Some(made) = made else {
        return 0;
    };
    // The largest size set for each drive's page file; 0 for one Windows sizes by itself.
    let mut largest: HashMap<String, i64> = HashMap::new();
    let mut any_drive = false;
    for entry in paging_files {
        let parts: Vec<&str> = entry.split(' ').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        if parts[0].starts_with('?') {
            any_drive = true;
        } else {
            let root = path_root(parts[0]);
            if !root.is_empty() {
                let max = if parts.len() >= 3 { parts[2].parse::<i64>().ok().filter(|m| *m > 0).unwrap_or(0) } else { 0 };
                largest.insert(root.to_uppercase(), max);
            }
        }
    }
    let mut room = 0i64;
    for (root, now_mb) in made {
        let key = root.to_uppercase();
        let most = match largest.get(&key) {
            Some(&set) if set > 0 => set,
            Some(_) => (3 * ram_mb).max(4096).min(size_mb(root) / 8),
            None if any_drive => (3 * ram_mb).max(4096).min(size_mb(root) / 8),
            None => continue, // one the setting doesn't name: how large it may grow isn't known
        };
        room += 0.max((most - now_mb).min(free_mb(root) - 1024));
    }
    room
}

/// PagingFiles, the existing page files and their drives' free space and size (by drive, upper case).
#[derive(Clone)]
struct Paging {
    at: Stamp,
    files: Vec<String>,
    made: Option<Vec<(String, i64)>>,
    drives: HashMap<String, (i64, i64)>,
}

static PAGING: Mutex<Option<Paging>> = Mutex::new(None);

/// PagingFiles, the existing page files and their drives' free space and size; read again once a minute.
fn paging_files() -> Paging {
    let mut kept = PAGING.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = kept.as_ref() {
        if Stamp::utc_now().secs_since(p.at) <= 60.0 {
            return p.clone();
        }
    }
    let files = RegKey::open(Hive::LocalMachine, r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management", false)
        .ok()
        .flatten()
        .and_then(|key| key.get_multi_string("PagingFiles"))
        .unwrap_or_default();
    let made = page_files_now();
    let mut drives = HashMap::new();
    for (root, _) in made.iter().flatten() {
        drives.insert(root.to_uppercase(), drive_space(root).unwrap_or((0, 0)));
    }
    let p = Paging { at: Stamp::utc_now(), files, made, drives };
    *kept = Some(p.clone());
    p
}

/// DriveInfo(root): AvailableFreeSpace and TotalSize in MB.
fn drive_space(root: &str) -> Option<(i64, i64)> {
    let r = wide(root);
    let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: plain call with three out values on the stack.
    let ok = unsafe { GetDiskFreeSpaceExW(r.as_ptr(), &mut available, &mut total, &mut free) };
    if ok == 0 { None } else { Some(((available / MB as u64) as i64, (total / MB as u64) as i64)) }
}

/// Environment.SystemPageSize.
fn page_size() -> i64 {
    // SAFETY: SYSTEM_INFO is plain data filled by Windows.
    unsafe {
        let mut info: SYSTEM_INFO = std::mem::zeroed();
        GetSystemInfo(&mut info);
        info.dwPageSize as i64
    }
}

/// The page files Windows has now by drive ("C:\"), size in MB, from the kernel's list (what Task Manager counts);
/// None when unreadable. One set in PagingFiles may not exist: Windows makes them as it starts. In the kernel's order,
/// one entry per drive (compared without regard to case).
pub fn page_files_now() -> Option<Vec<(String, i64)>> {
    let mut length = 4096usize;
    while length <= 1 << 20 {
        let mut buffer = vec![0u64; length / 8];
        let mut returned = 0i32;
        // SAFETY: the buffer holds `length` bytes, aligned for the structs; entries are read inside what was returned.
        let status = unsafe { NtQuerySystemInformation(SYSTEM_PAGE_FILE_INFORMATION, buffer.as_mut_ptr() as *mut c_void, length as i32, &mut returned) };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            length *= 4;
            continue;
        }
        if status != 0 {
            return None;
        }
        let mut files: Vec<(String, i64)> = Vec::new();
        // SYSTEM_PAGEFILE_INFORMATION: NextEntryOffset, TotalSize (in pages), TotalInUse, PeakUsage (ULONGs), then
        // the file's name as a UNICODE_STRING (Length, MaximumLength, the buffer pointer aligned after them):
        // "\??\C:\pagefile.sys".
        let base = buffer.as_ptr() as *const u8;
        let mut offset = 0usize;
        while returned > 0 {
            // SAFETY: offsets come from the kernel's own NextEntryOffset inside the buffer it filled.
            let (next, pages, name) = unsafe {
                let entry = base.add(offset);
                let next = std::ptr::read_unaligned(entry as *const i32);
                let pages = std::ptr::read_unaligned(entry.add(4) as *const u32) as i64;
                let bytes = std::ptr::read_unaligned(entry.add(16) as *const u16) as usize;
                let text = std::ptr::read_unaligned(entry.add(16 + std::mem::size_of::<usize>()) as *const *const u16);
                let name = if text.is_null() { String::new() } else { String::from_utf16_lossy(std::slice::from_raw_parts(text, bytes / 2)) };
                (next, pages, name)
            };
            if let Some(colon) = name.find(":\\") {
                if colon >= 1 {
                    let root: String = name[colon - 1..].chars().take(3).collect();
                    let add = pages * page_size() / MB;
                    match files.iter_mut().find(|f| f.0.eq_ignore_ascii_case(&root)) {
                        Some(f) => f.1 += add,
                        None => files.push((root, add)),
                    }
                }
            }
            if next <= 0 {
                break;
            }
            offset += next as usize;
        }
        return Some(files);
    }
    None
}
