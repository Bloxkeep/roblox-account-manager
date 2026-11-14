// RobloxUsage.cs: how hard Roblox's windows work the PC.
//! How hard Roblox's windows work the PC: CPU from their processes' time, GPU from Windows' "GPU Engine" counters, as Task
//! Manager shows them. Read on a thread of its own every few seconds, only while Roblox runs and the page shows it; the
//! first read of the counters takes about half a second (measured 2026-11-01). None when Windows doesn't say.
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use regex::{Regex, RegexBuilder};

use crate::core::roblox_api::lock;

const EVERY: Duration = Duration::from_secs(3);
const SLOW_EVERY: Duration = Duration::from_secs(15);

/// The PDH query over "\GPU Engine(*)\Utilization Percentage", kept open between reads (the C#'s InstanceDataCollection
/// of the last read: a value is the change between two collections).
struct GpuQuery {
    query: windows_sys::Win32::System::Performance::PDH_HQUERY,
    counter: windows_sys::Win32::System::Performance::PDH_HCOUNTER,
}

// SAFETY: PDH handles may be used from any thread; this one is only used under the state's lock.
unsafe impl Send for GpuQuery {}

impl Drop for GpuQuery {
    fn drop(&mut self) {
        // SAFETY: the query was opened by PdhOpenQueryW and is closed once (its counters with it).
        unsafe { windows_sys::Win32::System::Performance::PdhCloseQuery(self.query) };
    }
}

#[derive(Default)]
struct State {
    cpu_times: HashMap<i32, Duration>,
    read_at: Option<Instant>,
    gpu_before: Option<GpuQuery>,
    gpu_gone: bool,
    /// Reads of the GPU counters that failed in a row: one can fail as instances come and go (a client opening).
    gpu_failures: u32,
    cpu_pct: Option<i32>,
    gpu_pct: Option<i32>,
}

#[derive(Default)]
struct Inner {
    state: Mutex<State>,
    reading: AtomicBool,
}

#[derive(Clone, Default)]
pub struct RobloxUsage {
    inner: Arc<Inner>,
}

impl RobloxUsage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cpu_pct(&self) -> Option<i32> {
        lock(&self.inner.state).cpu_pct
    }

    pub fn gpu_pct(&self) -> Option<i32> {
        lock(&self.inner.state).gpu_pct
    }

    /// `slow`: every 15 s, not 3: the window is left behind the games (AppController.UnfocusedAfter).
    pub fn refresh(&self, pids: &[i32], slow: bool) {
        let inner = &self.inner;
        if pids.is_empty() {
            if lock(&inner.state).read_at.is_some() && inner.reading.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                let mut s = lock(&inner.state);
                s.read_at = None;
                s.gpu_before = None;
                s.cpu_pct = None;
                s.gpu_pct = None;
                s.cpu_times.clear();
                drop(s);
                inner.reading.store(false, Ordering::SeqCst);
            }
            return;
        }
        let due = lock(&inner.state).read_at.is_none_or(|at| at.elapsed() >= if slow { SLOW_EVERY } else { EVERY });
        if !due || inner.reading.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return;
        }
        let inner = Arc::clone(inner);
        let pids = pids.to_vec();
        std::thread::spawn(move || {
            // The flag goes down even if the read panics.
            struct Done<'a>(&'a AtomicBool);
            impl Drop for Done<'_> {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::SeqCst);
                }
            }
            let _done = Done(&inner.reading);
            // Read on a copy, the lock let go meanwhile: the page's tick asks cpu_pct/gpu_pct under the same lock, and
            // the first read takes about half a second (the C# read without a lock; review 2026-11-13). The last
            // numbers stay readable until the new ones are in.
            let mut work = {
                let mut s = lock(&inner.state);
                State {
                    cpu_times: std::mem::take(&mut s.cpu_times),
                    read_at: s.read_at,
                    gpu_before: s.gpu_before.take(),
                    gpu_gone: s.gpu_gone,
                    gpu_failures: s.gpu_failures,
                    cpu_pct: s.cpu_pct,
                    gpu_pct: s.gpu_pct,
                }
            };
            read(&mut work, &pids);
            *lock(&inner.state) = work;
        });
    }
}

fn read(s: &mut State, pids: &[i32]) {
    let now = Instant::now();
    let seconds = s.read_at.map(|at| (now - at).as_secs_f64()).unwrap_or(0.0);
    let mut times: HashMap<i32, Duration> = HashMap::new();
    for &pid in pids {
        if let Some(t) = processor_time(pid) {
            times.insert(pid, t);
        }
    }
    // Only what each window used since the last read: one that just opened counts from here.
    let used: f64 = times.iter().filter_map(|(pid, t)| s.cpu_times.get(pid).map(|before| t.as_secs_f64() - before.as_secs_f64())).map(|d| d.max(0.0)).sum();
    let processors = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
    s.cpu_pct = if s.read_at.is_none() { None } else { Some(percent(used / (seconds * processors) * 100.0)) };
    s.cpu_times = times;
    s.gpu_pct = read_gpu(s, pids);
    s.read_at = Some(now);
}

/// Process.TotalProcessorTime: kernel and user time; None when the process can't be opened.
fn processor_time(pid: i32) -> Option<Duration> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY: opens a handle only to query times; closed below.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid as u32) };
    if handle.is_null() {
        return None;
    }
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: `handle` is open; the four outputs are valid.
    let ok = unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) };
    // SAFETY: `handle` was opened above and is closed once.
    unsafe { CloseHandle(handle) };
    if ok == 0 {
        return None;
    }
    let ticks = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
    Some(Duration::from_nanos((ticks(kernel) + ticks(user)) * 100))
}

/// Failed reads in a row before the GPU figure is given up for the run.
const GPU_TRIES: u32 = 3;

fn read_gpu(s: &mut State, pids: &[i32]) -> Option<i32> {
    if s.gpu_gone {
        return None;
    }
    let Some(query) = s.gpu_before.take() else {
        // The first read only takes the counters' first sample: a value needs two.
        match open_gpu_query() {
            Some(q) => s.gpu_before = Some(q),
            None => gpu_failed(s), // Windows without the counters, or with them switched off: not asked again.
        }
        return None;
    };
    match gpu_rows(&query) {
        Some(rows) => {
            s.gpu_before = Some(query);
            s.gpu_failures = 0;
            Some(gpu_percent(&rows, pids))
        }
        None => {
            s.gpu_before = Some(query); // kept for the next try
            gpu_failed(s);
            None
        }
    }
}

/// One failed read; a few in a row: not asked again this run.
fn gpu_failed(s: &mut State) {
    s.gpu_failures += 1;
    if s.gpu_failures >= GPU_TRIES {
        s.gpu_gone = true;
        s.gpu_before = None;
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open_gpu_query() -> Option<GpuQuery> {
    use windows_sys::Win32::System::Performance::{PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhOpenQueryW};
    // SAFETY: zero is "no handle" for both PDH handle types.
    let mut query = unsafe { std::mem::zeroed() };
    // SAFETY: a real-time query (no data source); `query` receives the handle.
    if unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) } as u32 != 0 {
        return None;
    }
    let path = wide(r"\GPU Engine(*)\Utilization Percentage");
    // SAFETY: zero is "no handle".
    let mut counter = unsafe { std::mem::zeroed() };
    // SAFETY: `query` is open and `path` NUL-terminated.
    if unsafe { PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) } as u32 != 0 {
        // SAFETY: `query` was opened above and is closed once.
        unsafe { PdhCloseQuery(query) };
        return None;
    }
    let q = GpuQuery { query, counter };
    // SAFETY: `query` is open.
    if unsafe { PdhCollectQueryData(q.query) } as u32 != 0 {
        return None;
    }
    Some(q)
}

const PDH_MORE_DATA: u32 = 0x8000_07D2;
/// PDH_FMT_DOUBLE | PDH_FMT_NOCAP100: CounterSample.Calculate doesn't cap at 100 either.
const FORMAT: u32 = 0x0000_0200 | 0x0000_8000;

/// The change of each instance since the last collection, as (instance, percent); instances new since then are left out.
fn gpu_rows(q: &GpuQuery) -> Option<Vec<(String, f32)>> {
    use windows_sys::Win32::System::Performance::{PDH_FMT_COUNTERVALUE_ITEM_W, PdhCollectQueryData, PdhGetFormattedCounterArrayW};
    // SAFETY: `q.query` is open.
    if unsafe { PdhCollectQueryData(q.query) } as u32 != 0 {
        return None;
    }
    let (mut size, mut count) = (0u32, 0u32);
    // SAFETY: a null buffer asks only the size in bytes.
    let r = unsafe { PdhGetFormattedCounterArrayW(q.counter, FORMAT as _, &mut size, &mut count, std::ptr::null_mut()) } as u32;
    if r == 0 && count == 0 {
        return Some(Vec::new());
    }
    if r != PDH_MORE_DATA {
        return None;
    }
    let item = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
    let mut buffer: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = Vec::with_capacity((size as usize).div_ceil(item));
    // SAFETY: `buffer` holds at least `size` bytes, aligned for the items PDH writes first.
    let r = unsafe { PdhGetFormattedCounterArrayW(q.counter, FORMAT as _, &mut size, &mut count, buffer.as_mut_ptr()) } as u32;
    if r != 0 {
        return None;
    }
    let mut rows = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        // SAFETY: PDH wrote `count` items at the start of the buffer; their names point into the same buffer.
        let entry = unsafe { std::ptr::read(buffer.as_ptr().add(i)) };
        // PDH_CSTATUS_VALID_DATA 0, PDH_CSTATUS_NEW_DATA 1: anything else has no value for this pair of samples.
        if entry.FmtValue.CStatus > 1 || entry.szName.is_null() {
            continue;
        }
        // SAFETY: szName is a NUL-terminated string in `buffer`.
        let name = unsafe {
            let mut len = 0;
            while *entry.szName.add(len) != 0 {
                len += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(entry.szName, len))
        };
        // SAFETY: PDH_FMT_DOUBLE fills the double member.
        let value = unsafe { entry.FmtValue.Anonymous.doubleValue };
        rows.push((name, value as f32));
    }
    Some(rows)
}

static ENGINE_INSTANCE: LazyLock<Regex> =
    LazyLock::new(|| RegexBuilder::new(r"^pid_(\d+)_(luid_\w+?_phys_\d+_eng_\d+)(_|$)").case_insensitive(true).build().unwrap());

/// Task Manager's figure for a set of processes: their share of each GPU engine added up, then the busiest engine.
/// `rows`: the counters' instances ("pid_12_luid_0x0_0x14d4d_phys_0_eng_0_engtype_3D") and values.
pub fn gpu_percent(rows: &[(String, f32)], pids: &[i32]) -> i32 {
    let pids: HashSet<i32> = pids.iter().copied().collect();
    let mut engines: HashMap<String, f64> = HashMap::new();
    for (instance, pct) in rows {
        let Some(m) = ENGINE_INSTANCE.captures(instance) else { continue };
        let Ok(pid) = m[1].parse::<i32>() else { continue };
        if !pids.contains(&pid) {
            continue;
        }
        *engines.entry(m[2].to_string()).or_insert(0.0) += *pct as f64;
    }
    percent(engines.values().copied().fold(None::<f64>, |max, v| Some(max.map_or(v, |m| m.max(v)))).unwrap_or(0.0))
}

fn percent(value: f64) -> i32 {
    value.clamp(0.0, 100.0).round_ties_even() as i32
}
