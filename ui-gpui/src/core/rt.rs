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
use std::{
    cell::RefCell,
    future::Future,
    sync::{Mutex, OnceLock},
    time::Duration,
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
            f();
        }
    });
    pool.run_until(main());
}

/// A task of the core's own, not waited for. Only from the core's thread.
pub fn spawn(f: impl Future<Output = ()> + 'static) {
    SPAWNER.with(|s| {
        if let Some(sp) = s.borrow().as_ref() {
            let _ = sp.spawn_local(f);
        }
    });
}

/// From any thread: runs `f` on the core's thread, in its turn (a watcher's event, a window's click).
pub fn post(f: impl FnOnce() + Send + 'static) {
    if let Some(tx) = POSTS.get().and_then(|m| m.lock().ok()) {
        let _ = tx.unbounded_send(Box::new(f));
    }
}

/// Slow work on a thread of its own; the task goes on with its answer. Should that thread die without one, the task
/// simply never goes on (the core itself stays up).
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = oneshot::channel();
    let _ = std::thread::Builder::new().name("core-work".into()).spawn(move || {
        let _ = tx.send(f());
    });
    match rx.await {
        Ok(v) => v,
        Err(_) => std::future::pending().await,
    }
}

/// Task.Delay(d).
pub async fn sleep(d: Duration) {
    blocking(move || std::thread::sleep(d)).await
}

/// Task.Delay(d, token): false when it was called off before its time.
pub async fn sleep_or(cancel: &Cancel, d: Duration) -> bool {
    let cancel = cancel.clone();
    blocking(move || cancel.sleep(d)).await
}
