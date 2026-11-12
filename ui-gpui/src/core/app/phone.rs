// AppController.Phone.cs: Settings › Notifications › Phone: the same events as the Windows notifications (the same kinds
// switched off stay off), plus a macro's alerts, sent to the phone through ntfy (phone_alert) whether the window is in
// front or not. The topic is kept in the vault: whoever knows it reads the alerts.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use serde_json::{Value, json};

use super::App;
use super::controller::{JsonKind, bool_of, has, opt_str};
use super::notify::{NOTIFY_REPEAT, notify_kind};
use crate::core::base::{Cancel, Fail, Res, Stamp};
use crate::core::{l, log, phone_alert, prefs, rt};

/// The instance fields AppController.Phone.cs declares.
#[derive(Default)]
pub struct PhoneState {
    pub phoned: RefCell<HashMap<String, Stamp>>,
    pub phone_error: RefCell<Option<String>>,
}

impl App {
    /// From `notify`: at most one per account and event type every five minutes.
    pub fn notify_phone(&self, text: &str, kind: &str, r#type: &str) {
        if self.controller.disposed.get() {
            return;
        }
        let Some(vault) = self.controller.vault.borrow().clone() else {
            return;
        };
        let topic = {
            let d = vault.data();
            if !d.phone.enabled || d.phone.topic.is_empty() {
                return;
            }
            d.phone.topic.clone()
        };
        if r#type != "macro" && notify_kind(r#type).is_some_and(|off| prefs::lock().notify_off.contains(off)) {
            return;
        }
        let now = Stamp::utc_now();
        let key = format!("{}:{}", r#type, text);
        {
            let mut phoned = self.phone.phoned.borrow_mut();
            if let Some(last) = phoned.get(&key)
                && *last <= now
                && now.secs_since(*last) < NOTIFY_REPEAT.as_secs_f64()
            {
                return;
            }
            if phoned.len() > 256 {
                phoned.retain(|_, at| now.secs_since(*at) < NOTIFY_REPEAT.as_secs_f64());
            }
            phoned.insert(key, now);
        }
        let urgent = kind == "warn" || kind == "error";
        let (topic, text) = (topic, text.to_string());
        App::with(|app| {
            let app = app.clone();
            rt::spawn(async move { app.send_phone(topic, text, urgent).await });
        });
    }

    pub async fn send_phone(self: &Rc<Self>, topic: String, text: String, urgent: bool) {
        let error = rt::blocking(move || phone_alert::send(&topic, &text, urgent, &Cancel::new())).await;
        if error.is_none() && self.phone.phone_error.borrow().is_some() {
            log::note("Phone alert: recovered.");
        }
        if error != *self.phone.phone_error.borrow() {
            *self.phone.phone_error.borrow_mut() = error;
            self.push_state();
        }
    }

    pub fn phone_dto(&self) -> Value {
        let vault = self.controller.vault.borrow().clone();
        match vault {
            Some(v) => {
                let d = v.data();
                json!({ "enabled": d.phone.enabled, "topic": d.phone.topic, "error": self.phone.phone_error.borrow().clone() })
            }
            None => json!({ "enabled": false, "topic": "", "error": null }),
        }
    }

    /// enabled and/or topic ("" makes a new one). Switching on without a topic makes one.
    pub fn set_phone(&self, a: &Value) -> Res<Value> {
        let topic = opt_str(a, "topic");
        let enabled = if has(a, "enabled", JsonKind::True) || has(a, "enabled", JsonKind::False) { Some(bool_of(a, "enabled")) } else { None };
        let new_topic = match topic {
            Some(topic) => Some(if topic.trim().is_empty() {
                phone_alert::new_topic()
            } else if let Some(normalized) = phone_alert::try_normalize(&topic) {
                normalized
            } else {
                // Nothing changed yet: the C# threw before touching the vault too (RequireVault first, below).
                self.vault(|_| ())?;
                return Err(Fail::user(l::t("A topic is 6 to 64 letters, digits, - or _ (or https://your-server/topic).")));
            }),
            None => None,
        };
        self.vault_mut(|d| {
            let p = &mut d.phone;
            if let Some(t) = &new_topic {
                p.topic = t.clone();
            }
            if let Some(on) = enabled {
                p.enabled = on;
                if p.enabled && p.topic.is_empty() {
                    p.topic = phone_alert::new_topic();
                }
            }
        })?;
        if new_topic.is_some() {
            *self.phone.phone_error.borrow_mut() = None;
        }
        self.persist_soon();
        self.push_state();
        Ok(Value::Null)
    }

    pub async fn send_phone_test(self: &Rc<Self>) -> Res<Value> {
        // As the C#: a topic made here is kept in memory with the vault, saved with its next save.
        let topic = self.vault_mut(|d| {
            if d.phone.topic.is_empty() {
                d.phone.topic = phone_alert::new_topic();
            }
            d.phone.topic.clone()
        })?;
        let text = l::t("Bloxkeep can reach this phone. Alerts will come here.");
        let error = rt::blocking(move || phone_alert::send(&topic, &text, false, &Cancel::new())).await;
        *self.phone.phone_error.borrow_mut() = error.clone();
        self.push_state();
        if let Some(error) = error {
            return Err(Fail::user(error));
        }
        Ok(Value::Null)
    }
}
