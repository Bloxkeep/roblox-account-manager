// AppController.Cores.cs: fast cores for the window you play (AppSettings.split_cores): background Roblox windows run in
// Windows' efficiency mode, the front one at full speed (cpu_cores.rs). Followed on a foreground WinEvent and every tick;
// a client is touched only when its role changes. Off or Bloxkeep closing: Windows' own choice again.
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

use serde_json::{Value, json};
use windows_sys::Win32::{
    Foundation::HWND,
    System::Threading::GetCurrentThreadId,
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        WindowsAndMessaging::{EVENT_SYSTEM_FOREGROUND, GetMessageW, MSG, PostThreadMessageW, WINEVENT_OUTOFCONTEXT, WM_QUIT},
    },
};

use super::App;
use crate::core::{
    cpu_cores::{self, Role},
    log, roblox_processes, rt,
};

/// The core check's wait after a foreground change (C#'s 700 ms Timer).
pub const CORE_CHECK: Duration = Duration::from_millis(700);

#[derive(Default)]
pub struct CoresState {
    /// A client not in it has Windows' own role (never touched, or given back).
    pub core_roles: RefCell<HashMap<i32, Role>>,
    pub core_pids: RefCell<Vec<i32>>,
    /// The setting as the vault last had it, followed while the vault is locked too (the windows go on).
    pub split_cores_on: Cell<bool>,
    pub cores_refused_shown: Cell<bool>,
    // PORT: `_foregroundHook` and `_foregroundProc`: the hook lives on a thread of its own (an out-of-context WinEvent
    // needs a message loop on the thread that set it); this is that thread's id, 0 when none.
    pub foreground_thread: Cell<u32>,
    // PORT: `_coreCheck` (a WinForms Timer): a wait on the core's thread; each Start or Stop bumps the generation, so only
    // the latest wait ticks. `core_check_off`: disposed.
    pub core_check_gen: Cell<u64>,
    pub core_check_off: Cell<bool>,
}

/// The out-of-context WinEvent's callback, on the hook's own thread: handed to the core's.
unsafe extern "system" fn on_foreground_event(_hook: HWINEVENTHOOK, _event: u32, _hwnd: HWND, _object: i32, _child: i32, _thread: u32, _time: u32) {
    rt::post(|| App::with(|app| app.on_foreground()));
}

impl App {
    /// A window brought to the front gets its cores at once: dragging a background one was slow for 0.7 s (2026-10-21).
    /// One brought forward for a moment (`windows_in_turn`) waits, so a nudge doesn't flip the windows back and forth.
    pub fn watch_foreground(&self) {
        self.cores.core_check_off.set(false);
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        std::thread::spawn(move || {
            // SAFETY: plain calls; the callback is a plain function, the hook is unhooked on this same thread before it
            // ends, and the loop reads messages for this thread only.
            unsafe {
                let hook = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, std::ptr::null_mut(), Some(on_foreground_event), 0, 0, WINEVENT_OUTOFCONTEXT);
                let _ = tx.send(if hook.is_null() { 0 } else { GetCurrentThreadId() });
                if hook.is_null() {
                    return;
                }
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                    if msg.message == WM_QUIT {
                        break;
                    }
                }
                UnhookWinEvent(hook);
            }
        });
        self.cores.foreground_thread.set(rx.recv().unwrap_or(0));
    }

    /// The foreground WinEvent, on the core's thread.
    pub fn on_foreground(self: &Rc<Self>) {
        if self.controller.disposed.get() {
            return;
        }
        self.step("FrontMemory(RobloxProcesses.ForegroundPid())", || {
            self.front_memory(roblox_processes::foreground_pid());
            Ok(())
        });
        self.step("PanelsFollowForeground", || {
            self.panels_follow_foreground();
            Ok(())
        });
        if !self.cores.split_cores_on.get() || self.cores.core_pids.borrow().is_empty() {
            return;
        }
        self.core_check_stop();
        if self.windows_in_turn() {
            self.core_check_start();
        } else {
            self.step("SplitCores(_corePids)", || {
                let pids = self.cores.core_pids.borrow().clone();
                self.split_cores(&pids);
                Ok(())
            });
        }
    }

    fn core_check_stop(&self) {
        self.cores.core_check_gen.set(self.cores.core_check_gen.get() + 1);
    }

    fn core_check_start(self: &Rc<Self>) {
        if self.cores.core_check_off.get() {
            return;
        }
        let generation = self.cores.core_check_gen.get() + 1;
        self.cores.core_check_gen.set(generation);
        let app = self.clone();
        rt::spawn(async move {
            rt::sleep(CORE_CHECK).await;
            if app.cores.core_check_gen.get() != generation || app.cores.core_check_off.get() {
                return;
            }
            app.core_check_tick();
        });
    }

    fn core_check_tick(self: &Rc<Self>) {
        self.core_check_stop();
        // Still in a turn (a long macro, a nudge): later, or the cores flip while the window is only lent.
        if self.windows_in_turn() {
            self.core_check_start();
        } else {
            self.step("SplitCores(_corePids)", || {
                let pids = self.cores.core_pids.borrow().clone();
                self.split_cores(&pids);
                Ok(())
            });
        }
    }

    /// The front window gets its normal RAM priority back at once, not at the next scan seconds later.
    pub fn front_memory(&self, front: i32) {
        if self.memory.low_memory_priority.borrow_mut().remove(&front) {
            roblox_processes::set_memory_priority(front, false);
        }
        // Its RAM limit too (not for anti-AFK's moment in front: limited again right after, for nothing).
        if !self.windows_in_turn() && self.memory.ram_limited.borrow_mut().remove(&front).is_some() {
            // C#: `_ = LimitRamInTurn(front, null)`: asked in turn, its answer not awaited.
            drop(self.limit_ram_in_turn(front, None));
            log::note(&format!("RAM limit: lifted for Roblox window {front} (came to the front)."));
        }
    }

    pub fn split_cores<'a>(&self, pids: impl IntoIterator<Item = &'a i32>) {
        let pids: Vec<i32> = pids.into_iter().copied().collect();
        if let Ok(on) = self.vault(|d| d.settings.split_cores) {
            self.cores.split_cores_on.set(on);
        }
        let on = self.cores.split_cores_on.get();
        *self.cores.core_pids.borrow_mut() = pids.clone();
        self.cores.core_roles.borrow_mut().retain(|pid, _| pids.contains(pid));
        let front = if on { roblox_processes::foreground_pid() } else { 0 };
        let main = self.main_pids(); // a main account's window keeps the fast cores behind others too
        let mut refused = false;
        for &pid in &pids {
            let want = if !on {
                Role::Default
            } else if pid == front || main.contains(&pid) {
                Role::Front
            } else {
                Role::Background
            };
            let same = match self.cores.core_roles.borrow().get(&pid) {
                Some(had) => *had == want,
                None => want == Role::Default,
            };
            if same {
                continue;
            }
            if !cpu_cores::set(pid, want) {
                // A client that ended since the last scan (often what brought another to the front) is no refusal.
                refused |= !roblox_processes::ended(pid);
                continue;
            }
            if want == Role::Default {
                self.cores.core_roles.borrow_mut().remove(&pid);
            } else {
                self.cores.core_roles.borrow_mut().insert(pid, want);
            }
        }
        if refused && on && !self.cores.cores_refused_shown.get() {
            self.cores.cores_refused_shown.set(true);
            // Nothing the person can do about it: noted, not shown.
            log::note("Fast cores: Windows refused to change a Roblox window's priority; it was left as it was.");
        }
    }

    pub fn give_cores_back(&self) {
        let thread = self.cores.foreground_thread.replace(0);
        if thread != 0 {
            // SAFETY: plain call; the hook's thread unhooks and ends on WM_QUIT.
            unsafe {
                PostThreadMessageW(thread, WM_QUIT, 0, 0);
            }
        }
        self.cores.core_check_off.set(true);
        self.core_check_stop();
        let pids: Vec<i32> = self.cores.core_roles.borrow().keys().copied().collect();
        for pid in pids {
            cpu_cores::set(pid, Role::Default);
        }
        self.cores.core_roles.borrow_mut().clear();
    }

    pub fn cores_dto() -> Value {
        json!({ "hybrid": cpu_cores::hybrid(), "efficient": cpu_cores::efficient_count(), "logical": cpu_cores::logical_count() })
    }
}
