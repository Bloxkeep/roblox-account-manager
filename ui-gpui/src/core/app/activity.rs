// AppController.Activity.cs.
//! Statistics › Activity: what Bloxkeep saw happen, newest first (launches, rejoins, joins, drops with Roblox's reason,
//! closed windows, notifications). Noted at a few central points: StartSessionAsync, a stay's start and
//! end (stats.rs), OnSessionEnded and Notify. The last 1000 stay in the vault
//! since they name accounts; for backups they are bookkeeping (see ContentHash) and never make one due.
use std::cell::RefCell;
use std::collections::HashMap;

use serde_json::{Value, json};

use crate::core::app::App;
use crate::core::app::controller::PendingLaunch;
use crate::core::base::{Fail, Guid, Res, Stamp};
use crate::core::models::{Account, ActivityEntry, ActivityName};
use crate::core::roblox_log_watcher::ClientState;
use crate::core::{l, log, prefs};

pub const MAX_ACTIVITY: usize = 1000;
pub const ACTIVITY_PAGE: i64 = 100;
/// The kinds of event the page filters by (see `activity_group`).
pub const ACTIVITY_GROUPS: [&str; 5] = ["play", "drops", "schedule", "signin", "updates"];
/// Not noted: a drop has entries of its own (disconnected, closed, rejoined); low-RAM warnings would bury the rest.
pub const NOT_NOTED: [&str; 3] = ["drop", "afk", "ram"];
/// The placeholder for an account in an activity sentence (see `ActivityEntry::names`).
pub const NAME_START: char = '\u{E000}';
pub const NAME_END: char = '\u{E001}';

#[derive(Default)]
pub struct ActivityState {
    /// Noted while Bloxkeep was locked (a window closed meanwhile): added once a vault is open again.
    pub activity_while_locked: RefCell<Vec<ActivityEntry>>,
    /// The Bloxkeep update last noted as ready to install, so each is noted once.
    pub update_noted: RefCell<Option<String>>,
}

/// \p{L}, \p{N} or '_': what the C# lookarounds `(?<![\p{L}\p{N}_])` / `(?![\p{L}\p{N}_])` keep a name apart from.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Regex.Replace of `(?<!word)(?:name1|name2|…)(?!word)`: `names` in the order tried (longest first), each whole-word
/// match replaced by `with(name)`. Ordinal, one pass.
// PORT: the regex crate has no lookarounds, so the C# pattern is matched by hand here, with Rust's alphanumeric
// standing in for \p{L}\p{N}.
pub(crate) fn replace_whole_names(text: &str, names: &[String], with: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut prev: Option<char> = None;
    'scan: while i < text.len() {
        if !prev.is_some_and(is_word) {
            for name in names {
                if !name.is_empty() && text[i..].starts_with(name.as_str()) && !text[i + name.len()..].chars().next().is_some_and(is_word) {
                    out.push_str(&with(name));
                    i += name.len();
                    prev = name.chars().last();
                    continue 'scan;
                }
            }
        }
        let c = text[i..].chars().next().unwrap_or_default();
        out.push(c);
        prev = Some(c);
        i += c.len_utf8();
    }
    out
}

/// Each `\u{E000}n\u{E001}` (n: 1 to 3 digits) replaced by `with(n)`.
fn replace_placeholders(text: &str, with: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(NAME_START) {
        out.push_str(&rest[..start]);
        let after = &rest[start + NAME_START.len_utf8()..];
        let digits = after.chars().take_while(|c| c.is_ascii_digit()).count();
        if (1..=3).contains(&digits) && after[digits..].starts_with(NAME_END) {
            out.push_str(&with(&after[..digits]));
            rest = &after[digits + NAME_END.len_utf8()..];
        } else {
            out.push(NAME_START);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

pub fn add_activity(list: &mut Vec<ActivityEntry>, entry: ActivityEntry) {
    list.push(entry);
    if list.len() > MAX_ACTIVITY {
        let extra = list.len() - MAX_ACTIVITY;
        list.drain(0..extra);
    }
}

/// Kinds of event as the page's filter has them; a notification type added later counts with the problems.
pub fn activity_group(kind: &str) -> &'static str {
    match kind {
        "opened" | "rejoined" | "joined" | "left" | "closed" | "restarted" | "reopened" => "play",
        "signin" => "signin",
        "robloxUpdated" | "updateReady" => "updates",
        _ if kind.starts_with("schedule") => "schedule",
        _ => "drops",
    }
}

pub fn activity_tone(e: &ActivityEntry) -> String {
    if let Some(tone) = &e.tone {
        return tone.clone();
    }
    match e.kind.as_str() {
        "opened" | "rejoined" | "joined" | "reopened" => "ok",
        "disconnected" | "signedOut" | "closedByRoblox" => "warn",
        _ => "info",
    }
    .to_string()
}

pub fn hidden_name(account: &Account) -> String {
    App::short_name(account)
}

/// Roblox's disconnect codes in plain words (roblox_codes.rs: every code Roblox has), as the page words them.
pub fn kick_reason(code: i32) -> String {
    match crate::roblox_codes::why(code) {
        // Roblox says no more than "an error" for these: the number is what can be looked up (said as the C# app said
        // a code it had no words for; the Rust test rejoin holds it).
        Some(c) if c.key == "error" => l::f("Roblox code {0}", &[&code]),
        Some(c) => l::t(c.words),
        None => l::f("Roblox code {0}", &[&code]),
    }
}

pub fn activity_group_name(group: &str) -> String {
    match group {
        "play" => l::t("Games"),
        "schedule" => l::t("Schedules"),
        "signin" => l::t("Sign-in"),
        "updates" => l::t("Updates"),
        _ => l::t("Disconnects and problems"),
    }
}

/// One CSV line; a cell a spreadsheet would take for a formula ("=…") gets a leading apostrophe, so opening it runs nothing.
pub fn csv_row(csv: &mut String, cells: &[&str]) {
    for (i, cell) in cells.iter().enumerate() {
        let mut cell = cell.to_string();
        if cell.starts_with(['=', '+', '-', '@', '\t', '\r']) {
            cell = format!("'{cell}");
        }
        if cell.contains([',', '"', '\r', '\n']) {
            cell = format!("\"{}\"", cell.replace('"', "\"\""));
        }
        if i > 0 {
            csv.push(',');
        }
        csv.push_str(&cell);
    }
    csv.push_str("\r\n");
}

/// .NET's SaveFileDialog, shown over `owner`: the path picked, or None when cancelled.
fn save_file_dialog(owner: isize, title: &str, filter: &str, file_name: &str, default_ext: &str) -> Option<String> {
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetSaveFileNameW, OFN_EXPLORER, OFN_HIDEREADONLY, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    // "Name (*.csv)|*.csv" → "Name (*.csv)\0*.csv\0\0".
    let mut filter_w: Vec<u16> = filter.split('|').flat_map(|part| part.encode_utf16().chain(std::iter::once(0))).collect();
    filter_w.push(0);
    let title_w = wide(title);
    let ext_w = wide(default_ext);
    let mut file = vec![0u16; 1024];
    for (i, u) in file_name.encode_utf16().take(1023).enumerate() {
        file[i] = u;
    }
    // SAFETY: OPENFILENAMEW is plain data (numbers and pointers) for which all zeroes is the documented "not set".
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner as _;
    ofn.lpstrFilter = filter_w.as_ptr();
    ofn.nFilterIndex = 1;
    ofn.lpstrFile = file.as_mut_ptr();
    ofn.nMaxFile = file.len() as u32;
    ofn.lpstrTitle = title_w.as_ptr();
    ofn.lpstrDefExt = ext_w.as_ptr();
    ofn.Flags = OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_EXPLORER | OFN_HIDEREADONLY;
    // SAFETY: every pointer in `ofn` points at a buffer above that outlives the call; `file` is nMaxFile long.
    let ok = unsafe { GetSaveFileNameW(&mut ofn) } != 0;
    if !ok {
        return None;
    }
    let len = file.iter().position(|u| *u == 0).unwrap_or(file.len());
    Some(String::from_utf16_lossy(&file[..len]))
}

impl App {
    /// The vault open now, as a token to tell it from another one later (C#'s reference comparison of `_vault`).
    pub(crate) fn vault_token(&self) -> Option<usize> {
        let vault = self.controller.vault.borrow().clone()?;
        let mut last = self.controller.last_vault.borrow_mut();
        if !last.0.upgrade().is_some_and(|l| std::sync::Arc::ptr_eq(&l, &vault)) {
            *last = (std::sync::Arc::downgrade(&vault), last.1 + 1);
        }
        Some(last.1)
    }

    /// Adds an entry; the account's and game's names are kept as they are now, for after a deletion.
    /// `account`: Guid::nil() for none. `text`: a sentence of its own (a notification's), kept in both languages.
    #[allow(clippy::too_many_arguments)]
    pub fn note_activity(
        &self,
        kind: &str,
        account: Guid,
        place_id: Option<i64>,
        code: Option<i32>,
        at: Option<Stamp>,
        text: Option<&str>,
        tone: Option<&str>,
    ) {
        let place = place_id.filter(|p| *p > 0);
        let mut entry = ActivityEntry {
            at: at.unwrap_or_else(Stamp::utc_now),
            kind: kind.to_string(),
            account_id: if account.is_nil() { None } else { Some(account) },
            account_label: if account.is_nil() { None } else { self.find_account(account).map(|a| a.label()) },
            place_id: place,
            game_name: place.and_then(|p| self.known_game(p)).map(|g| g.name).filter(|n| !n.is_empty()),
            code,
            tone: tone.map(str::to_string),
            ..ActivityEntry::default()
        };
        if let Some(text) = text {
            let (english, others, names) = self.activity_sentences(text);
            entry.text = Some(english);
            entry.text_vi = others.iter().find(|(l, _)| *l == "vi").map(|(_, t)| t.clone());
            let more: std::collections::BTreeMap<String, String> = others.into_iter().filter(|(l, _)| *l != "vi").map(|(l, t)| (l.to_string(), t)).collect();
            entry.text_in = (!more.is_empty()).then_some(more);
            entry.names = names;
        }
        {
            // Statistics › Console: the sentence as shown now (its names too: the Console is only in memory), else the
            // game and Roblox's code.
            let mut words = text.unwrap_or_default().to_string();
            if words.is_empty() {
                words = entry.game_name.clone().or(place.map(|p| format!("place {p}"))).unwrap_or_default();
                if let Some(c) = code {
                    words = if words.is_empty() { format!("code {c}") } else { format!("{words} · code {c}") };
                }
            }
            if words.is_empty() {
                words = kind.to_string();
            }
            // A log's short tag, the same in both languages (the words after it are in the person's).
            let tag = match activity_group(kind) {
                "play" => "Game",
                "signin" => "Sign-in",
                "updates" => "Update",
                "schedule" => "Schedule",
                _ => "Drop",
            };
            log::console_line(kind, Some(tag), &activity_tone(&entry), entry.account_label.as_deref().unwrap_or(""), &words);
        }
        if kind == "disconnected" {
            if let (Some(place), Some(dropped)) = (place, code) {
                self.note_drop(place, dropped, entry.at);
            }
        }
        if !self.has_open_vault() {
            let mut locked = self.activity.activity_while_locked.borrow_mut();
            if locked.len() < 200 {
                locked.push(entry);
            }
            return;
        }
        self.add_locked_activity();
        let _ = self.vault_mut(|d| add_activity(&mut d.activity, entry));
        self.persist_soon();
    }

    /// A notification's sentence in English and Vietnamese. Account names L.F put in (Name, Names)
    /// become placeholders, so the log keeps no name: one hidden then would stay "Account 1" for good, one shown then would
    /// show while hidden.
    pub fn activity_sentences(&self, text: &str) -> (String, Vec<(&'static str, String)>, Option<Vec<ActivityName>>) {
        // PORT: C# told a name apart by its type (Shown); `l::made` gives every value as text, so a value is taken for
        // names when it reads as one account's name as the app writes it now (`name_text`), or a ", " list of them.
        let made = l::made(text);
        let accounts = self.vault(|d| d.accounts.clone()).unwrap_or_default();
        let shown: Vec<(String, Account)> = accounts.iter().map(|a| (self.name_text(a), a.clone())).collect();
        let as_names = |arg: &str| -> Option<Vec<Account>> {
            if arg.is_empty() {
                return None;
            }
            let one = |piece: &str| shown.iter().find(|(n, _)| n == piece).map(|(_, a)| a.clone());
            if let Some(a) = one(arg) {
                return Some(vec![a]);
            }
            arg.split(", ").map(one).collect::<Option<Vec<Account>>>()
        };
        let Some((english, args)) = made.filter(|(_, args)| args.iter().any(|a| as_names(a).is_some())) else {
            let (english, others) = l::each(text);
            return (english, others, None);
        };
        let mut names: Vec<ActivityName> = Vec::new();
        let mut placeholder = |account: &Account| -> String {
            let i = match names.iter().position(|n| n.id == account.id) {
                Some(i) => i,
                None => {
                    names.push(ActivityName { id: account.id, label: account.label(), ..ActivityName::default() });
                    names.len() - 1
                }
            };
            format!("{NAME_START}{i}{NAME_END}")
        };
        let written: Vec<String> = args
            .iter()
            .map(|arg| match as_names(arg) {
                Some(list) => list.iter().map(&mut placeholder).collect::<Vec<String>>().join(", "),
                None => arg.clone(),
            })
            .collect();
        // PORT: the words that come with the names (" and 2 more") were written in the app's language by L.F already.
        let in_language = |language: &str| l::format(&l::t_in(&english, language), &written);
        let others = crate::lang::ALL.iter().map(|l| l.code).filter(|c| *c != "en").map(|c| (c, in_language(c))).collect();
        (in_language("en"), others, Some(names))
    }

    /// Entries noted while locked join the vault now open, its own accounts only (it may be another vault: a restored backup).
    pub fn add_locked_activity(&self) {
        if self.activity.activity_while_locked.borrow().is_empty() {
            return;
        }
        let waiting: Vec<ActivityEntry> = self.activity.activity_while_locked.borrow_mut().drain(..).collect();
        let mut keep = Vec::new();
        for mut entry in waiting {
            if let Some(id) = entry.account_id {
                let Some(account) = self.find_account(id) else { continue };
                if entry.account_label.is_none() {
                    entry.account_label = Some(account.label());
                }
            }
            keep.push(entry);
        }
        let _ = self.vault_mut(|d| {
            for entry in keep {
                add_activity(&mut d.activity, entry);
            }
        });
        self.persist_soon();
    }

    /// A notification's event (see Notify); a scheduled preset's names a preset, not an account.
    pub fn note_notified(&self, text: &str, kind: &str, type_: &str, account: Guid) {
        if !NOT_NOTED.contains(&type_) {
            let account = if self.find_account(account).is_some() { account } else { Guid::nil() };
            self.note_activity(type_, account, None, None, None, Some(text), Some(kind));
        }
    }

    /// A client ended for good (not handed over to another process): closed by anyone, by Roblox, or for its preset's restart.
    pub fn note_session_end(&self, account_id: Guid, launch: &PendingLaunch, last_log: Option<&ClientState>) {
        let kind = if self.roblox_updates.closed_by_roblox.borrow().contains(&account_id) {
            "closedByRoblox"
        } else if self.presets.restarts.borrow().contains_key(&account_id) {
            "restarted"
        } else {
            "closed"
        };
        let place = last_log.and_then(|l| l.place_id).or_else(|| launch.target.as_ref().map(|t| t.place_id));
        self.note_activity(kind, account_id, place, None, None, None, None);
    }

    pub fn note_update_ready(&self) {
        let (status, version, current) = {
            let updater = self.updates.updater.borrow();
            (updater.status(), updater.new_version(), updater.current_version().to_string())
        };
        let Some(version) = version else { return };
        if status != "ready"
            || self.activity.update_noted.borrow().as_deref() == Some(version.as_str())
            || crate::core::whats_new::is_quiet_update(Some(&version), Some(&current))
        {
            return;
        }
        *self.activity.update_noted.borrow_mut() = Some(version.clone());
        let text = l::f("Bloxkeep {0} is ready to install: it installs when Bloxkeep restarts.", &[&version]);
        self.note_activity("updateReady", Guid::nil(), None, None, None, Some(&text), Some("info"));
    }

    /// The entries the page's filters leave, newest first (ones noted at the same moment: the later first).
    pub fn shown_activity(&self, a: &Value) -> Res<Vec<ActivityEntry>> {
        self.add_locked_activity();
        let account: Option<Guid> = a.get("account").and_then(Value::as_str).and_then(|s| Guid::parse_str(s).ok());
        let group: Option<String> = a.get("group").and_then(Value::as_str).filter(|g| ACTIVITY_GROUPS.contains(g)).map(str::to_string);
        let list = self.vault(|d| d.activity.clone())?;
        let mut shown: Vec<(usize, ActivityEntry)> = list
            .into_iter()
            .enumerate()
            .filter(|(_, e)| account.is_none_or(|id| e.account_id == Some(id)) && group.as_deref().is_none_or(|g| activity_group(&e.kind) == g))
            .collect();
        shown.sort_by(|x, y| y.1.at.cmp(&x.1.at).then(y.0.cmp(&x.0)));
        Ok(shown.into_iter().map(|(_, e)| e).collect())
    }

    /// {account?, group?, limit?}: a page of the list, how many there are, and the accounts it names (for the filter).
    pub fn activity_list(&self, a: &Value) -> Res<Value> {
        let shown = self.shown_activity(a)?;
        let limit = crate::core::app::controller::long(a, "limit").clamp(ACTIVITY_PAGE, MAX_ACTIVITY as i64) as usize;
        let mut page: Vec<ActivityEntry> = shown.iter().take(limit).cloned().collect();
        if self.name_activity_games(&mut page) {
            self.store_activity_names(&page);
            self.persist_soon();
        }
        let names = self.activity_names();
        let hide = prefs::lock().hide_names;
        let (all, accounts) = self.vault(|d| (d.activity.clone(), d.accounts.clone()))?;
        // GroupBy keeps the order each account first appears in.
        let mut groups: Vec<(Guid, Vec<&ActivityEntry>)> = Vec::new();
        for e in all.iter() {
            let Some(id) = e.account_id else { continue };
            match groups.iter_mut().find(|g| g.0 == id) {
                Some(g) => g.1.push(e),
                None => groups.push((id, vec![e])),
            }
        }
        let mut named: Vec<(Guid, String, usize)> = groups
            .iter()
            .map(|(id, entries)| {
                let pick = entries.iter().rev().find(|e| e.account_label.is_some()).or(entries.last()).copied();
                let label = pick.and_then(|e| self.activity_account(e)).unwrap_or_default();
                let place = accounts.iter().position(|x| x.id == *id).unwrap_or(usize::MAX);
                (*id, label, place)
            })
            .collect();
        // StringComparer.CurrentCultureIgnoreCase.
        named.sort_by(|x, y| {
            let by_place = if hide { x.2.cmp(&y.2) } else { std::cmp::Ordering::Equal };
            by_place.then_with(|| crate::core::win_kit::culture_cmp(&x.1, &y.1))
        });
        Ok(json!({
            "total": shown.len(),
            "entries": page.iter().map(|e| json!({
                "at": e.at,
                "kind": e.kind,
                "group": activity_group(&e.kind),
                "tone": activity_tone(e),
                "accountId": e.account_id,
                "account": self.activity_account(e),
                "text": self.activity_sentence(e, &names),
            })).collect::<Vec<_>>(),
            "accounts": named.iter().map(|(id, label, _)| json!({ "id": id, "label": label })).collect::<Vec<_>>(),
        }))
    }

    /// Writes game names filled in on copies back to the vault's own entries (C# changed the one object).
    fn store_activity_names(&self, entries: &[ActivityEntry]) {
        let _ = self.vault_mut(|d| {
            for e in entries.iter().filter(|e| e.game_name.is_some()) {
                for v in d.activity.iter_mut().filter(|v| v.game_name.is_none() && v.at == e.at && v.kind == e.kind && v.account_id == e.account_id && v.place_id == e.place_id) {
                    v.game_name = e.game_name.clone();
                }
            }
        });
    }

    /// Who an entry is about: the account's name now (its name then, once deleted), or "F…" while names are hidden.
    pub fn activity_account(&self, e: &ActivityEntry) -> Option<String> {
        let id = e.account_id?;
        let account = self.find_account(id);
        if !prefs::lock().hide_names {
            return Some(account.map(|a| a.label()).or_else(|| e.account_label.clone()).unwrap_or_else(|| l::t("Deleted account")));
        }
        Some(match account {
            None => l::t("Deleted account"),
            Some(a) => hidden_name(&a),
        })
    }

    /// While names are hidden, swaps them inside sentences too: whole words, three characters or more, longest first.
    pub fn activity_names(&self) -> Box<dyn Fn(&str) -> String> {
        if !prefs::lock().hide_names {
            return Box::new(|text: &str| text.to_string());
        }
        let accounts = self.vault(|d| d.accounts.clone()).unwrap_or_default();
        let mut by_name: HashMap<String, Account> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for account in &accounts {
            for name in [account.label(), account.username.clone()] {
                if name.encode_utf16().count() >= 3 && !by_name.contains_key(&name) {
                    by_name.insert(name.clone(), account.clone());
                    order.push(name);
                }
            }
        }
        if by_name.is_empty() {
            return Box::new(|text: &str| text.to_string());
        }
        // Longest first (a stable sort: equal lengths keep the order they were added in).
        order.sort_by_key(|n| std::cmp::Reverse(n.encode_utf16().count()));
        Box::new(move |text: &str| replace_whole_names(text, &order, |m| hidden_name(&by_name[m])))
    }

    /// Names places that weren't looked up when noted, from the lookup cache, a stay there or the recent games.
    /// Returns true when a name was filled in (worth saving).
    pub fn name_activity_games(&self, entries: &mut [ActivityEntry]) -> bool {
        let mut named = false;
        let mut known: Option<HashMap<i64, String>> = None;
        for e in entries.iter_mut() {
            let Some(place_id) = e.place_id.filter(|_| e.game_name.is_none()) else { continue };
            if known.is_none() {
                let pairs: Vec<(i64, String)> = self
                    .vault(|d| {
                        d.play_sessions
                            .iter()
                            .filter(|s| !s.game_name.is_empty())
                            .map(|s| (s.place_id, s.game_name.clone()))
                            .chain(d.settings.recent_games.iter().chain(d.settings.pinned_games.iter()).filter(|g| !g.name.is_empty()).map(|g| (g.place_id, g.name.clone())))
                            .collect()
                    })
                    .unwrap_or_default();
                let mut map = HashMap::new();
                for (place, name) in pairs {
                    map.insert(place, name); // the last one of each place
                }
                known = Some(map);
            }
            let name = self.known_game(place_id).map(|g| g.name).filter(|n| !n.is_empty()).or_else(|| known.as_ref().and_then(|k| k.get(&place_id).cloned()));
            if let Some(name) = name {
                e.game_name = Some(name);
                named = true;
            }
        }
        named
    }

    /// The sentence as page and export show it; `names` swaps names an older entry holds itself.
    pub fn activity_sentence(&self, e: &ActivityEntry, names: &dyn Fn(&str) -> String) -> String {
        let mut text = names(&self.activity_text(e));
        if let Some(named) = e.names.as_ref().filter(|n| !n.is_empty()) {
            return replace_placeholders(&text, |digits| match digits.parse::<usize>() {
                Ok(i) if i < named.len() => self.activity_name_now(&named[i]),
                _ => String::new(),
            });
        }
        // An entry from before the placeholders, about an account renamed or deleted since: its name then is swapped too.
        if prefs::lock().hide_names {
            if let Some(label) = e.account_label.as_ref().filter(|l| l.encode_utf16().count() >= 3) {
                if let Some(now) = self.activity_account(e).filter(|now| now != label) {
                    text = replace_whole_names(&text, std::slice::from_ref(label), |_| now.clone());
                }
            }
        }
        text
    }

    /// An account a sentence names, as it is called now; its name then once deleted (unless names are hidden).
    pub fn activity_name_now(&self, n: &ActivityName) -> String {
        let hide = prefs::lock().hide_names;
        match self.find_account(n.id) {
            Some(account) => {
                if hide {
                    hidden_name(&account)
                } else {
                    account.label()
                }
            }
            None => {
                if hide || n.label.is_empty() {
                    l::t("Deleted account")
                } else {
                    n.label.clone()
                }
            }
        }
    }

    /// The entry as a sentence in the app's language (the account is shown beside it).
    pub fn activity_text(&self, e: &ActivityEntry) -> String {
        if let Some(text) = &e.text {
            let lang = l::language();
            let own = if lang == "vi" { e.text_vi.as_ref() } else { e.text_in.as_ref().and_then(|m| m.get(&lang)) };
            return match own {
                Some(own) if !own.is_empty() => own.clone(),
                // (Noted before this language came: a plain sentence of the app's is still said in it.)
                _ if lang != "en" => l::t(text),
                _ => text.clone(),
            };
        }
        let game = match (&e.game_name, e.place_id) {
            (Some(name), _) => name.clone(),
            (None, Some(id)) => self.known_game(id).map(|g| g.name).filter(|n| !n.is_empty()).unwrap_or_else(|| l::f("Place {0}", &[&id])),
            (None, None) => String::new(),
        };
        let text = self.activity_kind_text(e, &game);
        // A game named "Adopt Me!" closes the sentence itself: no "Adopt Me!." after it.
        if game.ends_with(['!', '?', '.', '…']) && text.ends_with(&format!("{game}.")) {
            return text[..text.len() - 1].to_string();
        }
        text
    }

    pub fn activity_kind_text(&self, e: &ActivityEntry, game: &str) -> String {
        match e.kind.as_str() {
            "opened" => {
                if !game.is_empty() {
                    l::f("Opened Roblox to play {0}.", &[&game])
                } else {
                    l::t("Opened Roblox on its home screen.")
                }
            }
            "rejoined" => {
                if !game.is_empty() {
                    l::f("Auto-rejoin opened Roblox again to play {0}.", &[&game])
                } else {
                    l::t("Auto-rejoin opened Roblox again.")
                }
            }
            "restarted" => l::t("Closed for its preset's planned restart."),
            "reopened" => {
                if !game.is_empty() {
                    l::f("Opened again after its planned restart, to play {0}.", &[&game])
                } else {
                    l::t("Opened again after its planned restart.")
                }
            }
            "joined" => l::f("Joined {0}.", &[&game]),
            "left" => l::f("Left {0}.", &[&game]),
            "disconnected" => match e.code {
                Some(code) => l::f("Disconnected from {0}: {1}.", &[&game, &kick_reason(code)]),
                None => l::f("Disconnected from {0}.", &[&game]),
            },
            "signedOut" => l::t("Signed out in the Roblox window."),
            "closed" => l::t("Roblox window closed."),
            "closedByRoblox" => l::t("Roblox closed this window when another one opened."),
            _ => e.kind.clone(),
        }
    }

    /// {account?, group?}: every entry the page shows, into a CSV the person picks (newest first, this PC's time
    /// zone). Returns how many, or null when cancelled.
    pub async fn export_activity(&self, a: &Value) -> Res<Value> {
        let token = self.vault_token();
        let mut shown = self.shown_activity(a)?;
        if shown.is_empty() {
            return Err(Fail::user(l::t("There is nothing to export.")));
        }
        let today = Stamp::now().local();
        let file_name = format!("Bloxkeep-activity-{:04}-{:02}-{:02}.csv", today.year, today.month, today.day);
        let title = l::t("Export the activity log");
        let filter = l::t("CSV file") + " (*.csv)|*.csv";
        let owner = self.controller.owner.get();
        // On a worker thread: the core keeps answering while the box is open.
        let picked = crate::core::rt::blocking(move || crate::core::ui::modal(|| save_file_dialog(owner, &title, &filter, &file_name, "csv"))).await;
        let Some(path) = picked else { return Ok(Value::Null) };
        if self.vault_token() != token {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        if self.name_activity_games(&mut shown) {
            self.store_activity_names(&shown);
        }
        let names = self.activity_names();
        let mut csv = String::new();
        csv_row(&mut csv, &[&l::t("Time"), &l::t("Account"), &l::t("Kind"), &l::t("Event"), &l::t("Game")]);
        for e in &shown {
            let game = match e.place_id {
                Some(id) => e.game_name.clone().or_else(|| self.known_game(id).map(|g| g.name)).unwrap_or_else(|| id.to_string()),
                None => String::new(),
            };
            let c = e.at.local();
            let time = format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", c.year, c.month, c.day, c.hour, c.minute, c.second);
            csv_row(
                &mut csv,
                &[&time, &self.activity_account(e).unwrap_or_default(), &activity_group_name(activity_group(&e.kind)), &self.activity_sentence(e, &names), &game],
            );
        }
        // With a byte order mark, so Excel reads the names and the Vietnamese as UTF-8.
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(csv.as_bytes());
        if let Err(ex) = std::fs::write(&path, bytes) {
            let kind = if ex.kind() == std::io::ErrorKind::PermissionDenied { "UnauthorizedAccessException" } else { "IOException" };
            log::note(&format!("Activity: export to CSV failed ({kind})."));
            let fail = Fail::from(ex);
            return Err(Fail::user(l::f("Couldn't save the file: {0}", &[&crate::core::app::controller::error_text(&fail)])));
        }
        Ok(json!(shown.len()))
    }
}
