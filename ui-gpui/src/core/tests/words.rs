// The interface's words (assets/words.json): an English count of one says "1 server", not "1 servers". I18n::tv and
// tn take a key's "1" form when {n} is 1, so every English "{n} <noun>s" needs one (25 were missing, the server
// country menu said "1 servers": the owner, 2026-11-20).
use super::Checks;

pub async fn run(c: &mut Checks, _dir: &str) {
    let all: serde_json::Value = serde_json::from_str(include_str!("../../../assets/words.json")).unwrap_or_default();
    let en = all.get("en").and_then(serde_json::Value::as_object).cloned().unwrap_or_default();
    c.check("the words load, English among them", en.len() > 100, en.len());
    let plural = |v: &str| {
        v.match_indices("{n} ").any(|(at, _)| {
            let word: String = v[at + 4..].chars().take_while(|ch| ch.is_ascii_alphabetic()).collect();
            word.len() > 2 && word.ends_with('s') && !word.ends_with("ss")
        })
    };
    let missing: Vec<&String> = en
        .iter()
        .filter(|(k, v)| !k.ends_with('1') && v.as_str().is_some_and(plural) && !en.contains_key(&format!("{k}1")))
        .map(|(k, _)| k)
        .collect();
    c.check("every English \"{n} <things>\" has its count-of-one form (key + \"1\")", missing.is_empty(), &missing);
    // The server country menu names any code ipinfo.io may answer (it showed "MY", "MX", "ID" with 14 names).
    let codes: Vec<&str> = crate::countries::codes().collect();
    let unique: std::collections::HashSet<&str> = codes.iter().copied().collect();
    c.check("every country is listed once (ISO 3166-1, about 249)", codes.len() >= 245 && unique.len() == codes.len(), codes.len());
    let unnamed: Vec<&str> = ["MY", "MX", "ID", "VN", "TH", "PH", "ZA", "AE", "SE", "ES"]
        .into_iter()
        .filter(|cc| crate::countries::name(cc, false).is_none() || crate::countries::name(cc, true).is_none())
        .collect();
    c.check("the countries Roblox servers are found in have English and Vietnamese names", unnamed.is_empty(), &unnamed);
}
