// MacroRecorder.cs: recording a person's input into a macro, and TinyTask recordings read as one.
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx,
};

use crate::core::base::{Fail, Guid, Res, Stamp};
use crate::core::win_kit::{Point, Rect as Rectangle};
use crate::core::macro_desk::MacroDesk;
use crate::core::macro_model::{MacroStep, macro_keys};
use crate::core::roblox_input::RobloxInput;
use crate::core::{l, log, window_shot};

/// Something recorded: a key or mouse button going down or up, a wheel turn, a relative mouse move (raw units) or the
/// pointer's place, `ms` after the recording began.
/// `kind`: "keyDown", "keyUp", "down", "up", "wheel", "move" (relative) or "pos" (where the pointer is).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MacroEvent {
    pub ms: i32,
    pub kind: String,
    pub key: Option<String>,
    pub button: Option<String>,
    pub at: Point,
    pub dx: i32,
    pub dy: i32,
    pub notches: i32,
    /// For a click: a picture of where it clicked (PNG, base64), when one was taken.
    pub image: Option<String>,
}

impl MacroEvent {
    pub fn new(ms: i32, kind: &str) -> MacroEvent {
        MacroEvent { ms, kind: kind.into(), ..Default::default() }
    }
}

/// Recorded input made into a macro's steps: a key pressed alone becomes a tap with its hold time, keys pressed together
/// Hold and Release; clicks a click where they were (two quick ones a double click); a right-button drag the camera
/// turning (in pieces, so its path is kept); the wheel a scroll; the time between them waits (under 30 ms dropped).
pub mod macro_recording {
    use crate::core::base::{Fail, Res};
    use crate::core::win_kit::{Picture as Bitmap, Point, Rect as Rectangle};
    use crate::core::l;
    use crate::core::macro_follow::intersect;
    use crate::core::macro_model::{Macro, MacroSize, MacroStep, macro_keys, macro_rules};
    use crate::core::macro_player::round_to;

    use super::MacroEvent;

    /// Two clicks this close in time and place are a double click.
    const DOUBLE_MS: i32 = 400;
    const DOUBLE_PX: i32 = 6;
    /// A camera drag is cut into pieces this long at most.
    const CAMERA_PIECE: i32 = 400;
    const MIN_WAIT: i32 = 30;

    /// A step made, from when to when it plays (the waits go between), and where (a click).
    #[derive(Clone, Debug)]
    struct Act {
        start: i32,
        end: i32,
        step: MacroStep,
        at: Point,
    }

    fn act(start: i32, end: i32, step: MacroStep) -> Act {
        Act { start, end, step, at: Point::default() }
    }

    /// List.FindIndex(start, match): the first index from `start` on that matches, or -1.
    fn find_from(events: &[MacroEvent], start: usize, pred: impl Fn(&MacroEvent) -> bool) -> i32 {
        events.iter().enumerate().skip(start).find(|(_, e)| pred(e)).map_or(-1, |(i, _)| i as i32)
    }

    fn step(kind: &str) -> MacroStep {
        MacroStep { kind: kind.into(), ..Default::default() }
    }

    /// `place`: a screen point as 0..1 of the window's client area; None for one outside it.
    pub fn to_steps(recorded: &[MacroEvent], place: &dyn Fn(Point) -> Option<(f64, f64)>) -> Vec<MacroStep> {
        let mut events: Vec<MacroEvent> = recorded.to_vec();
        events.sort_by_key(|e| e.ms); // stable, as OrderBy
        let mut actions: Vec<Act> = Vec::new();
        let mut used: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut held: Vec<String> = Vec::new();
        for i in 0..events.len() {
            if used.contains(&i) {
                continue;
            }
            let e = events[i].clone();
            match e.kind.as_str() {
                "keyDown" => {
                    let Some(key) = e.key.clone() else { continue };
                    if held.contains(&key) {
                        continue; // the keyboard's own repeat while held
                    }
                    let up = find_from(&events, i + 1, |x| x.kind == "keyUp" && x.key.as_deref() == Some(key.as_str()));
                    let alone = up > 0
                        && key != "Alt"
                        && events[i + 1..up as usize].iter().all(|x| matches!(x.kind.as_str(), "move" | "pos") || (x.kind == "keyDown" && x.key.as_deref() == Some(key.as_str())));
                    if alone {
                        let up_ms = events[up as usize].ms;
                        actions.push(act(e.ms, up_ms, MacroStep { key: Some(key.clone()), ms: Some((up_ms - e.ms).clamp(30, 5000)), ..step("key") }));
                        for j in i + 1..=up as usize {
                            used.insert(j);
                        }
                    } else {
                        held.push(key.clone());
                        actions.push(act(e.ms, e.ms, MacroStep { key: Some(key), ..step("down") }));
                    }
                }
                "keyUp" => {
                    if let Some(key) = e.key.clone()
                        && let Some(at) = held.iter().position(|k| *k == key)
                    {
                        held.remove(at);
                        actions.push(act(e.ms, e.ms, MacroStep { key: Some(key), ..step("up") }));
                    }
                }
                "down" if e.button.as_deref() == Some("right") => {
                    let up = find_from(&events, i + 1, |x| x.kind == "up" && x.button.as_deref() == Some("right"));
                    let end = if up > 0 { events[up as usize].ms } else { events[events.len() - 1].ms };
                    let stop = if up > 0 { up as usize } else { events.len() };
                    let moves: Vec<MacroEvent> = events[i + 1..stop.max(i + 1)].iter().filter(|x| x.kind == "move").cloned().collect();
                    if up > 0 {
                        used.insert(up as usize);
                    }
                    for j in i + 1..stop.max(i + 1) {
                        if events[j].kind == "move" {
                            used.insert(j);
                        }
                    }
                    if moves.iter().map(|m| m.dx.abs() + m.dy.abs()).sum::<i32>() < 4 {
                        if let Some(p) = place(e.at) {
                            actions.push(act(
                                e.ms,
                                end,
                                MacroStep { x: Some(round(p.0)), y: Some(round(p.1)), button: Some("right".into()), image: e.image.clone(), ..step("click") },
                            ));
                        }
                        continue;
                    }
                    // In pieces: a drag one way then back would add up to nothing.
                    let mut piece_start = e.ms;
                    let (mut dx, mut dy) = (0, 0);
                    for m in &moves {
                        if m.ms - piece_start > CAMERA_PIECE && (dx != 0 || dy != 0) {
                            actions.push(act(piece_start, m.ms, camera(dx, dy, m.ms - piece_start)));
                            (piece_start, dx, dy) = (m.ms, 0, 0);
                        }
                        dx += m.dx;
                        dy += m.dy;
                    }
                    if dx != 0 || dy != 0 {
                        actions.push(act(piece_start, end, camera(dx, dy, end - piece_start)));
                    }
                }
                "down" => {
                    let up = find_from(&events, i + 1, |x| x.kind == "up" && x.button == e.button);
                    if up > 0 {
                        used.insert(up as usize);
                    }
                    let Some(p) = place(e.at) else { continue };
                    let end = if up > 0 { events[up as usize].ms } else { e.ms };
                    // A second quick click on the same place: the first becomes a double click.
                    if let Some(prev) = actions.last_mut()
                        && prev.step.kind == "click"
                        && prev.step.button == e.button
                        && prev.step.double != Some(true)
                        && e.ms - prev.end <= DOUBLE_MS
                        && (prev.at.x - e.at.x).abs() <= DOUBLE_PX
                        && (prev.at.y - e.at.y).abs() <= DOUBLE_PX
                    {
                        prev.step.double = Some(true);
                        prev.end = end;
                        continue;
                    }
                    actions.push(Act {
                        start: e.ms,
                        end,
                        step: MacroStep {
                            x: Some(round(p.0)),
                            y: Some(round(p.1)),
                            button: Some(e.button.clone().unwrap_or_else(|| "left".into())),
                            image: e.image.clone(),
                            ..step("click")
                        },
                        at: e.at,
                    });
                }
                "wheel" => {
                    let (mut notches, mut end) = (e.notches, e.ms);
                    let mut j = i + 1;
                    while j < events.len() && matches!(events[j].kind.as_str(), "wheel" | "move" | "pos") && events[j].ms - end <= 200 {
                        if events[j].kind == "wheel" {
                            notches += events[j].notches;
                            end = events[j].ms;
                            used.insert(j);
                        }
                        j += 1;
                    }
                    if notches != 0 {
                        actions.push(act(e.ms, end, MacroStep { notches: Some(notches.clamp(-50, 50)), ..step("scroll") }));
                    }
                }
                _ => {}
            }
        }
        // Still held when the recording ended: let go at the end.
        let last = events.last().map_or(0, |e| e.ms);
        for key in held {
            actions.push(act(last, last, MacroStep { key: Some(key), ..step("up") }));
        }

        actions.sort_by_key(|a| a.start); // stable, as OrderBy
        let mut steps = Vec::new();
        let mut at = -1;
        for a in actions {
            if at >= 0 && a.start - at >= MIN_WAIT {
                steps.push(MacroStep { ms: Some((a.start - at).min(macro_rules::MAX_WAIT_MS)), ..step("wait") });
            }
            steps.push(a.step);
            at = at.max(a.end);
        }
        steps.truncate(macro_rules::MAX_STEPS as usize);
        steps
    }

    fn camera(dx: i32, dy: i32, ms: i32) -> MacroStep {
        MacroStep {
            dx: Some(dx.clamp(-100_000, 100_000)),
            dy: Some(dy.clamp(-100_000, 100_000)),
            ms: Some(ms.clamp(16, 10_000)),
            button: Some("right".into()),
            ..step("camera")
        }
    }

    fn round(v: f64) -> f64 {
        round_to(v.clamp(0.0, 1.0), 4)
    }

    // ───────── TinyTask ─────────

    /// The frame TinyTask's screen positions are taken as (it records the whole screen, not a window): width, height.
    pub const TINY_TASK_FRAME: (i32, i32) = (1920, 1080);

    /// A TinyTask recording (.rec): Windows' journal records (EVENTMSG) one after another, 20 bytes each as the 32-bit
    /// TinyTask writes them: message, paramL, paramH, time (ms, GetTickCount), window. Keys: paramL's low byte is the
    /// virtual key, its next byte the scan code; paramH's bit 15 the extended flag. Mouse: paramL, paramH the screen
    /// position. The wheel's turn isn't in the record: one notch up each.
    pub fn read_tiny_task(file: &[u8]) -> Res<Vec<MacroEvent>> {
        if file.len() < 20 || file.len() % 20 != 0 {
            return Err(Fail::user(l::t("That isn't a TinyTask recording.")));
        }
        let u32_at = |o: usize| u32::from_le_bytes([file[o], file[o + 1], file[o + 2], file[o + 3]]);
        let mut events = Vec::new();
        let mut first: Option<u32> = None;
        let mut last = Point::default();
        let mut right_down = false;
        let mut o = 0;
        while o + 20 <= file.len() {
            let (message, param_l, param_h) = (u32_at(o), u32_at(o + 4), u32_at(o + 8));
            let (time, window) = (u32_at(o + 12), u32_at(o + 16));
            let start = *first.get_or_insert(time);
            let ms = (time.wrapping_sub(start) as i64).clamp(0, i32::MAX as i64) as i32;
            let at = Point { x: param_l as i32, y: param_h as i32 };
            match message {
                0x100 | 0x104 | 0x101 | 0x105 => {
                    if let Some(key) = macro_keys::from_scan(((param_l >> 8) & 0xFF) as u16, (param_h & 0x8000) != 0, (param_l & 0xFF) as u16) {
                        events.push(MacroEvent { key: Some(key.name.to_string()), ..MacroEvent::new(ms, if matches!(message, 0x100 | 0x104) { "keyDown" } else { "keyUp" }) });
                    }
                }
                0x200 => {
                    if right_down && (at.x != last.x || at.y != last.y) {
                        events.push(MacroEvent { dx: at.x - last.x, dy: at.y - last.y, ..MacroEvent::new(ms, "move") });
                    }
                    last = at;
                }
                0x201 | 0x204 | 0x207 => {
                    last = at;
                    right_down |= message == 0x204;
                    let button = if message == 0x201 { "left" } else if message == 0x204 { "right" } else { "middle" };
                    events.push(MacroEvent { button: Some(button.into()), at, ..MacroEvent::new(ms, "down") });
                }
                0x202 | 0x205 | 0x208 => {
                    right_down &= message != 0x205;
                    let button = if message == 0x202 { "left" } else if message == 0x205 { "right" } else { "middle" };
                    events.push(MacroEvent { button: Some(button.into()), at, ..MacroEvent::new(ms, "up") });
                }
                0x20A => {
                    let delta = (window >> 16) as u16 as i16;
                    events.push(MacroEvent { notches: if delta < 0 { -1 } else { 1 }, ..MacroEvent::new(ms, "wheel") });
                }
                _ => {
                    if !(0x100..=0x20E).contains(&message) {
                        return Err(Fail::user(l::t("That isn't a TinyTask recording.")));
                    }
                }
            }
            o += 20;
        }
        Ok(events)
    }

    /// A TinyTask recording as a macro (not saved), its places as parts of a 1920 × 1080 screen.
    pub fn from_tiny_task(file: &[u8], name: &str) -> Res<Macro> {
        let (fw, fh) = TINY_TASK_FRAME;
        let steps = to_steps(&read_tiny_task(file)?, &|p: Point| Some((p.x as f64 / fw as f64, p.y as f64 / fh as f64)));
        if steps.is_empty() {
            return Err(Fail::user(l::t("That recording has nothing Bloxkeep can play.")));
        }
        Ok(Macro { name: name.into(), steps, recorded_size: Some(MacroSize { w: fw, h: fh }), jitter_ms: 0, jitter_px: 0, ..Default::default() })
    }

    /// A square of about `side` px around `at` (client pixels), as PNG base64.
    pub fn cut(picture: &Bitmap, at: Point, side: i32) -> Res<Option<String>> {
        let r = intersect(
            Rectangle { x: at.x - side / 2, y: at.y - side / 2, width: side, height: side },
            Rectangle { x: 0, y: 0, width: picture.width, height: picture.height },
        );
        if r.width < 8 || r.height < 8 {
            return Ok(None);
        }
        // Format24bppRgb: the alpha channel dropped (every pixel opaque).
        let mut px = Vec::with_capacity((r.width * r.height) as usize);
        for y in 0..r.height {
            for x in 0..r.width {
                px.push(picture.get(r.x + x, r.y + y) | 0xFF00_0000);
            }
        }
        // PORT: the PNG is written by the image crate (RGBA, alpha all 255) where GDI+ wrote a 24-bit RGB one.
        let png = crate::core::macro_player::png_of(&Bitmap { width: r.width, height: r.height, pixels: px })?;
        Ok(Some(base64(&png)))
    }

    /// Convert.ToBase64String.
    pub(crate) fn base64(bytes: &[u8]) -> String {
        const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(ABC[(n >> 18) as usize & 63] as char);
            out.push(ABC[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 { ABC[(n >> 6) as usize & 63] as char } else { '=' });
            out.push(if chunk.len() > 2 { ABC[n as usize & 63] as char } else { '=' });
        }
        out
    }
}

/// Runs work on the window's (UI) thread, as C#'s SynchronizationContext.Post did.
pub type Post = Arc<dyn Fn(std::boxed::Box<dyn FnOnce() + Send>) + Send + Sync>;
type Callback = Arc<dyn Fn() + Send + Sync>;
type RawMoved = Arc<dyn Fn(i32, i32) + Send + Sync>;

const WH_KEYBOARD_LL: i32 = 13;
const WH_MOUSE_LL: i32 = 14;
const LLKHF_EXTENDED: u32 = 0x01;
const LLKHF_INJECTED: u32 = 0x10;
const LLKHF_UP: u32 = 0x80;
const LLMHF_INJECTED: u32 = 0x01;
const VK_F8: u32 = 0x77;
const WM_QUIT: u32 = 0x12;
const WM_MOUSEMOVE: u32 = 0x200;
const WM_LBUTTONDOWN: u32 = 0x201;
const WM_LBUTTONUP: u32 = 0x202;
const WM_RBUTTONDOWN: u32 = 0x204;
const WM_RBUTTONUP: u32 = 0x205;
const WM_MBUTTONDOWN: u32 = 0x207;
const WM_MBUTTONUP: u32 = 0x208;
const WM_MOUSEWHEEL: u32 = 0x20A;

/// A click picture being taken: still going, taken, or failed (the error's kind, as C#'s exception type name).
#[derive(Clone, Debug, PartialEq)]
enum Shot {
    Going,
    Done,
    Failed(String),
}

/// What the recorder, its hook thread, the raw input thread and the picture threads share.
struct Rec {
    window: isize,
    pid: i32,
    client: Rectangle,
    events: Mutex<Vec<MacroEvent>>,
    pictures: Mutex<Vec<Arc<Mutex<Shot>>>>,
    ui: Option<Post>,
    /// When the countdown ends: the recording's 0 ms.
    start: Instant,
    thread_id: AtomicU32,
    keyboard_hook: AtomicIsize,
    mouse_hook: AtomicIsize,
    down: Mutex<HashSet<String>>,
    right_down: AtomicBool,
    last_point: Mutex<Point>,
    raw_on: bool,
    actions: AtomicI32,
    /// error.log: F8 was written already.
    f8_noted: AtomicBool,
    stop_asked: RwLock<Option<Callback>>,
    changed: RwLock<Option<Callback>>,
}

thread_local! {
    /// The recording the hooks on this thread feed (a low-level hook's procedure gets no pointer of its own).
    static HOOKED: RefCell<Option<Arc<Rec>>> = const { RefCell::new(None) };
}

/// Records a person's input into one Roblox window: low-level keyboard and mouse hooks in Bloxkeep's own process (never
/// in Roblox), on a thread of their own, kept only while recording, taking only what goes to that window while it is in
/// front. Our own input (injected) is left out. F8 ends it; after 10 minutes it ends by itself.
pub struct MacroRecorder {
    pub account_id: Guid,
    pub starts_at: Stamp,
    /// The client area when recording began.
    pub client: Rectangle,
    rec: Arc<Rec>,
    thread: Option<JoinHandle<()>>,
    ended: Option<mpsc::Receiver<()>>,
    raw: Option<Arc<RobloxInput>>,
    raw_moved: Option<RawMoved>,
    disposed: bool,
}

pub const COUNTDOWN: Duration = Duration::from_secs(3);
pub const MAX_LENGTH: Duration = Duration::from_secs(10 * 60);

impl MacroRecorder {
    /// `ui`: how work reaches the window's thread (F8, the step count, click pictures); None: they never do, as in C#
    /// with no SynchronizationContext.
    pub fn new(account_id: Guid, pid: i32, window: isize, raw: Option<Arc<RobloxInput>>, ui: Option<Post>) -> Res<MacroRecorder> {
        let raw = raw.filter(|r| r.working());
        let client = MacroDesk::client_on_screen(window);
        let rec = Arc::new(Rec {
            window,
            pid,
            client,
            events: Mutex::new(Vec::new()),
            pictures: Mutex::new(Vec::new()),
            ui,
            start: Instant::now() + COUNTDOWN,
            thread_id: AtomicU32::new(0),
            keyboard_hook: AtomicIsize::new(0),
            mouse_hook: AtomicIsize::new(0),
            down: Mutex::new(HashSet::new()),
            right_down: AtomicBool::new(false),
            last_point: Mutex::new(Point::default()),
            raw_on: raw.is_some(),
            actions: AtomicI32::new(0),
            f8_noted: AtomicBool::new(false),
            stop_asked: RwLock::new(None),
            changed: RwLock::new(None),
        });
        let mut raw_moved: Option<RawMoved> = None;
        if let Some(r) = &raw {
            let target = rec.clone();
            let moved: RawMoved = Arc::new(move |dx, dy| target.on_raw_move(dx, dy));
            let handed = moved.clone();
            r.set_mouse_moved(Some(std::boxed::Box::new(move |dx, dy| handed(dx, dy))));
            raw_moved = Some(moved);
        }
        let (ready_tx, ready_rx) = mpsc::channel::<()>();
        let (ended_tx, ended_rx) = mpsc::channel::<()>();
        let worker = rec.clone();
        let thread = std::thread::Builder::new()
            .name("Bloxkeep macro recorder".into())
            .spawn(move || {
                run(&worker, ready_tx);
                let _ = ended_tx.send(());
            })
            .map_err(Fail::from)?;
        let mut me = MacroRecorder {
            account_id,
            starts_at: Stamp::utc_now().plus_secs(COUNTDOWN.as_secs_f64()),
            client,
            rec: rec.clone(),
            thread: Some(thread),
            ended: Some(ended_rx),
            raw,
            raw_moved,
            disposed: false,
        };
        let ready_in_time = ready_rx.recv_timeout(Duration::from_secs(5)).is_ok();
        let (keyboard, mouse) = (rec.keyboard_hook.load(Ordering::SeqCst), rec.mouse_hook.load(Ordering::SeqCst));
        if keyboard == 0 || mouse == 0 {
            log::note(&format!(
                "Macro recording: didn't start on window {pid}: keyboard hook {}, mouse hook {}{}.",
                if keyboard != 0 { "set" } else { "refused" },
                if mouse != 0 { "set" } else { "refused" },
                if ready_in_time { "" } else { ", its thread took over 5 s" }
            ));
            me.dispose();
            return Err(Fail::user(l::t("Windows didn't let Bloxkeep record. Try again.")));
        }
        log::note(&format!(
            "Macro recording: started on window {pid} ({}x{}), raw mouse input {}{}.",
            client.width,
            client.height,
            if rec.raw_on { "on" } else { "off" },
            if rec.ui.is_none() { "; no UI thread, so F8 and the step count won't reach the app" } else { "" }
        ));
        Ok(me)
    }

    /// F8 was pressed (on the window's thread).
    pub fn set_stop_asked(&self, f: Option<Callback>) {
        *self.rec.stop_asked.write().unwrap_or_else(|e| e.into_inner()) = f;
    }

    /// Something new was recorded (on the window's thread).
    pub fn set_changed(&self, f: Option<Callback>) {
        *self.rec.changed.write().unwrap_or_else(|e| e.into_inner()) = f;
    }

    /// Roughly how many steps so far (taps, clicks, scrolls).
    pub fn steps(&self) -> i32 {
        self.rec.actions.load(Ordering::SeqCst)
    }

    /// Stops recording; the steps recorded, as parts of the client area it had when recording began.
    pub fn finish(&mut self) -> Vec<MacroStep> {
        self.dispose();
        let pictures: Vec<Arc<Mutex<Shot>>> = self.rec.pictures.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let shot = |p: &Arc<Mutex<Shot>>| p.lock().unwrap_or_else(|e| e.into_inner()).clone();
        // Task.WhenAny(Task.WhenAll(pictures), Task.Delay(3000)).
        let until = Instant::now() + Duration::from_secs(3);
        while Instant::now() < until && pictures.iter().any(|p| shot(p) == Shot::Going) {
            std::thread::sleep(Duration::from_millis(25));
        }
        let events: Vec<MacroEvent> = self.rec.events.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let client = self.client;
        let steps = macro_recording::to_steps(&events, &|p: Point| {
            if contains(client, p) {
                Some(((p.x - client.x) as f64 / client.width as f64, (p.y - client.y) as f64 / client.height as f64))
            } else {
                None
            }
        });
        let states: Vec<Shot> = pictures.iter().map(shot).collect();
        let failed: Vec<&String> = states.iter().filter_map(|s| if let Shot::Failed(kind) = s { Some(kind) } else { None }).collect();
        let unfinished = states.iter().filter(|s| **s == Shot::Going).count();
        log::note(&format!(
            "Macro recording: finished on window {} with {} events, {} steps; click pictures {}{}{}.",
            self.rec.pid,
            events.len(),
            steps.len(),
            pictures.len(),
            if failed.is_empty() { String::new() } else { format!(", {} failed ({})", failed.len(), failed[0]) },
            if unfinished > 0 { format!(", {unfinished} not done in 3 s") } else { String::new() }
        ));
        steps
    }

    pub fn dispose(&mut self) {
        if self.disposed {
            return;
        }
        self.disposed = true;
        if let (Some(r), Some(_ours)) = (&self.raw, &self.raw_moved) {
            // PORT: C# cleared MouseMoved only while it was still this recorder's; RobloxInput's port doesn't let it be
            // read back, so it is cleared (one recording runs at a time).
            r.set_mouse_moved(None);
        }
        let thread_id = self.rec.thread_id.load(Ordering::SeqCst);
        let alive = self.thread.as_ref().is_some_and(|t| !t.is_finished());
        if thread_id != 0 && alive {
            // SAFETY: posts WM_QUIT to the hook thread's own queue; a thread that's gone just makes it fail.
            unsafe { PostThreadMessageW(thread_id, WM_QUIT, 0, 0) };
            let ended = self.ended.as_ref().is_some_and(|e| e.recv_timeout(Duration::from_secs(2)).is_ok());
            if !ended {
                log::note("Macro recording: its hook thread didn't end in 2 s.");
            }
        }
    }
}

impl Drop for MacroRecorder {
    fn drop(&mut self) {
        self.dispose();
    }
}

fn contains(r: Rectangle, p: Point) -> bool {
    p.x >= r.x && p.x < r.x + r.width && p.y >= r.y && p.y < r.y + r.height
}

/// The hook thread: both hooks set, then Windows' messages until WM_QUIT, then both taken off.
fn run(rec: &Arc<Rec>, ready: mpsc::Sender<()>) {
    // SAFETY: plain query of this thread's id.
    rec.thread_id.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
    HOOKED.with(|h| *h.borrow_mut() = Some(rec.clone()));
    // SAFETY: the module of this process; null asks for the exe's own handle.
    let module = unsafe { GetModuleHandleW(std::ptr::null()) };
    // SAFETY: a low-level hook with a procedure that lives as long as the process; taken off below on this thread.
    let keyboard = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_keyboard), module, 0) };
    // SAFETY: plain read of this thread's last error.
    let keyboard_error = if keyboard.is_null() { unsafe { GetLastError() } } else { 0 };
    // SAFETY: as above, for the mouse.
    let mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(on_mouse), module, 0) };
    // SAFETY: plain read of this thread's last error.
    let mouse_error = if mouse.is_null() { unsafe { GetLastError() } } else { 0 };
    rec.keyboard_hook.store(keyboard as isize, Ordering::SeqCst);
    rec.mouse_hook.store(mouse as isize, Ordering::SeqCst);
    if keyboard_error != 0 || mouse_error != 0 {
        log::note(&format!("Macro recording: SetWindowsHookEx failed (keyboard error {keyboard_error}, mouse error {mouse_error})."));
    }
    let _ = ready.send(());
    if !keyboard.is_null() && !mouse.is_null() {
        // SAFETY: MSG is plain data, filled by GetMessageW.
        let mut message: MSG = unsafe { std::mem::zeroed() };
        // SAFETY: this thread's own queue; the message is dispatched as Windows gave it.
        while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
            // SAFETY: as above.
            unsafe { DispatchMessageW(&message) };
        }
    }
    if !keyboard.is_null() {
        // SAFETY: the hook set above on this thread.
        unsafe { UnhookWindowsHookEx(keyboard) };
    }
    if !mouse.is_null() {
        // SAFETY: the hook set above on this thread.
        unsafe { UnhookWindowsHookEx(mouse) };
    }
    HOOKED.with(|h| *h.borrow_mut() = None);
}

fn hooked() -> Option<Arc<Rec>> {
    HOOKED.with(|h| h.borrow().clone())
}

fn next_hook(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    // SAFETY: hands the event on to the next hook, as every low-level hook must.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param) }
}

// Quick, on the hooks' thread: Windows drops a hook that takes long (LowLevelHooksTimeout).
unsafe extern "system" fn on_keyboard(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code >= 0
        && let Some(rec) = hooked()
    {
        // SAFETY: for a WH_KEYBOARD_LL hook with code >= 0, lParam points at a KBDLLHOOKSTRUCT.
        let k = unsafe { *(l_param as *const KBDLLHOOKSTRUCT) };
        rec.on_keyboard(k.vkCode, k.scanCode, k.flags);
    }
    next_hook(code, w_param, l_param)
}

unsafe extern "system" fn on_mouse(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code >= 0
        && let Some(rec) = hooked()
    {
        // SAFETY: for a WH_MOUSE_LL hook with code >= 0, lParam points at an MSLLHOOKSTRUCT.
        let m = unsafe { *(l_param as *const MSLLHOOKSTRUCT) };
        rec.on_mouse(w_param as u32, Point { x: m.pt.x, y: m.pt.y }, m.mouseData, m.flags);
    }
    next_hook(code, w_param, l_param)
}

/// The process whose window is in front.
fn front_pid() -> u32 {
    // SAFETY: plain queries; a null window gives pid 0.
    unsafe {
        let front: HWND = GetForegroundWindow();
        let mut pid = 0u32;
        GetWindowThreadProcessId(front, &mut pid);
        pid
    }
}

impl Rec {
    /// Milliseconds since the countdown ended (below zero before).
    fn now(&self) -> i32 {
        let now = Instant::now();
        match now.checked_duration_since(self.start) {
            Some(d) => d.as_millis().min(i32::MAX as u128) as i32,
            None => -((self.start - now).as_millis().min(i32::MAX as u128) as i32),
        }
    }

    /// Input for the recorded window: it is the one in front, and the countdown is over.
    fn ours(&self) -> bool {
        if Instant::now() < self.start {
            return false;
        }
        front_pid() == self.pid as u32
    }

    fn post(&self, work: impl FnOnce() + Send + 'static) {
        if let Some(ui) = &self.ui {
            ui(std::boxed::Box::new(work));
        }
    }

    /// Adds an event; its place in the list.
    fn add(self: &Arc<Self>, e: MacroEvent, action: bool) -> usize {
        let index = {
            let mut events = self.events.lock().unwrap_or_else(|e| e.into_inner());
            events.push(e);
            events.len() - 1
        };
        if action {
            self.actions.fetch_add(1, Ordering::SeqCst);
            let me = self.clone();
            self.post(move || {
                let changed = me.changed.read().unwrap_or_else(|e| e.into_inner()).clone();
                if let Some(f) = changed {
                    f();
                }
            });
        }
        index
    }

    fn on_keyboard(self: &Arc<Self>, vk: u32, scan: u32, flags: u32) {
        let up = (flags & LLKHF_UP) != 0;
        let injected = (flags & LLKHF_INJECTED) != 0;
        if !injected && vk == VK_F8 {
            if !up {
                let me = self.clone();
                self.post(move || {
                    let stop = me.stop_asked.read().unwrap_or_else(|e| e.into_inner()).clone();
                    // Written on the window's thread: the hook must stay quick. Once: a held F8 repeats.
                    if !me.f8_noted.swap(true, Ordering::SeqCst) {
                        log::note(&format!("Macro recording: F8 pressed{}.", if stop.is_none() { ", but nothing listens for it" } else { "" }));
                    }
                    if let Some(f) = stop {
                        f();
                    }
                });
            }
        } else if !injected
            && self.ours()
            && let Some(key) = macro_keys::from_scan(scan as u16, (flags & LLKHF_EXTENDED) != 0, vk as u16)
        {
            let name = key.name.to_string();
            let (went_down, went_up) = {
                let mut down = self.down.lock().unwrap_or_else(|e| e.into_inner());
                if !up { (down.insert(name.clone()), false) } else { (false, down.remove(&name)) }
            };
            if went_down {
                self.add(MacroEvent { key: Some(name), ..MacroEvent::new(self.now(), "keyDown") }, true);
            } else if went_up {
                self.add(MacroEvent { key: Some(name), ..MacroEvent::new(self.now(), "keyUp") }, false);
            }
        }
    }

    fn on_mouse(self: &Arc<Self>, message: u32, at: Point, mouse_data: u32, flags: u32) {
        if (flags & LLMHF_INJECTED) != 0 || !(self.ours() || matches!(message, WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP)) {
            return;
        }
        match message {
            WM_MOUSEMOVE => {
                // Without raw input, the camera's turn is guessed from where the pointer goes.
                let mut last = self.last_point.lock().unwrap_or_else(|e| e.into_inner());
                if self.right_down.load(Ordering::SeqCst) && !self.raw_on && (at.x != last.x || at.y != last.y) {
                    let (dx, dy) = (at.x - last.x, at.y - last.y);
                    self.add(MacroEvent { dx, dy, ..MacroEvent::new(self.now(), "move") }, false);
                }
                *last = at;
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                let button = if message == WM_LBUTTONDOWN { "left" } else if message == WM_RBUTTONDOWN { "right" } else { "middle" };
                if button == "right" {
                    self.right_down.store(true, Ordering::SeqCst);
                }
                *self.last_point.lock().unwrap_or_else(|e| e.into_inner()) = at;
                let index = self.add(MacroEvent { button: Some(button.into()), at, ..MacroEvent::new(self.now(), "down") }, true);
                if button != "middle" {
                    let me = self.clone();
                    self.post(move || me.picture(index, at));
                }
            }
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                if message == WM_RBUTTONUP {
                    self.right_down.store(false, Ordering::SeqCst);
                }
                let button = if message == WM_LBUTTONUP { "left" } else if message == WM_RBUTTONUP { "right" } else { "middle" };
                self.add(MacroEvent { button: Some(button.into()), at, ..MacroEvent::new(self.now(), "up") }, false);
            }
            WM_MOUSEWHEEL => {
                let notches = ((mouse_data >> 16) as u16 as i16) as i32 / 120;
                if notches != 0 {
                    self.add(MacroEvent { notches, ..MacroEvent::new(self.now(), "wheel") }, true);
                }
            }
            _ => {}
        }
    }

    /// A person's raw mouse move (RobloxInput's thread): the camera's turn while the right button is held.
    fn on_raw_move(self: &Arc<Self>, dx: i32, dy: i32) {
        if self.right_down.load(Ordering::SeqCst) && self.ours() {
            self.add(MacroEvent { dx, dy, ..MacroEvent::new(self.now(), "move") }, false);
        }
    }

    /// A picture of where a click landed, for "click by image" (asked on the window's thread, taken on a thread of its own).
    fn picture(self: &Arc<Self>, index: usize, click_at: Point) {
        let at = Point { x: click_at.x - self.client.x, y: click_at.y - self.client.y };
        let state = Arc::new(Mutex::new(Shot::Going));
        {
            let mut pictures = self.pictures.lock().unwrap_or_else(|e| e.into_inner());
            if pictures.len() >= 100 {
                return;
            }
            pictures.push(state.clone());
        }
        let me = self.clone();
        let spawned = std::thread::Builder::new().name("Bloxkeep click picture".into()).spawn(move || {
            let taken = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Res<()> {
                if let Some(picture) = window_shot::client(me.window, true)
                    && let Some(image) = macro_recording::cut(&picture, at, 64)?
                    && let Some(e) = me.events.lock().unwrap_or_else(|e| e.into_inner()).get_mut(index)
                {
                    e.image = Some(image);
                }
                Ok(())
            }));
            *state.lock().unwrap_or_else(|e| e.into_inner()) = match taken {
                Ok(Ok(())) => Shot::Done,
                // PORT: C# named the exception's type; a Fail carries no type, so its kind is named instead.
                Ok(Err(f)) => Shot::Failed(match f {
                    Fail::User(_) => "UserError".into(),
                    Fail::Roblox { .. } => "RobloxApiException".into(),
                    Fail::Other(_) => "Exception".into(),
                }),
                Err(_) => Shot::Failed("Exception".into()),
            };
        });
        if spawned.is_err() {
            *self.pictures.lock().unwrap_or_else(|e| e.into_inner()).last_mut().expect("just pushed").lock().unwrap_or_else(|e| e.into_inner()) = Shot::Failed("Exception".into());
        }
    }
}
