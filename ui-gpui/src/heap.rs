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
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as alloc.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            more(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: ptr came from this allocator with this layout.
        unsafe { System.dealloc(ptr, layout) };
        less(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: as GlobalAlloc::realloc requires, passed on.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            less(layout.size());
            more(new_size);
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
