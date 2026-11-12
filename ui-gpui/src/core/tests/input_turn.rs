// Tests/Bloxkeep.Tests/InputTurnSuite.cs: the one turn at the PC (InputTurn): anti-AFK's nudge, a macro's turn, a
// picture, a hotkey. Many askers at once, some giving up: one inside at a time, in the order asked, none that gave up ever
// let in, and nothing left held at the end. A turn's lease belongs to whoever got it: one from before a reset can't hand
// on someone else's.
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::Checks;
use crate::core::base::Cancel;
use crate::core::input_turn;

pub async fn run(c: &mut Checks, _dir: &str) {
    contention(c);
    leases(c);
    input_turn::reset();
}

/// A small seeded random (C#'s Random(5) stands for any fixed sequence).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, max: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % max.max(1)
    }
}

fn contention(c: &mut Checks) {
    input_turn::reset();
    const WORKERS: usize = 64;
    let mut rnd = Lcg(5);
    let first = input_turn::try_enter("first");
    let inside = Arc::new(AtomicI32::new(0));
    let most = Arc::new(AtomicI32::new(0));
    let order = Arc::new(Mutex::new(Vec::<usize>::new()));
    let mut workers = Vec::new();
    for i in 0..WORKERS {
        let cancel = Cancel::new();
        if rnd.next(4) == 0 {
            let (c2, after) = (cancel.clone(), rnd.next(40));
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(after));
                c2.cancel();
            });
        }
        let pause = rnd.next(3);
        let name = format!("w{i}");
        let started = Arc::new(AtomicBool::new(false));
        let (inside, most, order, started2) = (inside.clone(), most.clone(), order.clone(), started.clone());
        let handle = std::thread::spawn(move || {
            started2.store(true, Ordering::SeqCst);
            let Ok(lease) = input_turn::enter(&name, Some(&cancel)) else {
                return false;
            };
            let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
            {
                let mut o = order.lock().unwrap_or_else(|e| e.into_inner());
                o.push(i);
                most.fetch_max(now, Ordering::SeqCst);
            }
            std::thread::sleep(Duration::from_millis(pause));
            inside.fetch_sub(1, Ordering::SeqCst);
            lease.dispose();
            true
        });
        // Its place in the queue taken before the next asks (EnterAsync queues at once).
        let begun = Instant::now();
        while !(started.load(Ordering::SeqCst) && (input_turn::waits(&format!("w{i}")) || handle.is_finished())) && begun.elapsed() < Duration::from_millis(500) {
            std::thread::yield_now();
        }
        workers.push(handle);
    }
    std::thread::sleep(Duration::from_millis(20));
    drop(first);
    let begun = Instant::now();
    while !workers.iter().all(|w| w.is_finished()) && begun.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let finished = workers.iter().all(|w| w.is_finished());
    let results: Vec<bool> = if finished { workers.into_iter().map(|w| w.join().unwrap_or(false)).collect() } else { Vec::new() };
    let leased: Vec<usize> = results.iter().enumerate().filter(|(_, r)| **r).map(|(i, _)| i).collect();
    let order = order.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let most = most.load(Ordering::SeqCst);
    c.check("64 askers, some giving up: every one answered", finished, "");
    c.check("never two inside at once", most == 1, most);
    let mut sorted = order.clone();
    sorted.sort();
    c.check("the ones let in went in the order they asked", order == sorted, &order);
    c.check(
        "each one either went in or gave up, and those that gave up were passed over",
        order.len() == leased.len() && results.iter().filter(|r| finished && !**r).count() == WORKERS - leased.len() && !leased.is_empty(),
        (order.len(), leased.len()),
    );
    c.check("nothing held at the end", !input_turn::busy(), input_turn::holder());
}

fn leases(c: &mut Checks) {
    input_turn::reset();
    let stale = input_turn::try_enter("macro");
    input_turn::reset(); // the checks' reset, or a turn given up on
    let fresh = input_turn::try_enter("macro");
    drop(stale);
    c.check(
        "a lease from before can't hand on the turn someone has now",
        fresh.is_some() && input_turn::busy() && input_turn::holder().as_deref() == Some("macro"),
        input_turn::holder(),
    );
    drop(fresh);
    c.check("its own lease hands it on", !input_turn::busy(), input_turn::holder());
    let held = input_turn::try_enter("macro");
    c.check("nobody asking: anti-AFK isn't seen waiting", !input_turn::waits("afk"), "");
    let cancel = Cancel::new();
    let ct = cancel.clone();
    let afk = std::thread::spawn(move || input_turn::enter("afk", Some(&ct)).map(|l| l.dispose()));
    let begun = Instant::now();
    while !input_turn::waits("afk") && begun.elapsed() < Duration::from_secs(2) {
        std::thread::yield_now();
    }
    let waits = input_turn::waits("afk");
    cancel.cancel();
    // PORT: C#'s cancellation answered the task at once; here the waiting thread sees it within its 25 ms poll.
    let begun = Instant::now();
    while input_turn::waits("afk") && begun.elapsed() < Duration::from_secs(1) {
        std::thread::sleep(Duration::from_millis(2));
    }
    let gave_up = !input_turn::waits("afk");
    drop(held);
    let answer = afk.join().ok();
    let cancelled = matches!(answer, Some(Err(_)));
    c.check(
        "anti-AFK asking is seen by the one holding the turn while it waits, not once it gave up",
        waits && gave_up && cancelled && !input_turn::busy(),
        (waits, gave_up, cancelled),
    );
}
