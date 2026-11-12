// Tests/Bloxkeep.Tests/PlayLossesSuite.cs: Statistics' "Why they dropped": the time each kind of disconnect cost.
use super::Checks;
use crate::core::base::{Guid, Stamp, new_guid};
use crate::core::models::PlaySession;
use crate::core::play_losses;

/// new DateTime(y, m, d, 0, 0, 0, DateTimeKind.Utc).
pub(crate) fn utc_day(y: i64, m: i64, d: i64) -> Stamp {
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Stamp::from_unix_secs((era * 146097 + doe - 719468) * 86400)
}

pub async fn run(c: &mut Checks, _dir: &str) {
    let since = utc_day(2026, 11, 1);
    let now = since.plus_secs(2.0 * 86400.0);
    let (one, two) = (new_guid(), new_guid());
    let stay = |who: Guid, from: f64, to: f64, reason: Option<&str>, code: Option<i32>| PlaySession {
        account_id: who,
        started_at: since.plus_secs(from * 3600.0),
        ended_at: Some(since.plus_secs(to * 3600.0)),
        end_reason: reason.map(Into::into),
        reason_code: code,
        ..Default::default()
    };

    c.check("no drops: nothing", play_losses::of(&[stay(one, 0.0, 2.0, Some("left"), None)], since, now).is_empty(), "");

    let back = play_losses::of(&[stay(one, 0.0, 2.0, Some("disconnected"), Some(278)), stay(one, 5.0, 6.0, Some("left"), None)], since, now);
    c.check(
        "from the drop to the next stay",
        back.len() == 1 && back[0].kind == "idle" && back[0].times == 1 && back[0].seconds == 3 * 3600,
        back.iter().map(|l| (l.kind.clone(), l.times, l.seconds)).collect::<Vec<_>>(),
    );

    let never_back = play_losses::of(&[stay(one, 46.0, 47.0, Some("disconnected"), Some(278))], since, now);
    c.check("never back: up to now", never_back.len() == 1 && never_back[0].seconds == 3600, never_back.first().map(|l| l.seconds));
    let long_gone = play_losses::of(&[stay(one, 0.0, 1.0, Some("disconnected"), None)], since, now);
    c.check(
        "a long gap counts 12 hours at most",
        long_gone.len() == 1 && long_gone[0].kind == "other" && long_gone[0].seconds == 12 * 3600,
        long_gone.first().map(|l| (l.kind.clone(), l.seconds)),
    );

    let other_account =
        play_losses::of(&[stay(one, 0.0, 2.0, Some("disconnected"), Some(273)), stay(two, 3.0, 4.0, Some("left"), None)], since, since.plus_secs(5.0 * 3600.0));
    c.check(
        "another account's stay isn't this one coming back",
        other_account.len() == 1 && other_account[0].seconds == 3 * 3600,
        other_account.first().map(|l| l.seconds),
    );

    let before = play_losses::of(&[stay(one, -5.0, -4.0, Some("disconnected"), Some(278))], since, now);
    c.check("a drop before the period isn't in it", before.is_empty(), before.len());

    let sorted = play_losses::of(
        &[
            stay(one, 0.0, 1.0, Some("disconnected"), Some(267)),
            stay(one, 2.0, 3.0, Some("disconnected"), Some(278)),
            stay(one, 9.0, 10.0, Some("left"), None),
            stay(two, 0.0, 1.0, Some("disconnected"), Some(278)),
            stay(two, 2.0, 3.0, Some("left"), None),
        ],
        since,
        now,
    );
    let shown: Vec<_> = sorted.iter().map(|l| (l.kind.clone(), l.times, l.seconds, l.accounts.len())).collect();
    c.check("the kind that lost the most first", sorted.len() >= 2 && sorted[0].kind == "idle" && sorted[1].kind == "kicked", &shown);
    c.check(
        "…with its times and accounts",
        sorted.first().is_some_and(|l| l.times == 2 && l.seconds == 7 * 3600 && l.accounts.len() == 2),
        &shown,
    );
}
