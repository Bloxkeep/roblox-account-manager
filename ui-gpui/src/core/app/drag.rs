// AppController.Drag.cs: dragging a Roblox window's edge goes below Roblox's own least size (2026-10-23): Roblox holds a
// hand-sized window at 800 × 600 in its own units, so a window Arrange made small grew back on the first drag. A WinEvent
// hook (in Bloxkeep's own process: nothing goes into Roblox's) sees each drag start and end; at the end the window gets
// the size the mouse asked for (roblox_processes::after_drag), the way Arrange sizes its tiles. A move is left to Windows.
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetCursorPos, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, TranslateMessage, WINEVENT_OUTOFCONTEXT, WM_QUIT,
};

use super::App;
use super::panels::panel_native;
use crate::core::log;
use crate::core::roblox_processes;
use crate::core::win_kit::{Point, Rect};

pub const EVENT_SYSTEM_MOVESIZESTART: u32 = 0x000A;
pub const EVENT_SYSTEM_MOVESIZEEND: u32 = 0x000B;
pub const WINEVENT_SKIPOWNPROCESS: u32 = 2;

/// The instance fields AppController.Drag.cs declares.
#[derive(Default)]
pub struct DragState {
    pub drag_hook: Cell<isize>,
    // PORT: `_dragProc` (the delegate kept alive for Windows) is the `on_win_event` fn below; `_drag` (the window being
    // dragged now) lives on the hook's own thread (`DRAG`), where the callback runs. The hook needs a message loop, which
    // the core's thread doesn't run, so it gets a thread of its own; this is that thread's id, to end it.
    pub drag_thread: Cell<u32>,
}

/// The App was disposed (C#'s `_disposed`, read by the hook's callback on its own thread).
static DISPOSED: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// The Roblox window being dragged now: its place and the mouse where the drag started.
    static DRAG: Cell<Option<(isize, Rect, Point)>> = const { Cell::new(None) };
}

fn cursor_pos() -> Option<Point> {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: `p` is a valid POINT for GetCursorPos to fill.
    if unsafe { GetCursorPos(&mut p) } != 0 { Some(Point::new(p.x, p.y)) } else { None }
}

unsafe extern "system" fn on_win_event(_hook: HWINEVENTHOOK, event_type: u32, hwnd: HWND, id_object: i32, _child: i32, _thread: u32, _time: u32) {
    let hwnd = hwnd as isize;
    if DISPOSED.load(Ordering::SeqCst) || id_object != 0 || hwnd == 0 {
        return;
    }
    if event_type == EVENT_SYSTEM_MOVESIZESTART {
        let start = if roblox_processes::is_player_window(hwnd) {
            match (roblox_processes::window_rect(hwnd), cursor_pos()) {
                (Some(start), Some(grab)) => Some((hwnd, start, grab)),
                _ => None,
            }
        } else {
            None
        };
        DRAG.with(|d| d.set(start));
        return;
    }
    let Some(drag) = DRAG.with(|d| d.get()) else {
        return;
    };
    if drag.0 != hwnd {
        return;
    }
    DRAG.with(|d| d.set(None));
    if let Some(release) = cursor_pos() {
        // Not waited for (`_ = KeepDraggedSizeAsync(…)`): its waits run on a thread of their own.
        std::thread::spawn(move || keep_dragged_size(drag.0, drag.1, drag.2, release));
    }
}

/// The drag's end: the size the mouse meant, if Roblox kept another; again a moment later, since Roblox sizes
/// its window once more after a resize as it lays its game out.
pub fn keep_dragged_size(hwnd: isize, start: Rect, grab: Point, release: Point) {
    let border = roblox_processes::resize_border(hwnd);
    for i in 0..2 {
        std::thread::sleep(Duration::from_millis(if i == 0 { 30 } else { 350 }));
        let Some(now) = roblox_processes::window_rect(hwnd) else {
            return;
        };
        let Some(meant) = roblox_processes::after_drag(start, now, grab, release, border) else {
            return;
        };
        log::note(&format!(
            "Drag: Roblox window {} sized to the dragged {}×{} (Roblox kept {}×{}).",
            panel_native::pid_of(hwnd),
            meant.width,
            meant.height,
            now.width,
            now.height
        ));
        roblox_processes::place_window(hwnd, meant);
    }
}

impl App {
    pub fn watch_drags(&self) {
        DISPOSED.store(false, Ordering::SeqCst);
        let (tx, rx) = std::sync::mpsc::channel::<(u32, isize)>();
        let spawned = std::thread::Builder::new().name("drag-hook".into()).spawn(move || {
            // SAFETY: the hook calls `on_win_event` on this thread, out of context, for move/size start and end only; it
            // is removed below on the same thread, as UnhookWinEvent requires.
            let hook = unsafe {
                SetWinEventHook(
                    EVENT_SYSTEM_MOVESIZESTART,
                    EVENT_SYSTEM_MOVESIZEEND,
                    std::ptr::null_mut(),
                    Some(on_win_event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                )
            };
            // SAFETY: GetCurrentThreadId has no preconditions.
            let thread = unsafe { GetCurrentThreadId() };
            // The thread's message queue made now, so a stop posted right after the id is handed over isn't lost.
            // SAFETY: MSG is plain data PeekMessageW fills in.
            unsafe {
                let mut msg: MSG = std::mem::zeroed();
                PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
            }
            let _ = tx.send((thread, hook as isize));
            // SAFETY: a plain message loop; MSG is plain data filled by GetMessageW.
            unsafe {
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                if !hook.is_null() {
                    UnhookWinEvent(hook);
                }
            }
        });
        if spawned.is_ok()
            && let Ok((thread, hook)) = rx.recv()
        {
            self.drag.drag_thread.set(thread);
            self.drag.drag_hook.set(hook);
        }
    }

    pub fn stop_watching_drags(&self) {
        DISPOSED.store(true, Ordering::SeqCst);
        let thread = self.drag.drag_thread.get();
        if thread != 0 {
            // SAFETY: posts WM_QUIT to the hook's thread, which unhooks and ends.
            unsafe {
                PostThreadMessageW(thread, WM_QUIT, 0, 0);
            }
        }
        self.drag.drag_hook.set(0);
        self.drag.drag_thread.set(0);
    }
}
