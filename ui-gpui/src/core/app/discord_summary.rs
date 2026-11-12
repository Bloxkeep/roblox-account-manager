// AppController.DiscordSummary.cs.
//! Discord's play-time summary: an overview (totals against the period before, drops with Roblox's codes, records, top
//! games), a picture drawn here (summary_card), and an embed per preset that played with its accounts.
//! Play is grouped by the preset whose window it was (`PlaySession::preset_id`).
use std::collections::{HashMap, HashSet};

use crate::core::app::App;
use crate::core::app::discord::discord_message;
use crate::core::app::discord::DISCORD_LATE;
use crate::core::app::discord_alerts::threes_fields;
use crate::core::app::stats::{local_day, local_day_hour};
use crate::core::base::{Guid, Res, Stamp};
use crate::core::discord_webhook::{self as webhook, DiscordField, DiscordMessage};
use crate::core::models::{DiscordSettings, LaunchPreset, PlaySession};
use crate::core::summary_card::{GAME_COLORS, SummaryCardAccount, SummaryCardBucket, SummaryCardData};

pub const MAX_PRESET_EMBEDS: usize = 6;
pub const MAX_CARD_ACCOUNTS: usize = 12;
pub const MAX_BLOCK_ACCOUNTS: usize = 15;
pub const SUMMARY_PICTURE: &str = "bloxkeep-summary.png";

#[derive(Default)]
pub struct DiscordSummaryState {}

/// The part of a stay inside the period, and the game it counts for ("u<universe>", else "p<place>").
/// `index`: the stay's place in the vault's list (C# told stays apart by reference).
#[derive(Clone, Debug)]
pub struct StayPart {
    pub session: PlaySession,
    pub index: usize,
    pub from: Stamp,
    pub to: Stamp,
    pub game: String,
}

impl StayPart {
    pub fn seconds(&self) -> f64 {
        self.to.secs_since(self.from)
    }
}

/// How many different stays the parts come from.
fn distinct_stays<'a>(parts: impl IntoIterator<Item = &'a StayPart>) -> usize {
    parts.into_iter().map(|p| p.index).collect::<HashSet<usize>>().len()
}

/// Groups in the order each key first appears (LINQ's GroupBy).
fn group_by<'a, K: PartialEq + Clone>(parts: impl IntoIterator<Item = &'a StayPart>, key: impl Fn(&StayPart) -> K) -> Vec<(K, Vec<&'a StayPart>)> {
    let mut groups: Vec<(K, Vec<&'a StayPart>)> = Vec::new();
    for p in parts {
        let k = key(p);
        match groups.iter_mut().find(|g| g.0 == k) {
            Some(g) => g.1.push(p),
            None => groups.push((k, vec![p])),
        }
    }
    groups
}

fn sum_seconds(parts: &[&StayPart]) -> f64 {
    parts.iter().map(|p| p.seconds()).sum()
}

/// A stable sort, most first (OrderByDescending).
fn sort_desc<T>(list: &mut [T], by: impl Fn(&T) -> f64) {
    list.sort_by(|a, b| by(b).partial_cmp(&by(a)).unwrap_or(std::cmp::Ordering::Equal));
}

pub fn unix(utc: Stamp) -> i64 {
    utc.unix_secs()
}

pub fn initials(name: &str) -> String {
    let letters: String = name.chars().filter(|c| c.is_alphanumeric()).take(2).collect();
    if letters.is_empty() { "?".into() } else { letters.to_uppercase() }
}

/// The page's colour for an account without a picture (the same hash as app.js's hue()).
pub fn hue(text: &str) -> i32 {
    let mut h: u32 = 7;
    for c in text.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(c as u32);
    }
    (h % 360) as i32
}

impl App {
    /// `due_at`: when a daily or weekly summary was due (local): the period ends there, also when it goes out late.
    /// `today`: one day is today so far (after a scheduled preset), not the 24 hours before.
    /// `preset`: after that scheduled preset's stop: only its play, under its name.
    /// `kind`: "daily", "weekly", "both", "preset" or "now": which summary the footer says it is.
    #[allow(clippy::too_many_arguments)]
    pub fn summary_message(
        &self,
        d: &DiscordSettings,
        days: i32,
        due_at: Option<Stamp>,
        local: Option<Stamp>,
        today: bool,
        preset: Option<&LaunchPreset>,
        kind: &str,
    ) -> Res<DiscordMessage> {
        let (sessions, account_count) = self.vault(|v| (v.play_sessions.clone(), v.accounts.len()))?;
        let days = if days == 7 || days == 30 { days } else { 1 };
        let hourly = days == 1;
        let late = matches!((due_at, local), (Some(due), Some(at)) if at.secs_since(due) > DISCORD_LATE.as_secs_f64());
        let now = Stamp::utc_now();
        // A day is the 24 hours up to the summary's time; after a scheduled preset, today so far; 7 or 30: whole days to now.
        let to = match due_at {
            Some(end) if end.to_utc() < now => end.to_utc(),
            _ => now,
        };
        let from = if today {
            local_day(Stamp::now(), 0).to_utc()
        } else if hourly {
            to.plus_secs(-86400.0)
        } else {
            local_day(to, 1 - days as i64).to_utc()
        };

        let only: Option<HashSet<Guid>> = if preset.is_none() && !d.summary_accounts.is_empty() { Some(d.summary_accounts.iter().copied().collect()) } else { None };
        // A preset's own: what its windows played (not its accounts' play opened otherwise).
        let counts = |s: &PlaySession| match preset {
            Some(p) => s.preset_id == Some(p.id),
            None => only.as_ref().is_none_or(|o| o.contains(&s.account_id)),
        };
        let parts = self.stays(&sessions, from, to, &counts);
        let total: f64 = parts.iter().map(|p| p.seconds()).sum();
        let before: f64 = self.stays(&sessions, from.plus_secs(-to.secs_since(from)), from, &counts).iter().map(|p| p.seconds()).sum();
        let drops: Vec<&PlaySession> = sessions
            .iter()
            .filter(|s| counts(s) && s.end_reason.as_deref() == Some("disconnected") && s.ended_at.is_some_and(|e| e >= from && e <= to))
            .collect();
        let parts2 = &d.summary_parts;
        let has = |part: &str| parts2.iter().any(|p| p == part);

        // (key, seconds, the stay with the longest game name)
        let mut games: Vec<(String, f64, PlaySession)> = group_by(&parts, |p| p.game.clone())
            .into_iter()
            .map(|(key, g)| {
                let mut best = g[0];
                for p in &g {
                    if p.session.game_name.encode_utf16().count() > best.session.game_name.encode_utf16().count() {
                        best = p;
                    }
                }
                (key, sum_seconds(&g), best.session.clone())
            })
            .collect();
        sort_desc(&mut games, |g| g.1);
        let show_games = has("games");
        let colour: HashMap<String, i32> =
            games.iter().enumerate().map(|(i, g)| (g.0.clone(), if !show_games { 0 } else if i < GAME_COLORS.len() { i as i32 } else { -1 })).collect();
        let stay_game = |s: &PlaySession| if !s.game_name.is_empty() { s.game_name.clone() } else { self.game_name(s.place_id, None) };
        let stay_icon = |s: &PlaySession| self.game_icon(s.place_id, s.universe_id);

        struct AccountRow {
            id: Guid,
            seconds: f64,
            stays: usize,
            drops: usize,
            games: Vec<(i32, f64)>,
            label: Option<String>,
        }
        let mut accounts: Vec<AccountRow> = group_by(&parts, |p| p.session.account_id)
            .into_iter()
            .map(|(id, g)| AccountRow {
                id,
                seconds: sum_seconds(&g),
                stays: distinct_stays(g.iter().copied()),
                drops: drops.iter().filter(|s| s.account_id == id).count(),
                games: group_by(g.iter().copied(), |p| p.game.clone()).into_iter().map(|(game, x)| (colour[&game], sum_seconds(&x))).collect(),
                label: g.iter().rev().filter_map(|p| p.session.account_label.clone()).find(|l| !l.is_empty()),
            })
            .collect();
        sort_desc(&mut accounts, |a| a.seconds);
        let name_of = |id: Guid, label: Option<&str>| -> String {
            match self.find_account(id) {
                Some(account) => self.discord_name(&account),
                None => match label.filter(|l| !d.hide_names && !l.is_empty()) {
                    Some(l) => l.to_string(),
                    None => self.dt("Deleted account"),
                },
            }
        };

        // The title says the period (a late one says how long), the footer which summary; a preset's has the preset above the title.
        let title = match days {
            7 => self.dt(if late { "Play time for 7 days" } else { "Play time in the last 7 days" }),
            30 => self.dt(if late { "Play time for 30 days" } else { "Play time in the last 30 days" }),
            _ => self.dt(if today {
                "Play time today"
            } else if late {
                "Play time for one day"
            } else {
                "Play time in the last 24 hours"
            }),
        };
        let footer = match kind {
            "daily" => self.dt("Bloxkeep · Daily summary"),
            "weekly" => self.dt("Bloxkeep · Weekly summary"),
            "both" => self.dt("Bloxkeep · Daily and weekly summary"),
            _ => self.dt("Bloxkeep · Summary"),
        };

        let mut lines: Vec<String> = Vec::new();
        let mut fields: Vec<DiscordField> = Vec::new();
        let preset_groups: Vec<(Option<Guid>, Vec<&StayPart>)> = if preset.is_none() {
            group_by(&parts, |p| p.session.preset_id.filter(|id| self.preset_for(Some(*id)).is_some()))
        } else {
            Vec::new()
        };
        let presets_played = preset_groups.iter().filter(|g| g.0.is_some()).count();
        if total > 0.0 {
            let (mut peak, mut open) = (0i32, 0i32);
            let mut peak_at = from;
            let mut steps: Vec<(Stamp, i32)> = parts.iter().flat_map(|p| [(p.from, 1), (p.to, -1)]).collect();
            steps.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            for (when, step) in steps {
                open += step;
                if open > peak {
                    (peak, peak_at) = (open, when);
                }
            }
            let mut summary_line = vec![format!("**{}** {}", accounts.len(), self.dt(if accounts.len() == 1 { "account played" } else { "accounts played" }))];
            if presets_played > 0 {
                summary_line.push(format!("**{presets_played}** {}", self.dt(if presets_played == 1 { "preset" } else { "presets" })));
            }
            lines.push(summary_line.join(" · "));

            if has("total") {
                fields.push(DiscordField::new(self.dt("Play time"), format!("**{}**\n{}", self.duration(total as i64), self.compared(total, before)), true));
            }
            let stays = distinct_stays(&parts);
            if has("sessions") {
                fields.push(DiscordField::new(
                    self.dt("Sessions"),
                    format!("**{stays}**\n{}", self.df("about {0} each", &[&self.duration((total / stays.max(1) as f64) as i64)])),
                    true,
                ));
            }
            if has("kicks") {
                let mut codes: Vec<(i32, usize)> = Vec::new();
                for s in drops.iter().filter_map(|s| s.reason_code) {
                    match codes.iter_mut().find(|c| c.0 == s) {
                        Some(c) => c.1 += 1,
                        None => codes.push((s, 1)),
                    }
                }
                codes.sort_by(|a, b| b.1.cmp(&a.1));
                let codes = codes.iter().take(3).map(|(code, n)| format!("`{code}` ×{n}")).collect::<Vec<_>>().join(" · ");
                let more = if codes.is_empty() { String::new() } else { format!("\n{}: {codes}", self.dt("Roblox codes")) };
                fields.push(DiscordField::new(self.dt("Disconnects"), format!("**{}**{more}", drops.len()), true));
            }
            let of = match (preset, &only) {
                (Some(p), _) => p.account_ids.iter().filter(|id| self.find_account(**id).is_some()).count(),
                (None, Some(o)) => o.iter().filter(|id| self.find_account(**id).is_some()).count(),
                (None, None) => account_count,
            };
            fields.push(DiscordField::new(self.dt("Accounts"), format!("**{}**/{of} {}", accounts.len(), self.dt("played")), true));
            let mut longest = &parts[0];
            for p in &parts {
                if p.seconds() > longest.seconds() {
                    longest = p;
                }
            }
            fields.push(DiscordField::new(
                self.dt("Longest stay"),
                format!(
                    "**{}**\n{} · {}",
                    self.duration(longest.seconds() as i64),
                    webhook::escape(Some(&name_of(longest.session.account_id, longest.session.account_label.as_deref())[..])),
                    webhook::escape(Some(&stay_game(&longest.session)[..]))
                ),
                true,
            ));
            fields.push(DiscordField::new(
                self.dt("Most at once"),
                format!("**{peak}** {}\n<t:{}:t>", self.dt(if peak == 1 { "window" } else { "windows" }), unix(peak_at)),
                true,
            ));
            if show_games {
                let top: Vec<String> = games
                    .iter()
                    .take(5)
                    .enumerate()
                    .map(|(i, g)| {
                        format!(
                            "**{}.** {} · {} · {:.0}%",
                            i + 1,
                            webhook::game_link(&stay_game(&g.2), Some(g.2.place_id)),
                            self.duration(g.1 as i64),
                            (g.1 / total * 100.0)
                        )
                    })
                    .collect();
                fields.push(DiscordField::new(self.dt("Top games"), top.join("\n"), false));
            }
            // Without a preset embed below, the accounts are listed here (the picture shows them too).
            if has("accounts") && (preset.is_some() || presets_played == 0) {
                fields.push(DiscordField::new(
                    self.df(if accounts.len() == 1 { "{0} account" } else { "{0} accounts" }, &[&accounts.len()]),
                    self.account_block(&accounts.iter().map(|a| (name_of(a.id, a.label.as_deref()), a.seconds)).collect::<Vec<_>>()),
                    false,
                ));
            }
        } else {
            lines.push(self.dt("Nothing played in this period."));
        }
        // The period in each reader's own time, the accounts it covers, and why it comes late, in small text.
        let chosen = match &only {
            Some(o) => format!(" · {}", self.df(if o.len() == 1 { "{0} chosen account" } else { "{0} chosen accounts" }, &[&o.len()])),
            None => String::new(),
        };
        lines.push(format!("-# <t:{}:f> → <t:{}:f>{chosen}", unix(from), unix(to)));
        if late {
            if let Some(due) = due_at {
                lines.push(format!("-# {}", self.df("Sent late: it was due {0}.", &[&format!("<t:{}:f>", unix(due.to_utc()))])));
            }
        }
        let (shown, text) = threes_fields(&fields, Some(&lines.join("\n")));

        let card = if total <= 0.0 {
            None
        } else {
            Some(SummaryCardData {
                chart_title: self.dt(if hourly || today { "Activity by hour" } else { "Activity by day" }),
                total_text: self.duration(total as i64),
                buckets: self.buckets(&parts, from, to, hourly || today, &colour),
                accounts_title: if has("accounts") { self.dt("By account") } else { String::new() },
                accounts: if !has("accounts") {
                    Vec::new()
                } else {
                    accounts
                        .iter()
                        .take(MAX_CARD_ACCOUNTS)
                        .map(|a| {
                            let name = name_of(a.id, a.label.as_deref());
                            let detail = self.df(if a.stays == 1 { "{0} stay" } else { "{0} stays" }, &[&a.stays])
                                + &if a.drops > 0 { format!(" · {}", self.df(if a.drops == 1 { "{0} drop" } else { "{0} drops" }, &[&a.drops])) } else { String::new() };
                            let account = self.find_account(a.id);
                            SummaryCardAccount {
                                time: self.duration(a.seconds as i64),
                                seconds: a.seconds,
                                parts: a.games.clone(),
                                // A hidden name has no picture either: the avatar would tell who it is.
                                avatar_url: if d.hide_names { None } else { account.as_ref().and_then(|acc| self.controller.avatar_urls.borrow().get(&acc.user_id).cloned()) },
                                initials: initials(&name),
                                hue: hue(&account.as_ref().map(|acc| acc.username.clone()).filter(|u| !u.is_empty()).unwrap_or_else(|| name.clone())),
                                name,
                                detail,
                            }
                        })
                        .collect()
                },
                more_accounts: if has("accounts") && accounts.len() > MAX_CARD_ACCOUNTS {
                    Some(self.df("+{0} more accounts", &[&(accounts.len() - MAX_CARD_ACCOUNTS)]))
                } else {
                    None
                },
                games: if show_games { games.iter().map(|g| stay_game(&g.2)).collect() } else { Vec::new() },
                other_games: self.dt("Other games"),
                empty: None,
            })
        };

        // An embed per preset that played (and one for play outside presets, when some did): its own figures and accounts.
        let mut more: Vec<DiscordMessage> = Vec::new();
        let mut with_preset: Vec<&(Option<Guid>, Vec<&StayPart>)> = preset_groups.iter().filter(|g| g.0.is_some()).collect();
        sort_desc(&mut with_preset, |g| sum_seconds(&g.1));
        let embeds = with_preset
            .into_iter()
            .take(MAX_PRESET_EMBEDS)
            .chain(if presets_played > 0 { preset_groups.iter().filter(|g| g.0.is_none()).collect::<Vec<_>>() } else { Vec::new() });
        for (key, group) in embeds {
            let own = self.preset_for(*key);
            let mut group_accounts: Vec<(String, f64)> = group_by(group.iter().copied(), |p| p.session.account_id)
                .into_iter()
                .map(|(id, g)| {
                    let label = g.iter().rev().filter_map(|p| p.session.account_label.clone()).find(|l| !l.is_empty());
                    (name_of(id, label.as_deref()), sum_seconds(&g))
                })
                .collect();
            sort_desc(&mut group_accounts, |a| a.1);
            let seconds = sum_seconds(group);
            let group_stays = distinct_stays(group.iter().copied());
            let group_drops = drops.iter().filter(|s| s.preset_id == *key).count();
            let mut by_game = group_by(group.iter().copied(), |p| p.game.clone());
            sort_desc(&mut by_game, |g| sum_seconds(&g.1));
            let top_game = by_game[0].1[0].session.clone();
            let mut embed_fields: Vec<DiscordField> = Vec::new();
            if has("total") {
                embed_fields.push(DiscordField::new(self.dt("Play time"), format!("**{}**", self.duration(seconds as i64)), true));
            }
            if has("sessions") {
                embed_fields.push(DiscordField::new(self.dt("Sessions"), format!("**{group_stays}**"), true));
            }
            if has("kicks") {
                embed_fields.push(DiscordField::new(self.dt("Disconnects"), format!("**{group_drops}**"), true));
            }
            if has("accounts") {
                embed_fields.push(DiscordField::new(
                    self.df(if group_accounts.len() == 1 { "{0} account" } else { "{0} accounts" }, &[&group_accounts.len()]),
                    self.account_block(&group_accounts),
                    false,
                ));
            }
            let preset_place = own.as_ref().and_then(App::preset_place_id);
            let about = match &own {
                None => self.dt("Opened by hand, or before Bloxkeep 1.0.5 noted presets."),
                Some(own) => [
                    if !own.place_input.is_empty() {
                        webhook::game_link(
                            &if !own.game_name.is_empty() { own.game_name.clone() } else { stay_game(&top_game) },
                            Some(preset_place.unwrap_or(top_game.place_id)),
                        )
                    } else {
                        self.dt("Roblox home")
                    },
                    match own.schedule.as_ref().filter(|s| s.enabled) {
                        Some(schedule) => self.df("Scheduled at {0}", &[&schedule.time]),
                        None => self.dt("Run by hand"),
                    },
                ]
                .join(" · "),
            };
            let (preset_fields, preset_text) = threes_fields(&embed_fields, Some(&about));
            let mut m = discord_message(
                match &own {
                    None => self.dt("Not from a preset"),
                    Some(own) => webhook::escape(Some(&own.name[..])),
                },
                preset_text.unwrap_or_default(),
                preset_fields,
                None,
                if own.is_none() { webhook::GREY } else { webhook::BLUE },
                None,
            );
            m.thumbnail = preset_place.and_then(|place| self.game_icon(place, 0)).or_else(|| stay_icon(&top_game));
            more.push(m);
        }

        let mut m = discord_message(title, text.unwrap_or_default(), shown, Some(footer), webhook::BLUE, Some(to));
        m.author = preset.map(|p| p.name.clone());
        m.thumbnail = Some(games.first().and_then(|g| stay_icon(&g.2)).unwrap_or_else(|| webhook::LOGO.to_string()));
        m.card = card;
        m.more = more;
        Ok(m)
    }

    /// Each stay's part inside [from, to), with the game it counts for (places of one game together).
    pub fn stays(&self, sessions: &[PlaySession], from: Stamp, to: Stamp, counts: &dyn Fn(&PlaySession) -> bool) -> Vec<StayPart> {
        let mut universe_of: HashMap<i64, i64> = HashMap::new();
        for s in sessions {
            if s.universe_id > 0 {
                universe_of.insert(s.place_id, s.universe_id);
            }
        }
        let mut list = Vec::new();
        for (index, s) in sessions.iter().enumerate() {
            if !counts(s) || s.started_at >= to {
                continue;
            }
            let end = match s.ended_at {
                Some(ended) if ended < to => ended,
                _ => to,
            };
            let start = if s.started_at > from { s.started_at } else { from };
            if end <= start {
                continue;
            }
            let universe = if s.universe_id > 0 {
                s.universe_id
            } else {
                match universe_of.get(&s.place_id) {
                    Some(u) if *u > 0 => *u,
                    _ => self.known_game(s.place_id).map_or(0, |g| g.universe_id),
                }
            };
            let game = if universe > 0 { format!("u{universe}") } else { format!("p{}", s.place_id) };
            list.push(StayPart { session: s.clone(), index, from: start, to: end, game });
        }
        list
    }

    /// The picture's columns: whole clock hours over the period (or days), each split by game.
    pub fn buckets(&self, parts: &[StayPart], from: Stamp, to: Stamp, hourly: bool, colour: &HashMap<String, i32>) -> Vec<SummaryCardBucket> {
        // Invariant culture: its English names suit the labels.
        const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        let vi = self.discord_language() == "vi";
        let first_hour = from.local().hour;
        let at_step = |k: i64| if hourly { local_day_hour(from, 0, first_hour + k as u32) } else { local_day(from, k) };
        let mut buckets = Vec::new();
        let mut k = 0i64;
        loop {
            let at = at_step(k);
            if !(at.to_utc() < to && buckets.len() < 32) {
                break;
            }
            let (a, b) = (at.to_utc(), at_step(k + 1).to_utc());
            let mut split: Vec<(i32, f64)> = Vec::new();
            for p in parts {
                let seconds = (if p.to < b { p.to } else { b }).secs_since(if p.from > a { p.from } else { a }).max(0.0);
                if seconds > 0.0 {
                    let game = colour.get(&p.game).copied().unwrap_or(-1);
                    match split.iter_mut().find(|x| x.0 == game) {
                        Some(x) => x.1 += seconds,
                        None => split.push((game, seconds)),
                    }
                }
            }
            let c = at.local();
            let label = if hourly {
                if vi {
                    format!("{}h", c.hour)
                } else {
                    // "h tt", lower case, no space: "1am", "12pm".
                    let h12 = if c.hour % 12 == 0 { 12 } else { c.hour % 12 };
                    format!("{h12}{}", if c.hour < 12 { "am" } else { "pm" })
                }
            } else if vi {
                format!("{:02}/{:02}", c.day, c.month)
            } else {
                format!("{} {}", MONTHS[(c.month - 1) as usize], c.day)
            };
            buckets.push(SummaryCardBucket::new(label, split));
            k += 1;
        }
        buckets
    }

    /// "▲ 35 min more than before" (the period as long before this one), and the like.
    pub fn compared(&self, now: f64, before: f64) -> String {
        if before <= 0.0 {
            return self.dt("Nothing the period before");
        }
        let diff = (now - before).abs() as i64;
        if diff < 60 {
            return self.dt("Same as before");
        }
        if now > before {
            format!("▲ {}", self.df("{0} more than before", &[&self.duration(diff)]))
        } else {
            format!("▼ {}", self.df("{0} less than before", &[&self.duration(diff)]))
        }
    }

    pub fn account_block(&self, accounts: &[(String, f64)]) -> String {
        let most = (if accounts.is_empty() { 1.0 } else { accounts.iter().map(|a| a.1).fold(f64::MIN, f64::max) }).max(1.0);
        let mut rows: Vec<String> = accounts
            .iter()
            .take(MAX_BLOCK_ACCOUNTS)
            .map(|(name, seconds)| {
                let name = name.replace('`', "ˋ");
                let units: Vec<u16> = name.encode_utf16().collect();
                let name = if units.len() > 16 {
                    String::from_utf16_lossy(&units[..15]) + "…"
                } else {
                    name.clone() + &" ".repeat(16 - units.len())
                };
                let minutes = (seconds / 60.0) as i64;
                // Math.Round: to even at .5, as Rust's round_ties_even.
                let blocks = (10.0 * seconds / most).round_ties_even() as usize;
                format!("{name} {:>2}:{:02}  {}{}", minutes / 60, minutes % 60, "█".repeat(blocks), "░".repeat(10usize.saturating_sub(blocks)))
            })
            .collect();
        if accounts.len() > MAX_BLOCK_ACCOUNTS {
            rows.push(self.df("and {0} more", &[&(accounts.len() - MAX_BLOCK_ACCOUNTS)]));
        }
        webhook::cut(Some(&format!("```\n{}\n```", rows.join("\n"))[..]), webhook::MAX_FIELD_VALUE)
    }
}
