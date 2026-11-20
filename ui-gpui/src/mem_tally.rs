// What the app's memory is made of, in error.log: the process's private bytes and working set beside what the app can
// count itself (Rust's heap, the pixels pictures hold, the graphics card's picture atlas, the pictures kept). The
// owner, 2026-11-20: Task Manager showed 167 MB, then over 250, a peak of 596 MB, and nothing said of what; measured
// from outside, a dozen private blocks of 11-27 MB grew while the Roblox tab was used, too big and too few to name.
// A line a minute after the start, every 10 minutes, and whenever Task Manager's number grew 64 MB past the last line
// (at most one a minute), so a peak is caught with what made it.
use std::time::{Duration, Instant};

use gpui_kit::App;

const FIRST: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_secs(600);
const LOOK: Duration = Duration::from_secs(10);
const JUMP: usize = 64 << 20;
const QUIET: Duration = Duration::from_secs(60);

pub fn start(cx: &mut App) {
    cx.spawn(async move |cx| {
        // (A shot's hidden window lives seconds: BLOXKEEP_SHOT_MEM_AT=ms writes the first line then.)
        #[cfg(feature = "shot")]
        let first = std::env::var("BLOXKEEP_SHOT_MEM_AT").ok().and_then(|v| v.parse().ok()).map(Duration::from_millis).unwrap_or(FIRST);
        #[cfg(not(feature = "shot"))]
        let first = FIRST;
        cx.background_executor().timer(first).await;
        let mut last = cx.update(|cx| line("a minute after the start", cx));
        let mut at = Instant::now();
        loop {
            cx.background_executor().timer(LOOK).await;
            let ram = process().1;
            let why = if at.elapsed() >= EVERY {
                "every 10 min".to_string()
            } else if ram >= last + JUMP && at.elapsed() >= QUIET {
                format!("up {} MB since the last line", mb(ram - last))
            } else {
                continue;
            };
            last = cx.update(|cx| line(&why, cx));
            at = Instant::now();
        }
    })
    .detach();
}

/// Writes the line; the memory in RAM it saw (Task Manager's number).
fn line(why: &str, cx: &mut App) -> usize {
    let (private, private_working, working, peak_working) = process();
    let heap = crate::heap::in_use();
    #[cfg(feature = "shot")]
    if let Ok(to) = std::env::var("BLOXKEEP_SHOT_SITES") {
        crate::heap::sites::report(&to, heap);
    }
    let heap_peak = crate::heap::take_peak();
    let pics = gpui_kit::held_pictures(6);
    let (textures, atlas) = gpui_kit::atlas_textures();
    let (kept, files, copies) = crate::images::kept(cx);
    let biggest: Vec<String> = pics
        .biggest
        .iter()
        .map(|((w, h), frames, bytes, what)| {
            let size = *bytes as f64 / 1048576.;
            let what = if what.is_empty() { String::new() } else { format!(" {what}") };
            if *frames > 1 { format!("{w}x{h} x{frames} frames{what} {size:.1} MB") } else { format!("{w}x{h}{what} {size:.1} MB") }
        })
        .collect();
    let (card, system) = gpu();
    let ((own_committed, own_used), (other_committed, other_used, other_count)) = heaps();
    write(&format!(
        "Memory ({why}): private {} MB, in RAM {} MB (Task Manager's number), working set {} MB (peak {} MB); Rust heap {} MB (peak {} MB since the last line), of it picture pixels {} MB in {} of {} pictures (biggest: {}); graphics card atlas {} textures, {} MB; pictures kept {} ({} MB as files, {} smaller copies); graphics memory this process uses {} MB on the card, {} MB in system memory; not in Rust's heap (Windows, graphics driver, freed heap): {} MB; Windows heaps: the app's {} MB kept, {} MB of it in use; {} others (graphics driver, Windows) {} MB kept, {} MB in use.",
        mb(private),
        mb(private_working),
        mb(working),
        mb(peak_working),
        mb(heap),
        mb(heap_peak),
        mb(pics.bytes),
        pics.holding,
        pics.images,
        if biggest.is_empty() { "none".to_string() } else { biggest.join(", ") },
        textures,
        mb(atlas),
        kept,
        mb(files),
        copies,
        mb(card),
        mb(system),
        mb(private.saturating_sub(heap)),
        mb(own_committed),
        mb(own_used),
        other_count,
        mb(other_committed),
        mb(other_used),
    ));
    #[cfg(feature = "core")]
    write(&crate::heap::sites::holders(12));
    private_working
}

fn mb(bytes: usize) -> usize {
    (bytes + (1 << 19)) >> 20
}

/// The graphics memory this process uses, as the graphics card's driver says (DXGI's QueryVideoMemoryInfo, every
/// adapter): on the card, and in system memory (counted in private bytes too).
fn gpu() -> (usize, usize) {
    use windows::{
        Win32::Graphics::Dxgi::{CreateDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO, IDXGIAdapter3, IDXGIFactory1},
        core::Interface,
    };
    // SAFETY: plain DXGI calls with this frame's own values; nothing is kept.
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else { return (0, 0) };
        let (mut card, mut system) = (0u64, 0u64);
        let mut i = 0;
        while let Ok(adapter) = factory.EnumAdapters1(i) {
            i += 1;
            let Ok(adapter) = adapter.cast::<IDXGIAdapter3>() else { continue };
            let mut m = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
            if adapter.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut m).is_ok() {
                card += m.CurrentUsage;
            }
            let mut m = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
            if adapter.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL, &mut m).is_ok() {
                system += m.CurrentUsage;
            }
        }
        (card as usize, system as usize)
    }
}

/// The process's heaps as Windows keeps them (HeapSummary): memory kept for each (committed) and what of it is in use,
/// for the app's own heap (Rust's allocations) and for the others together (the graphics driver's, Windows'). Kept and
/// not in use is freed memory the heap holds on to.
#[cfg(windows)]
fn heaps() -> ((usize, usize), (usize, usize, usize)) {
    use windows_sys::Win32::{
        Foundation::HANDLE,
        System::Memory::{GetProcessHeap, GetProcessHeaps, HEAP_SUMMARY, HeapSummary},
    };
    let summary = |h: HANDLE| {
        let mut s = HEAP_SUMMARY { cb: std::mem::size_of::<HEAP_SUMMARY>() as u32, cbAllocated: 0, cbCommitted: 0, cbReserved: 0, cbMaxReserve: 0 };
        // SAFETY: h is one of this process's heaps; s is this frame's own, its size in cb.
        if unsafe { HeapSummary(h, 0, &mut s) } == 0 { (0, 0) } else { (s.cbCommitted, s.cbAllocated) }
    };
    let mut all: Vec<HANDLE> = vec![std::ptr::null_mut(); 256];
    // SAFETY: the buffer holds as many handles as it says; the call writes at most that many.
    let n = (unsafe { GetProcessHeaps(all.len() as u32, all.as_mut_ptr()) } as usize).min(all.len());
    // SAFETY: no arguments; the process heap of this process.
    let own = unsafe { GetProcessHeap() };
    let mut others = (0, 0, 0);
    for &h in &all[..n] {
        if h != own {
            let (k, u) = summary(h);
            others = (others.0 + k, others.1 + u, others.2 + 1);
        }
    }
    (summary(own), others)
}

#[cfg(not(windows))]
fn heaps() -> ((usize, usize), (usize, usize, usize)) {
    ((0, 0), (0, 0, 0))
}

/// This process's private bytes, private working set (Task Manager's "Memory"), working set and peak working set.
#[cfg(windows)]
fn process() -> (usize, usize, usize, usize) {
    use windows_sys::Win32::System::{
        ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        Threading::GetCurrentProcess,
    };
    /// PROCESS_MEMORY_COUNTERS_EX2 (Windows 10 1809 on), which adds the private working set.
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        quota: [usize; 4],
        pagefile: usize,
        peak_pagefile: usize,
        private: usize,
        private_working_set: usize,
        shared_commit: u64,
    }
    let mut c = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Default::default() };
    // SAFETY: GetCurrentProcess is a pseudo handle; c is this frame's own, laid out as Windows's, its size in cb.
    if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), (&mut c as *mut Counters).cast::<PROCESS_MEMORY_COUNTERS>(), c.cb) } == 0 {
        return (0, 0, 0, 0);
    }
    (c.private, c.private_working_set, c.working_set, c.peak_working_set)
}

#[cfg(not(windows))]
fn process() -> (usize, usize, usize, usize) {
    (0, 0, 0, 0)
}

fn write(line: &str) {
    #[cfg(feature = "core")]
    crate::core::log::note_file(line);
    #[cfg(not(feature = "core"))]
    eprintln!("{line}");
}
