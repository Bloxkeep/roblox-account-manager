//! AppController.Names.cs: Settings › Appearance › Hide account names, for screenshots and streams: wherever Bloxkeep writes an
//! account's name (page, toasts, notifications, progress, mini window, monitor tab, Roblox window titles) it writes its first
//! letter and "…", as Discord's streamer mode shortens names, the pictures kept. The page gets the real names and shortens them
//! itself, so search finds them; copying copies the real one. Discord has its own switch (Settings › Discord), since a channel
//! may be shared while the screen isn't; there an account stays "Account N" (its place in the list), to tell them apart in a
//! message.
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

use super::App;
use crate::core::models::Account;
use crate::core::{l, prefs};

#[derive(Default)]
pub struct NamesState {
    /// Set while a Discord copy of a message is made: its switch and language instead of the app's.
    pub name_style: RefCell<Option<(bool, String)>>,
}

/// Text written out only when its message is (see `App::name`): names as they read now, or another way via
/// `write` (the activity log keeps a placeholder per account, so its sentences never hold a name).
#[derive(Clone)]
pub struct Shown {
    write: Rc<dyn Fn(&dyn Fn(&Account) -> String) -> String>,
}

impl Shown {
    /// `new Shown(write, NameText)`: `write` gets how each account's name is written out.
    pub fn new(write: impl Fn(&dyn Fn(&Account) -> String) -> String + 'static) -> Shown {
        Shown { write: Rc::new(write) }
    }

    pub fn write(&self, other: &dyn Fn(&Account) -> String) -> String {
        (self.write)(other)
    }
}

impl fmt::Display for Shown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // C#'s `name` is the App's NameText; the App is reached on its thread (a Shown is only written there).
        let mut text = None;
        App::with(|app| text = Some(self.write(&|a| app.name_text(a))));
        f.write_str(&text.unwrap_or_else(|| self.write(&|a| a.label())))
    }
}

/// A letter, digit or underscore: what `[\p{L}\p{N}_]` matches.
fn word_char(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

impl App {
    pub fn set_hide_names(&self, enabled: bool) -> Value {
        prefs::lock().hide_names = enabled;
        prefs::save();
        self.push_state(); // the mini window, the monitor tab and the window titles follow at their next refresh
        Value::Null
    }

    /// An account's name for a message: its label, or "Account 3" while hidden. Pass it to l::f as is: it's written out when
    /// the text is made, so the Discord copy (`discord_text`) follows Discord's switch.
    pub fn name(&self, account: &Account) -> Shown {
        let account = account.clone();
        Shown { write: Rc::new(move |name| name(&account)) }
    }

    /// Several accounts' names, comma-separated, written out like `name`.
    pub fn names(&self, accounts: &[Account]) -> Shown {
        let list = accounts.to_vec();
        Shown { write: Rc::new(move |name| list.iter().map(name).collect::<Vec<_>>().join(", ")) }
    }

    /// The name as it reads right now: in the app (window titles, the mini window) or in the message being made.
    pub fn name_text(&self, account: &Account) -> String {
        let style = self.names.name_style.borrow().clone();
        let (hide, language) = match &style {
            Some((hide, language)) => (*hide, language.clone()),
            None => (prefs::lock().hide_names, l::language()),
        };
        if !hide {
            account.label()
        } else if style.is_none() {
            App::short_name(account)
        } else {
            self.masked_name(account, &language)
        }
    }

    /// "F…": the name's first letter (one character as it reads, an emoji included) and an ellipsis.
    pub fn short_name(account: &Account) -> String {
        let label = account.label();
        let name = if !label.trim().is_empty() { label.trim().to_string() } else { account.username.trim().to_string() };
        match name.graphemes(true).next() {
            None => "…".into(),
            Some(first) => format!("{first}…"),
        }
    }

    /// "Account 3": the account's place in the list, as the page numbers them. Never through l::f (see `l::made`).
    pub fn masked_name(&self, account: &Account, language: &str) -> String {
        // C# found the account object itself in the list; the same account is the one with its id.
        let index = self.vault(|d| d.accounts.iter().position(|a| a.id == account.id)).ok().flatten();
        match index {
            Some(i) => l::format(&l::t_in("Account {0}", language), &[(i + 1).to_string()]),
            None => l::t_in("Deleted account", language),
        }
    }

    /// An account's name in a Discord status message or summary, shown or hidden by Discord's own switch.
    pub fn discord_name(&self, account: &Account) -> String {
        if self.vault(|d| d.discord.hide_names).unwrap_or(false) { self.masked_name(account, &self.discord_language()) } else { account.label() }
    }

    /// An event's text for Discord: remade from the same sentence and values in Discord's language and name switch.
    /// Call it before any other l::f (it needs the one that made the text).
    pub fn discord_text(&self, text: &str) -> String {
        let hide = self.vault(|d| d.discord.hide_names).unwrap_or(false);
        let mut text = text.to_string();
        if let Some(made) = l::made(&text) {
            let language = self.discord_language();
            *self.names.name_style.borrow_mut() = Some((hide, language.clone()));
            // PORT: C# formatted the values again (a Shown name wrote itself under `_nameStyle`); l::made gives them as the
            // text they were, so a name in them is not remade here: hide_names_in below still hides it when Discord's switch is on.
            text = made.in_language(&language);
            *self.names.name_style.borrow_mut() = None;
        }
        if hide { self.hide_names_in(&text) } else { text }
    }

    /// A last guard for Discord's switch: names in already-written text (a launch error kept as text) become "Account N".
    /// Whole words only, names of 3+ characters, one pass.
    pub fn hide_names_in(&self, text: &str) -> String {
        let Some(vault) = self.vault_ref() else { return text.to_string() };
        let mut by_name: HashMap<String, Account> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        {
            let d = vault.data();
            for account in &d.accounts {
                for name in [account.label(), account.username.clone()] {
                    if super::accounts::len16(&name) >= 3 && !by_name.contains_key(&name) {
                        by_name.insert(name.clone(), account.clone());
                        order.push(name);
                    }
                }
            }
        }
        if by_name.is_empty() {
            return text.to_string();
        }
        // Longest first, as the C# alternation was ordered (a stable sort keeps the list's order between equal lengths).
        order.sort_by_key(|n| std::cmp::Reverse(super::accounts::len16(n)));
        let language = self.discord_language();
        // PORT: the `regex` crate has no look-behind or look-ahead; the same whole-word match is done by hand: at each
        // position, the first name (longest first) found there with no letter, digit or _ on either side.
        let mut out = String::with_capacity(text.len());
        let mut pos = 0;
        let mut prev: Option<char> = None;
        while pos < text.len() {
            let rest = &text[pos..];
            let before_ok = !prev.is_some_and(word_char);
            let hit = if before_ok {
                order.iter().find(|n| rest.starts_with(n.as_str()) && !rest[n.len()..].chars().next().is_some_and(word_char))
            } else {
                None
            };
            match hit {
                Some(name) => {
                    out.push_str(&self.masked_name(&by_name[name], &language));
                    prev = name.chars().last();
                    pos += name.len();
                }
                None => {
                    let c = rest.chars().next().unwrap_or_default();
                    out.push(c);
                    prev = Some(c);
                    pos += c.len_utf8();
                }
            }
        }
        out
    }
}
