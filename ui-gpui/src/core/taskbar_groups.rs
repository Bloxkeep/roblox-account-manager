//! Which taskbar button a Roblox window sits under: Windows groups by AppUserModelID, so an id per account or preset
//! gives its own button, which also shows the window's icon (window_icons). A window property only, nothing
//! in Roblox is touched; each window's own id is put back when turned off or on close. UI thread only.
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::{LazyLock, Mutex},
};

use windows_sys::core::GUID;

use super::{log, win_kit::hwnd};

/// IPropertyStore's table of calls (IUnknown's three first).
#[repr(C)]
#[allow(dead_code)]
struct PropertyStoreVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32,
    get_at: unsafe extern "system" fn(*mut c_void, u32, *mut PropertyKey) -> i32,
    get_value: unsafe extern "system" fn(*mut c_void, *const PropertyKey, *mut PropVariant) -> i32,
    set_value: unsafe extern "system" fn(*mut c_void, *const PropertyKey, *const PropVariant) -> i32,
    commit: unsafe extern "system" fn(*mut c_void) -> i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PropertyKey {
    format: GUID,
    id: u32,
}

/// LayoutKind.Explicit, Size = 24: the type at 0, the pointer at 8.
#[repr(C, align(8))]
struct PropVariant {
    kind: u16,
    _reserved: [u16; 3],
    pointer: *mut c_void,
    _rest: [u8; 8],
}

impl PropVariant {
    fn empty() -> PropVariant {
        PropVariant { kind: VT_EMPTY, _reserved: [0; 3], pointer: std::ptr::null_mut(), _rest: [0; 8] }
    }
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetPropertyStoreForWindow(hwnd: *mut c_void, iid: *const GUID, store: *mut *mut c_void) -> i32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn PropVariantClear(pvar: *mut PropVariant) -> i32;
    fn CoTaskMemAlloc(size: usize) -> *mut c_void;
}

const VT_EMPTY: u16 = 0;
const VT_LPWSTR: u16 = 31;
const APP_USER_MODEL_ID: PropertyKey = PropertyKey { format: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3), id: 5 };
const PROPERTY_STORE_IID: GUID = GUID::from_u128(0x886D8EEB_8CF2_4446_8D02_CDBA1DBDCF99);

/// A client's group, and each of its windows' own id from before (put back when it goes).
#[derive(Clone)]
struct Placed {
    group: String,
    own: HashMap<isize, Option<String>>,
}

static BY_PID: LazyLock<Mutex<HashMap<i32, Placed>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Puts the client's windows in this group (an AppUserModelID of Bloxkeep's), noting their own ids first.
pub fn apply(pid: i32, windows: &[isize], group: &str) {
    let mut by_pid = BY_PID.lock().unwrap_or_else(|e| e.into_inner());
    let had_group = by_pid.get(&pid).map(|p| p.group.clone());
    let placed = by_pid.entry(pid).or_insert_with(|| Placed { group: group.to_string(), own: HashMap::new() });
    placed.group = group.to_string();
    let mut written = 0;
    for &h in windows {
        let now = read(h);
        if now.as_deref() == Some(group) {
            continue;
        }
        // An id of Bloxkeep's already there was left by a run that ended suddenly: not the window's own.
        placed.own.entry(h).or_insert_with(|| if is_ours(now.as_deref()) { None } else { now.clone() });
        write(h, Some(group));
        written += 1;
    }
    // Once per client and group change (the group names an account or preset id: only its kind is logged).
    if written > 0 && had_group.as_deref() != Some(group) {
        log::note(&format!(
            "Taskbar buttons: Roblox window {pid} put under its own {} button ({written} window(s)).",
            if group.starts_with("Bloxkeep.Preset.") { "preset" } else { "account" }
        ));
    }
}

/// The windows' own ids back (turned off, the client's account no longer grouped, Bloxkeep closing).
pub fn remove(pid: i32, windows: &[isize]) {
    let Some(placed) = BY_PID.lock().unwrap_or_else(|e| e.into_inner()).remove(&pid) else {
        return;
    };
    log::note(&format!("Taskbar buttons: Roblox window {pid} given back to Roblox's own button."));
    for &h in windows {
        if let Some(own) = placed.own.get(&h) {
            write(h, own.as_deref());
        } else if is_ours(read(h).as_deref()) {
            write(h, None); // already in its group when this run found it (left by a run that ended suddenly)
        }
    }
}

/// At start: the ids a run that ended suddenly (a crash, End task) left on Roblox windows. Kept, they held the windows
/// in a group of Bloxkeep's with the setting off, and were taken for the windows' own (review 2026-10-30).
pub fn clear_leftovers(windows: &[isize]) -> i32 {
    let mut cleared = 0;
    for &h in windows {
        if is_ours(read(h).as_deref()) {
            write(h, None);
            cleared += 1;
        }
    }
    cleared
}

fn is_ours(id: Option<&str>) -> bool {
    id.is_some_and(|i| i.starts_with("Bloxkeep."))
}

/// Clients that have closed: nothing of theirs to put back.
pub fn forget(alive: &HashSet<i32>) {
    BY_PID.lock().unwrap_or_else(|e| e.into_inner()).retain(|pid, _| alive.contains(pid));
}

pub fn pids() -> Vec<i32> {
    BY_PID.lock().unwrap_or_else(|e| e.into_inner()).keys().copied().collect()
}

/// The window's property store, released when dropped.
struct Store(*mut c_void);

impl Store {
    fn of(h: isize) -> Option<Store> {
        let mut store: *mut c_void = std::ptr::null_mut();
        // SAFETY: plain call; on success `store` holds one reference, released by Drop.
        let hr = unsafe { SHGetPropertyStoreForWindow(hwnd(h), &PROPERTY_STORE_IID, &mut store) };
        if hr != 0 || store.is_null() { None } else { Some(Store(store)) }
    }
    fn vtbl(&self) -> &PropertyStoreVtbl {
        // SAFETY: a COM object's first field is its table of calls.
        unsafe { &**(self.0 as *mut *const PropertyStoreVtbl) }
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        // SAFETY: the one reference SHGetPropertyStoreForWindow gave (Marshal.ReleaseComObject).
        unsafe {
            (self.vtbl().release)(self.0);
        }
    }
}

/// The window's AppUserModelID, or None when it has none (it groups by its program).
pub fn read(h: isize) -> Option<String> {
    let store = Store::of(h)?;
    let mut value = PropVariant::empty();
    // SAFETY: GetValue fills `value`, cleared below whatever it holds.
    unsafe {
        if (store.vtbl().get_value)(store.0, &APP_USER_MODEL_ID, &mut value) != 0 {
            return None;
        }
        let id = if value.kind == VT_LPWSTR && !value.pointer.is_null() {
            let p = value.pointer as *const u16;
            let mut len = 0usize;
            while *p.add(len) != 0 {
                len += 1;
            }
            Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, len)))
        } else {
            None
        };
        PropVariantClear(&mut value);
        id
    }
}

/// Sets the window's AppUserModelID; None clears it (Windows then groups it by its program again).
fn write(h: isize, id: Option<&str>) {
    let Some(store) = Store::of(h) else {
        return;
    };
    let mut value = PropVariant::empty();
    if let Some(id) = id {
        let text: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: Marshal.StringToCoTaskMemUni: a CoTaskMem copy, freed by PropVariantClear below.
        unsafe {
            let p = CoTaskMemAlloc(text.len() * 2) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(text.as_ptr(), p, text.len());
                value.kind = VT_LPWSTR;
                value.pointer = p as *mut c_void;
            }
        }
    }
    // SAFETY: plain COM calls on a live store; the value is cleared after.
    unsafe {
        // Clearing answers 0x8007007B though the id is gone (seen 2026-10-17): the answer isn't relied on.
        (store.vtbl().set_value)(store.0, &APP_USER_MODEL_ID, &value);
        (store.vtbl().commit)(store.0);
        PropVariantClear(&mut value);
    }
}
