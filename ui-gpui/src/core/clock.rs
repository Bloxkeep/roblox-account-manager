//! The time the schedules read: the wall clock and uptime, swapped by tests to walk a clock forward, back or across
//! a DST change (each test suite runs in a process of its own, so a swap never leaks into another suite).
use std::sync::RwLock;

use super::base::Stamp;

type NowFn = Box<dyn Fn() -> Stamp + Send + Sync>;
type TickFn = Box<dyn Fn() -> i64 + Send + Sync>;

static NOW: RwLock<Option<NowFn>> = RwLock::new(None);
static UTC_NOW: RwLock<Option<NowFn>> = RwLock::new(None);
static TICK: RwLock<Option<TickFn>> = RwLock::new(None);

/// DateTime.Now, unless a test swapped it.
pub fn now() -> Stamp {
    match NOW.read().ok().as_deref().and_then(|f| f.as_ref().map(|f| f())) {
        Some(at) => at,
        None => Stamp::now(),
    }
}

/// DateTime.UtcNow, unless a test swapped it.
pub fn utc_now() -> Stamp {
    match UTC_NOW.read().ok().as_deref().and_then(|f| f.as_ref().map(|f| f())) {
        Some(at) => at,
        None => Stamp::utc_now(),
    }
}

/// Environment.TickCount64: milliseconds since Windows started, never set back.
pub fn tick() -> i64 {
    match TICK.read().ok().as_deref().and_then(|f| f.as_ref().map(|f| f())) {
        Some(t) => t,
        None => tick_count64(),
    }
}

/// Swaps the wall clock (None: the real one again).
pub fn set_now(f: Option<NowFn>) {
    if let Ok(mut w) = NOW.write() {
        *w = f;
    }
}

pub fn set_utc_now(f: Option<NowFn>) {
    if let Ok(mut w) = UTC_NOW.write() {
        *w = f;
    }
}

pub fn set_tick(f: Option<TickFn>) {
    if let Ok(mut w) = TICK.write() {
        *w = f;
    }
}

/// Environment.TickCount64 itself.
pub fn tick_count64() -> i64 {
    // SAFETY: plain call without arguments.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() as i64 }
}
