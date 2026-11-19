// Work done off the window (a command to the app, an ask of Roblox's) and what comes back, and the toasts that say it:
// one way for every page. (Presets, the palette and Settings each had their own `ask`, and a hundred places spelled out
// the wait for an answer by hand: spawn, await, update the window.)
use std::future::Future;

use futures::channel::oneshot::Receiver;
use gpui_kit::{prelude::*, *};
use serde_json::Value;

use crate::shell::Root;

/// Waits off the window for `job` (`link.call`'s answer, `roblox::run`'s, any future) and hands its outcome to `done`
/// on the window.
pub fn answer<T: 'static>(job: impl Future<Output = T> + 'static, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, T, &mut Context<Root>) + 'static) {
    cx.spawn(async move |this, cx| {
        let got = job.await;
        let _ = this.update(cx, |r, cx| done(r, got, cx));
    })
    .detach();
}

/// `answer` for a job that gives a result or why not (`roblox::run`, `link.call`): a job dropped before it answered
/// is a failure with no reason ("").
pub fn reply<T: 'static>(rx: Receiver<Result<T, String>>, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Result<T, String>, &mut Context<Root>) + 'static) {
    answer(rx, cx, |r, got, cx| done(r, got.unwrap_or_else(|_| Err(String::new())), cx));
}

/// `reply` when only the result matters (a failure is already in error.log: `roblox::run` writes it there).
pub fn found<T: 'static>(rx: Receiver<Result<T, String>>, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Option<T>, &mut Context<Root>) + 'static) {
    answer(rx, cx, |r, got, cx| done(r, got.ok().and_then(Result::ok), cx));
}

impl Root {
    /// A page's Refresh / Try again: unless it is already loading, a failed answer gives way (so the error makes room
    /// for the wait) and `load` asks anew. `state` picks the page's loading flag and answer.
    pub fn again<T, E>(
        &mut self,
        cx: &mut Context<Root>,
        state: impl FnOnce(&mut Root) -> (bool, &mut Option<Result<T, E>>),
        load: impl FnOnce(&mut Root, &mut Context<Root>),
    ) {
        let (loading, got) = state(self);
        if loading {
            return;
        }
        if got.as_ref().is_some_and(|g| g.is_err()) {
            *got = None;
        }
        load(self, cx);
        cx.notify();
    }

    /// A toast: `tone` is "ok", "info", "warn" or "bad".
    pub fn say(&mut self, text: impl Into<String>, tone: &str) {
        self.toast = Some((text.into(), tone.into()));
    }

    /// A toast that something worked.
    pub fn ok(&mut self, text: impl Into<String>) {
        self.say(text, "ok");
    }

    /// A command whose answer matters: what it returns goes to `done`; a refusal is said in a toast.
    pub fn ask(&mut self, cmd: &str, args: Value, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Value) + 'static) {
        let Some(link) = self.link.clone() else { return };
        answer(link.call(cmd, args), cx, |r, got, cx| {
            match got {
                Ok(Ok(v)) => done(r, v),
                Ok(Err(e)) => r.say(e, "bad"),
                Err(_) => {}
            }
            cx.notify();
        });
    }

    /// `ask` for a button that must not send twice (app.js's guards): pressed again before the answer, nothing.
    pub fn ask_once(&mut self, key: &'static str, cmd: &str, args: Value, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Value) + 'static) {
        if self.link.is_none() || !self.in_flight.insert(key) {
            return;
        }
        let Some(link) = self.link.clone() else { return };
        answer(link.call(cmd, args), cx, move |r, got, cx| {
            r.in_flight.remove(key);
            match got {
                Ok(Ok(v)) => done(r, v),
                Ok(Err(e)) => r.say(e, "bad"),
                Err(_) => {}
            }
            cx.notify();
        });
    }
}
