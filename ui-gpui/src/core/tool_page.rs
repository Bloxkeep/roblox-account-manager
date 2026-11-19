// What is drawn in the app's own small windows (the mini window, the macro panel, the overlay and colour picker, the
// dialogs): until 2026-11-19 a WebView2 browser with one of ui/'s pages in each, whose browser processes came and went
// beside Bloxkeep in Task Manager. Now the interface draws them (ui-gpui/src/toolpage.rs): a GPUI window of its own,
// put inside the core's window as its child, filling it. The core's window stays what it was (its place and size, on
// top or not, its frame, the drag by its title row, the keyboard); only its face changed hands.
//
// The two talk as the page and its host did: the core sends events by name (send_event), the page sends back { cmd,
// args } ("size" with the size it drew, "act" with what was clicked, "drag", "resize"). Each page has an id; the core
// asks the interface to make it ("toolPage" { op: "open", id, kind, parent }), the interface puts it inside `parent`
// and says so (`FromPage::Attached`). The dialogs live on a thread of their own, so each host says how its messages
// reach it (`register`).
use std::{
    collections::HashMap,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use serde_json::{Value, json};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{GetClientRect, IsWindow, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos},
};

use super::gpui_host;

/// What the interface says about a page.
pub enum FromPage {
    /// Its window is made and put inside the host's (the child's handle).
    Attached(isize),
    /// One of its messages: { cmd, args }.
    Message(Value),
    /// Its window went without being asked (the interface closed it): drawn again on the next show.
    Gone,
}

type Deliver = Box<dyn Fn(u64, FromPage) + Send + Sync>;

static HOSTS: LazyLock<Mutex<HashMap<u64, Deliver>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT: AtomicU64 = AtomicU64::new(1);

/// A new page id; what the interface says about it goes to `deliver` (from the interface's thread: `deliver` takes it
/// to its host's thread, after this turn).
pub fn register(deliver: impl Fn(u64, FromPage) + Send + Sync + 'static) -> u64 {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    HOSTS.lock().unwrap_or_else(|e| e.into_inner()).insert(id, Box::new(deliver));
    id
}

/// The host let the page go: nothing more reaches it.
pub fn unregister(id: u64) {
    HOSTS.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
}

/// From the interface: what it says about page `id` (nothing when its host let it go).
pub fn from_ui(id: u64, m: FromPage) {
    let hosts = HOSTS.lock().unwrap_or_else(|e| e.into_inner());
    match hosts.get(&id) {
        Some(deliver) => deliver(id, m),
        None => {
            drop(hosts);
            // Made after its host let it go: it goes too.
            if matches!(m, FromPage::Attached(_)) {
                close(id);
            }
        }
    }
}

/// Whether the interface is there to draw a page (it isn't before it started, in the tests, in the uninstaller).
pub fn interface_there() -> bool {
    gpui_host::listening()
}

/// Asks the interface for page `id` of `kind` ("mini", "panel", "overlay", "dialog") inside window `parent`; false
/// when there is no interface to ask.
pub fn open(id: u64, kind: &str, parent: isize) -> bool {
    if !interface_there() {
        return false;
    }
    post(json!({ "op": "open", "id": id, "kind": kind, "parent": parent }));
    true
}

/// An event to page `id`, as a page got them: by name, with its data.
pub fn event(id: u64, name: &str, data: Value) {
    post(json!({ "op": "event", "id": id, "name": name, "data": data }));
}

/// The host showed its window (unseen until the page draws): the page draws once more and says so with `seq`.
pub fn wake(id: u64, seq: u64) {
    post(json!({ "op": "wake", "id": id, "seq": seq }));
}

/// Page `id` goes (its window with it).
pub fn close(id: u64) {
    post(json!({ "op": "close", "id": id }));
}

fn post(data: Value) {
    gpui_host::post(json!({ "type": "event", "name": "toolPage", "data": data }).to_string());
}

/// The page's window fills its host's inside. `other_thread`: the host is on another thread than the page (the dialogs):
/// the move is posted, never waited for (each thread waiting on the other would hang both).
pub fn fit(parent: isize, child: isize, other_thread: bool) {
    // SAFETY: plain calls on windows of ours; a RECT on the stack.
    unsafe {
        if parent == 0 || child == 0 || IsWindow(child as HWND) == 0 {
            return;
        }
        let mut r = std::mem::zeroed();
        GetClientRect(parent as HWND, &mut r);
        let flags = SWP_NOZORDER | SWP_NOACTIVATE | if other_thread { SWP_ASYNCWINDOWPOS } else { 0 };
        SetWindowPos(child as HWND, std::ptr::null_mut(), 0, 0, (r.right - r.left).max(1), (r.bottom - r.top).max(1), flags);
    }
}

/// The page's size message as numbers: what it drew (w, h), the window it drew in (vw, vh) and the showing it drew for.
pub fn drawn(args: &Value) -> (i32, i32, i32, i32, u64) {
    let n = |k: &str| args.get(k).and_then(Value::as_f64).map(|v| v.ceil() as i32).unwrap_or(0);
    let r = |k: &str| args.get(k).and_then(Value::as_f64).map(|v| v.round() as i32).unwrap_or(0);
    (n("w"), n("h"), r("vw"), r("vh"), args.get("seq").and_then(Value::as_u64).unwrap_or(0))
}
