// Where the app's memory in RAM (Task Manager's number: private pages in the working set) lies, by kind: the app's own
// heap (Rust's allocations and the C code sharing it), Windows' other heaps, thread stacks, and the rest (memory taken
// straight from Windows: the graphics driver's, Direct3D's, a decoder's), the biggest of those by allocation. The
// owner, 2026-11-22: "điều tra kỹ đi" — the memory line said what Rust's heap holds but not what else was in RAM.
// The heap's own memory is told by where the heap samples (heap.rs sites) lie, not by walking the heap: HeapWalk reads
// every block's header and brought every page of the heap back into RAM (a shot: 49 MB in RAM before, 243 after).
#![cfg(windows)]

use std::collections::{HashMap, HashSet};

use windows_sys::Win32::{
    System::{
        Memory::{GetProcessHeap, MEM_COMMIT, MEM_PRIVATE, MEMORY_BASIC_INFORMATION, PAGE_GUARD, VirtualQuery},
        ProcessStatus::{K32QueryWorkingSetEx, PSAPI_WORKING_SET_EX_INFORMATION},
        Threading::GetCurrentProcess,
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
