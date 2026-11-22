// Where the app's memory in RAM (Task Manager's number: private pages in the working set) lies, by kind: the app's own
// heap (Rust's allocations and the C code sharing it), Windows' other heaps, thread stacks, and the rest (memory taken
// straight from Windows: the graphics driver's, Direct3D's, a decoder's), the biggest of those by allocation. The
// owner, 2026-11-22: "điều tra kỹ đi" — the memory line said what Rust's heap holds but not what else was in RAM.
// The heap's own memory is told by where the heap samples (heap.rs sites) lie, not by walking the heap: HeapWalk reads
// every block's header and brought every page of the heap back into RAM (a shot: 49 MB in RAM before, 243 after).
#![cfg(windows)]

use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use windows_sys::Win32::{
    System::{
        Memory::{GetProcessHeap, MEM_COMMIT, MEM_PRIVATE, MEMORY_BASIC_INFORMATION, PAGE_GUARD, VirtualQuery},
        ProcessStatus::{K32EnumProcessModules, K32GetModuleBaseNameW, K32QueryWorkingSetEx, PSAPI_WORKING_SET_EX_INFORMATION},
        Threading::GetCurrentProcess,
        Diagnostics::Debug::ReadProcessMemory,
    },
};

fn query(at: usize) -> Option<MEMORY_BASIC_INFORMATION> {
    // SAFETY: plain query of this process's own address space into a local struct.
    let mut m: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let n = unsafe { VirtualQuery(at as *const _, &mut m, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) };
    (n != 0).then_some(m)
}

#[derive(Default, Clone, Copy)]
struct Part {
    committed: usize,
    resident: usize,
}

/// One line for error.log: the private memory in RAM by kind, the biggest other allocations, and how long it took.
pub fn line() -> String {
    let started = std::time::Instant::now();
    // The allocations holding Rust's memory (where its samples lie), and the process heap's first one.
    let mut own = HashSet::new();
    #[cfg(any(feature = "shot", feature = "core"))]
    for p in crate::heap::sites::addresses() {
        if let Some(m) = query(p) {
            own.insert(m.AllocationBase as usize);
        }
    }
    // SAFETY: no arguments; this process's own heap.
    let me = unsafe { GetProcessHeap() } as usize;
    own.insert(me);
    let walked = started.elapsed();
    // Every private committed region, by its allocation, and whether an allocation has a guard page (a thread stack).
    let mut regions: Vec<(usize, usize, usize)> = Vec::new(); // (allocation, start, size)
    let mut guarded = HashSet::new();
    let mut at = 0usize;
    while let Some(m) = query(at) {
        let size = m.RegionSize;
        if m.State == MEM_COMMIT && m.Type == MEM_PRIVATE {
            regions.push((m.AllocationBase as usize, m.BaseAddress as usize, size));
            if m.Protect & PAGE_GUARD != 0 {
                guarded.insert(m.AllocationBase as usize);
            }
        }
        let next = (m.BaseAddress as usize).saturating_add(size);
        if size == 0 || next <= at {
            break;
        }
        at = next;
    }
    // Which of their pages are in RAM and not shared.
    let pages: usize = regions.iter().map(|r| r.2 / 4096).sum();
    let mut info: Vec<PSAPI_WORKING_SET_EX_INFORMATION> = Vec::with_capacity(pages);
    for r in &regions {
        for p in 0..r.2 / 4096 {
            // SAFETY: a plain struct of an address and its attributes, filled in by the call below.
            let mut i: PSAPI_WORKING_SET_EX_INFORMATION = unsafe { std::mem::zeroed() };
            i.VirtualAddress = (r.1 + p * 4096) as *mut _;
            info.push(i);
        }
    }
    let mut resident_of: HashMap<usize, usize> = HashMap::new();
    let mut committed_of: HashMap<usize, usize> = HashMap::new();
    // SAFETY: the buffer holds `info.len()` entries and says so in bytes; the call fills in their attributes.
    let ok = info.is_empty()
        || unsafe { K32QueryWorkingSetEx(GetCurrentProcess(), info.as_mut_ptr().cast(), (info.len() * std::mem::size_of::<PSAPI_WORKING_SET_EX_INFORMATION>()) as u32) } != 0;
    let mut k = 0;
    for r in &regions {
        *committed_of.entry(r.0).or_default() += r.2;
        for _ in 0..r.2 / 4096 {
            // SAFETY: the union's Flags is the whole attribute word: bit 0 Valid, bit 15 Shared.
            let flags = unsafe { info[k].VirtualAttributes.Flags };
            if ok && flags & 1 != 0 && flags & (1 << 15) == 0 {
                *resident_of.entry(r.0).or_default() += 4096;
            }
            k += 1;
        }
    }
    let (mut heap_own, mut stacks, mut rest) = (Part::default(), Part::default(), Part::default());
    let mut others: Vec<(usize, Part)> = Vec::new();
    for (&base, &committed) in &committed_of {
        let p = Part { committed, resident: resident_of.get(&base).copied().unwrap_or(0) };
        let to = if own.contains(&base) {
            &mut heap_own
        } else if guarded.contains(&base) {
            &mut stacks
        } else {
            others.push((base, p));
            &mut rest
        };
        to.committed += p.committed;
        to.resident += p.resident;
    }
    others.sort_by(|a, b| b.1.resident.cmp(&a.1.resident).then(b.1.committed.cmp(&a.1.committed)));
    let mb = |b: usize| b as f64 / 1048576.;
    let part = |p: Part| format!("{:.0} MB in RAM of {:.0} MB", mb(p.resident), mb(p.committed));
    let biggest: Vec<String> = others
        .iter()
        .take(10)
        .map(|(base, p)| {
            let protect = query(*base).map(|m| m.AllocationProtect).unwrap_or(0);
            format!("{base:#x} {:.1}/{:.1} MB (protect {protect:#x})", mb(p.resident), mb(p.committed))
        })
        .collect();
    let all_resident = heap_own.resident + stacks.resident + rest.resident;
    format!(
        "Memory by kind (private, in RAM of kept): all {:.0} MB in RAM; heap memory holding Rust's allocations {} in {} allocations; thread stacks {} ({} threads' worth); the rest (other heaps, graphics driver, Direct3D, decoders) {} in {} allocations, the biggest in RAM/kept: {}; looked at in {} ms ({} ms placing the heap).",
        mb(all_resident),
        part(heap_own),
        own.len(),
        part(stacks),
        guarded.len(),
        part(rest),
        others.len(),
        biggest.join(", "),
        started.elapsed().as_millis(),
        walked.as_millis(),
    )
}

/// A block at least this big, new and not Rust's heap, is told when it comes.
const BIG: usize = 24 << 20;

/// The big blocks and the modules seen at the last look.
static SEEN: Mutex<Option<(HashSet<usize>, HashSet<String>)>> = Mutex::new(None);

/// The allocations holding Rust's memory now (where its samples lie), and the process heap.
fn own_allocations() -> HashSet<usize> {
    let mut own = HashSet::new();
    #[cfg(any(feature = "shot", feature = "core"))]
    for p in crate::heap::sites::addresses() {
        if let Some(m) = query(p) {
            own.insert(m.AllocationBase as usize);
        }
    }
    // SAFETY: no arguments; this process's own heap.
    own.insert(unsafe { GetProcessHeap() } as usize);
    own
}

/// The names of the modules (DLLs) loaded now.
fn modules() -> HashSet<String> {
    let mut list = vec![std::ptr::null_mut(); 1024];
    let mut needed = 0u32;
    // SAFETY: the buffer holds as many handles as it says in bytes; the call writes at most that many.
    let ok = unsafe { K32EnumProcessModules(GetCurrentProcess(), list.as_mut_ptr(), (list.len() * std::mem::size_of::<usize>()) as u32, &mut needed) } != 0;
    if !ok {
        return HashSet::new();
    }
    list.truncate((needed as usize / std::mem::size_of::<usize>()).min(1024));
    list.iter()
        .filter_map(|&m| {
            let mut name = [0u16; 260];
            // SAFETY: m is one of this process's modules; the name buffer holds as many as it says.
            let n = unsafe { K32GetModuleBaseNameW(GetCurrentProcess(), m, name.as_mut_ptr(), name.len() as u32) } as usize;
            (n > 0).then(|| String::from_utf16_lossy(&name[..n]))
        })
        .collect()
}

/// For error.log, every 10 s or so: a big block that came since the last look and isn't Rust's heap (the owner's alpha.15,
/// 2026-11-22: 55 MB, all zero, all in RAM, came between two memory lines 10 minutes apart, with nothing to say whose),
/// with the modules loaded meanwhile, how much of it is in RAM and whether what is in RAM is all zero. None when nothing
/// came; the first look only takes note. A walk of the address space: about a millisecond.
pub fn news() -> Option<String> {
    let peak = crate::heap::take_look_peak();
    let own = own_allocations();
    let mut size_of: HashMap<usize, usize> = HashMap::new();
    let mut at = 0usize;
    let mut stacks = HashSet::new();
    while let Some(m) = query(at) {
        let size = m.RegionSize;
        if m.State == MEM_COMMIT && m.Type == MEM_PRIVATE {
            *size_of.entry(m.AllocationBase as usize).or_default() += size;
            if m.Protect & PAGE_GUARD != 0 {
                stacks.insert(m.AllocationBase as usize);
            }
        }
        let next = (m.BaseAddress as usize).saturating_add(size);
        if size == 0 || next <= at {
            break;
        }
        at = next;
    }
    // (A shot has no big block: BLOXKEEP_SHOT_BIG_MB makes smaller ones count, to try it.)
    #[cfg(feature = "shot")]
    let big_at = std::env::var("BLOXKEEP_SHOT_BIG_MB").ok().and_then(|v| v.parse::<usize>().ok()).map_or(BIG, |mb| mb << 20);
    #[cfg(not(feature = "shot"))]
    let big_at = BIG;
    let big: HashSet<usize> = size_of.iter().filter(|(b, n)| **n >= big_at && !own.contains(*b) && !stacks.contains(*b)).map(|(b, _)| *b).collect();
    let mods = modules();
    let mut g = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let Some((seen, seen_mods)) = g.as_mut() else {
        *g = Some((big, mods));
        return None;
    };
    let new: Vec<usize> = big.iter().filter(|b| !seen.contains(b)).copied().collect();
    let mut loaded: Vec<String> = mods.difference(seen_mods).cloned().collect();
    loaded.sort();
    *seen = big;
    *seen_mods = mods;
    if new.is_empty() {
        return None;
    }
    let told: Vec<String> = new
        .iter()
        .map(|&b| {
            let size = size_of[&b];
            let (resident, zero) = look_at(b, size);
            format!("{b:#x} {:.1} MB, {:.1} MB of it in RAM{}", size as f64 / 1048576., resident as f64 / 1048576., if resident == 0 { "" } else if zero { ", all zero" } else { ", with data" })
        })
        .collect();
    Some(format!(
        "Memory: a new block outside Rust's heap: {}; modules loaded since the last look (10 s): {}; Rust's heap {:.0} MB, at most {:.0} MB meanwhile.",
        told.join("; "),
        if loaded.is_empty() { "none".to_string() } else { loaded.join(", ") },
        crate::heap::in_use() as f64 / 1048576.,
        peak as f64 / 1048576.,
    ))
}

/// How much of the block is in RAM, and whether those pages are all zero (only pages already in RAM are read).
fn look_at(base: usize, size: usize) -> (usize, bool) {
    let pages = size / 4096;
    let mut info: Vec<PSAPI_WORKING_SET_EX_INFORMATION> = (0..pages)
        .map(|p| {
            // SAFETY: a plain struct of an address and its attributes, filled in by the call below.
            let mut i: PSAPI_WORKING_SET_EX_INFORMATION = unsafe { std::mem::zeroed() };
            i.VirtualAddress = (base + p * 4096) as *mut _;
            i
        })
        .collect();
    // SAFETY: the buffer holds `info.len()` entries and says so in bytes; the call fills in their attributes.
    if info.is_empty() || unsafe { K32QueryWorkingSetEx(GetCurrentProcess(), info.as_mut_ptr().cast(), (info.len() * std::mem::size_of::<PSAPI_WORKING_SET_EX_INFORMATION>()) as u32) } == 0 {
        return (0, false);
    }
    let mut resident = 0;
    let mut zero = true;
    for (p, i) in info.iter().enumerate() {
        // SAFETY: the union's Flags is the whole attribute word: bit 0 Valid.
        if unsafe { i.VirtualAttributes.Flags } & 1 == 0 {
            continue;
        }
        resident += 4096;
        // (Every 16th page in RAM is read: in RAM already, so reading it brings nothing in.)
        if zero && p % 16 == 0 {
            // (ReadProcessMemory, not a plain read: a block given back meanwhile fails the call instead of the app.)
            let mut words = [0u64; 512];
            let mut got = 0usize;
            // SAFETY: reads this process's own memory into this frame's buffer, at most its size; fails if unmapped.
            let ok = unsafe { ReadProcessMemory(GetCurrentProcess(), (base + p * 4096) as *const _, words.as_mut_ptr().cast(), 4096, &mut got) } != 0;
            zero = !ok || got != 4096 || words.iter().all(|&w| w == 0);
        }
    }
    (resident, zero)
}
