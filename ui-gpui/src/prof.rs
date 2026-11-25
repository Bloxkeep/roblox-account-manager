//! A shot's CPU sampler (BLOXKEEP_SHOT_PROF=<file>, with BLOXKEEP_SHOT_BENCH): about every millisecond the UI thread
//! is stopped, its stack walked (the exe's unwind tables) and let go; at the end every stack is written as
//! "1<TAB>1<TAB>rva,rva,..." (E:/Rust/pdb/rva.py's heap-sites format), read against the build's pdb. Built only into
//! shot builds: it stops the thread it measures.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering::Relaxed},
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        Diagnostics::Debug::{CONTEXT, CONTEXT_CONTROL_AMD64, CONTEXT_INTEGER_AMD64, GetThreadContext, RtlLookupFunctionEntry, RtlVirtualUnwind},
        LibraryLoader::GetModuleHandleW,
        Threading::{GetCurrentThreadId, OpenThread, ResumeThread, SuspendThread, THREAD_GET_CONTEXT, THREAD_QUERY_INFORMATION, THREAD_SUSPEND_RESUME},
    },
};

pub struct Sampler {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<Vec<Vec<u64>>>>,
}

/// Starts sampling the calling thread.
pub fn start() -> Sampler {
    let id = unsafe { GetCurrentThreadId() };
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let join = crate::threads::own("prof", move || {
        let h: HANDLE = unsafe { OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, 0, id) };
        let mut out: Vec<Vec<u64>> = Vec::with_capacity(20_000);
        if h.is_null() {
            return out;
        }
        let mut frames = [0u64; 96];
        while !flag.load(Relaxed) {
            std::thread::sleep(std::time::Duration::from_micros(900));
            let n = unsafe { sample(h, &mut frames) };
            if n > 0 {
                out.push(frames[..n].to_vec());
            }
        }
        unsafe { CloseHandle(h) };
        out
    })
    .ok();
    Sampler { stop, join }
}

#[repr(C, align(16))]
struct Aligned(CONTEXT);

/// One stack, walked while the thread stands still (nothing allocated meanwhile: it may hold the heap's lock).
unsafe fn sample(h: HANDLE, frames: &mut [u64; 96]) -> usize {
    unsafe {
        if SuspendThread(h) == u32::MAX {
            return 0;
        }
        // (GetThreadContext wants its CONTEXT on 16 bytes, as winnt.h declares it; windows-sys' is only 8-aligned and
        // every call failed: no stack was ever taken.)
        let mut held: Aligned = std::mem::zeroed();
        let ctx = &mut held.0;
        ctx.ContextFlags = CONTEXT_CONTROL_AMD64 | CONTEXT_INTEGER_AMD64;
        let mut n = 0;
        if GetThreadContext(h, ctx) != 0 {
            while n < frames.len() {
                let rip = ctx.Rip;
                if rip == 0 {
                    break;
                }
                frames[n] = rip;
                n += 1;
                let mut base = 0u64;
                let f = RtlLookupFunctionEntry(rip, &mut base, std::ptr::null_mut());
                if f.is_null() {
                    if ctx.Rsp == 0 {
                        break;
                    }
                    ctx.Rip = *(ctx.Rsp as *const u64);
                    ctx.Rsp += 8;
                } else {
                    let mut data = std::ptr::null_mut();
                    let mut frame = 0u64;
                    RtlVirtualUnwind(0, base, rip, f, ctx, &mut data, &mut frame, std::ptr::null_mut());
                }
            }
        }
        ResumeThread(h);
        n
    }
}

impl Sampler {
    /// Stops and writes the stacks to `path` as offsets into the exe (frames outside it are left out).
    pub fn finish(mut self, path: &std::path::Path) {
        self.stop.store(true, Relaxed);
        let Some(all) = self.join.take().and_then(|j| j.join().ok()) else { return };
        let base = unsafe { GetModuleHandleW(std::ptr::null()) } as u64;
        let size = 512 << 20;
        let mut text = String::new();
        for s in &all {
            let rvas: Vec<String> = s.iter().filter(|a| **a >= base && **a < base + size).map(|a| format!("{:x}", a - base)).collect();
            if !rvas.is_empty() {
                text.push_str("1\t1\t");
                text.push_str(&rvas.join(","));
                text.push('\n');
            }
        }
        let _ = std::fs::write(path, text);
        eprintln!("prof: {} samples", all.len());
    }
}
