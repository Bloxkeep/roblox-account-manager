//! One thing at a time brings a Roblox window forward or shows a hidden one for a moment: anti-AFK's nudge, /pc
//! screenshot's picture, a macro's turn, Arrange windows. Each takes its turn in the order asked (a macro's next turn
//! waits behind a nudge asked meanwhile), so none undoes another's window halfway. Work that only moves or sizes windows
//! on a timer (Maximise, window sizes) skips a tick while `busy`; a click or hotkey waits for the turn.
//! The queue has its own lock: callers may be on any thread.
use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use super::{
    base::{Cancel, Fail, Res},
    log,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    Pending,
    Granted(u64),
    Cancelled,
}

/// A TaskCompletionSource<IDisposable>: answered once, by the turn or by the asker giving up.
struct Ready {
    answer: Mutex<Answer>,
    signal: Condvar,
}

impl Ready {
    fn is_completed(&self) -> bool {
        *self.answer.lock().unwrap_or_else(|e| e.into_inner()) != Answer::Pending
    }
    /// TrySetResult / TrySetCanceled: only the first answer counts.
    fn try_set(&self, to: Answer) -> bool {
        let mut a = self.answer.lock().unwrap_or_else(|e| e.into_inner());
        if *a != Answer::Pending {
            return false;
        }
        *a = to;
        self.signal.notify_all();
        true
    }
}

struct Gate {
    waiting: VecDeque<(String, Arc<Ready>)>,
    /// The lease of the one holding the turn (its serial and who): only that lease hands it on (one from before `reset` can't).
    holder: Option<(u64, String)>,
}

static GATE: Mutex<Gate> = Mutex::new(Gate { waiting: VecDeque::new(), holder: None });
static SERIAL: AtomicU64 = AtomicU64::new(1);

fn gate() -> std::sync::MutexGuard<'static, Gate> {
    GATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn new_lease(g: &mut Gate, who: &str) -> Lease {
    let id = SERIAL.fetch_add(1, Ordering::Relaxed);
    g.holder = Some((id, who.to_string()));
    Lease { id, who: who.to_string(), done: AtomicBool::new(false) }
}

/// A window is someone's right now (or someone waits for one).
pub fn busy() -> bool {
    gate().holder.is_some()
}

/// Who has the turn ("afk", "picture", "macro", "arrange"…); None when free.
pub fn holder() -> Option<String> {
    gate().holder.as_ref().map(|h| h.1.clone())
}

/// Someone of this name waits for the turn (anti-AFK's "afk": a long turn hands the PC on).
pub fn waits(who: &str) -> bool {
    gate().waiting.iter().any(|w| w.0 == who && !w.1.is_completed())
}

/// The turn at once, or None when someone has it.
pub fn try_enter(who: &str) -> Option<Lease> {
    let mut g = gate();
    if g.holder.is_some() {
        return None;
    }
    Some(new_lease(&mut g, who))
}

/// The turn once those asking before have had theirs (blocking). Dropping the answer hands it on. Err when `ct` called
/// the wait off first (TaskCanceledException).
pub fn enter(who: &str, ct: Option<&Cancel>) -> Res<Lease> {
    let ready = {
        let mut g = gate();
        if g.holder.is_none() {
            return Ok(new_lease(&mut g, who));
        }
        let ready = Arc::new(Ready { answer: Mutex::new(Answer::Pending), signal: Condvar::new() });
        g.waiting.push_back((who.to_string(), ready.clone()));
        ready
    };
    let mut a = ready.answer.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        match *a {
            Answer::Granted(id) => return Ok(Lease { id, who: who.to_string(), done: AtomicBool::new(false) }),
            Answer::Cancelled => return Err(Fail::other("cancelled")),
            Answer::Pending => {}
        }
        if ct.is_some_and(|c| c.is_cancelled()) {
            *a = Answer::Cancelled;
            ready.signal.notify_all();
            return Err(Fail::other("cancelled"));
        }
        a = ready.signal.wait_timeout(a, Duration::from_millis(25)).unwrap_or_else(|e| e.into_inner()).0;
    }
}

/// Runs `action` now when the turn is free, else once it is (a click, a hotkey).
pub fn run_when_free(who: &str, action: impl FnOnce() + Send + 'static) {
    if let Some(lease) = try_enter(who) {
        action();
        drop(lease);
        return;
    }
    // PORT: the C# continued on the window's thread once the turn came; here the wait and the action run on a pool thread.
    let who = who.to_string();
    crate::threads::run(move || match enter(&who, None) {
        Ok(lease) => {
            action();
            drop(lease);
        }
        Err(ex) => log::write(&ex),
    });
}

fn leave(id: u64) {
    let mut g = gate();
    if g.holder.as_ref().map(|h| h.0) != Some(id) {
        return;
    }
    g.holder = None;
    while let Some((who, ready)) = g.waiting.pop_front() {
        // One that gave up (cancelled) is passed over.
        if ready.is_completed() {
            continue;
        }
        let next = SERIAL.fetch_add(1, Ordering::Relaxed);
        g.holder = Some((next, who));
        if ready.try_set(Answer::Granted(next)) {
            return;
        }
        g.holder = None;
    }
}

/// For the checks: nobody holds or waits for a turn.
pub fn reset() {
    let mut g = gate();
    g.holder = None;
    g.waiting.clear();
}

/// The turn, held until dropped (IDisposable).
pub struct Lease {
    id: u64,
    who: String,
    done: AtomicBool,
}

impl Lease {
    pub fn who(&self) -> &str {
        &self.who
    }
    /// Dispose: hands the turn on (once).
    pub fn dispose(&self) {
        if !self.done.swap(true, Ordering::SeqCst) {
            leave(self.id);
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.dispose();
    }
}
