//! Borrowing a Roblox window for a moment and giving it back as it was: anti-AFK's nudge, a macro's turn, a picture of
//! a minimised or hidden window (/pc screenshot, the windows view's stills, a macro's look) and the live view. A
//! borrower may open a minimised or hidden window, make it all but invisible (and click-through), bring it to the front
//! or give it another place; give-back undoes each of those it did, unless the window was changed meanwhile by someone
//! else (`back` decides), and a lent window is left alone by the rules that run on the app's tick (`is_lent`), it
//! alone, not the whole pass.
//!
//! Every window's place in the stack is read before any window of the same round moves (`borrow_round`): one brought
//! forward first would otherwise be read as standing over the others.
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use windows_sys::Win32::{
    Foundation::GetLastError,
    Graphics::{
        Dwm::DwmSetWindowAttribute,
        Gdi::{RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RedrawWindow},
    },
    UI::WindowsAndMessaging::{
        GetForegroundWindow, GetLayeredWindowAttributes, GetWindow, GetWindowLongW, GetWindowThreadProcessId, IsHungAppWindow, IsIconic, IsWindow, IsWindowVisible, IsZoomed,
        SetLayeredWindowAttributes, SetWindowLongW, SetWindowPos, ShowWindowAsync,
    },
};

use super::{
    anti_afk::{self, HIDDEN_ALPHA},
    log, roblox_processes,
    win_kit::{Rect, hwnd},
};

/// How long a window put away again (minimised or hidden) is waited for before it is asked once more, and again after.
pub const WAIT: Duration = Duration::from_secs(1);

const SW_HIDE: i32 = 0;
const SW_SHOWNOACTIVATE: i32 = 4;
const SW_SHOWMINNOACTIVE: i32 = 7;
const SW_RESTORE: i32 = 9;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOPMOST: i32 = 0x8;
const WS_EX_TRANSPARENT: i32 = 0x20;
const WS_EX_LAYERED: i32 = 0x80000;
const LWA_ALPHA: u32 = 0x2;
const GW_HWNDNEXT: u32 = 2;
const GW_HWNDPREV: u32 = 3;
const SWP_NOSIZE: u32 = 0x1;
const SWP_NOMOVE: u32 = 0x2;
const SWP_NOACTIVATE: u32 = 0x10;
const SWP_NOOWNERZORDER: u32 = 0x200;
// Not async: a window put back under yours while yours is still always-on-top would become always-on-top itself.
// So a window that has stopped answering isn't moved (the move waits for its answer).
const RESTACK: u32 = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER;
const DWMWA_TRANSITIONS_FORCEDISABLED: u32 = 3;
/// The windows read above and below a lent one, nearest first.
const NEIGHBOURS: usize = 12;

// ───────── which windows are lent ─────────

/// Lent windows, each with how many borrowers have it (the live view holds one while a nudge borrows it too).
static LENT: Mutex<Option<HashMap<isize, u32>>> = Mutex::new(None);

fn lent() -> MutexGuard<'static, Option<HashMap<isize, u32>>> {
    LENT.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether a borrower has this window now: from `borrow` (or `borrow_round`) to the end of its give-back.
pub fn is_lent(h: isize) -> bool {
    lent().as_ref().is_some_and(|set| set.contains_key(&h))
}

fn mark(h: isize) {
    *lent().get_or_insert_with(HashMap::new).entry(h).or_insert(0) += 1;
}

fn unmark(h: isize) {
    let mut g = lent();
    if let Some(set) = g.as_mut()
        && let Some(n) = set.get_mut(&h)
    {
        *n -= 1;
        if *n == 0 {
            set.remove(&h);
        }
    }
}

// ───────── Windows' answers ─────────

fn is_window(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsWindow(hwnd(h)) != 0 }
}
fn is_iconic(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsIconic(hwnd(h)) != 0 }
}
fn is_zoomed(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsZoomed(hwnd(h)) != 0 }
}
fn is_visible(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsWindowVisible(hwnd(h)) != 0 }
}
/// Windows' own "not responding" (no answer for 5 seconds).
fn is_hung(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsHungAppWindow(hwnd(h)) != 0 }
}
fn foreground() -> isize {
    // SAFETY: plain call.
    unsafe { GetForegroundWindow() as isize }
}
// Async: ShowWindow waits for the window to answer, and a hung Roblox would hold whoever asked.
fn show_async(h: isize, command: i32) -> bool {
    // SAFETY: plain call; posted to the window's thread.
    unsafe { ShowWindowAsync(hwnd(h), command) != 0 }
}
fn get_ex_style(h: isize) -> i32 {
    // SAFETY: plain query of a handle.
    unsafe { GetWindowLongW(hwnd(h), GWL_EXSTYLE) }
}
fn set_ex_style(h: isize, value: i32) {
    // SAFETY: plain call; only the extended style is written.
    unsafe {
        SetWindowLongW(hwnd(h), GWL_EXSTYLE, value);
    }
}
fn redraw(h: isize) {
    // SAFETY: plain call over the whole window.
    unsafe {
        RedrawWindow(hwnd(h), std::ptr::null(), std::ptr::null_mut(), RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN);
    }
}
fn layered_attributes(h: isize) -> Option<(u32, u8, u32)> {
    let (mut key, mut alpha, mut flags) = (0u32, 0u8, 0u32);
    // SAFETY: three out values on the stack.
    if unsafe { GetLayeredWindowAttributes(hwnd(h), &mut key, &mut alpha, &mut flags) } != 0 { Some((key, alpha, flags)) } else { None }
}
fn set_layered_attributes(h: isize, key: u32, alpha: u8, flags: u32) {
    // SAFETY: plain call on a layered window.
    unsafe {
        SetLayeredWindowAttributes(hwnd(h), key, alpha, flags);
    }
}
fn animations(h: isize, on: bool) {
    let disabled: i32 = if on { 0 } else { 1 };
    // SAFETY: one int, its size given.
    unsafe {
        DwmSetWindowAttribute(hwnd(h), DWMWA_TRANSITIONS_FORCEDISABLED as _, &disabled as *const i32 as *const c_void, std::mem::size_of::<i32>() as u32);
    }
}
/// The window's process id, for error.log and to tell a reused handle (0 when gone).
fn pid_of(h: isize) -> u32 {
    let mut pid = 0u32;
    // SAFETY: one out value on the stack.
    unsafe {
        GetWindowThreadProcessId(hwnd(h), &mut pid);
    }
    pid
}
fn next(h: isize) -> isize {
    // SAFETY: plain query of a handle.
    unsafe { GetWindow(hwnd(h), GW_HWNDNEXT) as isize }
}
fn prev(h: isize) -> isize {
    // SAFETY: plain query of a handle.
    unsafe { GetWindow(hwnd(h), GW_HWNDPREV) as isize }
}

/// `h` straight under `after` in the stack; Windows' error code when it refused (5: `after` belongs to a program run as
/// administrator, Task Manager say).
pub(crate) fn set_under(h: isize, after: isize) -> Result<(), u32> {
    // SAFETY: plain call; no size or place is given (RESTACK keeps them).
    if unsafe { SetWindowPos(hwnd(h), hwnd(after), 0, 0, 0, 0, RESTACK) } != 0 {
        Ok(())
    } else {
        // SAFETY: plain call, right after the one that failed.
        Err(unsafe { GetLastError() })
    }
}

/// A window others are placed by: shown, not always-on-top. Hidden ones don't count: a program's hidden input-method
/// window rides along with its front window, so going back under one would leave the window in front.
fn counts(x: isize) -> bool {
    is_visible(x) && (get_ex_style(x) & WS_EX_TOPMOST) == 0
}

/// The shown, not-topmost windows above `h`, nearest first, and how many windows of any kind stand above it.
fn above_of(h: isize) -> (Vec<isize>, i32) {
    let mut list = Vec::new();
    let mut depth = 0;
    let mut win = prev(h);
    while win != 0 && depth < 5000 {
        if list.len() < NEIGHBOURS && counts(win) {
            list.push(win);
        }
        win = prev(win);
        depth += 1;
    }
    (list, depth)
}

/// The shown, not-minimised, not-topmost windows under `h`, nearest first (the desktop's own left out).
fn below_of(h: isize) -> Vec<isize> {
    let mut list = Vec::new();
    let (mut win, mut steps) = (next(h), 0);
    while win != 0 && steps < 5000 && list.len() < NEIGHBOURS {
        if counts(win) && !is_iconic(win) && !anti_afk::is_desktop(win) {
            list.push(win);
        }
        win = next(win);
        steps += 1;
    }
    list
}

/// Whether `x` is under `h` now (a window of the same round still in front, waiting for its own turn, isn't).
fn is_under(h: isize, x: isize) -> bool {
    let (mut win, mut steps) = (next(h), 0);
    while win != 0 && steps < 5000 {
        if win == x {
            return true;
        }
        win = next(win);
        steps += 1;
    }
    false
}

// ───────── what a borrower does ─────────

/// Whether a window is made all but invisible while it is lent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unseen {
    /// Left as it looks (the live view: the person may look at it).
    No,
    /// Only when the loan opens it (minimised or hidden): one in view stays as it is.
    IfOpened,
    /// Always (anti-AFK: neither its coming to the front nor a click at that moment shows).
    Always,
}

/// What a borrower does with the window.
#[derive(Clone, Copy, Debug)]
pub struct How {
    /// For error.log.
    pub who: &'static str,
    pub unseen: Unseen,
    /// Clicks pass through it while it is unseen (not for a macro's turn: its clicks must land on it).
    pub click_through: bool,
    /// A window Bloxkeep hid (Hide windows) is shown for the loan; a minimised one always is opened.
    pub open_hidden: bool,
    /// The borrower brings it to the front and gives it the keyboard: at give-back the keyboard goes back to the window
    /// that had it, and the window goes back to its place in the stack.
    pub to_front: bool,
}

impl How {
    /// Anti-AFK's nudge.
    pub const NUDGE: How = How { who: "nudge", unseen: Unseen::Always, click_through: true, open_hidden: true, to_front: true };
    /// A macro's turn: shown unseen when minimised or hidden, but not click-through.
    pub const TURN: How = How { who: "macro turn", unseen: Unseen::IfOpened, click_through: false, open_hidden: true, to_front: true };
    /// A picture of a minimised or hidden window: shown unseen, click-through, never given the keyboard.
    pub const PICTURE: How = How { who: "picture", unseen: Unseen::IfOpened, click_through: true, open_hidden: true, to_front: false };
    /// The live view: a minimised window opened in view without taking the keyboard, for as long as the view is open.
    pub const VIEW: How = How { who: "live view", unseen: Unseen::No, click_through: false, open_hidden: false, to_front: false };
}

/// A window as it was when lent.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub window: isize,
    pub pid: u32,
    pub minimised: bool,
    /// Not shown (Bloxkeep's Hide windows).
    pub unshown: bool,
    pub maximised: bool,
    /// On Bloxkeep's own list of hidden windows (roblox_processes::hide_windows).
    pub hidden_by_us: bool,
    pub rect: Option<Rect>,
    pub ex_style: i32,
    /// Its own layered attributes when it was layered already; (0, 255, LWA_ALPHA) for one layered another way
    /// (UpdateLayeredWindow: nothing to read, shown fully again).
    pub layered: Option<(u32, u8, u32)>,
    /// The windows that stood over it, nearest first; read before any window of the round moved.
    pub above: Vec<isize>,
    /// The windows under it, nearest first.
    pub below: Vec<isize>,
    /// How many windows stood over it: the round gives back the highest first.
    pub depth: i32,
    /// It had the keyboard itself (a minimised window can keep it): being in front at give-back then says nothing.
    pub had_front: bool,
}

/// What the loan opened, for the borrower's own waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lent {
    pub minimised: bool,
    pub unshown: bool,
}

// ───────── the decision at give-back: pure ─────────

/// What the loan did to the window, as give-back weighs it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Was {
    /// It was minimised and the loan opened it.
    pub minimised: bool,
    /// It was hidden and the loan showed it.
    pub unshown: bool,
    /// Its hiding was Bloxkeep's own (Hide windows).
    pub hidden_by_us: bool,
    /// It had the keyboard when lent.
    pub had_front: bool,
    /// The loan brought it to the front.
    pub to_front: bool,
    /// The loan gave it another place.
    pub moved: bool,
}

/// The window as Windows shows it at give-back (after the keyboard went back to the window that had it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Seen {
    /// Still a window of the same process.
    pub exists: bool,
    pub hung: bool,
    pub minimised: bool,
    pub unshown: bool,
    pub hidden_by_us: bool,
    /// An ordinary window, title bar and all (not maximised, not full screen).
    pub plain: bool,
    /// It has the keyboard.
    pub front: bool,
    /// Input came since it was lent that wasn't Bloxkeep's own.
    pub person: bool,
}

/// Why a window is left as it is now instead of as it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    Gone,
    NotAnswering,
    /// In front with a person's input: the person has it.
    PersonInFront,
    /// Shown by Bloxkeep's own Show windows meanwhile.
    ShownMeanwhile,
    MinimisedMeanwhile,
    HiddenMeanwhile,
}

/// What give-back does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Back {
    /// Back to the place it had before the loan moved it.
    pub place_back: bool,
    /// Back under the window it stood under.
    pub restack: bool,
    pub minimise: bool,
    pub hide: bool,
    /// Left as it is now, and why.
    pub kept: Option<Kept>,
}

/// Back to the state it had, unless someone changed it meanwhile: then it is left as they have it. A window the loan
/// opened that is now in front with input that wasn't Bloxkeep's (and wasn't in front already when lent) is the
/// person's; one that was open and is minimised or hidden now was put away by someone; a hidden one shown by
/// Bloxkeep's own Show windows stays shown. A place given by the loan is taken back only while the window is still an
/// ordinary one (maximised or made full screen since, that is someone's doing).
pub fn back(was: &Was, now: &Seen) -> Back {
    let keep = |why: Kept| Back { kept: Some(why), ..Back::default() };
    if !now.exists {
        return keep(Kept::Gone);
    }
    if now.hung {
        return keep(Kept::NotAnswering);
    }
    let persons = now.front && now.person && !was.had_front;
    let place_back = was.moved && now.plain;
    // Minimised and hidden are two facts, and a window can have had both (hidden by Bloxkeep while minimised): it goes
    // back to both, or it stays on the taskbar after the loan. Shown meanwhile by Bloxkeep's Show windows, it is left
    // as that left it.
    let shown_meanwhile = was.unshown && was.hidden_by_us && !now.hidden_by_us;
    if was.minimised {
        if shown_meanwhile && !now.unshown {
            return keep(Kept::ShownMeanwhile);
        }
        let hide = was.unshown && !now.unshown;
        if now.minimised {
            return Back { hide, ..Back::default() };
        }
        if now.unshown {
            // Hidden is what it was too: it is off the screen and the taskbar as it was.
            return if was.unshown { Back::default() } else { keep(Kept::HiddenMeanwhile) };
        }
        if persons {
            return keep(Kept::PersonInFront);
        }
        return Back { place_back, minimise: true, hide, ..Back::default() };
    }
    if was.unshown {
        if now.unshown {
            return Back::default();
        }
        if was.hidden_by_us && !now.hidden_by_us {
            return keep(Kept::ShownMeanwhile);
        }
        if persons {
            return keep(Kept::PersonInFront);
        }
        return Back { place_back, hide: true, ..Back::default() };
    }
    if now.minimised {
        return keep(Kept::MinimisedMeanwhile);
    }
    if now.unshown {
        return keep(Kept::HiddenMeanwhile);
    }
    if persons {
        return keep(Kept::PersonInFront);
    }
    Back { place_back, restack: was.to_front, ..Back::default() }
}

/// How a window went back in the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restacked {
    /// Nothing stood over it: it is where it was.
    Top,
    /// Under `above[at]`; `refused` of the nearer ones still there refused (the last refusal's error code).
    Under { at: usize, refused: usize, error: Option<u32> },
    /// Over `below[at]`, the nearest window that was under it and still is; `second` whether that one went under it
    /// in turn.
    Over { at: usize, second: bool, error: Option<u32> },
    /// Neither: it stays where it is (in front).
    Stuck { error: Option<u32> },
}

/// Back under the nearest window that stood over it and is still a shown window and accepts. Windows refuses that
/// when that window belongs to a program run as administrator (Task Manager: error 5), and it may be gone by now (a
/// tooltip, a menu): failing every one above, the window goes back over the nearest window that was under it and still
/// is, placed under that one and then that one under it, which needs no word from the windows above. `set` is
/// `set_under` (a check's stand-in may refuse).
pub(crate) fn restack(h: isize, above: &[isize], below: &[isize], set: &dyn Fn(isize, isize) -> Result<(), u32>) -> Restacked {
    if above.is_empty() {
        return Restacked::Top;
    }
    let (mut refused, mut error) = (0, None);
    for (at, &x) in above.iter().enumerate() {
        if x == h || !is_window(x) || !is_visible(x) {
            continue;
        }
        match set(h, x) {
            Ok(()) => return Restacked::Under { at, refused, error },
            Err(e) => {
                refused += 1;
                error = Some(e);
            }
        }
    }
    for (at, &x) in below.iter().enumerate() {
        if x == h || !is_window(x) || is_hung(x) || !is_under(h, x) {
            continue;
        }
        match set(h, x) {
            Ok(()) => {
                let second = set(x, h);
                return Restacked::Over { at, second: second.is_ok(), error: second.err().or(error) };
            }
            Err(e) => error = Some(e),
        }
    }
    Restacked::Stuck { error }
}

// ───────── a round of loans ─────────

struct Entry {
    snap: Snapshot,
    /// The borrower took it (`lend`); its state in `snap` was read then.
    lent: bool,
    made_unseen: bool,
    opened: bool,
    /// Asks to minimise or hide it again so far.
    asked: u8,
    /// Where it stood before `move_to`.
    moved_from: Option<Rect>,
}

/// How one window went back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Returned {
    pub window: isize,
    /// Left as it is now instead of as it was, and why.
    pub kept: Option<Kept>,
    /// Asks to minimise or hide it again (0: none was needed).
    pub asked: u8,
    /// Every step held.
    pub held: bool,
    /// Put back at the place it had before the loan moved it.
    pub placed_back: bool,
    pub restacked: Option<Restacked>,
}

/// Several windows lent together (anti-AFK's nudge): every one's place in the stack read before any of them moves, each
/// taken by `lend` when its turn comes, all given back together (the highest first, so one that goes under another
/// finds it there). Given back on drop too.
pub struct Round {
    how: How,
    entries: Vec<Entry>,
    /// The window that had the keyboard when the round began.
    front_before: isize,
    /// The person's last input (Bloxkeep's own left out) when the round began.
    person_tick: u32,
    done: bool,
}

/// Every window's place in the stack, read now, before any of them moves; each is lent from now on.
pub fn borrow_round(windows: &[isize], how: How) -> Round {
    let front = foreground();
    let person_tick = anti_afk::person_tick();
    let mut entries: Vec<Entry> = Vec::new();
    for &h in windows {
        if h == 0 || entries.iter().any(|e| e.snap.window == h) || !is_window(h) {
            continue;
        }
        let (above, depth) = above_of(h);
        let snap = Snapshot { window: h, pid: pid_of(h), above, below: below_of(h), depth, had_front: front == h, ..Snapshot::default() };
        mark(h);
        entries.push(Entry { snap, lent: false, made_unseen: false, opened: false, asked: 0, moved_from: None });
    }
    Round { how, entries, front_before: front, person_tick, done: false }
}

/// One window lent at once: its state read, then unseen and opened as `how` says. None when it is gone or not
/// answering (a window that has stopped answering would hold whoever touches it).
pub fn borrow(h: isize, how: How) -> Option<Loan> {
    let mut round = borrow_round(&[h], how);
    round.lend(h)?;
    Some(Loan { round })
}

impl Round {
    /// The window's turn: its state read now, made unseen (and click-through) as `How` says, and opened when minimised
    /// (or hidden, as `How` says), without waiting for it. None when it isn't in the round, was lent already, is gone or
    /// isn't answering.
    pub fn lend(&mut self, h: isize) -> Option<Lent> {
        let how = self.how;
        let e = self.entries.iter_mut().find(|e| e.snap.window == h && !e.lent)?;
        if !is_window(h) || is_hung(h) || pid_of(h) != e.snap.pid {
            return None;
        }
        let s = &mut e.snap;
        s.minimised = is_iconic(h);
        s.unshown = !is_visible(h);
        s.maximised = is_zoomed(h);
        s.hidden_by_us = roblox_processes::is_hidden(h);
        s.rect = roblox_processes::window_rect(h);
        s.ex_style = get_ex_style(h);
        if (s.ex_style & WS_EX_LAYERED) != 0 {
            s.layered = Some(layered_attributes(h).unwrap_or((0, 255, LWA_ALPHA)));
        }
        let opening = s.minimised || (s.unshown && how.open_hidden);
        let unseen = match how.unseen {
            Unseen::No => false,
            Unseen::IfOpened => opening,
            Unseen::Always => true,
        };
        if unseen {
            // Unseen first, so neither its coming up nor a click at that moment shows.
            set_ex_style(h, s.ex_style | WS_EX_LAYERED | if how.click_through { WS_EX_TRANSPARENT } else { 0 });
            set_layered_attributes(h, 0, HIDDEN_ALPHA, LWA_ALPHA);
            // A minimised one also loses Windows' restore animation, which the taskbar would show.
            if s.minimised {
                animations(h, false);
            }
        }
        if s.minimised {
            show_async(h, if how.to_front { SW_RESTORE } else { SW_SHOWNOACTIVATE });
        } else if s.unshown && how.open_hidden {
            show_async(h, SW_SHOWNOACTIVATE);
        }
        let lent = Lent { minimised: s.minimised, unshown: s.unshown };
        e.lent = true;
        e.made_unseen = unseen;
        e.opened = opening;
        Some(lent)
    }

    /// Asks a window the loan opened to go back down now (minimised or hidden again), without waiting: Roblox has had
    /// what it was opened for, and the next window's turn needn't wait for it. Give-back waits for it and asks again
    /// if it didn't land; the window is seen again only then.
    pub fn put_away(&mut self, h: isize) {
        let Some(e) = self.entries.iter_mut().find(|e| e.snap.window == h && e.lent && e.opened && e.asked == 0) else { return };
        if !is_window(h) || pid_of(h) != e.snap.pid {
            return;
        }
        // Both when it was both: minimised first, then hidden (asked in that order, they land in that order).
        if e.snap.minimised && !is_iconic(h) {
            show_async(h, SW_SHOWMINNOACTIVE);
            e.asked = 1;
        }
        if e.snap.unshown && is_visible(h) {
            show_async(h, SW_HIDE);
            e.asked = 1;
        }
    }

    /// For the checks: give-back hands the keyboard to nobody (a check's desktop has no window of the person's).
    pub(crate) fn forget_front(&mut self) {
        self.front_before = 0;
    }

    /// Each window back as it was (see `back`): the keyboard first back to the window that had it (when the borrower
    /// took it), then each window back in its place (the highest first) and state, a re-minimise or re-hide waited for
    /// and asked once more when it didn't land, and only then seen again. One line in error.log for a window when a
    /// step didn't hold.
    pub fn give_back(mut self) -> Vec<Returned> {
        self.finish(None, WAIT)
    }

    /// `give_back`, the borrower saying whether a person's input came meanwhile (`person`; None: Bloxkeep's own
    /// reckoning, input newer than its own) and how long each wait for a window to go back down may take.
    pub fn give_back_with(mut self, person: Option<bool>, wait: Duration) -> Vec<Returned> {
        self.finish(person, wait)
    }

    fn finish(&mut self, person: Option<bool>, wait: Duration) -> Vec<Returned> {
        if self.done {
            return Vec::new();
        }
        self.done = true;
        let how = self.how;
        let mut order: Vec<usize> = (0..self.entries.len()).filter(|&i| self.entries[i].lent).collect();
        // Nothing taken (turned off, a person back, no window answering): the window you're in is left alone.
        if how.to_front && !order.is_empty() {
            anti_afk::give_back(self.front_before);
        }
        let person = person.unwrap_or_else(|| anti_afk::person_tick() != self.person_tick);
        let front = foreground();
        // OrderBy: stable, the highest in the stack first.
        order.sort_by_key(|&i| self.entries[i].snap.depth);
        let mut out: Vec<Returned> = Vec::new();
        let mut notes: Vec<Vec<String>> = Vec::new();
        // (its place in `out`, and what it goes down to)
        let mut down: Vec<(usize, Down)> = Vec::new();
        for &i in &order {
            let e = &mut self.entries[i];
            let h = e.snap.window;
            let mut said: Vec<String> = Vec::new();
            let mut r = Returned { window: h, kept: None, asked: 0, held: true, placed_back: false, restacked: None };
            // A loan that changed nothing (the live view of a window in view, not moved) has nothing to give back.
            if !e.opened && !e.made_unseen && e.moved_from.is_none() && !how.to_front {
                out.push(r);
                notes.push(said);
                continue;
            }
            let exists = is_window(h) && pid_of(h) == e.snap.pid;
            let seen = Seen {
                exists,
                hung: exists && is_hung(h),
                minimised: exists && is_iconic(h),
                unshown: exists && !is_visible(h),
                hidden_by_us: exists && roblox_processes::is_hidden(h),
                plain: exists && roblox_processes::is_plain_window(h),
                front: exists && front == h,
                person,
            };
            let was = Was {
                minimised: e.snap.minimised,
                unshown: e.snap.unshown && e.opened,
                hidden_by_us: e.snap.hidden_by_us,
                had_front: e.snap.had_front,
                to_front: how.to_front,
                moved: e.moved_from.is_some(),
            };
            let b = back(&was, &seen);
            r.kept = b.kept;
            match b.kept {
                Some(Kept::PersonInFront) => said.push("left open: it is in front with a person's input".into()),
                Some(Kept::ShownMeanwhile) => said.push("left shown: Bloxkeep's Show windows showed it meanwhile".into()),
                _ => {}
            }
            if b.place_back
                && let Some(place) = e.moved_from
            {
                roblox_processes::place_window(h, place);
                r.placed_back = true;
            }
            if b.restack {
                let how_back = restack(h, &e.snap.above, &e.snap.below, &set_under);
                r.restacked = Some(how_back);
                match how_back {
                    Restacked::Top | Restacked::Under { refused: 0, .. } => {}
                    Restacked::Under { refused, error, .. } => {
                        said.push(format!("{refused} window(s) above it refused it{}; it went under the next one above", code(error)));
                    }
                    Restacked::Over { second, error, .. } => {
                        r.held = false;
                        said.push(format!(
                            "couldn't go back under any window that was above it (Windows refused{}, or they are gone); put back over the one below it instead{}",
                            code(error),
                            if second { "" } else { ", which only partly held" }
                        ));
                    }
                    Restacked::Stuck { error } => {
                        r.held = false;
                        said.push(format!("couldn't be put back where it stood{}; it stays in front", code(error)));
                    }
                }
            }
            if b.minimise || b.hide {
                if e.asked == 0 {
                    ask_down(h, b.minimise, b.hide);
                    e.asked = 1;
                }
                down.push((out.len(), Down { minimise: b.minimise, hide: b.hide }));
            }
            out.push(r);
            notes.push(said);
        }
        // Waited for, then asked once more when it didn't land (a full-screen Roblox comes back up by itself once).
        let landed = |h: isize, d: Down| !is_window(h) || ((!d.minimise || is_iconic(h)) && (!d.hide || !is_visible(h)));
        let wait_down = |out: &[Returned], down: &[(usize, Down)]| {
            let until = Instant::now() + wait;
            while Instant::now() < until && down.iter().any(|&(k, d)| !landed(out[k].window, d)) {
                std::thread::sleep(Duration::from_millis(25));
            }
        };
        wait_down(&out, &down);
        let again: Vec<(usize, Down)> = down.iter().copied().filter(|&(k, d)| !landed(out[k].window, d)).collect();
        for &(k, d) in &again {
            let h = out[k].window;
            ask_down(h, d.minimise && !is_iconic(h), d.hide);
            if let Some(e) = self.entries.iter_mut().find(|e| e.snap.window == h) {
                e.asked += 1;
            }
        }
        wait_down(&out, &again);
        for &(k, d) in &again {
            if !landed(out[k].window, d) {
                out[k].held = false;
                let what = match (d.minimise, d.hide) {
                    (true, true) => "minimise and hide",
                    (true, false) => "minimise",
                    _ => "hide",
                };
                notes[k].push(format!("didn't {what} again, asked twice"));
            }
        }
        // Seen again only once in place (it would flash up otherwise); a window that stopped answering is left to
        // `recover` (a change of style waits for the window's answer).
        for (k, r) in out.iter_mut().enumerate() {
            let Some(e) = self.entries.iter().find(|e| e.snap.window == r.window) else { continue };
            r.asked = e.asked;
            if !e.made_unseen || !is_window(r.window) || pid_of(r.window) != e.snap.pid {
                continue;
            }
            if is_hung(r.window) {
                r.held = false;
                notes[k].push("stopped responding while shown unseen; left unseen until it answers".into());
                continue;
            }
            seen_again(&e.snap);
        }
        for (k, r) in out.iter().enumerate() {
            if !notes[k].is_empty() {
                let pid = self.entries.iter().find(|e| e.snap.window == r.window).map_or(0, |e| e.snap.pid);
                log::note(&format!("Window loan: Roblox window {pid} ({}): {}.", how.who, notes[k].join("; ")));
            }
        }
        for e in &self.entries {
            unmark(e.snap.window);
        }
        out
    }
}

/// What a window goes back down to after a loan: minimised, hidden, or both.
#[derive(Clone, Copy)]
struct Down {
    minimise: bool,
    hide: bool,
}

/// Asks the window down: minimised first, then hidden, so one that was both ends off the taskbar too.
fn ask_down(h: isize, minimise: bool, hide: bool) {
    if minimise {
        show_async(h, SW_SHOWMINNOACTIVE);
    }
    if hide {
        show_async(h, SW_HIDE);
    }
}

fn code(error: Option<u32>) -> String {
    error.map(|e| format!(", error {e}")).unwrap_or_default()
}

/// Only the bits `lend` set are touched: its own style and opacity back, its animations on again.
fn seen_again(s: &Snapshot) {
    const OURS: i32 = WS_EX_LAYERED | WS_EX_TRANSPARENT;
    let h = s.window;
    set_ex_style(h, (get_ex_style(h) & !OURS) | (s.ex_style & OURS));
    if let Some((key, alpha, flags)) = s.layered {
        set_layered_attributes(h, key, alpha, flags);
    }
    if s.minimised {
        animations(h, true);
    }
    redraw(h);
}

impl Drop for Round {
    fn drop(&mut self) {
        self.finish(None, WAIT);
    }
}

/// One window lent (a macro's turn, a picture, the live view); given back on drop too.
pub struct Loan {
    round: Round,
}

impl Loan {
    pub fn window(&self) -> isize {
        self.round.entries[0].snap.window
    }

    /// The window as it was when lent.
    pub fn was(&self) -> &Snapshot {
        &self.round.entries[0].snap
    }

    /// Opened again when it went back down by itself meanwhile (a full-screen Roblox minimises when it isn't in front).
    pub fn open_again(&self) {
        let e = &self.round.entries[0];
        let h = e.snap.window;
        if e.opened && is_window(h) && (e.snap.minimised && is_iconic(h) || e.snap.unshown && !is_visible(h)) {
            show_async(h, SW_SHOWNOACTIVATE);
        }
    }

    /// The window to `r` for the loan (without coming to the front); give-back puts it where it stood before, while
    /// it is still an ordinary window.
    pub fn move_to(&mut self, r: Rect) {
        let e = &mut self.round.entries[0];
        if e.moved_from.is_none() {
            e.moved_from = roblox_processes::window_rect(e.snap.window);
        }
        if e.moved_from.is_some() {
            roblox_processes::place_window(e.snap.window, r);
        }
    }

    /// The loan gave it another place.
    pub fn moved(&self) -> bool {
        self.round.entries[0].moved_from.is_some()
    }

    pub fn give_back(self) -> Returned {
        self.give_back_with(None, WAIT)
    }

    /// See `Round::give_back_with`.
    pub fn give_back_with(mut self, person: Option<bool>, wait: Duration) -> Returned {
        let h = self.window();
        self.round.finish(person, wait).into_iter().next().unwrap_or(Returned { window: h, kept: None, asked: 0, held: true, placed_back: false, restacked: None })
    }

    /// For the checks: see `Round::forget_front`.
    pub(crate) fn forget_front(&mut self) {
        self.round.forget_front();
    }
}

/// Undoes a loan cut short by Bloxkeep closing (a nudge, a picture, a macro's turn): a window left all but invisible,
/// told apart by HIDDEN_ALPHA, which nothing else uses, made visible again. A macro's turn leaves its unseen window
/// layered but not click-through: undone the same way. A window lent now is unseen on purpose and left alone.
pub fn recover(windows: &[isize]) {
    const OURS: i32 = WS_EX_LAYERED | WS_EX_TRANSPARENT;
    let mut recovered: Vec<u32> = Vec::new();
    for &win in windows {
        if is_lent(win) {
            continue;
        }
        let ex = get_ex_style(win);
        if (ex & WS_EX_LAYERED) == 0 {
            continue;
        }
        let Some((_, alpha, flags)) = layered_attributes(win) else {
            continue;
        };
        if (flags & LWA_ALPHA) == 0 || alpha != HIDDEN_ALPHA || is_hung(win) {
            continue;
        }
        set_ex_style(win, ex & !OURS);
        animations(win, true);
        redraw(win);
        recovered.push(pid_of(win));
    }
    if !recovered.is_empty() {
        log::note(&format!(
            "Unseen window: made {} unseen Roblox window(s) visible again after a cut-short nudge or turn (pids {}).",
            recovered.len(),
            recovered.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")
        ));
    }
}
