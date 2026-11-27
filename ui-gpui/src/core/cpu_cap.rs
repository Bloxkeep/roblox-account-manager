//! A hard cap on how much of the PC's CPU one Roblox process may use (Settings › CPU limit for resting windows): the
//! process goes into a job object of its own whose CPU rate control holds it at that share of all the PC's processors,
//! the number Task Manager's CPU column shows for it. Only Windows' switches: nothing in Roblox is touched.
//!
//! A process cannot leave a job, so a lift switches the same job's rate control off, and the job is kept for as long as
//! its process lives (by pid and the process's creation time, so a reused pid isn't taken for the old process). The
//! job never has JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: closing its handle leaves the process running. Every cap is
//! switched off at closing and at locking (`lift_all`), and when Bloxkeep crashes (`lift_all_at_crash`, from the crash
//! filter): a job outlives Bloxkeep while its process runs, and no later Bloxkeep can reach it. A name doesn't help:
//! Windows takes a job's name away with its last handle, while the process keeps the job itself (tried: OpenJobObject
//! found nothing once the handle was closed). Only an end Bloxkeep has no say in (ended from Task Manager, the power
//! gone) leaves a cap on, until that Roblox closes.
//!
//! Called on the RAM turn's worker (memory.rs `in_ram_turn`), never on the app's thread.
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{LazyLock, Mutex},
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, GetLastError},
    System::Threading::{GetProcessTimes, OpenProcess},
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> *mut c_void;
    fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    fn SetInformationJobObject(job: *mut c_void, class: i32, info: *const c_void, size: u32) -> i32;
    fn QueryInformationJobObject(job: *mut c_void, class: i32, info: *mut c_void, size: u32, returned: *mut u32) -> i32;
}

const PROCESS_TERMINATE: u32 = 0x0001;
const PROCESS_SET_QUOTA: u32 = 0x0100;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

const JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION: i32 = 1;
const JOB_OBJECT_CPU_RATE_CONTROL_INFORMATION: i32 = 15;
pub const JOB_OBJECT_CPU_RATE_CONTROL_ENABLE: u32 = 0x1;
pub const JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP: u32 = 0x4;

/// JOBOBJECT_CPU_RATE_CONTROL_INFORMATION: the flags, then a union whose first member is CpuRate (cycles per 10 000).
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct RateControl {
    control_flags: u32,
    cpu_rate: u32,
}

/// JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, read for its count of processes still running in the job.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Accounting {
    total_user_time: i64,
    total_kernel_time: i64,
    this_period_total_user_time: i64,
    this_period_total_kernel_time: i64,
    total_page_fault_count: u32,
    total_processes: u32,
    active_processes: u32,
    total_terminated_processes: u32,
}

/// One process's job: its handle (kept as a number so the map may live in a static) and the process's creation time.
struct Job {
    handle: isize,
    created: i64,
}

/// Every job Bloxkeep made, by pid.
static JOBS: LazyLock<Mutex<HashMap<i32, Job>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// An open process handle, closed when dropped.
struct Proc(*mut c_void);

impl Drop for Proc {
    fn drop(&mut self) {
        // SAFETY: a handle this module opened and nobody else closes.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn last_error() -> u32 {
    // SAFETY: plain call, right after the call that failed.
    unsafe { GetLastError() }
}

fn creation_time(process: *mut c_void) -> Option<i64> {
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: four out values on the stack, a handle opened with PROCESS_QUERY_LIMITED_INFORMATION.
    let ok = unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) } != 0;
    ok.then(|| ((created.dwHighDateTime as i64) << 32) | created.dwLowDateTime as i64)
}

/// The job's CpuRate (cycles per 10 000 of the whole PC) for `percent` % on a PC with `cpus` logical processors: never
/// under a quarter of one of them. Held lower, a Roblox window took 18 s for one frame, and other players saw its account
/// jump about when it caught up.
pub fn rate_for(percent: i32, cpus: u32) -> u32 {
    let floor = 2500u32.div_ceil(cpus.max(1));
    (percent.clamp(1, 100) as u32 * 100).max(floor).min(10_000)
}

fn cpus() -> u32 {
    std::thread::available_parallelism().map_or(1, |n| n.get() as u32)
}

fn set_rate(job: *mut c_void, percent: Option<i32>) -> Result<(), u32> {
    let info = match percent {
        Some(p) => RateControl { control_flags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP, cpu_rate: rate_for(p, cpus()) },
        None => RateControl::default(), // flags 0: rate control off
    };
    // SAFETY: the struct and its size; a job handle this module made.
    let ok = unsafe { SetInformationJobObject(job, JOB_OBJECT_CPU_RATE_CONTROL_INFORMATION, &info as *const RateControl as *const c_void, std::mem::size_of::<RateControl>() as u32) } != 0;
    if ok { Ok(()) } else { Err(last_error()) }
}

fn read_rate(job: *mut c_void) -> Option<(u32, u32)> {
    let mut info = RateControl::default();
    let mut returned = 0u32;
    // SAFETY: the struct and its size as the out buffer; a job handle this module made.
    let ok = unsafe {
        QueryInformationJobObject(job, JOB_OBJECT_CPU_RATE_CONTROL_INFORMATION, &mut info as *mut RateControl as *mut c_void, std::mem::size_of::<RateControl>() as u32, &mut returned)
    } != 0;
    ok.then_some((info.control_flags, info.cpu_rate))
}

/// Whether no process runs in the job any more (None when Windows didn't say).
fn emptied(job: *mut c_void) -> Option<bool> {
    let mut info = Accounting::default();
    // SAFETY: the struct and its size as the out buffer; a job handle this module made.
    let ok = unsafe {
        QueryInformationJobObject(job, JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION, &mut info as *mut Accounting as *mut c_void, std::mem::size_of::<Accounting>() as u32, std::ptr::null_mut())
    } != 0;
    ok.then_some(info.active_processes == 0)
}

fn close_job(job: &Job) {
    // SAFETY: a job handle this module made, taken out of the map before it is closed. The job has no kill-on-close
    // limit, so the process goes on.
    unsafe {
        CloseHandle(job.handle as *mut c_void);
    }
}

/// Jobs whose processes have all ended are closed and forgotten.
fn tidy_locked(jobs: &mut HashMap<i32, Job>) {
    jobs.retain(|_, job| {
        let gone = emptied(job.handle as *mut c_void) == Some(true);
        if gone {
            close_job(job);
        }
        !gone
    });
}

/// Closes the jobs whose processes have all ended.
pub fn tidy() {
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    tidy_locked(&mut jobs);
}

/// Holds the process at `percent` % of the PC's whole CPU (`Some`), or switches the cap off (None). Err holds
/// GetLastError of the call Windows refused. A lift for a process Bloxkeep never capped has nothing to do.
pub fn set(pid: i32, percent: Option<i32>) -> Result<(), u32> {
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    tidy_locked(&mut jobs);
    // SAFETY: plain call; the handle is checked and closed by `Proc`.
    let process = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid as u32) };
    if process.is_null() {
        return Err(last_error());
    }
    let process = Proc(process);
    let Some(created) = creation_time(process.0) else { return Err(last_error()) };
    // The pid's job from before, of another process that had the same pid: that process is gone.
    if jobs.get(&pid).is_some_and(|j| j.created != created) {
        if let Some(old) = jobs.remove(&pid) {
            close_job(&old);
        }
    }
    if let Some(job) = jobs.get(&pid) {
        return set_rate(job.handle as *mut c_void, percent);
    }
    if percent.is_none() {
        return Ok(());
    }
    // SAFETY: an unnamed job with default security; the handle is checked.
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        return Err(last_error());
    }
    let made = Job { handle: job as isize, created };
    // The rate first, so the process is never in a job of ours without its cap. A process already in a job (Roblox's
    // own, a launcher's) goes into this one as a nested job.
    if let Err(code) = set_rate(job, percent) {
        close_job(&made);
        return Err(code);
    }
    // SAFETY: both handles valid; the process handle has PROCESS_SET_QUOTA | PROCESS_TERMINATE as the call needs.
    if unsafe { AssignProcessToJobObject(job, process.0) } == 0 {
        let code = last_error();
        close_job(&made);
        return Err(code);
    }
    jobs.insert(pid, made);
    Ok(())
}

/// The cap of the process's job as Windows has it: its control flags and CpuRate (cycles per 10 000). None when Bloxkeep
/// made no job for it or Windows didn't say.
pub fn read(pid: i32) -> Option<(u32, u32)> {
    let jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    jobs.get(&pid).and_then(|j| read_rate(j.handle as *mut c_void))
}

/// Every cap switched off (Bloxkeep closing or locking). The jobs are kept while their processes run, so a cap asked
/// after an unlock uses the same job. Returns the pids Windows refused, with the error code.
pub fn lift_all() -> Vec<(i32, u32)> {
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    tidy_locked(&mut jobs);
    jobs.iter().filter_map(|(pid, job)| set_rate(job.handle as *mut c_void, None).err().map(|code| (*pid, code))).collect()
}

/// Every cap switched off as Bloxkeep crashes (log.rs's crash filter), the last moment anything can reach the jobs.
/// Nothing that could hang or need the heap: the map only if no thread holds it (the crashing one may), each job's
/// rate control off. A job whose process ended has nothing to switch off.
pub fn lift_all_at_crash() {
    let Ok(jobs) = JOBS.try_lock() else { return };
    for job in jobs.values() {
        let _ = set_rate(job.handle as *mut c_void, None);
    }
}

/// Closes the process's job handle and forgets it; the process stays in the job, its cap as it was. For the checks: a
/// closed handle must leave the process running.
pub fn forget(pid: i32) -> bool {
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    match jobs.remove(&pid) {
        Some(job) => {
            close_job(&job);
            true
        }
        None => false,
    }
}
