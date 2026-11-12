// Tests/Bloxkeep.Tests/GameTargetSuite.cs: what a game box holds, read as a place to go to (GameTarget::try_parse),
// against the table Tests/game-target-cases.json.
use serde_json::Value;

use super::Checks;
use super::played_games::cases;
use crate::core::game_target::GameTarget;

pub async fn run(c: &mut Checks, _dir: &str) {
    let doc = cases("game-target-cases.json");
    c.check("The table of cases is found", doc.is_some(), "");
    let Some(doc) = doc else { return };
    let mut n = 0;
    for x in doc.as_array().cloned().unwrap_or_default() {
        n += 1;
        let text = |k: &str| x.get(k).and_then(Value::as_str).map(str::to_string);
        let input = text("in").unwrap_or_default();
        let job = text("job").unwrap_or_default();
        let parsed = GameTarget::try_parse(&input, &job);
        let what = format!("\"{input}\"{}", if job.is_empty() { String::new() } else { format!(" with job \"{job}\"") });
        if x["ok"].as_bool() == Some(true) {
            let ok = parsed.as_ref().is_ok_and(|t| {
                Some(t.place_id) == x["place"].as_i64() && t.job_id == text("jobId") && t.link_code == text("link") && t.share_code == text("share") && t.access_code == text("access")
            });
            let detail = match &parsed {
                Ok(t) => format!("place {}, job {:?}, link {:?}, share {:?}, access {:?}", t.place_id, t.job_id, t.link_code, t.share_code, t.access_code),
                Err(e) => e.clone(),
            };
            c.check(&format!("{what} is taken"), ok, detail);
        } else {
            let detail = match &parsed {
                Ok(_) => "taken".to_string(),
                Err(e) => e.clone(),
            };
            c.check(&format!("{what} is refused, with its reason"), parsed.as_ref().err().cloned() == text("error"), detail);
        }
    }
    c.check("The table has its cases", n >= 30, n);
}
