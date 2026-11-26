// Bloxkeep's threads: the one place a thread is made (clippy.toml forbids std::thread::spawn, scope and Builder
// everywhere else).
//
// Why: NVIDIA's graphics driver (nvwgf2umx.dll) keeps about 184 bytes for every thread the process ever starts and never
// gives them back, however briefly the thread lived (NVIDIA forum 381379; measured here: 12,696 threads, +2.6 MB). A
// thread made per frame, per poll or per web call grew a user's Bloxkeep past 1 GB. So:
//
//   * short work (a web call, a file, a picture's band) goes to the shared pool: `run`, `call`, `scope`. Its threads are
//     made as they are first needed, at most `MOST`, and never end: past them work waits its turn. Over a whole run
//     Bloxkeep starts at most `MOST` of them, however much work passes.
//   * a thread that lives as long as the app, or holds something Windows ties to its thread for as long as that lives
//     (a hook, a window and its message loop, a browser's COM apartment), is made with `own`. Never per frame, poll or
//     click of something done often.
use std::{
    any::Any,
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, MutexGuard},
};

/// A lock taken whatever happened before: when work panicked while it held the lock, what it guards is taken as that
/// work left it. `lock().ok()`, `if let Ok(..) = m.lock()` and `lock().unwrap()` instead made every later use of a
/// "poisoned" lock silently do nothing (the picture loaders all stopped, a book of prices was never filled again) or
/// panic in turn; build.rs stops the build on those. What Bloxkeep guards this way is a cache, a list or a value: a
/// panic halfway leaves at worst one entry missing or stale, never a broken map (std's collections stay whole when a
/// panic stops a change). A lock guarding a state that must change in several steps together isn't to use this.
pub trait Hold<T: ?Sized> {
    fn hold(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> Hold<T> for Mutex<T> {
    fn hold(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The most pool threads there will ever be. Enough for the web calls of a big page asked for side by side (each mostly
/// waiting on the network); an idle one costs a few KB.
pub const MOST: usize = 32;

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Pool {
    queue: VecDeque<Job>,
    /// Threads waiting for work.
    idle: usize,
    /// Threads made (they never end).
    made: usize,
}

static POOL: Mutex<Pool> = Mutex::new(Pool { queue: VecDeque::new(), idle: 0, made: 0 });
static WAKE: Condvar = Condvar::new();

fn pool() -> MutexGuard<'static, Pool> {
    POOL.lock().unwrap_or_else(|e| e.into_inner())
}

fn worker() {
    loop {
        let job = {
            let mut p = pool();
            loop {
                if let Some(job) = p.queue.pop_front() {
                    break job;
                }
                p.idle += 1;
                p = WAKE.wait(p).unwrap_or_else(|e| e.into_inner());
                p.idle -= 1;
            }
        };
        // One that panics (the panic hook has put it in error.log) leaves the thread to the work after it.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
    }
}

fn submit(job: Job) {
    let mut p = pool();
    p.queue.push_back(job);
    if p.queue.len() <= p.idle {
        WAKE.notify_one();
        return;
    }
    if p.made >= MOST {
        // All busy: it waits for the first one free.
        return;
    }
    p.made += 1;
    let n = p.made;
    drop(p);
    if own(&format!("work-{n}"), worker).is_err() {
        // (Windows out of threads: the work waits for one already made; error.log has why.)
        pool().made -= 1;
    }
}

/// Runs `job` on a pool thread, not waited for.
pub fn run(job: impl FnOnce() + Send + 'static) {
    submit(Box::new(job));
}

/// Runs `job` on a pool thread; the answer comes on the receiver (Err(payload) when it panicked). Dropped unanswered
/// only when no pool thread could ever be made.
pub fn call<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> futures::channel::oneshot::Receiver<std::thread::Result<T>> {
    let (tx, rx) = futures::channel::oneshot::channel();
    run(move || {
        let _ = tx.send(std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)));
    });
    rx
}

/// A thread of its own, kept as long as `f` runs: only for one that lives as long as the app, or that holds something
/// Windows ties to its thread (a hook, a window and its message loop, a COM apartment) — see the top of this file. One
/// that can't start (Windows out of threads or memory) is in error.log: what it was for then doesn't happen.
pub fn own<T: Send + 'static>(name: &str, f: impl FnOnce() -> T + Send + 'static) -> std::io::Result<std::thread::JoinHandle<T>> {
    made(name, std::thread::Builder::new().name(name.into()), f)
}

/// A kept thread that does `round` again and again until it answers false. A round that panics (the panic hook has put
/// it in error.log) doesn't end the thread and what it was for: the next round comes after a pause, 1 s doubling to a
/// minute while they keep panicking (a picture that can't be decoded once stopped that loader for the rest of the run).
/// What a round keeps between rounds lives in the closure, so after a panic it goes on from the state the last good
/// round left.
pub fn keep(name: &str, mut round: impl FnMut() -> bool + Send + 'static) -> std::io::Result<std::thread::JoinHandle<()>> {
    own(name, move || {
        let mut pause = std::time::Duration::from_secs(1);
        loop {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut round)) {
                Ok(true) => pause = std::time::Duration::from_secs(1),
                Ok(false) => return,
                Err(_) => {
                    std::thread::sleep(pause);
                    pause = (pause * 2).min(std::time::Duration::from_secs(60));
                }
            }
        }
    })
}

/// As `own`, with a stack of `size` bytes.
pub fn own_with_stack<T: Send + 'static>(name: &str, size: usize, f: impl FnOnce() -> T + Send + 'static) -> std::io::Result<std::thread::JoinHandle<T>> {
    made(name, std::thread::Builder::new().name(name.into()).stack_size(size), f)
}

/// The one place Bloxkeep starts a thread.
#[allow(clippy::disallowed_methods)]
fn made<T: Send + 'static>(name: &str, b: std::thread::Builder, f: impl FnOnce() -> T + Send + 'static) -> std::io::Result<std::thread::JoinHandle<T>> {
    let r = b.spawn(f);
    if let Err(e) = &r {
        crate::net::note_failed("Background work", "", &format!("\"{name}\" couldn't start: {e}"));
    }
    r
}

// ---- after: work at a later time ----

struct Due {
    at: std::time::Instant,
    n: u64,
    job: Job,
}

impl PartialEq for Due {
    fn eq(&self, o: &Self) -> bool {
        (self.at, self.n) == (o.at, o.n)
    }
}
impl Eq for Due {}
impl PartialOrd for Due {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Due {
    /// Soonest first in the (max-)heap.
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        (o.at, o.n).cmp(&(self.at, self.n))
    }
}

static DUE: Mutex<(std::collections::BinaryHeap<Due>, u64, bool)> = Mutex::new((std::collections::BinaryHeap::new(), 0, false));
static DUE_WAKE: Condvar = Condvar::new();

/// Runs `job` on the pool once `delay` has passed: one kept thread waits for all of them, so a wait (a copied password
/// wiped after 30 s) holds no pool thread meanwhile.
pub fn after(delay: std::time::Duration, job: impl FnOnce() + Send + 'static) {
    let at = std::time::Instant::now().checked_add(delay).unwrap_or_else(|| std::time::Instant::now() + std::time::Duration::from_secs(86_400 * 365));
    let mut d = DUE.lock().unwrap_or_else(|e| e.into_inner());
    d.1 += 1;
    let n = d.1;
    d.0.push(Due { at, n, job: Box::new(job) });
    if !d.2 {
        d.2 = own("timers", || {
            let mut d = DUE.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                let now = std::time::Instant::now();
                while d.0.peek().is_some_and(|x| x.at <= now) {
                    if let Some(x) = d.0.pop() {
                        submit(x.job);
                    }
                }
                d = match d.0.peek().map(|x| x.at) {
                    Some(at) => DUE_WAKE.wait_timeout(d, at.saturating_duration_since(now)).unwrap_or_else(|e| e.into_inner()).0,
                    None => DUE_WAKE.wait(d).unwrap_or_else(|e| e.into_inner()),
                };
            }
        })
        .is_ok();
    }
    DUE_WAKE.notify_one();
}

// ---- scope: std::thread::scope on the pool ----

const WAITING: u8 = 0;
const TAKEN: u8 = 1;
const DONE: u8 = 2;

struct Task<'env, T> {
    state: Mutex<u8>,
    done: Condvar,
    work: Mutex<Option<Box<dyn FnOnce() -> T + Send + 'env>>>,
    answer: Mutex<Option<std::thread::Result<T>>>,
}

impl<T> Task<'_, T> {
    /// Takes the work if no pool thread has: true when this caller is to run it.
    fn take(&self) -> bool {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if *s == WAITING {
            *s = TAKEN;
            true
        } else {
            false
        }
    }

    fn work(&self) {
        let work = self.work.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(work) = work {
            let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
            *self.answer.lock().unwrap_or_else(|e| e.into_inner()) = Some(answer);
        }
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = DONE;
        self.done.notify_all();
    }

    /// Its end: run here when no pool thread took it yet (so a scope inside pool work never waits for a thread that
    /// isn't free: it does the work itself), else waited for.
    fn finish(&self) {
        if self.take() {
            self.work();
            return;
        }
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while *s != DONE {
            s = self.done.wait(s).unwrap_or_else(|e| e.into_inner());
        }
    }
}

trait Finish: Send + Sync {
    /// Ends the task and lets go of what it borrowed; Some(payload) when it panicked and nobody joined it.
    fn finish_unjoined(&self) -> Option<Box<dyn Any + Send>>;
}

impl<T: Send> Finish for Task<'_, T> {
    fn finish_unjoined(&self) -> Option<Box<dyn Any + Send>> {
        self.finish();
        match self.answer.lock().unwrap_or_else(|e| e.into_inner()).take() {
            Some(Err(panic)) => Some(panic),
            _ => None,
        }
    }
}

/// Like std::thread::Scope: work spawned on it may borrow from outside the scope.
pub struct Scope<'scope, 'env: 'scope> {
    /// Each task's end, its lifetime erased (see `spawn`): all are finished before `scope` returns.
    tasks: Mutex<Vec<Arc<dyn Finish + 'static>>>,
    _scope: std::marker::PhantomData<&'scope mut &'scope ()>,
    _env: std::marker::PhantomData<&'env mut &'env ()>,
}

/// Like std::thread::ScopedJoinHandle.
pub struct ScopedJoinHandle<'scope, T> {
    task: Arc<Task<'scope, T>>,
}

impl<'scope, 'env> Scope<'scope, 'env> {
    /// Like std::thread::Scope::spawn, on a pool thread (or, when none got to it first, on the thread that joins it).
    pub fn spawn<T: Send + 'scope>(&'scope self, f: impl FnOnce() -> T + Send + 'scope) -> ScopedJoinHandle<'scope, T> {
        let task: Arc<Task<'scope, T>> = Arc::new(Task {
            state: Mutex::new(WAITING),
            done: Condvar::new(),
            work: Mutex::new(Some(Box::new(f))),
            answer: Mutex::new(None),
        });
        let end: Arc<dyn Finish + 'scope> = task.clone();
        // SAFETY: as for the job below: `scope` finishes every task (and lets go of what it borrowed) before it returns.
        let end: Arc<dyn Finish + 'static> = unsafe { std::mem::transmute::<Arc<dyn Finish + 'scope>, Arc<dyn Finish + 'static>>(end) };
        self.tasks.lock().unwrap_or_else(|e| e.into_inner()).push(end);
        let queued = task.clone();
        let job: Box<dyn FnOnce() + Send + 'scope> = Box::new(move || {
            if queued.take() {
                queued.work();
            }
        });
        // SAFETY: the scope doesn't end before every task is done (`scope` finishes each one); a queued job still in
        // the pool after that finds its task taken and does nothing but drop its handle to it, whose work and answer
        // were already let go (emptied by `finish`/`join`), so nothing borrowed is touched past the scope.
        let job: Job = unsafe { std::mem::transmute::<Box<dyn FnOnce() + Send + 'scope>, Job>(job) };
        submit(job);
        ScopedJoinHandle { task }
    }
}

impl<T> ScopedJoinHandle<'_, T> {
    /// Like std's join: the work's answer, or Err(payload) when it panicked.
    pub fn join(self) -> std::thread::Result<T> {
        self.task.finish();
        let answer = self.task.answer.lock().unwrap_or_else(|e| e.into_inner()).take();
        answer.unwrap_or_else(|| Err(Box::new("the work was already joined")))
    }
}

/// Like std::thread::scope, on the pool: returns once all the work spawned in it has ended; work nobody joined that
/// panicked panics here, as std's does.
pub fn scope<'env, T>(f: impl for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> T) -> T {
    let scope = Scope { tasks: Mutex::new(Vec::new()), _scope: std::marker::PhantomData, _env: std::marker::PhantomData };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&scope)));
    // Every task ended, newest last (one spawned while finishing another is in the list too).
    let mut panicked = None;
    loop {
        let next = scope.tasks.lock().unwrap_or_else(|e| e.into_inner()).pop();
        let Some(task) = next else { break };
        if let Some(p) = task.finish_unjoined() {
            panicked.get_or_insert(p);
        }
    }
    match result {
        Err(p) => std::panic::resume_unwind(p),
        Ok(_) if panicked.is_some() => panic!("a scoped piece of work panicked"),
        Ok(v) => v,
    }
}
