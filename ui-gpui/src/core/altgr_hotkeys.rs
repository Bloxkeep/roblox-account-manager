// AltGrKeys.cs + AltGrHotkeys.cs: the keys an installed keyboard types a character with under AltGr, and the Ctrl+Alt
// hotkeys on them. Windows sends AltGr as Ctrl+Alt, so RegisterHotKey on one of them took that character from every app:
// @ on a French keyboard (AltGr+0, and Ctrl+Alt+0 stops macros: a user's report on 1.0.21, 2026-11-11), { [ ] } on a
// German one (AltGr+7…0), ą ń on a Polish one. A low-level keyboard hook can tell them apart by which Alt is down: the
// left one (and a Ctrl) is the hotkey, kept from the app; the right one (AltGr) is typing and goes through untouched.
// Only these keys use the hook, and only on a PC whose keyboard needs it: the hook sees every key pressed, so the plain
// hotkeys stay on RegisterHotKey.
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::{GetLastError, LPARAM, LRESULT, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, GetKeyboardLayoutList, HKL, MapVirtualKeyExW, ToUnicodeEx},
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, MSG, PostThreadMessageW, SetWindowsHookExW,
            UnhookWindowsHookEx, WM_QUIT,
        },
    },
};

use super::log;

const MAP_VK_TO_SCAN: u32 = 0;
/// Leaves the keyboard's state (a dead key waiting) as it was (Windows 10 1607 and later).
const KEEP_STATE: u32 = 0x4;
const DOWN: u8 = 0x80;
const VK_SHIFT: i32 = 0x10;
const VK_CONTROL: usize = 0x11;
const VK_MENU: usize = 0x12;
const VK_LWIN: i32 = 0x5B;
const VK_RWIN: i32 = 0x5C;
const VK_LCONTROL: i32 = 0xA2;
const VK_RCONTROL: i32 = 0xA3;
const VK_LMENU: i32 = 0xA4;
const VK_RMENU: i32 = 0xA5;
const WH_KEYBOARD_LL: i32 = 13;
const LLKHF_INJECTED: u32 = 0x10;
const LLKHF_UP: u32 = 0x80;

/// The hotkeys' keys (digits 0x30–0x39 and letters 0x41–0x5A) that any installed keyboard types with under AltGr.
pub fn typed() -> HashSet<u32> {
    let mut keys = HashSet::new();
    // SAFETY: asks for the count first, then fills a buffer of that size.
    let count = unsafe { GetKeyboardLayoutList(0, std::ptr::null_mut()) };
    if count <= 0 {
        return keys;
    }
    let mut layouts: Vec<HKL> = vec![std::ptr::null_mut(); count as usize];
    // SAFETY: the buffer holds `count` layouts.
    let count = unsafe { GetKeyboardLayoutList(count, layouts.as_mut_ptr()) };
    for layout in layouts.into_iter().take(count.max(0) as usize) {
        keys.extend(typed_by(layout));
    }
    keys
}

/// The letters and digits `layout` types a character with under AltGr (a dead key counts) that the key doesn't type
/// without it. The Vietnamese keyboard gives AltGr+H as h, the key's own letter: nobody types h that way, and counting it
/// left that keyboard's owners with no hotkey at all (its AltGr+0…9, the digits, do count).
fn typed_by(layout: HKL) -> Vec<u32> {
    let mut alt_gr = [0u8; 256];
    for vk in [VK_CONTROL, VK_MENU, VK_LCONTROL as usize, VK_RMENU as usize] {
        alt_gr[vk] = DOWN;
    }
    let mut shift = [0u8; 256];
    shift[VK_SHIFT as usize] = DOWN;
    let plain = [0u8; 256];
    let mut out = Vec::new();
    for key in (0x30u32..=0x39).chain(0x41..=0x5A) {
        // SAFETY: plain query.
        let scan = unsafe { MapVirtualKeyExW(key, MAP_VK_TO_SCAN, layout) };
        let (n, text) = type_key(key, scan, &alt_gr, layout);
        // Under 0 is a dead key (^ ` ~ waiting for the next letter): typed as well. A control character isn't one.
        let control = text.chars().next().is_some_and(char::is_control);
        if n < 0 || (n > 0 && !control && text != type_key(key, scan, &plain, layout).1 && text != type_key(key, scan, &shift, layout).1) {
            out.push(key);
        }
    }
    out
}

fn type_key(key: u32, scan: u32, state: &[u8; 256], layout: HKL) -> (i32, String) {
    let mut buf = [0u16; 8];
    // SAFETY: the state is 256 bytes and the buffer's length is passed.
    let n = unsafe { ToUnicodeEx(key, scan, state.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, KEEP_STATE, layout) };
    let text = if n > 0 { String::from_utf16_lossy(&buf[..(n as usize).min(buf.len())]) } else { String::new() };
    (n, text)
}

/// What the hook's thread holds: the keys (virtual-key code to hotkey id), the keys taken as a hotkey and still down
/// (their repeats and their release are kept from the app too), and what a hotkey calls.
struct Hook {
    keys: HashMap<u32, i32>,
    held: Mutex<HashSet<u32>>,
    fire: Box<dyn Fn(i32) + Send + Sync>,
}

thread_local! {
    static HOOKED: RefCell<Option<Arc<Hook>>> = const { RefCell::new(None) };
}

/// The hook, set on its own thread; dropping it takes it off.
pub struct AltGrHotkeys {
    thread_id: u32,
}

impl AltGrHotkeys {
    /// The hook, set; None when Windows refused it (the keys are then left to the keyboard).
    /// `fire` is called on the hook's thread with the hotkey's id: it must only post, not do the work.
    pub fn start(keys: HashMap<u32, i32>, fire: impl Fn(i32) + Send + Sync + 'static) -> Option<AltGrHotkeys> {
        let hook = Arc::new(Hook { keys, held: Mutex::new(HashSet::new()), fire: Box::new(fire) });
        let (ready, ready_rx) = mpsc::channel::<(u32, u32)>();
        let spawned = crate::threads::own("Bloxkeep AltGr hotkeys", move || run(hook, ready));
        if spawned.is_err() {
            log::note("Hotkeys: the keyboard hook for AltGr keys was refused (no thread).");
            return None;
        }
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok((thread_id, 0)) => Some(AltGrHotkeys { thread_id }),
            Ok((_, error)) => {
                log::note(&format!("Hotkeys: the keyboard hook for AltGr keys was refused (error {error})."));
                None
            }
            Err(_) => {
                log::note("Hotkeys: the keyboard hook for AltGr keys was refused, its thread took over 5 s.");
                None
            }
        }
    }
}

impl Drop for AltGrHotkeys {
    fn drop(&mut self) {
        // SAFETY: a quit message to the hook's own thread; it takes the hook off itself.
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
    }
}

fn run(hook: Arc<Hook>, ready: mpsc::Sender<(u32, u32)>) {
    // SAFETY: plain query of this thread's id.
    let thread_id = unsafe { GetCurrentThreadId() };
    HOOKED.with(|h| *h.borrow_mut() = Some(hook));
    // SAFETY: the module of this process; null asks for the exe's own handle.
    let module = unsafe { GetModuleHandleW(std::ptr::null()) };
    // SAFETY: a low-level hook with a procedure that lives as long as the process; taken off below on this thread.
    let handle = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_keyboard), module, 0) };
    // SAFETY: plain read of this thread's last error.
    let error = if handle.is_null() { unsafe { GetLastError() }.max(1) } else { 0 };
    // The one who started it gave up after 5 s: nobody would take the hook off.
    if ready.send((thread_id, error)).is_err() || handle.is_null() {
        if !handle.is_null() {
            // SAFETY: the hook set above on this thread.
            unsafe { UnhookWindowsHookEx(handle) };
        }
        return;
    }
    // SAFETY: MSG is plain data, filled by GetMessageW.
    let mut message: MSG = unsafe { std::mem::zeroed() };
    // SAFETY: this thread's own queue; the message is dispatched as Windows gave it.
    while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
        // SAFETY: as above.
        unsafe { DispatchMessageW(&message) };
    }
    // SAFETY: the hook set above on this thread.
    unsafe { UnhookWindowsHookEx(handle) };
    HOOKED.with(|h| *h.borrow_mut() = None);
}

fn down(vk: i32) -> bool {
    // SAFETY: plain query.
    (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0
}

/// Ctrl and the left Alt, nothing else. AltGr is the right Alt (with a Ctrl Windows adds itself): typing. Before this
/// key's own event the keys' state is up to date.
fn is_hotkey() -> bool {
    (down(VK_LCONTROL) || down(VK_RCONTROL)) && down(VK_LMENU) && !down(VK_RMENU) && !down(VK_SHIFT) && !down(VK_LWIN) && !down(VK_RWIN)
}

// Quick, on the hook's thread: Windows drops a hook that takes long (LowLevelHooksTimeout), and every key on the PC
// waits for it.
unsafe extern "system" fn on_keyboard(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code >= 0
        && let Some(hook) = HOOKED.with(|h| h.borrow().clone())
    {
        // SAFETY: for a WH_KEYBOARD_LL hook with code >= 0, lParam points at a KBDLLHOOKSTRUCT.
        let k = unsafe { *(l_param as *const KBDLLHOOKSTRUCT) };
        if let Some(&id) = hook.keys.get(&k.vkCode)
            && k.flags & LLKHF_INJECTED == 0
        {
            let mut held = hook.held.lock().unwrap_or_else(|e| e.into_inner());
            if k.flags & LLKHF_UP != 0 {
                if held.remove(&k.vkCode) {
                    return 1;
                }
            } else if held.contains(&k.vkCode) {
                return 1; // a repeat of a hotkey held down (as MOD_NOREPEAT)
            } else if is_hotkey() {
                held.insert(k.vkCode);
                drop(held);
                (hook.fire)(id);
                return 1;
            }
        }
    }
    // SAFETY: hands the event on to the next hook, as every low-level hook must.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param) }
}
