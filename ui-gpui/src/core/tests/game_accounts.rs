// Tests/Bloxkeep.Tests/GameAccountsSuite.cs: the accounts that played a game in 7 days, summed from the stays
// (App::game_accounts_of), against the table Tests/game-accounts-cases.json. Its times are minutes from its `now`.
use serde_json::Value;

use super::Checks;
use super::played_games::{cases, known_games};
use crate::core::app::App;
use crate::core::base::{Guid, Stamp};
use crate::core::models::PlaySession;

pub async fn run(c: &mut Checks, _dir: &str) {
    let doc = cases("game-accounts-cases.json");
    c.check("The table of cases is found", doc.is_some(), "");
    let Some(doc) = doc else { return };
    let now = Stamp::from_unix_ms(doc["now"].as_i64().unwrap_or(0));
    let at = |minutes: &Value| now.plus_secs(minutes.as_f64().unwrap_or(0.0) * 60.0);
    let text = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).map(str::to_string);
    let guid = |v: &Value| v.as_str().and_then(|s| Guid::parse_str(s).ok()).unwrap_or_default();
    let mut n = 0;
    for x in doc["cases"].as_array().cloned().unwrap_or_default() {
        n += 1;
        let sessions: Vec<PlaySession> = x["sessions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|s| PlaySession {
                account_id: guid(&s["account"]),
                account_label: text(s, "label"),
                place_id: s["place"].as_i64().unwrap_or(0),
                universe_id: s["universe"].as_i64().unwrap_or(0),
                started_at: at(&s["start"]),
                ended_at: s["end"].is_number().then(|| at(&s["end"])),
                end_reason: text(s, "reason"),
                reason_code: s["code"].as_i64().map(|c| c as i32),
                ..Default::default()
            })
            .collect();
        let known = known_games(&x);
        let got = App::game_accounts_of(&sessions, &|p| known.get(&p).cloned(), x["place"].as_i64().unwrap_or(0), x["universe"].as_i64().unwrap_or(0), now);
        let want = x["expect"].as_array().cloned().unwrap_or_default();
        let same = got.len() == want.len()
            && got.iter().zip(&want).all(|(g, w)| {
                g.id == guid(&w["account"])
                    && (g.seconds - w["seconds"].as_f64().unwrap_or(f64::NAN)).abs() < 1e-6
                    && Some(g.kicks as i64) == w["kicks"].as_i64()
                    && Some(g.idle) == w["idle"].as_bool()
                    && Some(g.drops as i64) == w["drops"].as_i64()
                    && Some(g.now) == w["now"].as_bool()
                    && g.last == at(&w["last"])
                    && g.label == text(w, "label")
            });
        c.check(
            x["what"].as_str().unwrap_or(""),
            same,
            got.iter()
                .map(|g| {
                    format!(
                        "{} {} s, {} kicks{}, {} drops{}, last {} min, \"{}\"",
                        &g.id.to_string()[..4],
                        g.seconds,
                        g.kicks,
                        if g.idle { " idle" } else { "" },
                        g.drops,
                        if g.now { ", now" } else { "" },
                        g.last.secs_since(now) / 60.0,
                        g.label.clone().unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
        );
    }
    c.check("The table has its cases", n >= 10, n);
}
