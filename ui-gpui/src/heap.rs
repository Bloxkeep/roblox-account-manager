// The app's own memory as Rust asked for it (Box, Vec, String...: every picture's pixels too), counted as it is asked
// for and given back, for error.log's memory line (mem_tally.rs). Beside the process's private bytes it splits what the
// app holds from what Windows, the graphics driver and freed-but-kept heap hold (the owner, 2026-11-20: 596 MB at the
// peak, and no way to say of what). Two atomic adds a call: nothing to notice beside the allocation itself.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

pub struct Counted;

static IN_USE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn more(n: usize) {
    let now = IN_USE.fetch_add(n, Relaxed) + n;
    if now > PEAK.load(Relaxed) {
        PEAK.fetch_max(now, Relaxed);
    }
}

fn less(n: usize) {
    IN_USE.fetch_sub(n, Relaxed);
}

// SAFETY: every call is passed on to the system allocator unchanged; only the sizes are counted.
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's layout, as GlobalAlloc requires.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            more(layout.size());
            #[cfg(any(feature = "shot", feature = "core"))]
            sites::took(p, layout.size());
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as alloc.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            more(layout.size());
            #[cfg(any(feature = "shot", feature = "core"))]
            sites::took(p, layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: ptr came from this allocator with this layout.
        #[cfg(any(feature = "shot", feature = "core"))]
        sites::gave(ptr);
        unsafe { System.dealloc(ptr, layout) };
        less(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: as GlobalAlloc::realloc requires, passed on.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            less(layout.size());
            more(new_size);
            #[cfg(any(feature = "shot", feature = "core"))]
            {
                sites::gave(ptr);
                sites::took(p, new_size);
            }
        }
        p
    }
}

/// The bytes Rust holds now.
pub fn in_use() -> usize {
    IN_USE.load(Relaxed)
}

/// The most it held since the last call (then counting starts over from now).
pub fn take_peak() -> usize {
    PEAK.swap(IN_USE.load(Relaxed), Relaxed)
}

/// Where Rust's heap goes (the owner, 2026-11-20: 52-70 MB held, of it 2-5 MB pictures, and nothing said of the rest).
/// About one allocation in every 256 KB asked for (32 KB in a shot) is kept with the calls that asked for it, until it
/// is given back; error.log's memory line is followed by the biggest holders, as addresses in the exe (read with the
/// build's .pdb), a shot's BLOXKEEP_SHOT_SITES file by name. Freeing looks at one counter unless a kept one may be it.
#[cfg(any(feature = "shot", feature = "core"))]
pub mod sites {
    use std::{
        cell::Cell,
        collections::HashMap,
        sync::{
            Mutex,
            atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering::Relaxed},
        },
    };

    use crate::threads::Hold;

    #[cfg(feature = "shot")]
    const RATE: isize = 32 * 1024;
    #[cfg(not(feature = "shot"))]
    const RATE: isize = 256 * 1024;
    const DEPTH: usize = 32;
    const BUCKETS: usize = 1 << 16;

    type Stack = [usize; DEPTH];

    static ON: AtomicBool = AtomicBool::new(false);
    static BASE: AtomicUsize = AtomicUsize::new(0);
    /// Per bucket of addresses, how many kept allocations are in it: freeing anything else stops at one load.
    static COUNT: [AtomicU16; BUCKETS] = [const { AtomicU16::new(0) }; BUCKETS];
    static STATE: Mutex<Option<State>> = Mutex::new(None);

    struct State {
        held: HashMap<usize, (u32, usize)>,
        stacks: Vec<Stack>,
        ids: HashMap<Stack, u32>,
    }

    thread_local! {
        static BUSY: Cell<bool> = const { Cell::new(false) };
        static LEFT: Cell<isize> = const { Cell::new(RATE) };
    }

    pub fn start() {
        #[cfg(windows)]
        // SAFETY: the exe's own module handle (its load address); nothing is handed over.
        BASE.store(unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) } as usize, Relaxed);
        ON.store(true, Relaxed);
    }

    fn bucket(p: usize) -> usize {
        ((p >> 4).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 48) & (BUCKETS - 1)
    }

    /// Runs `f` unless this thread is already in here (the map's and the stack walk's own allocations).
    fn guarded(f: impl FnOnce()) {
        let Ok(busy) = BUSY.try_with(|b| b.replace(true)) else { return };
        if busy {
            return;
        }
        f();
        let _ = BUSY.try_with(|b| b.set(false));
    }

    pub(super) fn took(p: *mut u8, size: usize) {
        if !ON.load(Relaxed) {
            return;
        }
        let pick = LEFT
            .try_with(|l| {
                let v = l.get() - size as isize;
                l.set(if v <= 0 { RATE } else { v });
                v <= 0
            })
            .unwrap_or(false);
        if !pick {
            return;
        }
        guarded(|| {
            let mut ips: Stack = [0; DEPTH];
            let mut n = 0;
            backtrace::trace(|f| {
                ips[n] = f.ip() as usize;
                n += 1;
                n < DEPTH
            });
            let mut g = STATE.hold();
            let st = g.get_or_insert_with(|| State { held: HashMap::new(), stacks: Vec::new(), ids: HashMap::new() });
            let id = match st.ids.get(&ips) {
                Some(&id) => id,
                None => {
                    let id = st.stacks.len() as u32;
                    st.stacks.push(ips);
                    st.ids.insert(ips, id);
                    id
                }
            };
            if st.held.insert(p as usize, (id, size.max(RATE as usize))).is_none() {
                COUNT[bucket(p as usize)].fetch_add(1, Relaxed);
            }
        });
    }

    pub(super) fn gave(p: *mut u8) {
        if !ON.load(Relaxed) || COUNT[bucket(p as usize)].load(Relaxed) == 0 {
            return;
        }
        guarded(|| {
            if let Some(st) = STATE.hold().as_mut() {
                if st.held.remove(&(p as usize)).is_some() {
                    COUNT[bucket(p as usize)].fetch_sub(1, Relaxed);
                }
            }
        });
    }

    /// The bytes the kept allocations stand for, by the calls that asked for them, the biggest first.
    fn by_stack() -> (Vec<(Stack, usize)>, usize) {
        let _ = BUSY.try_with(|b| b.set(true));
        let mut sum: HashMap<u32, usize> = HashMap::new();
        let mut all = 0;
        let mut out = Vec::new();
        if let Some(st) = STATE.hold().as_ref() {
            for &(id, w) in st.held.values() {
                *sum.entry(id).or_default() += w;
                all += w;
            }
            out = sum.into_iter().map(|(id, w)| (st.stacks[id as usize], w)).collect();
        }
        out.sort_by(|a, b| b.1.cmp(&a.1));
        let _ = BUSY.try_with(|b| b.set(false));
        (out, all)
    }

    /// For error.log: the `top` biggest holders, each its MB and its calls as offsets in the exe (x: outside it).
    pub fn holders(top: usize) -> String {
        if !ON.load(Relaxed) {
            return String::new();
        }
        let base = BASE.load(Relaxed);
        let (stacks, all) = by_stack();
        let list: Vec<String> = stacks
            .iter()
            .take(top)
            .map(|(ips, w)| {
                let calls: Vec<String> =
                    ips.iter().filter(|&&ip| ip != 0).map(|&ip| if ip > base && ip - base < 1 << 30 { format!("{:x}", ip - base) } else { "x".into() }).collect();
                format!("{:.1} MB [{}]", *w as f64 / 1048576., calls.join(","))
            })
            .collect();
        format!(
            "Heap holders (one sample per {} KB asked for, {:.0} MB sampled, exe build {}): {}",
            RATE / 1024,
            all as f64 / 1048576.,
            env!("CARGO_PKG_VERSION"),
            list.join("; ")
        )
    }

    #[cfg(feature = "shot")]
    fn name(ip: usize, names: &mut HashMap<usize, Vec<String>>) -> Vec<String> {
        names
            .entry(ip)
            .or_insert_with(|| {
                let mut out = Vec::new();
                backtrace::resolve(ip as *mut std::ffi::c_void, |s| {
                    let mut n = s.name().map(|n| format!("{n:#}")).unwrap_or_else(|| format!("{ip:#x}"));
                    if let (Some(f), Some(l)) = (s.filename(), s.lineno()) {
                        let f = f.to_string_lossy();
                        let short = f.rsplit(['\\', '/']).take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/");
                        n = format!("{n} ({short}:{l})");
                    }
                    out.push(n);
                });
                if out.is_empty() {
                    out.push(format!("{ip:#x}"));
                }
                out
            })
            .clone()
    }

    /// The allocator's and Rust's own collections' frames, skipped to reach the code that asked.
    #[cfg(feature = "shot")]
    fn plumbing(n: &str) -> bool {
        ["backtrace::", "bloxkeep_ui::heap::", "bloxkeep_ui::_::__rust", "alloc::", "core::", "std::", "hashbrown::", "__rust", "<alloc::", "<core::", "<std::", "<hashbrown::", "smallvec::", "<smallvec::"]
            .iter()
            .any(|p| n.starts_with(p))
            || n.starts_with("0x")
    }

    /// A shot's report, by name: the first function past the allocator, the first Bloxkeep one, and whole stacks.
    #[cfg(feature = "shot")]
    pub fn report(path: &str, total: usize) {
        let (stacks, all) = by_stack();
        let _ = BUSY.try_with(|b| b.set(true));
        let mut names = HashMap::new();
        let mut by_first: HashMap<String, usize> = HashMap::new();
        let mut by_app: HashMap<String, usize> = HashMap::new();
        let mut by_calls: HashMap<Vec<String>, usize> = HashMap::new();
        for (ips, w) in &stacks {
            let frames: Vec<String> = ips.iter().filter(|&&ip| ip != 0).flat_map(|&ip| name(ip, &mut names)).collect();
            let from = frames.iter().position(|f| !plumbing(f)).unwrap_or(0);
            let asked = &frames[from..];
            *by_first.entry(asked.first().cloned().unwrap_or_default()).or_default() += w;
            let app = asked
                .iter()
                .find(|f| f.starts_with("bloxkeep_ui::") || f.starts_with("<bloxkeep_ui::"))
                .cloned()
                .unwrap_or_else(|| "(no bloxkeep_ui frame)".into());
            *by_app.entry(app).or_default() += w;
            *by_calls.entry(asked.iter().take(9).cloned().collect()).or_default() += w;
        }
        let mb = |b: usize| b as f64 / 1048576.;
        let mut out = format!("Rust heap {:.1} MB; held by sampled allocations about {:.1} MB\n", mb(total), mb(all));
        let top = |m: HashMap<String, usize>, k: usize| {
            let mut v: Vec<_> = m.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            v.into_iter().take(k).map(|(n, b)| format!("{:8.2} MB  {n}\n", mb(b))).collect::<String>()
        };
        out += "\n== By the first function past the allocator ==\n";
        out += &top(by_first, 40);
        out += "\n== By the first Bloxkeep function ==\n";
        out += &top(by_app, 40);
        out += "\n== By stack ==\n";
        let mut v: Vec<_> = by_calls.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        for (frames, b) in v.into_iter().take(30) {
            out += &format!("\n{:.2} MB\n", mb(b));
            for f in frames {
                out += &format!("    {f}\n");
            }
        }
        out += &format!("
{}
", holders(12));
        let _ = std::fs::write(path, out);
        let _ = BUSY.try_with(|b| b.set(false));
    }
}
