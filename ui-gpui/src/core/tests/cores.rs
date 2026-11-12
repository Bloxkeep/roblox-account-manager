// Tests/Bloxkeep.Tests/CoresSuite.cs: the fast cores for the window played (cpu_cores, app/cores.rs). The C# used a
// stand-in process it started (ping.exe); no program is started here, so the stand-in is the test's own process (as
// RamLimitSuite does): a window in the background goes into Windows' efficiency mode on every core, never pinned to the
// efficiency cores; the one in front gets full speed at normal priority, and turning it off (or Bloxkeep closing) gives
// Windows its own choice back. The check on a process that has exited while a handle holds it needs one started: skipped.
use std::ffi::c_void;

use serde_json::json;

use super::{call, unlock, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    base::Stamp,
    cpu_cores::{self, Role},
    roblox_processes::{self, PlayerProcess},
    system_memory::MemoryStatus,
    vault::Vault,
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn GetProcessAffinityMask(process: *mut c_void, mine: *mut usize, system: *mut usize) -> i32;
    fn SetProcessAffinityMask(process: *mut c_void, mask: usize) -> i32;
    fn GetProcessInformation(process: *mut c_void, class: i32, info: *mut c_void, size: u32) -> i32;
    fn GetPriorityClass(process: *mut c_void) -> u32;
    fn K32GetProcessMemoryInfo(process: *mut c_void, counters: *mut [u64; 10], size: u32) -> i32;
}

const BELOW_NORMAL: u32 = 0x4000;
const NORMAL: u32 = 0x20;

#[derive(Debug)]
#[allow(dead_code)]
struct Seen {
    affinity: u64,
    control: u32,
    state: u32,
    priority: u32,
}

/// The test process's cores, power throttling and priority.
fn read() -> Seen {
    // SAFETY: the pseudo-handle of this process; plain data out.
    unsafe {
        let me = GetCurrentProcess();
        let (mut mine, mut all) = (0usize, 0usize);
        GetProcessAffinityMask(me, &mut mine, &mut all);
        let mut t = [1u32, 0, 0];
        GetProcessInformation(me, 4, t.as_mut_ptr() as *mut c_void, 12);
        Seen { affinity: mine as u64, control: t[1], state: t[2], priority: GetPriorityClass(me) }
    }
}

/// The test process's RAM priority: 5 normal, 2 low.
pub(crate) fn memory_priority() -> u32 {
    let mut p = 0u32;
    // SAFETY: as above.
    unsafe { GetProcessInformation(GetCurrentProcess(), 0, &mut p as *mut u32 as *mut c_void, 4) };
    p
}

/// The test process's working set, in bytes.
pub(crate) fn working_set() -> i64 {
    // PROCESS_MEMORY_COUNTERS (64-bit): cb and PageFaultCount (4 bytes each), then PeakWorkingSetSize, WorkingSetSize…
    let mut c = [0u64; 10];
    c[0] = 72;
    // SAFETY: a buffer at least as large as the struct.
    unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, 72) };
    c[2] as i64
}

pub async fn run(c: &mut Checks, _dir: &str) {
    let me = std::process::id() as i32;
    let count = cpu_cores::logical_count();
    let all: u64 = if count >= 64 { u64::MAX } else { (1u64 << count) - 1 };
    let eff = cpu_cores::efficient_mask();
    let hybrid = eff != 0;
    println!("  (this CPU: {})", if hybrid { format!("two kinds of cores, efficiency mask 0x{eff:X}") } else { "one kind of core".into() });
    if hybrid {
        c.check("the efficiency cores are some of the cores, not all", eff != all && (eff & all) == eff, format!("0x{eff:X} of 0x{all:X}"));
    }

    c.check("a process in the background can be moved", cpu_cores::set(me, Role::Background), ());
    let bg = read();
    c.check("…not pinned to the efficiency cores: every core, so a fast one that is free still serves it", bg.affinity == all, format!("0x{:X}", bg.affinity));
    c.check("…in power-saving mode", bg.control == 1 && bg.state == 1, &bg);
    c.check("…at below-normal priority (it gives way to the one in front)", bg.priority == BELOW_NORMAL, &bg);
    if hybrid {
        // SAFETY: this process's own affinity, set back by the call checked.
        unsafe { SetProcessAffinityMask(GetCurrentProcess(), eff as usize) };
        cpu_cores::set(me, Role::Background);
        c.check("…and one an older Bloxkeep pinned to the efficiency cores is unpinned", read().affinity == all, format!("0x{:X}", read().affinity));
    }

    cpu_cores::set(me, Role::Front);
    let front = read();
    c.check("in front: every core again", front.affinity == all, format!("0x{:X}", front.affinity));
    c.check("…at full speed (power saving explicitly off), normal priority", front.control == 1 && front.state == 0 && front.priority == NORMAL, &front);

    cpu_cores::set(me, Role::Default);
    let back = read();
    c.check(
        "given back: every core, Windows deciding the speed, normal priority",
        back.affinity == all && back.control == 0 && back.state == 0 && back.priority == NORMAL,
        &back,
    );

    // Through the app: the setting on puts a client that isn't in front in the background; off gives it back.
    let seed = Vault::create(PASSWORD, None).expect("vault");
    seed.data().settings.split_cores = true;
    seed.save().expect("saved");
    drop(seed);
    let (app, said) = app();
    unlock(&app).await;
    app.split_cores(&[me]);
    let on = read();
    c.check("the setting on: a client not in front goes into efficiency mode", on.state == 1 && on.priority == BELOW_NORMAL && on.affinity == all, &on);
    let _ = call(&app, "setSettings", json!({ "splitCores": false })).await;
    let off = read();
    c.check("turned off: Windows' own choice at once", off.affinity == all && off.control == 0 && off.priority == NORMAL, &off);
    let _ = call(&app, "setSettings", json!({ "splitCores": true })).await;
    c.check("on again: back to the background cores", read().state == 1, read());

    // Another window brought to the front by the person: this one, in front before, goes to the background at once.
    // One anti-AFK brings forward for a moment: only a moment later.
    cpu_cores::set(me, Role::Front);
    app.cores.core_roles.borrow_mut().insert(me, Role::Front);
    app.controller.nudging.set(true);
    app.on_foreground();
    c.check("a window anti-AFK brings forward for a moment: the cores follow only a moment later", read().state == 0, read());
    app.controller.nudging.set(false);
    app.on_foreground();
    c.check("another window brought forward by the person: this one goes to the background at once", read().state == 1, read());

    // A client in front that ended since the last scan: nothing to move, and Windows refused nothing. "Gone altogether"
    // is a process id no process has (ids are multiples of 4; this one is far past any in use).
    const GONE: i32 = 0x7FFF_FFF0;
    c.skip("a client that has exited (a handle still on it): no \"Windows refused\" notice", "starts a program");
    app.cores.core_roles.borrow_mut().insert(GONE, Role::Front);
    app.cores.cores_refused_shown.set(false);
    said.borrow_mut().clear();
    app.split_cores(&[me, GONE]);
    let told = said.borrow().iter().any(|j| j.contains("refused to move"));
    c.check("…nor one that is gone altogether", !told, ());

    // The same for the background windows' memory (Settings' trim): one that closed since the scan.
    let _ = app.vault_mut(|d| d.settings.trim_background = true);
    said.borrow_mut().clear();
    app.guard_resources(
        &[PlayerProcess { pid: GONE, ..Default::default() }],
        &MemoryStatus { total_mb: 16000, available_mb: 8000, load_percent: 50, ..Default::default() },
        Stamp::utc_now(),
    );
    let trim_told = said.borrow().iter().any(|j| j.contains("memory priority"));
    c.check("a window that closed since the scan: no memory-priority notice", !trim_told, ());

    // The PC short of RAM (97 % used) with the setting on: a window in the background gets a low RAM priority, but its RAM
    // isn't emptied outright.
    let before = working_set();
    app.guard_resources(
        &[PlayerProcess { pid: me, ..Default::default() }],
        &MemoryStatus { total_mb: 32000, available_mb: 900, load_percent: 97, ..Default::default() },
        Stamp::utc_now(),
    );
    let after = working_set();
    c.check(
        "short of RAM: a window in the background keeps its RAM (not emptied)",
        after > before / 2,
        format!("{} KB -> {} KB", before / 1024, after / 1024),
    );
    c.check("…and has a low RAM priority", memory_priority() == 2, memory_priority());
    app.front_memory(me);
    c.check("brought to the front: its normal RAM priority back at once", memory_priority() == 5, memory_priority());

    // PORT: the C# disposed the whole app here. App::dispose also tells the bot this PC went offline and puts back
    // whatever Roblox windows the PC has open, so only its cores step runs (give_cores_back, as dispose calls it).
    app.give_cores_back();
    let closed = read();
    c.check("Bloxkeep closing gives the cores back", closed.affinity == all && closed.control == 0, &closed);
    let _ = roblox_processes::set_memory_priority(me, false);
}
