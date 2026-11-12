// AppController.Notify.cs: events that matter while the person isn't looking (a drop, auto-rejoin slowed, a scheduled
// preset, a sign-in to renew, low RAM, a failed launch) also go out as Windows notifications when the window isn't in
// front. Settings has one switch for all and one per kind; Discord (discord.rs) has switches of its own.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use serde_json::Value;

use super::App;
use super::controller::{bool_of, opt_str};
use crate::core::base::{Fail, Guid, Res, Stamp};
use crate::core::{l, prefs};

pub const NOTIFY_REPEAT: Duration = Duration::from_secs(5 * 60);
/// The kinds Settings offers, each covering the event types `notify_kind` maps to it.
pub const NOTIFY_KINDS: &[&str] = &["drop", "schedule", "signin", "ram", "launch"];

/// Launch warnings with "Don't ask again", each on its own: Multi-Roblox off (someone switching accounts one at a
/// time on purpose), the Roblox watch's warning and low RAM. Moving accounts out of a game and Multi-Roblox not working
/// always ask. A skipped one doesn't hold back a scheduled preset either.
pub const SKIPPABLE_LAUNCH_WARNINGS: &[&str] = &["multiOff", "watch", "memory"];

pub fn launch_warning_key(kind: &str) -> String {
    format!("launch.{kind}")
}

/// Questions with "Don't ask again": often asked, and their yes can be undone. Deleting accounts, restoring a
/// backup and removing the Discord link always ask. (Before 1.1.0 one "openAnyway" answered every launch warning; it is
/// no longer read, so those are asked again.)
pub fn skippable_confirms() -> Vec<String> {
    let mut list: Vec<String> = ["deletePreset", "closeAll", "killAll", "multi", "discard", "lock"].iter().map(|s| s.to_string()).collect();
    list.extend(SKIPPABLE_LAUNCH_WARNINGS.iter().map(|k| launch_warning_key(k)));
    list
}

/// The Settings kind an event type belongs to; None for one no switch covers (always shown).
pub fn notify_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "drop" | "rejoinFail" | "rejoinSlow" | "afk" | "gameBan" => Some("drop"),
        "signin" | "banned" => Some("signin"),
        "ram" | "restartHeavy" => Some("ram"),
        "launch" => Some("launch"),
        _ if kind.starts_with("schedule") => Some("schedule"),
        _ => None,
    }
}

/// The instance fields AppController.Notify.cs declares.
#[derive(Default)]
pub struct NotifyState {
    /// When each event type was last shown per account, so a rejoin loop can't spam.
    pub notified: RefCell<HashMap<String, Stamp>>,
    /// Shows a Windows notification (text, kind) unless the window is in front; returns whether it did.
    pub show_notification: RefCell<Option<Rc<dyn Fn(&str, &str) -> bool>>>,
}

impl App {
    /// The in-app toast, plus a Windows notification while the window isn't in front: at most one per account and
    /// event type every five minutes, kinds left on in Settings only (the toast shows either way).
    /// `toast`: false when the page shows its own message for this (a launch result).
    /// `repeat_key`: what counts as the same notification (by default its type and account).
    pub fn notify(&self, text: &str, kind: &str, r#type: &str, account: Guid, toast: bool, repeat_key: Option<&str>) {
        // Statistics › Activity keeps the text in both languages: like Discord's copy, before any other L.F.
        self.note_notified(text, kind, r#type, account);
        // First, while the L.F that made the text is still the last (Discord may want another language or its own name switch).
        self.notify_discord(text, r#type, account);
        self.notify_phone(text, kind, r#type);
        if toast {
            self.toast(text, kind);
        }
        // None behind the lock screen: it would show the text (an account's name) on the desktop, which the page withholds too.
        let show = self.notify.show_notification.borrow().clone();
        let (notifications, off) = {
            let p = prefs::lock();
            (p.notifications, notify_kind(r#type).is_some_and(|k| p.notify_off.contains(k)))
        };
        let Some(show) = show else {
            return;
        };
        if self.controller.disposed.get() || self.auto_lock.screen_locked.get() || !notifications {
            return;
        }
        if off {
            return;
        }
        let now = Stamp::utc_now();
        let key = match repeat_key {
            Some(k) => k.to_string(),
            None => format!("{}:{}", r#type, account),
        };
        // A time ahead of now (the clock was set back) doesn't hold the notification back.
        if let Some(last) = self.notify.notified.borrow().get(&key)
            && *last <= now
            && now.secs_since(*last) < NOTIFY_REPEAT.as_secs_f64()
        {
            return;
        }
        {
            let mut notified = self.notify.notified.borrow_mut();
            if notified.len() > 256 {
                notified.retain(|_, at| now.secs_since(*at) < NOTIFY_REPEAT.as_secs_f64());
            }
        }
        if show(text, kind) {
            self.notify.notified.borrow_mut().insert(key, now);
        }
    }

    pub fn skip_confirm(&self, key: &str) -> Res<Value> {
        if !skippable_confirms().iter().any(|k| k == key) {
            return Err(Fail::user(l::t("Unknown question.")));
        }
        let added = prefs::lock().skip_confirm.insert(key.to_string());
        if added {
            prefs::save();
            self.push_state();
        }
        Ok(Value::Null)
    }

    /// Settings › Notifications › Ask again: every "Don't ask again" is forgotten. Returns how many there were.
    pub fn reset_skip_confirm(&self) -> i32 {
        let n = App::skipped_confirms().len() as i32;
        prefs::lock().skip_confirm.clear();
        prefs::save();
        self.push_state();
        n
    }

    /// The questions answered "Don't ask again" that are still in use (an old key from an earlier version isn't).
    pub fn skipped_confirms() -> Vec<String> {
        let known = skippable_confirms();
        let mut list: Vec<String> = prefs::lock().skip_confirm.iter().filter(|k| known.contains(k)).cloned().collect();
        list.sort(); // StringComparer.Ordinal
        list
    }

    /// {enabled} for the switch over all of them; {kind, enabled} for one kind.
    pub fn set_notifications(&self, a: &Value) -> Res<Value> {
        let enabled = bool_of(a, "enabled");
        if let Some(kind) = opt_str(a, "kind") {
            if !NOTIFY_KINDS.contains(&kind.as_str()) {
                return Err(Fail::user(l::t("Unknown notification kind.")));
            }
            let mut p = prefs::lock();
            if enabled {
                p.notify_off.remove(&kind);
            } else {
                p.notify_off.insert(kind);
            }
        } else {
            prefs::lock().notifications = enabled;
        }
        prefs::save();
        self.push_state();
        Ok(Value::Null)
    }
}
