// PlayLosses.cs: play time lost to disconnects, by why they happened.
//! Play time lost to disconnects, by why they happened: from a drop to the account's next stay. Statistics' "Why they dropped".
use serde::{Deserialize, Serialize};

use crate::core::base::{Guid, Stamp};
use crate::core::models::PlaySession;

/// One drop counts this much at most: a longer gap is more likely a PC turned off than time lost.
const MOST_PER_DROP_SECS: f64 = 12.0 * 3600.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Loss {
    pub kind: String,
    pub times: i32,
    pub seconds: i64,
    pub accounts: Vec<Guid>,
}

/// Roblox's disconnect code as one of the kinds the page words: 278 idle, 273 the account joined elsewhere, 267 kicked.
pub fn kind_of(code: Option<i32>) -> &'static str {
    match code {
        Some(278) => "idle",
        Some(273) => "elsewhere",
        Some(267) => "kicked",
        _ => "other",
    }
}

/// The drops inside the period, the kind that lost the most time first.
pub fn of(stays: &[PlaySession], since: Stamp, now: Stamp) -> Vec<Loss> {
    let drops = stays
        .iter()
        .filter(|s| s.end_reason.as_deref() == Some("disconnected") && s.ended_at.is_some_and(|e| e >= since && e <= now));
    // GroupBy: the kinds in the order they first come.
    let mut groups: Vec<(&'static str, Vec<&PlaySession>)> = Vec::new();
    for s in drops {
        let kind = kind_of(s.reason_code);
        match groups.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, members)) => members.push(s),
            None => groups.push((kind, vec![s])),
        }
    }
    let mut losses: Vec<Loss> = groups
        .into_iter()
        .map(|(kind, members)| {
            let mut accounts: Vec<Guid> = Vec::new();
            for s in &members {
                if !accounts.contains(&s.account_id) {
                    accounts.push(s.account_id);
                }
            }
            Loss {
                kind: kind.to_string(),
                times: members.len() as i32,
                seconds: members.iter().map(|drop| lost(drop, stays, now)).sum::<f64>() as i64,
                accounts,
            }
        })
        .collect();
    losses.sort_by(|a, b| b.seconds.cmp(&a.seconds).then(b.times.cmp(&a.times)));
    losses
}

/// Seconds lost to one drop.
fn lost(drop: &PlaySession, stays: &[PlaySession], now: Stamp) -> f64 {
    let Some(from) = drop.ended_at else { return 0.0 };
    let back = stays.iter().filter(|s| s.account_id == drop.account_id && s.started_at >= from).map(|s| s.started_at).min();
    let until = match back {
        Some(next) if next < now => next,
        _ => now,
    };
    until.secs_since(from).min(MOST_PER_DROP_SECS)
}
