// AppController.DiscordAlerts.cs.
//! Discord's alerts (see docs/discord-style.md): each event the Windows notifications get becomes a short title coloured
//! by what it means, the account with its Roblox avatar above it, the game's icon beside it, and the facts side by side.
//! Made when the event happens from the notification's sentence and values, in Discord's language, names shown or
//! hidden by Discord's switch. Events within a few seconds go out as one message, a field per event.
use std::time::Duration;

use crate::core::app::App;
use crate::core::app::discord::discord_message;
use crate::core::app::notify::notify_kind;
use crate::core::base::{Guid, Stamp};
use crate::core::discord_webhook::{self as webhook, DiscordField, DiscordMessage};
use crate::core::l;
use crate::core::models::Account;

/// A stay that ended this recently is the one an event (a drop, a kick) is about.
pub const RECENT_STAY: Duration = Duration::from_secs(3 * 60);

#[derive(Default)]
pub struct DiscordAlertsState {}

/// An event as Discord shows it; `author` is the account's name (not escaped).
#[derive(Clone, Debug)]
pub struct DiscordAlert {
    pub title: String,
    pub text: Option<String>,
    pub fields: Vec<DiscordField>,
    pub color: i32,
    pub at: Stamp,
    pub author: Option<String>,
    pub author_icon: Option<String>,
    pub thumbnail: Option<String>,
}

impl DiscordAlert {
    pub fn new(title: String, text: Option<String>, fields: Vec<DiscordField>, color: i32, at: Stamp) -> DiscordAlert {
        DiscordAlert { title, text, fields, color, at, author: None, author_icon: None, thumbnail: None }
    }
}

/// `played`: seconds (a TimeSpan in C#).
#[derive(Clone, Debug)]
pub struct AlertSubject {
    pub name: String,
    pub icon: Option<String>,
    pub place_id: Option<i64>,
    pub game: Option<String>,
    pub game_icon: Option<String>,
    pub played: Option<f64>,
    pub reason: Option<String>,
}

/// Discord puts three short fields to a row: a lone one on the last row goes to a small line under the description.
pub fn threes(alert: DiscordAlert) -> DiscordAlert {
    let (fields, text) = threes_fields(&alert.fields, alert.text.as_deref());
    DiscordAlert { fields, text, ..alert }
}

pub fn threes_fields(fields: &[DiscordField], text: Option<&str>) -> (Vec<DiscordField>, Option<String>) {
    let mut list = fields.to_vec();
    let inline = list.iter().filter(|f| f.inline).count();
    let lone = list.iter().rposition(|f| f.inline);
    match lone {
        Some(i) if inline % 3 == 1 => {
            let lone = list.remove(i);
            let before = match text {
                Some(t) if !t.is_empty() => format!("{t}\n"),
                _ => String::new(),
            };
            (list, Some(format!("{before}-# {}: {}", lone.name, lone.value.replace('\n', " · "))))
        }
        _ => (list, text.map(str::to_string)),
    }
}

/// The first character in capitals (char.ToUpperInvariant of the first).
pub fn capital(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// How much an alert asks of the reader: a message of several takes the colour of the one that asks most.
pub fn urgency(color: i32) -> i32 {
    match color {
        webhook::RED => 4,
        webhook::YELLOW => 3,
        webhook::GREEN => 2,
        webhook::BLUE => 1,
        _ => 0,
    }
}

/// `{0}`-style value as a whole number, as C#'s `args[i] is IConvertible n and not string`.
// PORT: the values come from `l::made` as text; a value that reads as a whole number counts as one.
fn count_of(args: &[String], i: usize) -> i64 {
    args.get(i).and_then(|a| a.trim().parse::<i64>().ok()).unwrap_or(0)
}

impl App {
    /// The alert for `type_` about `subject` (an account, or a schedule's preset), from
    /// the notification's sentence and values (`made`; None: `text` as it is).
    pub fn alert_for(&self, type_: &str, subject: Guid, made: Option<&(String, Vec<String>)>, text: &str, now: Stamp) -> DiscordAlert {
        let en: &str = made.map(|m| m.0.as_str()).unwrap_or("");
        let args: &[String] = made.map(|m| m.1.as_slice()).unwrap_or(&[]);
        let arg = |i: usize| if i < args.len() { self.written(&format!("{{{i}}}"), args) } else { String::new() };
        let count = |i: usize| count_of(args, i);
        let why = made.and_then(|m| self.why(m));
        let escaped_why = || why.as_ref().map(|w| webhook::escape(Some(&w[..])));
        let plain = |title: String, description: Option<String>, color: i32, fields: Vec<DiscordField>| {
            let mut alert = DiscordAlert::new(title, description, fields, color, now);
            alert.thumbnail = Some(webhook::LOGO.to_string());
            threes(alert)
        };

        match type_ {
            "drop" | "rejoinSlow" | "rejoinFail" | "afk" => {
                if let Some(who) = self.subject_of(subject, now) {
                    let starts = |s: &str| en.starts_with(s);
                    let (title, description, color) = match type_ {
                        "drop" if starts("{0} lost connection") => (self.dt("Lost connection, restarting Roblox"), None, webhook::YELLOW),
                        "drop" if starts("{0} dropped out of the game, reason unknown") => (self.dt("Disconnected, reason unknown"), None, webhook::YELLOW),
                        "drop" if starts("{0} was kicked by the game") => (self.dt("Kicked by the game, rejoining"), escaped_why(), webhook::YELLOW),
                        "drop" if starts("{0}'s server shut down") => (self.dt("Server shut down, joining another"), None, webhook::YELLOW),
                        "drop" if starts("{0} ran out of memory") => (self.dt("Crashed for want of memory, rejoining"), None, webhook::YELLOW),
                        "rejoinFail" if starts("{0} waits to join") => (self.dt("Waiting to join a player"), escaped_why(), webhook::YELLOW),
                        "rejoinFail" if starts("{0} waits to rejoin") => (self.dt("Waiting for memory to rejoin"), escaped_why(), webhook::YELLOW),
                        "drop" if starts("{0} isn't rejoined") => (self.dt("Disconnected, not rejoining"), escaped_why(), webhook::RED),
                        "drop" if starts("{0} couldn't get into that server") => (self.dt("Couldn't get into the server, joining another"), None, webhook::YELLOW),
                        "drop" if starts("{0} couldn't get into the game") => (self.dt("Couldn't get into the game, trying again"), None, webhook::YELLOW),
                        "drop" if starts("{0} can't get into the game: Roblox wants a check") => {
                            (self.dt("Roblox wants a bot check, not trying again"), escaped_why(), webhook::RED)
                        }
                        "drop" if starts("{0} can't get into the game") => (self.dt("Roblox won't let it in, not trying again"), escaped_why(), webhook::RED),
                        "drop" => (self.dt("Disconnected, rejoining"), None, webhook::YELLOW),
                        "rejoinSlow" => (self.dt("Keeps getting disconnected"), Some(self.dt("Bloxkeep keeps rejoining every 10 seconds.")), webhook::RED),
                        "rejoinFail" => (self.dt("Couldn't rejoin"), escaped_why(), webhook::RED),
                        _ => (self.dt("Kicked for being AFK"), escaped_why(), webhook::YELLOW),
                    };
                    // A kick says it in its title, with the game's words: no "Reason" field saying it again.
                    let who = if en.starts_with("{0} was kicked by the game") { AlertSubject { reason: None, ..who } } else { who };
                    return self.account_alert(who, title, description, color, now, true);
                }
            }
            "macro" => {
                if let Some(who) = self.subject_of(subject, now) {
                    return self.account_alert(AlertSubject { reason: None, ..who }, self.dt("Macro alert"), Some(webhook::escape(Some(&text[..]))), webhook::YELLOW, now, true);
                }
            }
            "gameBan" => {
                if let Some(who) = self.subject_of(subject, now) {
                    // The game's words are the description: no "Reason" field saying "Kicked by the game" with them again.
                    let won_t = self.dt("Bloxkeep won't rejoin this game for this account.");
                    let description = match &why {
                        None => won_t,
                        Some(w) => format!("{}\n-# {won_t}", webhook::escape(Some(&w[..]))),
                    };
                    return self.account_alert(AlertSubject { reason: None, ..who }, self.dt("Banned by the game"), Some(description), webhook::RED, now, true);
                }
            }
            _ => {}
        }
        match type_ {
            "banned" => {
                if let Some(who) = self.subject_of(subject, now) {
                    return self.account_alert(
                        who,
                        self.dt("Banned on Roblox"),
                        Some(self.dt("Bloxkeep won't open this account. Once the ban is over, Check sign-in clears it.")),
                        webhook::RED,
                        now,
                        false,
                    );
                }
                return plain(
                    self.df("{0} accounts banned on Roblox", &[&count(0)]),
                    Some(self.dt("Bloxkeep won't open these accounts. Once a ban is over, Check sign-in clears it.")),
                    webhook::RED,
                    vec![DiscordField::new(self.dt("Accounts"), webhook::escape(Some(&arg(1)[..])), false)],
                );
            }
            "signin" => {
                let signed_out = count(0);
                return plain(
                    self.df(if signed_out == 1 { "{0} account needs signing in again" } else { "{0} accounts need signing in again" }, &[&signed_out]),
                    Some(self.dt(if signed_out == 1 {
                        "Roblox signed this account out. Sign in again in Bloxkeep to keep it playing."
                    } else {
                        "Roblox signed these accounts out. Sign them in again in Bloxkeep to keep them playing."
                    })),
                    webhook::YELLOW,
                    vec![DiscordField::new(self.dt(if signed_out == 1 { "Account" } else { "Accounts" }), webhook::escape(Some(&arg(1)[..])), false)],
                );
            }
            "ram" => {
                let mut fields = vec![DiscordField::new(self.dt("RAM in use"), format!("**{}%**", count(0)), true)];
                let windows = self.controller.running.borrow().len();
                if windows > 0 {
                    fields.push(DiscordField::new(self.dt("Roblox windows"), format!("{windows}"), true));
                }
                return plain(self.dt("RAM is running low"), Some(self.dt("Close some Roblox windows if things get slow.")), webhook::YELLOW, fields);
            }
            "restartHeavy" => return plain(self.dt("A heavy Roblox window restarted"), Some(webhook::escape(Some(&text[..]))), webhook::YELLOW, Vec::new()),
            "launch" if en.starts_with("Roblox closed") => {
                return plain(
                    self.df(if count(0) == 1 { "Roblox closed {0} window" } else { "Roblox closed {0} windows" }, &[&count(0)]),
                    Some(self.dt("Roblox may have blocked running several windows at once. Multi-Roblox may need an update.")),
                    webhook::RED,
                    Vec::new(),
                );
            }
            "launch" if en.ends_with("didn't open.") => {
                return plain(
                    self.df(if count(0) == 1 { "{0} account didn't open" } else { "{0} accounts didn't open" }, &[&count(0)]),
                    Some(self.dt("Open Bloxkeep to see why.")),
                    webhook::RED,
                    Vec::new(),
                );
            }
            "schedule" | "scheduleFail" | "scheduleMissed" | "scheduleStop" => {
                return self.preset_alert(type_, en, subject, &arg, &count, why.as_deref(), now);
            }
            _ => {}
        }
        let kind = if type_ == "banned" { type_.to_string() } else { notify_kind(type_).map(str::to_string).unwrap_or_else(|| type_.to_string()) };
        plain(self.event_title(&kind), Some(webhook::escape(Some(&text[..]))), webhook::YELLOW, Vec::new())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn preset_alert(&self, type_: &str, en: &str, preset_id: Guid, arg: &dyn Fn(usize) -> String, count: &dyn Fn(usize) -> i64, why: Option<&str>, now: Stamp) -> DiscordAlert {
        let preset = self.preset_for(Some(preset_id));
        let place = preset.as_ref().and_then(App::preset_place_id);
        let name = webhook::escape(Some(&arg(0)[..]));
        let reason = why.map(|w| webhook::escape(Some(w)));
        let accounts = |n: i64| self.df(if n == 1 { "{0} account" } else { "{0} accounts" }, &[&n]);
        let mut fields = Vec::new();
        let (title, text, color) = match type_ {
            "schedule" if en.contains("already open") => (self.df("Preset {0} started", &[&name]), Some(self.dt("Its accounts were already open.")), webhook::GREY),
            "schedule" => (self.df("Preset {0} started", &[&name]), None, webhook::GREEN),
            "scheduleFail" if en.contains("didn't open") => (
                self.df(if count(1) == 1 { "Preset {0}: {1} account didn't open" } else { "Preset {0}: {1} accounts didn't open" }, &[&name, &count(1)]),
                Some(webhook::escape(Some(&arg(2)[..]))),
                webhook::RED,
            ),
            "scheduleFail" => (self.df("Preset {0} didn't run", &[&name]), reason.clone(), webhook::RED),
            // The preset's own time, as the person set it (not a moment a timestamp could show).
            "scheduleMissed" => (
                self.df("Preset {0} didn't run", &[&name]),
                Some(format!("{}-# {}", reason.as_ref().map(|r| format!("{r}\n")).unwrap_or_default(), self.df("It was due at {0}.", &[&arg(1)]))),
                webhook::YELLOW,
            ),
            _ => (
                self.df("Preset {0} finished", &[&name]),
                Some(self.df(if count(1) == 1 { "Closing its {0} account." } else { "Closing its {0} accounts." }, &[&count(1)])),
                webhook::BLUE,
            ),
        };
        if type_ == "schedule" && text.is_none() {
            fields.push(DiscordField::new(self.dt("Opened"), accounts(count(1)), true));
        }
        if let Some(game) = place {
            if type_ == "schedule" || type_ == "scheduleStop" {
                fields.push(DiscordField::new(self.dt("Game"), webhook::game_link(&self.game_name(game, preset.as_ref().map(|p| p.game_name.as_str())), Some(game)), true));
            }
        }
        let mut alert = DiscordAlert::new(title, text, fields, color, now);
        alert.thumbnail = Some(place.and_then(|p| self.game_icon(p, 0)).unwrap_or_else(|| webhook::LOGO.to_string()));
        threes(alert)
    }

    pub fn account_alert(&self, who: AlertSubject, title: String, text: Option<String>, color: i32, at: Stamp, facts: bool) -> DiscordAlert {
        let mut fields = Vec::new();
        if facts {
            if let (Some(place), Some(game)) = (who.place_id, &who.game) {
                fields.push(DiscordField::new(self.dt("Game"), webhook::game_link(game, Some(place)), true));
            }
        }
        if facts {
            if let Some(played) = who.played.filter(|p| *p >= 60.0) {
                fields.push(DiscordField::new(self.dt("Played"), self.duration(played as i64), true));
            }
        }
        if facts {
            if let Some(reason) = &who.reason {
                fields.push(DiscordField::new(self.dt("Reason"), reason.clone(), true));
            }
        }
        let mut alert = DiscordAlert::new(title, text, fields, color, at);
        alert.author = Some(who.name.clone());
        alert.author_icon = who.icon.clone();
        alert.thumbnail = Some((if facts { who.game_icon.clone() } else { None }).unwrap_or_else(|| webhook::LOGO.to_string()));
        threes(alert)
    }

    pub fn subject_of(&self, id: Guid, now: Stamp) -> Option<AlertSubject> {
        let account = self.find_account(id)?;
        let (hide_names, stays) = self.vault(|d| (d.discord.hide_names, d.play_sessions.clone())).ok()?;
        // A hidden name has no picture either: the avatar would tell who it is.
        let icon = if hide_names {
            None
        } else {
            self.controller.avatar_urls.borrow().get(&account.user_id).cloned().filter(|url| webhook::is_roblox_picture(Some(url)))
        };
        // The stay it was in: still open, or one that ended just now (a drop ends it right before the event).
        let stay = self.stats.playing.borrow().get(&id).cloned().or_else(|| {
            stays.iter().rev().find(|s| s.account_id == id && s.ended_at.is_some_and(|end| now.secs_since(end) < RECENT_STAY.as_secs_f64())).cloned()
        });
        let run = self.controller.running.borrow().get(&id).cloned();
        let log = run.as_ref().and_then(|(pid, _)| self.controller.logs.borrow().state_of(*pid));
        // The game: where its window is, else the stay's, the rejoin's, or the last one it played within the hour.
        let mut place = log
            .as_ref()
            .and_then(|l| l.place_id)
            .or_else(|| run.as_ref().and_then(|(_, launch)| launch.target.as_ref().map(|t| t.place_id)))
            .or_else(|| stay.as_ref().map(|s| s.place_id))
            .or_else(|| self.controller.rejoin_queue.borrow().iter().find(|r| r.account_id == id).map(|r| r.target.place_id))
            .or_else(|| stays.iter().rev().find(|s| s.account_id == id && now.secs_since(s.ended_at.unwrap_or(now)) < 3600.0).map(|s| s.place_id));
        if place.is_some_and(|p| p <= 0) {
            place = None;
        }
        let code = match (&log, &stay) {
            (Some(l), _) if l.status == "disconnected" => l.reason,
            (_, Some(s)) if s.end_reason.as_deref() == Some("disconnected") => s.reason_code,
            _ => None,
        };
        let same = stay.as_ref().filter(|s| Some(s.place_id) == place);
        let reason = code.map(|c| {
            // The game's own words with its kick, under Roblox's reason.
            let said = log.as_ref().and_then(|l| l.kick_message.clone()).filter(|m| !m.is_empty());
            self.kick_text(c) + &said.map(|s| format!("\n{}", webhook::escape(Some(&s[..])))).unwrap_or_default()
        });
        Some(AlertSubject {
            name: self.discord_name(&account),
            icon,
            place_id: place,
            game: place.map(|p| self.game_name(p, same.map(|s| s.game_name.as_str()))),
            game_icon: place.and_then(|g| self.game_icon(g, same.map_or(0, |s| s.universe_id))),
            played: stay.as_ref().map(|s| s.ended_at.unwrap_or(now).secs_since(s.started_at)),
            reason,
        })
    }

    /// A game's name: as Roblox gave it, else as noted with the stay or preset, else "Place 123".
    pub fn game_name(&self, place_id: i64, noted: Option<&str>) -> String {
        if let Some(name) = self.known_game(place_id).map(|g| g.name).filter(|n| !n.is_empty()) {
            return name;
        }
        match noted.filter(|n| !n.is_empty()) {
            Some(noted) => noted.to_string(),
            None => self.df("Place {0}", &[&place_id]),
        }
    }

    pub fn game_icon(&self, place_id: i64, universe_id: i64) -> Option<String> {
        self.icon_of(place_id, universe_id).filter(|icon| webhook::is_roblox_picture(Some(icon)))
    }

    /// Roblox's disconnect code in plain words (as kick_reason words them), with the code: "Connection lost `277`".
    pub fn kick_text(&self, code: i32) -> String {
        match self.kick_words(code) {
            Some(words) => format!("{} `{code}`", capital(&words)),
            None => capital(&self.df("Roblox code {0}", &[&format!("`{code}`")])),
        }
    }

    /// Roblox's disconnect code in Discord's words; None for a code without any.
    pub fn kick_words(&self, code: i32) -> Option<String> {
        // (Roblox's "an error" codes have none: they go by their number, as the C# said them.)
        crate::roblox_codes::why(code).filter(|c| c.key != "error").map(|c| self.dt(c.words))
    }

    /// The values `l::made` gave as the app wrote them, with account names written Discord's way.
    // PORT: C# wrote each name value (Shown) again under `_nameStyle`; the values come here as text, so one that reads
    // as an account's name as the app writes it (`name_text`), or a ", " list of them, is written again here.
    fn discord_args(&self, args: &[String], hide: bool, language: &str) -> Vec<String> {
        let accounts: Vec<Account> = self.vault(|d| d.accounts.clone()).unwrap_or_default();
        let shown: Vec<(String, &Account)> = accounts.iter().map(|a| (self.name_text(a), a)).collect();
        let restyle = |a: &Account| if hide { self.masked_name(a, language) } else { a.label() };
        args.iter()
            .map(|arg| {
                if arg.is_empty() {
                    return arg.clone();
                }
                let one = |piece: &str| shown.iter().find(|(n, _)| n == piece).map(|(_, a)| restyle(a));
                if let Some(name) = one(arg) {
                    return name;
                }
                match arg.split(", ").map(one).collect::<Option<Vec<String>>>() {
                    Some(names) => names.join(", "),
                    None => arg.clone(),
                }
            })
            .collect()
    }

    /// A sentence's values written out the way Discord's messages want them: names shown or hidden by its switch.
    pub fn written(&self, format: &str, args: &[String]) -> String {
        let hide = self.vault(|d| d.discord.hide_names).unwrap_or(false);
        let language = self.discord_language();
        // Read as the app writes names, before the style below changes how `name_text` writes them.
        let args = self.discord_args(args, hide, &language);
        *self.names.name_style.borrow_mut() = Some((hide, language.clone()));
        let text = l::format(format, &args);
        *self.names.name_style.borrow_mut() = None;
        if hide { self.hide_names_in(&text) } else { text }
    }

    /// The event's sentence after its first ": " (why it happened), in Discord's language: "Bloxkeep was locked."
    pub fn why(&self, made: &(String, Vec<String>)) -> Option<String> {
        let sentence = l::t_in(&made.0, &self.discord_language());
        let at = sentence.find(": ")?;
        let why = self.written(&sentence[at + 2..], &made.1).trim().to_string();
        if why.is_empty() { None } else { Some(capital(&why)) }
    }

    /// One message for the events of a few seconds: one alert as it is, several as a field each.
    pub fn events_message(&self, alerts: &[DiscordAlert]) -> DiscordMessage {
        let footer = self.dt("Bloxkeep · Alerts");
        if alerts.len() == 1 {
            let a = &alerts[0];
            let mut m = discord_message(a.title.clone(), a.text.clone().unwrap_or_default(), a.fields.clone(), Some(footer), a.color, Some(a.at));
            m.author = a.author.clone();
            m.author_icon = a.author_icon.clone();
            m.thumbnail = a.thumbnail.clone();
            return m;
        }
        // All about one account: it goes above the title once, not into every field.
        let mut authors: Vec<&Option<String>> = Vec::new();
        for a in alerts {
            if !authors.contains(&&a.author) {
                authors.push(&a.author);
            }
        }
        let one: Option<String> = if authors.len() == 1 { alerts[0].author.clone() } else { None };
        // A field each: its text (a side note joins it: a field has no small line), then its facts on one line; facts the
        // field before gave about the same account aren't repeated.
        let (game, reason) = (self.dt("Game"), self.dt("Reason"));
        let mut before: Option<(Option<String>, String)> = None;
        let fields: Vec<DiscordField> = alerts
            .iter()
            .map(|a| {
                let name = match (&one, &a.author) {
                    (None, Some(author)) => format!("{} · {}", webhook::escape(Some(&author[..])), a.title),
                    _ => a.title.clone(),
                };
                let mut lines: Vec<String> = Vec::new();
                if let Some(text) = a.text.as_ref().filter(|t| !t.is_empty()) {
                    lines.push(text.strip_prefix("-# ").unwrap_or(text).replace("\n-# ", " · "));
                }
                let facts = a
                    .fields
                    .iter()
                    .map(|f| format!("{}{}", if f.name == game || f.name == reason { String::new() } else { format!("{}: ", f.name) }, f.value.replace('\n', " ")))
                    .collect::<Vec<_>>()
                    .join(" · ");
                let now = (a.author.clone(), facts.clone());
                if !facts.is_empty() && before.as_ref() != Some(&now) {
                    lines.push(facts);
                }
                before = Some(now);
                if lines.is_empty() {
                    lines.push(format!("<t:{}:t>", a.at.unix_secs()));
                }
                DiscordField::new(name, lines.join("\n"), false)
            })
            .collect();
        // MaxBy: the first of those that ask most.
        let mut top = &alerts[0];
        for a in alerts {
            if urgency(a.color) > urgency(top.color) {
                top = a;
            }
        }
        let latest = alerts.iter().map(|a| a.at).max().unwrap_or_else(Stamp::utc_now);
        let mut m = discord_message(self.df("{0} alerts", &[&alerts.len()]), String::new(), fields, Some(footer), top.color, Some(latest));
        m.author_icon = if one.is_none() { None } else { alerts[0].author_icon.clone() };
        m.author = one;
        m.thumbnail = Some(webhook::LOGO.to_string());
        m
    }
}
