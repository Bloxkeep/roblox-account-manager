// The core's own thread, and how its work waits. The C# app ran its controller on the UI thread: one thing at a time,
// an `await` letting other work in until its answer came. The core keeps that shape: the App lives on one thread
// (never shared, never locked), its async methods are tasks on that thread, and anything slow (a web call, a file,
// a sleep) runs on another thread while the task waits for it.
//
//   C#                                   here
//   await SomethingSlowAsync()           rt::blocking(move || something_slow()).await
//   await Task.Delay(d)                  rt::sleep(d).await
//   _ = FooAsync();  (not waited for)    rt::spawn(async move { app.foo().await; })
//   BeginInvoke(() => …) from a thread   rt::post(move || …)   (runs on the core's thread)
use crate::threads::Hold;
use std::{
    cell::RefCell,
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
    future::Future,
    sync::{Condvar, Mutex, MutexGuard, OnceLock},
    time::{Duration, Instant},
};

use futures::{
    StreamExt,
    channel::{mpsc, oneshot},
    executor::{LocalPool, LocalSpawner},
    task::LocalSpawnExt,
};

use crate::core::base::Cancel;

thread_local! {
    static SPAWNER: RefCell<Option<LocalSpawner>> = const { RefCell::new(None) };
}

type Posted = Box<dyn FnOnce() + Send + 'static>;
static POSTS: OnceLock<Mutex<mpsc::UnboundedSender<Posted>>> = OnceLock::new();

/// Makes this thread the core's and runs `main` on it to its end; tasks started with `spawn` and closures sent with
/// `post` run here meanwhile.
pub fn run<F: Future<Output = ()> + 'static>(main: impl FnOnce() -> F) {
    let mut pool = LocalPool::new();
    SPAWNER.with(|s| *s.borrow_mut() = Some(pool.spawner()));
    let (tx, mut rx) = mpsc::unbounded::<Posted>();
    let _ = POSTS.set(Mutex::new(tx));
    spawn(async move {
        while let Some(f) = rx.next().await {
            // One that panics is shown as an unexpected error; the core goes on with the next (Application.ThreadException).
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        }
    });
    pool.run_until(main());
}

/// A task of the core's own, not waited for. Only from the core's thread. One that panics ends there (shown as an
/// unexpected error, its values dropped); the core's thread and its other tasks go on.
pub fn spawn(f: impl Future<Output = ()> + 'static) {
    use futures::FutureExt;
    let f = std::panic::AssertUnwindSafe(f).catch_unwind().map(|_| ());
    SPAWNER.with(|s| {
        if let Some(sp) = s.borrow().as_ref() {
            let _ = sp.spawn_local(f);
        }
    });
}

/// From any thread: runs `f` on the core's thread, in its turn (a watcher's event, a window's click).
pub fn post(f: impl FnOnce() + Send + 'static) {
    if let Some(tx) = POSTS.get().map(|m| m.hold()) {
        let _ = tx.unbounded_send(Box::new(f));
    }
}

/// Slow work on a pool thread (threads.rs); the task goes on with its answer. Should the work panic, the panic goes on
/// in the waiting task (no second error box), which ends there as `spawn` says: its values are dropped, so a guard it
/// holds (a busy flag's) is let go rather than the task waiting forever.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    match crate::threads::call(f).await {
        Ok(Ok(v)) => v,
        Ok(Err(panic)) => std::panic::resume_unwind(panic),
        // No pool thread could ever be made: nothing will answer.
        Err(_) => std::future::pending().await,
    }
}

/// The sleeps waiting: one timer thread wakes each at its time (a thread per sleep kept a called-off long one, a
/// backup's or an import's, asleep to its end).
struct Timers {
    /// (when, id), soonest first; an id no longer in `live` was dropped and is skipped.
    due: BinaryHeap<Reverse<(Instant, u64)>>,
    live: BTreeMap<u64, oneshot::Sender<()>>,
    next: u64,
}

static TIMERS: Mutex<Timers> = Mutex::new(Timers { due: BinaryHeap::new(), live: BTreeMap::new(), next: 0 });
static TIMERS_WAKE: Condvar = Condvar::new();
static TIMER_THREAD: OnceLock<bool> = OnceLock::new();

fn timers() -> MutexGuard<'static, Timers> {
    TIMERS.lock().unwrap_or_else(|e| e.into_inner())
}

fn timer_thread() {
    let mut t = timers();
    loop {
        let now = Instant::now();
        while let Some(&Reverse((at, id))) = t.due.peek() {
            if at > now {
                break;
            }
            t.due.pop();
            if let Some(tx) = t.live.remove(&id) {
                let _ = tx.send(());
            }
        }
        t = match t.due.peek() {
            Some(&Reverse((at, _))) => TIMERS_WAKE.wait_timeout(t, at.saturating_duration_since(now)).unwrap_or_else(|e| e.into_inner()).0,
            None => TIMERS_WAKE.wait(t).unwrap_or_else(|e| e.into_inner()),
        };
    }
}

/// Task.Delay(d). Dropped before its time (a task called off), it lets go of its place at once.
pub async fn sleep(d: Duration) {
    struct Slot(u64);
    impl Drop for Slot {
        fn drop(&mut self) {
            let mut t = timers();
            t.live.remove(&self.0);
            // Many called off: their old places go too, so rescheduling a long sleep again and again doesn't pile up.
            if t.due.len() > 64 && t.due.len() > 2 * t.live.len() {
                let live = std::mem::take(&mut t.live);
                t.due.retain(|Reverse((_, id))| live.contains_key(id));
                t.live = live;
            }
        }
    }
    let started = *TIMER_THREAD.get_or_init(|| crate::threads::own("core-timer", timer_thread).is_ok());
    if !started {
        // No timer thread could be made: a thread of its own, as before.
        return blocking(move || std::thread::sleep(d)).await;
    }
    let Some(at) = Instant::now().checked_add(d) else {
        // Beyond any clock: never wakes.
        return std::future::pending().await;
    };
    let (tx, rx) = oneshot::channel();
    let id = {
        let mut t = timers();
        t.next += 1;
        let id = t.next;
        t.due.push(Reverse((at, id)));
        t.live.insert(id, tx);
        id
    };
    let _slot = Slot(id);
    TIMERS_WAKE.notify_one();
    let _ = rx.await;
}

/// Task.Delay(d, token): false when it was called off before its time.
// On the shared timer in short steps (no thread of its own per call, as before): a call off is seen within a
// quarter of a second, and a task dropped meanwhile leaves at most one short slot behind.
pub async fn sleep_or(cancel: &Cancel, d: Duration) -> bool {
    const STEP: Duration = Duration::from_millis(250);
    let until = std::time::Instant::now() + d;
    loop {
        if cancel.is_cancelled() {
            return false;
        }
        let left = until.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return true;
        }
        sleep(left.min(STEP)).await;
    }
}
