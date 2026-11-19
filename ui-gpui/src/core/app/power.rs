//! AppController.Power.cs: sleep and wake, the network coming and going, and the clock changing: what the app holds back or
//! catches up on meanwhile.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use super::App;
use super::controller::RESUME_GRACE;
use crate::core::base::Stamp;
use crate::core::models::stamp_min;
use crate::core::rt;

pub const NETWORK_ASK_INTERVAL: Duration = Duration::from_secs(30);

/// SystemEvents.PowerModeChanged's PowerModes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerMode {
    Resume,
    StatusChange,
    Suspend,
}

pub struct PowerState {
    /// Kept by Windows' notification: asking lists every adapter, too slow for each process tick.
    pub network_up: Cell<bool>,
    pub network_asked_at: Cell<Stamp>,
    /// Raised on the core's thread when the PC wakes from sleep.
    pub resumed: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Default for PowerState {
    fn default() -> PowerState {
        PowerState { network_up: Cell::new(is_network_available()), network_asked_at: Cell::new(stamp_min()), resumed: RefCell::new(None) }
    }
}

/// NetworkInterface.GetIsNetworkAvailable: some adapter that is up and isn't the loopback or a tunnel.
pub fn is_network_available() -> bool {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, IF_TYPE_SOFTWARE_LOOPBACK, IF_TYPE_TUNNEL,
        IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Networking::WinSock::AF_UNSPEC;
    const ERROR_BUFFER_OVERFLOW: u32 = 111;
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size: u32 = 16 * 1024;
    for _ in 0..3 {
        // u64 units keep the buffer aligned for IP_ADAPTER_ADDRESSES_LH.
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let first = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        // SAFETY: `buffer` holds `size` bytes and outlives the call; Windows writes the list into it.
        let result = unsafe { GetAdaptersAddresses(AF_UNSPEC as u32, flags, std::ptr::null(), first, &mut size) };
        if result == ERROR_BUFFER_OVERFLOW {
            continue;
        }
        if result != 0 {
            return false;
        }
        let mut next = first.cast_const();
        while !next.is_null() {
            // SAFETY: each entry is in `buffer`, linked by Windows; read while `buffer` lives.
            let adapter = unsafe { &*next };
            if adapter.OperStatus == IfOperStatusUp && adapter.IfType != IF_TYPE_SOFTWARE_LOOPBACK && adapter.IfType != IF_TYPE_TUNNEL {
                return true;
            }
            next = adapter.Next.cast_const();
        }
        return false;
    }
    false
}

/// SystemEvents.PowerModeChanged, from the window's thread (WM_POWERBROADCAST). Going to sleep: a pending save and backup are
/// written first (the PC may never wake up the same way); waited for, since Windows suspends right after this. Waking: on the
/// core's thread, when it gets there.
pub fn on_power_mode_changed(mode: PowerMode) {
    match mode {
        PowerMode::Suspend => {
            let (done, wait) = std::sync::mpsc::channel::<()>();
            rt::post(move || {
                App::with(|app| app.on_suspending());
                let _ = done.send(());
            });
            // The core's thread is gone (closing): nothing more to wait for. Windows gives this message only a few
            // seconds; a core held up elsewhere mustn't freeze the window's thread past that.
            let _ = wait.recv_timeout(Duration::from_secs(3));
        }
        PowerMode::Resume => rt::post(|| App::with(|app| app.on_resumed())),
        PowerMode::StatusChange => {}
    }
}

/// SystemEvents.TimeChanged: the time zone was changed: local times (the schedules' minutes) must come from the new one, not a
/// cached one.
pub fn on_time_changed() {
    // PORT: TimeZoneInfo.ClearCachedData(): crate::core::time reads Windows' offset at each call and caches nothing, so there
    // is nothing to clear.
}

/// NetworkChange.NetworkAvailabilityChanged's watcher: a thread that waits on Windows' address-change notice and passes on
/// each change of `is_network_available`.
pub fn watch_network() {
    let _ = crate::threads::own("Bloxkeep network watch", || {
        let mut was = is_network_available();
        loop {
            // SAFETY: both null: the call blocks until an IPv4 address changes.
            let r = unsafe { windows_sys::Win32::NetworkManagement::IpHelper::NotifyAddrChange(std::ptr::null_mut(), std::ptr::null()) };
            if r != 0 {
                // Not offered: network_up asks again every 30 s while down.
                return;
            }
            // Adapters settle over a moment: one look after it.
            std::thread::sleep(Duration::from_millis(500));
            let now = is_network_available();
            if now != was {
                was = now;
                on_network_availability_changed(now);
            }
        }
    });
}

/// NetworkChange.NetworkAvailabilityChanged, from the network watcher's thread.
pub fn on_network_availability_changed(available: bool) {
    rt::post(move || App::with(|app| app.power.network_up.set(available)));
}

impl App {
    /// The PC woke up less than a minute ago, or has no network: auto-rejoin holds off.
    pub fn settling(&self) -> bool {
        Stamp::utc_now().secs_since(self.controller.resumed_at.get()) < RESUME_GRACE.as_secs_f64() || !self.network_up()
    }

    /// Asked again every 30 s while "down": an "available" notification that never came (a wake, a Wi-Fi flap, two in
    /// the wrong order) would otherwise hold auto-rejoin and the sign-in check back until the next change.
    pub fn network_up(&self) -> bool {
        let now = Stamp::utc_now();
        let asked = self.power.network_asked_at.get();
        if !self.power.network_up.get() && (now.secs_since(asked) >= NETWORK_ASK_INTERVAL.as_secs_f64() || now < asked) {
            self.power.network_asked_at.set(now);
            self.power.network_up.set(is_network_available());
        }
        self.power.network_up.get()
    }

    pub fn on_suspending(self: &Rc<Self>) {
        if self.controller.disposed.get() {
            return;
        }
        self.stats.asleep_tick.set(tick_count64());
        self.stop_macros_for_sleep();
        if self.vault_ref().is_none() {
            // Stays left open by the lock end at this sleep, once unlocked (see ResumePlaySessions).
            if self.stats.slept_while_locked.get().is_none() {
                self.stats.slept_while_locked.set(Some(Stamp::utc_now()));
            }
            return;
        }
        // Open stays end here: time asleep isn't play time, and the drop every client logs at wake isn't a kick.
        let suspended = self.suspend_play(Stamp::utc_now());
        let open = self.note_open_sessions();
        let runs = self.note_scheduled_runs();
        if suspended | open | runs | self.persist.save_due.get().is_some() {
            self.persist();
        }
        self.backup_if_dirty();
    }

    pub fn on_resumed(&self) {
        if self.controller.disposed.get() {
            return;
        }
        self.stats.asleep_tick.set(0);
        let resumed_at = Stamp::utc_now();
        self.controller.resumed_at.set(resumed_at);
        self.controller.offline_strikes.borrow_mut().clear();
        self.controller.next_presence.set(resumed_at.plus_secs(RESUME_GRACE.as_secs_f64()));
        let soon = resumed_at.plus_secs(120.0);
        if self.updates.next_update_check.get() > soon {
            self.updates.next_update_check.set(soon);
        }
        let resumed = self.power.resumed.borrow().clone();
        if let Some(resumed) = resumed {
            resumed();
        }
    }
}

/// Environment.TickCount64.
fn tick_count64() -> i64 {
    // SAFETY: no arguments; reads the milliseconds since Windows started.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() as i64 }
}
