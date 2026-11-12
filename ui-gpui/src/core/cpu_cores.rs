//! Background Roblox windows run in Windows' efficiency mode (EcoQoS, leaning on efficiency cores where the CPU has them,
//! and below-normal priority), giving way to the one in front. Never pinned to the efficiency cores: pinned, 4 windows
//! filled them while the fast cores idled and one got 2-5 % of a core (the owner's PC, 2026-10-21). Only Windows'
//! switches: nothing in Roblox is touched.
use std::{ffi::c_void, sync::OnceLock};

use windows_sys::Win32::{
    Foundation::CloseHandle,
    System::{
        SystemInformation::GetSystemCpuSetInformation,
        Threading::{GetProcessAffinityMask, OpenProcess, SetPriorityClass, SetProcessAffinityMask, SetProcessInformation},
    },
};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ProcessPowerThrottlingState {
    version: u32,
    control_mask: u32,
    state_mask: u32,
}

const NORMAL_PRIORITY_CLASS: u32 = 0x20;
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x4000;

const PROCESS_SET_INFORMATION: u32 = 0x0200;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const PROCESS_POWER_THROTTLING: i32 = 4;
const PROCESS_POWER_THROTTLING_CURRENT_VERSION: u32 = 1;
const PROCESS_POWER_THROTTLING_EXECUTION_SPEED: u32 = 1;

static EFFICIENT_MASK: OnceLock<u64> = OnceLock::new();

/// The efficiency cores' bits in an affinity mask, 0 when the CPU has one kind of core (or over 64 threads).
pub fn efficient_mask() -> u64 {
    *EFFICIENT_MASK.get_or_init(read_efficient_mask)
}

/// Logical processors that are efficiency cores, and all of them (for Settings' words).
pub fn efficient_count() -> i32 {
    efficient_mask().count_ones() as i32
}

/// Environment.ProcessorCount.
pub fn logical_count() -> i32 {
    std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(1)
}

pub fn hybrid() -> bool {
    efficient_mask() != 0
}

fn read_efficient_mask() -> u64 {
    let mut length = 0u32;
    // SAFETY: the first call only asks the size; the buffer (u64s, so aligned as the structs need) has `length` bytes.
    unsafe {
        GetSystemCpuSetInformation(std::ptr::null_mut(), 0, &mut length, std::ptr::null_mut(), 0);
        if length == 0 {
            return 0;
        }
        let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
        if GetSystemCpuSetInformation(buffer.as_mut_ptr() as *mut _, length, &mut length, std::ptr::null_mut(), 0) == 0 {
            return 0;
        }
        let bytes = std::slice::from_raw_parts(buffer.as_ptr() as *const u8, buffer.len() * 8);
        // SYSTEM_CPU_SET_INFORMATION: Size, Type, then the CPU set: Id (8), Group (12), LogicalProcessorIndex (14),
        // CoreIndex, LastLevelCacheIndex, NumaNodeIndex, EfficiencyClass (18). A higher class is a faster core.
        let mut sets: Vec<(i32, i32, i32)> = Vec::new();
        let mut offset = 0usize;
        while offset < length as usize {
            if offset + 19 > bytes.len() {
                break;
            }
            let size = i32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]);
            if size <= 0 {
                break;
            }
            let group = i16::from_le_bytes([bytes[offset + 12], bytes[offset + 13]]) as i32;
            sets.push((group, bytes[offset + 14] as i32, bytes[offset + 18] as i32));
            offset += size as usize;
        }
        // One group only: an affinity mask covers 64 logical processors of one group.
        if sets.is_empty() || sets.iter().any(|s| s.0 != 0 || s.1 >= 64) {
            return 0;
        }
        let slowest = sets.iter().map(|s| s.2).min().unwrap_or(0);
        if sets.iter().all(|s| s.2 == slowest) {
            return 0; // one kind of core
        }
        let mut mask = 0u64;
        for s in sets.iter().filter(|s| s.2 == slowest) {
            mask |= 1u64 << s.1;
        }
        mask
    }
}

/// What a client gets: the efficiency cores and power saving, every core at full speed, or Windows' own choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Background,
    Front,
    Default,
}

/// Default (setting off, Bloxkeep closing) gives the choice back to Windows. Every core in all three roles
/// (undoing the efficiency-cores-only pinning of 1.0.8-beta.9 and before). False when Windows refused the handle.
pub fn set(pid: i32, role: Role) -> bool {
    // SAFETY: the handle is checked, used for these calls only and closed at the end.
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid as u32);
        if handle.is_null() {
            return false;
        }
        let mut ok = SetPriorityClass(handle, if role == Role::Background { BELOW_NORMAL_PRIORITY_CLASS } else { NORMAL_PRIORITY_CLASS }) != 0;
        let (mut mine, mut all) = (0usize, 0usize);
        if GetProcessAffinityMask(handle, &mut mine, &mut all) != 0 && mine != all {
            ok &= SetProcessAffinityMask(handle, all) != 0;
        }
        let state = ProcessPowerThrottlingState {
            version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            control_mask: if role == Role::Default { 0 } else { PROCESS_POWER_THROTTLING_EXECUTION_SPEED },
            state_mask: if role == Role::Background { PROCESS_POWER_THROTTLING_EXECUTION_SPEED } else { 0 },
        };
        ok &= SetProcessInformation(
            handle,
            PROCESS_POWER_THROTTLING,
            &state as *const ProcessPowerThrottlingState as *const c_void,
            std::mem::size_of::<ProcessPowerThrottlingState>() as u32,
        ) != 0;
        CloseHandle(handle);
        ok
    }
}
