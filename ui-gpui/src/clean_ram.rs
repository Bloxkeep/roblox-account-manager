//! Clean RAM, after Mem Reduct (henrypp/memreduct, `_app_memoryclean`): one Windows call for the whole PC per area,
//! so every program's working set is pushed out together with Windows' caches. It needs an administrator: a press
//! starts Bloxkeep.exe again as one (`ARG` and the areas, Windows asks first), that copy does the calls and leaves, and
//! this one tells how much RAM in use went down meanwhile.
//!
//! The areas are Mem Reduct's, in its order and with its defaults (`DEFAULT_AREAS`): every working set, the system file
//! cache, the modified page list, the whole standby list, the low-priority standby list, the volumes' write caches, the
//! registry's cache and identical pages combined. The modified list and the whole standby list are off unless picked,
//! as in Mem Reduct: they can freeze a PC for a moment.
//!
//! Auto clean (above a RAM level, or every so many minutes; Mem Reduct does both, being an administrator all along):
//! turned on, an administrator copy (`SETUP_ARG`, Windows asks once) copies Bloxkeep.exe to Program Files and adds a
//! Task Scheduler task that runs that copy with the highest rights. A copy where only an administrator can change it:
//! the task then can't be used to run something else as one (Bloxkeep.exe itself sits in the person's own AppData).
//! From then on Bloxkeep starts the task (a person may start their own task) and Windows asks no more, for the button
//! too. The task can't be given arguments, so what it is to do is left in a file (`mark("ask")`: the areas, or
//! "remove"), and the copy only reads it; it says it is done through a named event, never writing a file. Both auto
//! cleans turned off, the task removes itself and its copy.
use std::{
    ffi::c_void,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_CANCELLED, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LUID, WAIT_OBJECT_0},
    Security::{
        AdjustTokenPrivileges, DuplicateTokenEx, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, SecurityImpersonation, TOKEN_ADJUST_PRIVILEGES, TOKEN_ALL_ACCESS, TOKEN_ASSIGN_PRIMARY,
        TOKEN_DUPLICATE, TOKEN_PRIVILEGES, TOKEN_QUERY, TokenPrimary,
    },
    Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, FlushFileBuffers, GetDriveTypeW, GetLogicalDrives, OPEN_EXISTING},
    System::{
        LibraryLoader::{GetModuleHandleW, GetProcAddress},
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::{
            CreateEventW, CreateProcessAsUserW, DeleteProcThreadAttributeList, EVENT_MODIFY_STATE, EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess, GetExitCodeProcess,
            InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST, OpenEventW, OpenProcess, OpenProcessToken, PROC_THREAD_ATTRIBUTE_PARENT_PROCESS, PROCESS_CREATE_PROCESS,
            PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW, STARTUPINFOEXW, SetEvent, UpdateProcThreadAttribute, WaitForSingleObject,
        },
    },
    UI::{
        Shell::{SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
        WindowsAndMessaging::SW_HIDE,
    },
};

/// The argument that makes Bloxkeep.exe the cleaning copy (the areas after it; none: read from `mark("ask")`).
pub const ARG: &str = "--clean-ram";
/// The argument that makes it the copy adding auto clean's task.
pub const SETUP_ARG: &str = "--clean-ram-setup";
/// Mem Reduct's page, linked beside the button.
pub const MEM_REDUCT: &str = "https://github.com/henrypp/memreduct";

/// The areas, one bit each (Settings keeps the picked ones as `cleanAreas`).
pub const WORKING_SETS: u32 = 1;
pub const FILE_CACHE: u32 = 1 << 1;
pub const MODIFIED_LIST: u32 = 1 << 2;
pub const STANDBY_ALL: u32 = 1 << 3;
pub const STANDBY_LOW: u32 = 1 << 4;
pub const VOLUMES: u32 = 1 << 5;
pub const REGISTRY: u32 = 1 << 6;
pub const COMBINE: u32 = 1 << 7;
pub const ALL_AREAS: u32 = 0xFF;
/// Mem Reduct's defaults: all but the modified list and the whole standby list.
pub const DEFAULT_AREAS: u32 = ALL_AREAS & !MODIFIED_LIST & !STANDBY_ALL;

/// Auto clean's recommended level on this PC (`total_mb` of RAM, `each_mb` what one of this person's Roblox windows
/// takes, capacity.rs): the highest choice at which RAM still has room for two more of them, a window starting or one
/// loading its next place. Under it a clean frees nothing that lasts: the windows read back what was taken within
/// minutes (cleaned every 5 minutes at under 40 % in use, 1 to 4 GB "freed" each time and back by the next).
pub fn best_at(total_mb: f64, each_mb: f64) -> i64 {
    let level = 100. * (1. - 2. * each_mb / total_mb.max(1.));
    [95, 90, 85, 80].into_iter().find(|n| *n as f64 <= level).unwrap_or(80)
}

/// The task's name in Task Scheduler (its root folder, so nothing is left behind once it is deleted).
const TASK: &str = "Bloxkeep Clean RAM";
/// The event the copy sets when it is done (the person's own session: the task runs in it).
const DONE_EVENT: &str = "Local\\Bloxkeep.CleanRam.Done";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const DRIVE_FIXED: u32 = 3;

const SYSTEM_MEMORY_LIST_INFORMATION: i32 = 80;
const SYSTEM_FILE_CACHE_INFORMATION_EX: i32 = 81;
const SYSTEM_COMBINE_PHYSICAL_MEMORY_INFORMATION: i32 = 130;
const SYSTEM_REGISTRY_RECONCILIATION_INFORMATION: i32 = 155;
const MEMORY_EMPTY_WORKING_SETS: i32 = 2;
const MEMORY_FLUSH_MODIFIED_LIST: i32 = 3;
const MEMORY_PURGE_STANDBY_LIST: i32 = 4;
const MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST: i32 = 5;

/// The copy's exit code, past the areas' own bits (each set when that area failed).
const NO_PRIVILEGES: u32 = 1 << 8;
const NO_NT_CALL: u32 = 1 << 9;
const NOT_REMOVED: u32 = 1 << 10;

#[repr(C)]
struct FileCacheInfo {
    current_size: usize,
    peak_size: usize,
    page_fault_count: u32,
    minimum_working_set: usize,
    maximum_working_set: usize,
    current_size_including_transition_in_pages: usize,
    peak_size_including_transition_in_pages: usize,
    transition_re_purpose_count: u32,
    flags: u32,
}

#[repr(C)]
struct CombineInfoEx {
    handle: HANDLE,
    pages_combined: usize,
    flags: u32,
}

type NtSetSystemInformation = unsafe extern "system" fn(i32, *mut c_void, u32) -> i32;

// ───────────────────────── the administrator copies ─────────────────────────

/// The cleaning copy (started with `ARG`, as an administrator, by the button or by the task): cleans the areas asked
/// for, or removes the task, tells a waiting Bloxkeep it is done and answers what failed (its exit code).
pub fn run_as_admin() -> i32 {
    let order = match std::env::args().nth(2) {
        Some(order) => order,
        None => {
            // Started by the task: its parent is Task Scheduler's service, so Task Manager showed it on its own, apart
            // from Bloxkeep. It runs again under the Bloxkeep that asked (the same rights, the order as an argument)
            // and leaves; done here only when that can't be.
            let order = std::fs::read_to_string(mark("ask")).unwrap_or_default();
            if again_under_bloxkeep(order.trim()) {
                return 0;
            }
            order
        }
    };
    let order = order.trim();
    let code = if order == "remove" { remove_task() } else { clean_areas(order.parse::<u32>().map_or(DEFAULT_AREAS, |a| a & ALL_AREAS)) };
    let name = wide(DONE_EVENT);
    unsafe {
        let done = OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr());
        if !done.is_null() {
            SetEvent(done);
            CloseHandle(done);
        }
    }
    code as i32
}

/// This copy started again, with `order`, as a child of the Bloxkeep whose pid is in `mark("by")` (checked to be a
/// Bloxkeep.exe): with this copy's own rights (its token, duplicated), only its parent changed, as Windows' own
/// "run as administrator" does it. Whether it started.
fn again_under_bloxkeep(order: &str) -> bool {
    let Some(pid) = std::fs::read_to_string(mark("by")).ok().and_then(|s| s.trim().parse::<u32>().ok()) else { return false };
    let Ok(exe) = std::env::current_exe() else { return false };
    if order.is_empty() || !order.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return false;
    }
    unsafe {
        let parent = OpenProcess(PROCESS_CREATE_PROCESS | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if parent.is_null() {
            return false;
        }
        let started = is_bloxkeep(parent) && start_under(parent, &exe, order);
        CloseHandle(parent);
        started
    }
}

/// Whether process `p` runs a Bloxkeep.exe.
unsafe fn is_bloxkeep(p: HANDLE) -> bool {
    let mut buf = [0u16; 1024];
    let mut n = buf.len() as u32;
    if unsafe { QueryFullProcessImageNameW(p, 0, buf.as_mut_ptr(), &mut n) } == 0 {
        return false;
    }
    String::from_utf16_lossy(&buf[..n as usize]).rsplit('\\').next().is_some_and(|f| f.eq_ignore_ascii_case("Bloxkeep.exe"))
}

/// `exe` started with `ARG order`, a child of `parent`, with this process's own token.
unsafe fn start_under(parent: HANDLE, exe: &std::path::Path, order: &str) -> bool {
    unsafe {
        let mut own: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY, &mut own) == 0 {
            return false;
        }
        let mut token: HANDLE = std::ptr::null_mut();
        let made = DuplicateTokenEx(own, TOKEN_ALL_ACCESS, std::ptr::null(), SecurityImpersonation, TokenPrimary, &mut token) != 0;
        CloseHandle(own);
        if !made {
            return false;
        }
        let mut size = 0usize;
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
        // (usize-aligned room for the list.)
        let mut room = vec![0usize; size.div_ceil(size_of::<usize>()).max(1)];
        let list: LPPROC_THREAD_ATTRIBUTE_LIST = room.as_mut_ptr().cast();
        let mut started = false;
        if InitializeProcThreadAttributeList(list, 1, 0, &mut size) != 0 {
            let parent_slot = parent;
            if UpdateProcThreadAttribute(list, 0, PROC_THREAD_ATTRIBUTE_PARENT_PROCESS as usize, (&raw const parent_slot).cast(), size_of::<HANDLE>(), std::ptr::null_mut(), std::ptr::null()) != 0 {
                let mut si: STARTUPINFOEXW = std::mem::zeroed();
                si.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
                si.lpAttributeList = list;
                let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
                let app: Vec<u16> = {
                    use std::os::windows::ffi::OsStrExt;
                    exe.as_os_str().encode_wide().chain([0]).collect()
                };
                let mut line = wide(&format!("\"{}\" {ARG} {order}", exe.display()));
                started = CreateProcessAsUserW(
                    token,
                    app.as_ptr(),
                    line.as_mut_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
                    std::ptr::null(),
                    std::ptr::null(),
                    &si.StartupInfo,
                    &mut pi,
                ) != 0;
                if started {
                    CloseHandle(pi.hThread);
                    CloseHandle(pi.hProcess);
                }
            }
            DeleteProcThreadAttributeList(list);
        }
        CloseHandle(token);
        started
    }
}

fn clean_areas(areas: u32) -> u32 {
    let mut failed = 0;
    if !privilege("SeProfileSingleProcessPrivilege") || !privilege("SeIncreaseQuotaPrivilege") {
        failed |= NO_PRIVILEGES;
    }
    let Some(set) = nt_set() else { return failed | NO_NT_CALL };
    // (The sizes checked against Windows 2026-11-24: one short gives 0xc0000004, a wrong size.)
    let call = |class: i32, data: *mut c_void, size: usize| unsafe { set(class, data, size as u32) } >= 0;
    let list = |command: i32| {
        let mut command = command;
        call(SYSTEM_MEMORY_LIST_INFORMATION, (&raw mut command).cast(), 4)
    };
    let mut step = |area: u32, done: &mut dyn FnMut() -> bool| {
        if areas & area != 0 && !done() {
            failed |= area;
        }
    };
    step(WORKING_SETS, &mut || list(MEMORY_EMPTY_WORKING_SETS));
    step(FILE_CACHE, &mut || {
        let mut cache: FileCacheInfo = unsafe { std::mem::zeroed() };
        cache.minimum_working_set = usize::MAX;
        cache.maximum_working_set = usize::MAX;
        call(SYSTEM_FILE_CACHE_INFORMATION_EX, (&raw mut cache).cast(), size_of::<FileCacheInfo>())
    });
    step(MODIFIED_LIST, &mut || list(MEMORY_FLUSH_MODIFIED_LIST));
    step(STANDBY_ALL, &mut || list(MEMORY_PURGE_STANDBY_LIST));
    step(STANDBY_LOW, &mut || list(MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST));
    step(VOLUMES, &mut flush_volumes);
    step(REGISTRY, &mut || call(SYSTEM_REGISTRY_RECONCILIATION_INFORMATION, std::ptr::null_mut(), 0));
    step(COMBINE, &mut || {
        let mut combine: CombineInfoEx = unsafe { std::mem::zeroed() };
        call(SYSTEM_COMBINE_PHYSICAL_MEMORY_INFORMATION, (&raw mut combine).cast(), size_of::<CombineInfoEx>())
    });
    failed
}

/// Every fixed drive's write cache written to the disk (Mem Reduct's "modified file cache"): pages waiting to be
/// written can then be let go. True when every drive took it.
fn flush_volumes() -> bool {
    let drives = unsafe { GetLogicalDrives() };
    let mut all = true;
    for i in 0..26u32 {
        if drives & (1 << i) == 0 {
            continue;
        }
        let letter = char::from(b'A' + i as u8);
        if unsafe { GetDriveTypeW(wide(&format!("{letter}:\\")).as_ptr()) } != DRIVE_FIXED {
            continue;
        }
        let path = wide(&format!("\\\\.\\{letter}:"));
        unsafe {
            let h = CreateFileW(path.as_ptr(), GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut());
            if h == INVALID_HANDLE_VALUE {
                all = false;
                continue;
            }
            all &= FlushFileBuffers(h) != 0;
            CloseHandle(h);
        }
    }
    all
}

/// The copy adding the task (started with `SETUP_ARG`, as an administrator): Bloxkeep.exe copied to Program Files, the
/// task made to run that copy. 0 when both are done.
pub fn set_up_task() -> i32 {
    let Ok(exe) = std::env::current_exe() else { return 1 };
    let dir = program_dir();
    let copy = dir.join("BloxkeepCleanRam.exe");
    if std::fs::create_dir_all(&dir).is_err() || std::fs::copy(&exe, &copy).is_err() {
        return 2;
    }
    let command = copy.to_string_lossy().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    // No trigger: only Bloxkeep starts it. Two minutes at most; a start while one runs is let go.
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Bloxkeep's Clean RAM. Started by Bloxkeep, so Windows asks for administrator rights only once. Turning auto clean off in Bloxkeep deletes it.</Description></RegistrationInfo>
  <Principals><Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <ExecutionTimeLimit>PT2M</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author"><Exec><Command>{command}</Command><Arguments>{ARG}</Arguments></Exec></Actions>
</Task>
"#
    );
    // schtasks reads the file as UTF-16 (its byte order mark first). Written where only an administrator writes.
    let bytes: Vec<u8> = [0xFEFFu16].into_iter().chain(xml.encode_utf16()).flat_map(u16::to_le_bytes).collect();
    let file = dir.join("task.xml");
    if std::fs::write(&file, bytes).is_err() {
        return 3;
    }
    let added = schtasks(&["/create", "/tn", TASK, "/xml", &file.to_string_lossy(), "/f"]);
    let _ = std::fs::remove_file(&file);
    if added { 0 } else { 4 }
}

/// The copy asked to remove the task: the task deleted, and its folder once this copy has left (it runs from it).
fn remove_task() -> u32 {
    let gone = schtasks(&["/delete", "/tn", TASK, "/f"]);
    let dir = program_dir();
    let from_there = std::env::current_exe().is_ok_and(|e| e.starts_with(&dir));
    if from_there {
        let _ = Command::new(system32("cmd.exe"))
            .raw_arg(format!("/c ping -n 3 127.0.0.1 >nul & rmdir /s /q \"{}\"", dir.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    } else {
        let _ = std::fs::remove_dir_all(&dir);
    }
    if gone { 0 } else { NOT_REMOVED }
}

/// Where the task's copy lives: Program Files, which only an administrator can change.
fn program_dir() -> PathBuf {
    std::env::var_os("ProgramFiles").map_or_else(|| PathBuf::from(r"C:\Program Files"), PathBuf::from).join("Bloxkeep Clean RAM")
}

fn system32(exe: &str) -> PathBuf {
    std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from).join("System32").join(exe)
}

/// Windows' schtasks.exe, hidden; whether it went through.
fn schtasks(args: &[&str]) -> bool {
    Command::new(system32("schtasks.exe"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|s| s.success())
}

fn nt_set() -> Option<NtSetSystemInformation> {
    let ntdll = wide("ntdll.dll");
    unsafe {
        let module = GetModuleHandleW(ntdll.as_ptr());
        if module.is_null() {
            return None;
        }
        let f = GetProcAddress(module, c"NtSetSystemInformation".as_ptr().cast())?;
        Some(std::mem::transmute::<unsafe extern "system" fn() -> isize, NtSetSystemInformation>(f))
    }
}

fn privilege(name: &str) -> bool {
    let wide = wide(name);
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut luid = LUID { LowPart: 0, HighPart: 0 };
        let mut ok = LookupPrivilegeValueW(std::ptr::null(), wide.as_ptr(), &mut luid) != 0;
        if ok {
            let tp = TOKEN_PRIVILEGES { PrivilegeCount: 1, Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }] };
            ok = AdjustTokenPrivileges(token, 0, &tp, 0, std::ptr::null_mut(), std::ptr::null_mut()) != 0 && GetLastError() == 0;
        }
        CloseHandle(token);
        ok
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

// ───────────────────────── Bloxkeep's side ─────────────────────────

/// A file both copies see: the same person's temp folder, administrator or not. Only Bloxkeep writes it.
fn mark(what: &str) -> PathBuf {
    std::env::temp_dir().join(format!("bloxkeep-clean-ram.{what}"))
}

/// The PC's RAM in use, in bytes.
fn in_use() -> u64 {
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
        return 0;
    }
    m.ullTotalPhys.saturating_sub(m.ullAvailPhys)
}

/// How a clean went.
pub enum Cleaned {
    /// RAM in use went down this many bytes (0 when it didn't).
    Freed(u64),
    /// The person said no to Windows' question.
    Refused,
    /// It couldn't be done (why, for the log).
    Failed(String),
    /// Auto clean's task isn't there (deleted in Task Scheduler, or never added).
    Gone,
}

/// What only a copy that runs its clean under Bloxkeep holds (`again_under_bloxkeep`): a copy without it is older.
#[used]
static COPY_MARK: [u8; 31] = *b"bloxkeep-clean-ram-copy/under-1";

/// Whether the task's copy in Program Files is one made before `COPY_MARK` (read off the UI thread).
fn copy_is_old() -> bool {
    std::fs::read(program_dir().join("BloxkeepCleanRam.exe")).is_ok_and(|b| !b.windows(COPY_MARK.len()).any(|w| w == COPY_MARK))
}

/// Whether auto clean's task is there.
fn task_there() -> bool {
    schtasks(&["/query", "/tn", TASK])
}

/// Starts the task with an order (the areas, or "remove") and waits for its copy (run off the UI thread): whether it
/// said it was done in time.
fn run_task(order: &str) -> Result<(), Cleaned> {
    if !task_there() {
        return Err(Cleaned::Gone);
    }
    if std::fs::write(mark("ask"), order).is_err() {
        return Err(Cleaned::Failed("the order couldn't be written".into()));
    }
    // (Which Bloxkeep asked: the task's copy runs its clean under it, in Task Manager with Bloxkeep's own.)
    let _ = std::fs::write(mark("by"), std::process::id().to_string());
    let name = wide(DONE_EVENT);
    let done = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) };
    if done.is_null() {
        return Err(Cleaned::Failed("no event".into()));
    }
    let answered = schtasks(&["/run", "/tn", TASK]) && unsafe { WaitForSingleObject(done, 60_000) } == WAIT_OBJECT_0;
    unsafe { CloseHandle(done) };
    let _ = std::fs::remove_file(mark("ask"));
    if answered { Ok(()) } else { Err(Cleaned::Failed("the task gave no answer".into())) }
}

/// A clean through the task (no question): `Gone` when auto clean is off.
fn clean_by_task(areas: u32) -> Cleaned {
    let before = in_use();
    match run_task(&areas.to_string()) {
        Ok(()) => Cleaned::Freed(before.saturating_sub(in_use())),
        Err(e) => e,
    }
}

/// The button's clean (run off the UI thread): through the task when auto clean is on, else a copy started as an
/// administrator (Windows' question first).
fn clean(areas: u32) -> Cleaned {
    if task_there() && copy_is_old() {
        // (Refused or failed: the old copy still cleans.)
        let _ = elevated(SETUP_ARG);
    }
    match clean_by_task(areas) {
        Cleaned::Gone => {}
        got => return got,
    }
    let before = in_use();
    match elevated(&format!("{ARG} {areas}")) {
        Ok(code) => {
            if code != 0 {
                eprintln!("Clean RAM: some areas failed ({code:#x})");
            }
            if code & NO_NT_CALL != 0 {
                return Cleaned::Failed(format!("{code:#x}"));
            }
            Cleaned::Freed(before.saturating_sub(in_use()))
        }
        Err(e) => e,
    }
}

/// Bloxkeep.exe again as an administrator with `params`, waited for: its exit code, or why not.
fn elevated(params: &str) -> Result<u32, Cleaned> {
    let Ok(exe) = std::env::current_exe() else { return Err(Cleaned::Failed("no exe path".into())) };
    let file: Vec<u16> = {
        use std::os::windows::ffi::OsStrExt;
        exe.as_os_str().encode_wide().chain([0]).collect()
    };
    let verb = wide("runas");
    let params = wide(params);
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_HIDE;
    unsafe {
        if ShellExecuteExW(&mut info) == 0 {
            let e = GetLastError();
            return Err(if e == ERROR_CANCELLED { Cleaned::Refused } else { Cleaned::Failed(format!("start: {e}")) });
        }
        if info.hProcess.is_null() {
            return Err(Cleaned::Failed("no process".into()));
        }
        WaitForSingleObject(info.hProcess, 60_000);
        let mut code = 0u32;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        Ok(code)
    }
}

/// A clean is under way.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Whether a clean is under way (its buttons wait).
pub fn running() -> bool {
    RUNNING.load(Relaxed)
}

/// The areas picked in Settings (the app's state), Mem Reduct's defaults before any pick.
pub fn areas(state: &serde_json::Value) -> u32 {
    state.pointer("/settings/cleanAreas").and_then(serde_json::Value::as_u64).map_or(DEFAULT_AREAS, |a| a as u32 & ALL_AREAS)
}

/// Auto clean's run (the core's memory watch, off the UI thread): through the task only, never a question.
pub fn auto_clean(areas: u32) -> Cleaned {
    if RUNNING.swap(true, Relaxed) {
        return Cleaned::Failed("a clean is under way".into());
    }
    let got = clean_by_task(areas);
    RUNNING.store(false, Relaxed);
    got
}

/// The Clean RAM button's press: Windows asks (unless auto clean is on), then a toast says how much RAM it freed.
pub fn press(r: &Root, cx: &mut Context<Root>) {
    if RUNNING.swap(true, Relaxed) {
        return;
    }
    cx.notify();
    let areas = areas(&r.state);
    let before = meters().0.part();
    let got = crate::threads::call(move || clean(areas));
    cx.spawn(async move |this, cx| {
        let got = got.await.ok().and_then(|g| g.ok());
        RUNNING.store(false, Relaxed);
        let _ = this.update(cx, |r, cx| {
            match got {
                // With the panel open it shows there (the figure, the bar's old place); else a toast.
                Some(Cleaned::Freed(bytes)) if PANEL.with(std::cell::Cell::get) => FREED.with(|f| f.set(Some((bytes, before)))),
                Some(Cleaned::Freed(bytes)) => {
                    let mb = (bytes / (1 << 20)) as f64;
                    let text = r.i18n.tv("ram.cleaned", &[("n", &crate::lang::cur().mb(mb))]);
                    r.ok(text);
                }
                Some(Cleaned::Refused) => {}
                Some(Cleaned::Failed(why)) => {
                    eprintln!("Clean RAM failed: {why}");
                    r.say(r.tr("ram.cleanFailed"), "bad");
                }
                _ => r.say(r.tr("ram.cleanFailed"), "bad"),
            }
            cx.notify();
        });
    })
    .detach();
}

/// Auto clean picked in Settings: above `at` % or every `every` minutes (one of them; both 0: off). On: the task is
/// added first (Windows asks) and the choice is kept only once it is there. Off: kept at once, and the task removes
/// itself and its copy.
pub fn choose_auto(r: &mut Root, cx: &mut Context<Root>, at: i64, every: i64) {
    let keep = move |r: &mut Root, cx: &mut Context<Root>| {
        r.ask("setSettings", serde_json::json!({ "autoCleanAt": at, "autoCleanEvery": every }), cx, |_, _| {});
    };
    if at <= 0 && every <= 0 {
        keep(r, cx);
        crate::threads::run(|| {
            if let Err(Cleaned::Failed(why)) = run_task("remove") {
                eprintln!("Clean RAM: the task wasn't removed ({why})");
            }
        });
        return;
    }
    let got = crate::threads::call(|| if task_there() { Ok(0) } else { elevated(SETUP_ARG) });
    cx.spawn(async move |this, cx| {
        let got = got.await.ok().and_then(|g| g.ok());
        let _ = this.update(cx, |r, cx| {
            match got {
                Some(Ok(0)) => keep(r, cx),
                Some(Err(Cleaned::Refused)) => r.say(r.tr("ram.autoNoAdmin"), "bad"),
                other => {
                    match other {
                        Some(Err(Cleaned::Failed(why))) => eprintln!("Auto clean RAM couldn't be turned on: {why}"),
                        Some(Ok(code)) => eprintln!("Auto clean RAM couldn't be turned on: setup answered {code}"),
                        _ => {}
                    }
                    r.say(r.tr("ram.autoFailed"), "bad");
                }
            }
            cx.notify();
        });
    })
    .detach();
}

// ───────────────────────── the panel ─────────────────────────
// Mem Reduct's window in Bloxkeep's clothes: its three memories (physical, virtual, the system cache), each with how
// full it is, what is in use, free and in all; the clean button at the bottom, auto clean and the areas under the
// figures. RAM leads: it is what a clean is for, so it gets the big figure; the other two sit under it, smaller.

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::{prelude::*, *};

use crate::{kit::*, shell::Root, theme::*};

thread_local! {
    /// The panel is open (its figures are read again every second while it is).
    static PANEL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The opening whose once-a-second loop runs.
    static TICK: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// The last clean from the panel: the bytes it freed, and how full RAM was before (a mark on the bar).
    static FREED: std::cell::Cell<Option<(u64, f32)>> = const { std::cell::Cell::new(None) };
    /// The areas are shown (folded to a count, as Mem Reduct keeps them on a page of their own).
    static AREAS_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// One memory's figures, in bytes.
#[derive(Clone, Copy)]
struct Meter {
    used: u64,
    total: u64,
}

impl Meter {
    fn part(self) -> f32 {
        if self.total == 0 { 0. } else { (self.used as f64 / self.total as f64) as f32 }
    }
}

type NtQuerySystemInformation = unsafe extern "system" fn(i32, *mut c_void, u32, *mut u32) -> i32;
const SYSTEM_FILE_CACHE_INFORMATION: i32 = 21;

/// Physical memory, virtual memory (what is committed against RAM and the page file together) and the system cache
/// (its size now against the largest it has been), as Mem Reduct reads them.
fn meters() -> (Meter, Meter, Option<Meter>) {
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
        return (Meter { used: 0, total: 0 }, Meter { used: 0, total: 0 }, None);
    }
    let physical = Meter { used: m.ullTotalPhys.saturating_sub(m.ullAvailPhys), total: m.ullTotalPhys };
    let virt = Meter { used: m.ullTotalPageFile.saturating_sub(m.ullAvailPageFile), total: m.ullTotalPageFile };
    let cache = (|| {
        let ntdll = wide("ntdll.dll");
        unsafe {
            let module = GetModuleHandleW(ntdll.as_ptr());
            if module.is_null() {
                return None;
            }
            let f = GetProcAddress(module, c"NtQuerySystemInformation".as_ptr().cast())?;
            let query = std::mem::transmute::<unsafe extern "system" fn() -> isize, NtQuerySystemInformation>(f);
            let mut info: FileCacheInfo = std::mem::zeroed();
            let mut got = 0u32;
            if query(SYSTEM_FILE_CACHE_INFORMATION, (&raw mut info).cast(), size_of::<FileCacheInfo>() as u32, &mut got) < 0 || info.peak_size == 0 {
                return None;
            }
            Some(Meter { used: info.current_size as u64, total: info.peak_size as u64 })
        }
    })();
    (physical, virt, cache)
}

fn size(bytes: u64) -> String {
    crate::lang::cur().mb(bytes as f64 / (1u64 << 20) as f64)
}

fn percent(part: f32) -> String {
    format!("{}%", (part * 100.).round() as i64)
}

/// Opens the panel (Settings' Clean RAM, the RAM pill).
pub fn open(_r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    // Open already: nothing. (The mark alone isn't trusted: a way of closing that missed it would keep it shut for good.)
    if PANEL.with(std::cell::Cell::get) && window.has_active_dialog(cx) {
        return;
    }
    PANEL.with(|p| p.set(true));
    FREED.with(|f| f.set(None));
    // The figures again every second while it is open; one loop at a time (each opening starts its own, the one before
    // ends).
    let mine = TICK.with(|t| {
        t.set(t.get().wrapping_add(1));
        t.get()
    });
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
            if !PANEL.with(std::cell::Cell::get) || TICK.with(std::cell::Cell::get) != mine || this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }
    })
    .detach();
    let this = cx.weak_entity();
    window.open_dialog(cx, move |dialog, _window, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        let r = root.read(cx);
        let title = r.tr("ram.clean");
        let close = r.tr("common.close");
        let body = body(r, &this, crate::pages::easy_setup::each_mb(r, cx));
        let foot = foot(r, &this);
        // (Closed by its ×, the dialog's on_close isn't called: close_dialog skips it, so both say it is closed.)
        framed(dialog, title, close, |_, w, cx| {
            PANEL.with(|p| p.set(false));
            w.close_dialog(cx);
        })
            .w(px(500.))
            .centered()
            .entrance(DialogEntrance::FadeSlide)
            .on_close(|_, _, _| PANEL.with(|p| p.set(false)))
            .child(dialog_body(body))
            .footer(foot)
    });
}

/// BLOXKEEP_SHOT_ASK=ram: the panel (=BLOXKEEP_SHOT_RAM_AREAS: its areas open; BLOXKEEP_SHOT_RAM_FREED=MB: as after a
/// clean that freed that much).
#[cfg(feature = "shot")]
pub fn shot_open(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if std::env::var("BLOXKEEP_SHOT_RAM_AREAS").is_ok() {
        AREAS_OPEN.with(|o| o.set(true));
    }
    open(r, window, cx);
    if let Ok(mb) = std::env::var("BLOXKEEP_SHOT_RAM_FREED").map(|v| v.parse::<u64>().unwrap_or(0)) {
        let (ram, _, _) = meters();
        let bytes = mb << 20;
        FREED.with(|f| f.set(Some((bytes, (ram.part() + bytes as f32 / ram.total.max(1) as f32).min(1.)))));
    }
}

/// The figures, auto clean and the areas.
fn body(r: &Root, this: &WeakEntity<Root>, each_mb: f64) -> Div {
    let (ram, virt, cache) = meters();
    let freed = FREED.with(std::cell::Cell::get);

    // RAM: the big figure, the bar, its three facts; after a clean, what it freed and where the bar stood before.
    let fact = |label: String, value: String| {
        div()
            .flex()
            .flex_col()
            .items_end()
            .gap(px(2.))
            .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(label))
            .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).font_features(tnum()).child(value))
    };
    let mut big = div()
        .flex()
        .items_end()
        .gap(px(10.))
        .child(div().text_size(px(40.)).line_height(px(44.)).font_weight(FontWeight::BOLD).text_color(c(TEXT)).font_features(tnum()).child(percent(ram.part())));
    if let Some((bytes, _)) = freed {
        big = big.child(
            div()
                .mb(px(7.))
                .px(px(8.))
                .h(px(22.))
                .flex()
                .items_center()
                .rounded_full()
                .bg(ca(OK, 0.14))
                .text_size(px(T_S))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(OK))
                .font_features(tnum())
                .child(r.i18n.tv("ram.freedPill", &[("n", &size(bytes))])),
        );
    }
    let mut bar_box = div().relative().w_full().child(bar("ram-p", ram.part(), None).w_full().h(px(8.)));
    if let Some((_, before)) = freed {
        bar_box = bar_box.child(div().absolute().top(px(-3.)).h(px(14.)).w(px(2.)).rounded_full().left(relative(before.clamp(0., 1.))).bg(ca(TEXT, 0.55)));
    }
    let hero = div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .gap(px(16.))
                .child(div().flex().flex_col().gap(px(2.)).child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).child(r.tr("ram.physical"))).child(big))
                .child(
                    div()
                        .flex()
                        .gap(px(20.))
                        .pb(px(6.))
                        .child(fact(r.tr("ram.used"), size(ram.used)))
                        .child(fact(r.tr("ram.free"), size(ram.total.saturating_sub(ram.used))))
                        .child(fact(r.tr("ram.total"), size(ram.total))),
                ),
        )
        .child(bar_box);

    // The other two: a line each, the bar under it.
    let small = |id: &'static str, name: String, m: Meter| {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(T_M))
                    .child(div().flex_1().min_w_0().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).child(name))
                    .child(div().text_color(c(MUTED)).font_features(tnum()).child(format!("{} / {}", size(m.used), size(m.total))))
                    .child(div().w(px(40.)).flex().justify_end().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).font_features(tnum()).child(percent(m.part()))),
            )
            .child(bar(id, m.part(), None).w_full())
    };
    let mut others = div().flex().flex_col().gap(px(14.)).child(small("ram-v", r.tr("ram.virtual"), virt));
    if let Some(cache) = cache {
        others = others.child(small("ram-c", r.tr("ram.cache"), cache));
    }

    div()
        .flex()
        .flex_col()
        .pt(px(4.))
        .pb(px(8.))
        .gap(px(20.))
        .child(hero)
        .child(others)
        .child(div().h(px(1.)).bg(c(LINE)))
        .child(auto_part(r, this, ram.total as f64 / (1u64 << 20) as f64, each_mb))
        .child(areas_part(r, this))
}

/// Auto clean, as Mem Reduct has it: above a RAM level, every so many minutes, or both. Turning one on asks Windows
/// once (choose_auto).
fn auto_part(r: &Root, this: &WeakEntity<Root>, total_mb: f64, each_mb: f64) -> Div {
    let at = r.state.pointer("/settings/autoCleanAt").and_then(serde_json::Value::as_i64).unwrap_or(0);
    let every = r.state.pointer("/settings/autoCleanEvery").and_then(serde_json::Value::as_i64).unwrap_or(0);
    let areas = areas(&r.state);
    let off = r.tr("ram.autoOff");
    let mut ats = Segmented::new("ram-at", at.to_string()).item("0", off.clone());
    for n in [80, 85, 90, 95] {
        ats = ats.item(n.to_string(), format!("{n}%"));
    }
    let me = this.clone();
    let ats = ats.on_select(move |v, _, cx| {
        let v = v.parse::<i64>().unwrap_or(0);
        let _ = me.update(cx, |r, cx| choose_auto(r, cx, v, every));
    });
    let mut everys = Segmented::new("ram-every", every.to_string()).item("0", off);
    for n in [5, 10, 15, 30] {
        everys = everys.item(n.to_string(), r.i18n.tv("ram.min", &[("n", &n.to_string())]));
    }
    everys = everys.item("60", r.tr("ram.hour"));
    let me = this.clone();
    let everys = everys.on_select(move |v, _, cx| {
        let v = v.parse::<i64>().unwrap_or(0);
        let _ = me.update(cx, |r, cx| choose_auto(r, cx, at, v));
    });
    // (A row whose choices don't fit beside its label, in a long language or the timer's six, puts them under it.)
    let line = |label: String, ctl: Segmented| {
        div().flex().flex_wrap().items_center().justify_between().gap_x(px(12.)).gap_y(px(8.)).child(div().flex_none().whitespace_nowrap().text_size(px(T_M)).text_color(c(TEXT_2)).child(label)).child(ctl)
    };
    // The recommended setting in one press, worked out for this PC (best_at): no timer, Mem Reduct's default areas
    // (the standby cache, Windows' copy of what was read lately, kept). Its tip says why, in this PC's own figures.
    let best_at = best_at(total_mb, each_mb);
    let best = at == best_at && every == 0 && areas == DEFAULT_AREAS;
    let mb = |v: f64| crate::lang::cur().mb(v);
    let best_tip = r.i18n.tv(
        "ram.useBestTip",
        &[("n", &best_at.to_string()), ("left", &mb(total_mb * (100 - best_at) as f64 / 100.)), ("each", &mb(each_mb))],
    );
    let me = this.clone();
    let head = div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(28.))
        .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(r.tr("ram.auto")))
        .when(!best, |d| {
            d.child(Button::new("ram-best", r.tr("ram.useBest")).small().quiet().tip(best_tip).on_click(move |_, _, cx| {
                let _ = me.update(cx, |r, cx| {
                    r.ask("setSettings", serde_json::json!({ "cleanAreas": DEFAULT_AREAS }), cx, |_, _| {});
                    choose_auto(r, cx, best_at, 0);
                });
            }))
        });
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(head)
        .child(line(r.tr("ram.whenAbove"), ats))
        .child(line(r.tr("ram.onTimer"), everys))
        .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("ram.autoDesc")))
}

/// The areas, Mem Reduct's list: folded to a count; open, a tick each in two columns (the last one can't be
/// unticked).
fn areas_part(r: &Root, this: &WeakEntity<Root>) -> Div {
    let areas = areas(&r.state);
    let list = [
        (WORKING_SETS, "ram.area.workingSets"),
        (FILE_CACHE, "ram.area.fileCache"),
        (STANDBY_LOW, "ram.area.standbyLow"),
        (VOLUMES, "ram.area.volumes"),
        (REGISTRY, "ram.area.registry"),
        (COMBINE, "ram.area.combine"),
        (STANDBY_ALL, "ram.area.standbyAll"),
        (MODIFIED_LIST, "ram.area.modified"),
    ];
    let open = AREAS_OPEN.with(std::cell::Cell::get);
    let count = list.iter().filter(|(bit, _)| areas & bit != 0).count();
    let toggle = div()
        .id("ram-areas")
        .h(px(28.))
        .flex()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .child(div().flex_1().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(r.tr("ram.areas")))
        .child(div().text_size(px(T_M)).text_color(c(MUTED)).font_features(tnum()).child(r.i18n.tv("ram.areasCount", &[("n", &count.to_string())])))
        .child(icon(if open { "chevronUp" } else { "chevronDown" }, 14., c(MUTED)))
        .on_click(|_, w, _| {
            AREAS_OPEN.with(|o| o.set(!o.get()));
            w.refresh();
        });
    let mut part = div().flex().flex_col().gap(px(12.)).child(toggle);
    if open {
        let mut grid = div().w_full().grid().grid_cols(2).gap_x(px(16.)).gap_y(px(10.));
        for (bit, key) in list {
            let me = this.clone();
            grid = grid.child(Check::new(("ram-area", bit as usize), areas & bit != 0).label(r.tr(key)).on_toggle(move |now, w, cx| {
                let next = if *now { areas | bit } else { areas & !bit };
                if next != 0 {
                    let _ = me.update(cx, |r, cx| r.ask("setSettings", serde_json::json!({ "cleanAreas": next }), cx, |_, _| {}));
                }
                w.refresh();
            }));
        }
        part = part.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("ram.areasDesc"))).child(grid);
    }
    part
}

/// Mem Reduct's page at the left (where the idea is from), the clean at the right.
fn foot(r: &Root, this: &WeakEntity<Root>) -> Div {
    let busy = running();
    let me = this.clone();
    dialog_foot()
        .justify_between()
        .child(Button::new("ram-mr", r.tr("ram.fromMemReduct")).icon("external").small().quiet().tip(r.tr("ram.memReductTip")).on_click(|_, _, cx| cx.open_url(MEM_REDUCT)))
        .child(
            Button::new("ram-clean", r.tr(if busy { "ram.cleaning" } else { "ram.cleanNow" }))
                .icon("shield")
                .primary()
                .disabled(busy)
                .on_click(move |_, _, cx| {
                    let _ = me.update(cx, |r, cx| press(r, cx));
                }),
        )
}
