//! A game's codes (the words a game takes in its Codes box for free rewards), working and expired, for any game:
//! Roblox keeps no list of them (each game checks its own in its own scripts), so they come from two places that keep
//! them, asked from this PC as a browser would:
//! - Roblox Den's codes list (robloxden.com, over 3,000 games; its own site's search, /api/game-codes/search), each
//!   code with whether it still works, its reward and when it was added. Each game there carries its Roblox universe,
//!   so the game is matched by that, never by name alone.
//! - The game's fan wiki (Fandom), found by its name as the Wiki tab finds one (the wiki picked for it there first):
//!   most keep a "Codes" page with a table of working codes and the expired ones under a tab or heading of their own.
//! Measured 2026-11-19 on 60 popular games: the wikis had a Codes page for 37; Roblox Den had the games the wikis
//! lacked (Hunty Zombie, Da Hood, Rivals, Peroxide, Arm Wrestle Simulator…). Roblox Den first, the wiki adding codes it
//! hasn't.
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde_json::{Value, json};

use crate::core::base::{Cancel, Res};
use crate::core::{log, prefs, wiki_net};

static HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| crate::net::builder().redirects(2).timeout(Duration::from_secs(20)).build());
/// The words of a Roblox name kept for a search: "[🎃UPDATE] Blox Fruits" → "Blox Fruits".
static BRACKETS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]]*\]|\([^)]*\)|【[^】]*】").unwrap());
static NOT_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}\s'&!.:-]").unwrap());
/// Markdown in Roblox Den's rewards ("**2x Experience**").
static MD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*|__|`").unwrap());
/// The words Roblox Den puts before each reward ("This code credits your account with a …").
static LEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(this code )?(credits your account with|gives you|grants you|rewards you with|will give you|unlocks|redeem (this|it) for)\s+(an? |the |some )?").unwrap()
});

/// One code: what it is, what it gives, whether it still works, when it was added (ISO, may be empty).
pub struct Code {
    pub code: String,
    pub reward: String,
    pub works: bool,
    pub added: String,
}

/// At most this many codes from one page.
const MOST: usize = 60;

static HEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<h[2-4][^>]*>(.*?)</h[2-4]>").unwrap());
/// Fandom's tabber: its tabs' names in order, then a panel for each in the same order.
static TAB_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?s)class="wds-tabs__tab-label"[^>]*>(.*?)</div>"#).unwrap());
static TAB_PANEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"class="wds-tab__content[ "]"#).unwrap());
/// The newer tabber (wiki.gg, some Fandom wikis): each panel carries its name.
static PANEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<article[^>]*class="tabber__panel[^"]*"[^>]*>"#).unwrap());
static PANEL_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"data-(?:mw-tabber-)?title="([^"]*)""#).unwrap());
static TABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<table[^>]*>(.*?)</table>").unwrap());
static ROW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<tr[^>]*>(.*?)</tr>").unwrap());
static CELL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<t([dh])[^>]*>(.*?)</t[dh]>").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<svg.*?</svg>|<style.*?</style>|<sup.*?</sup>|<br\s*/?>|<[^>]+>").unwrap());
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
/// A table under one of these is of codes that no longer work.
static OLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)expired|inactive|invalid|outdated|no longer|unavailable|removed|old codes|patched|not working").unwrap());
/// A code: one word of letters, digits and a few marks.
static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_\-!.#&'?$]{1,39}$").unwrap());
/// Marks a wiki puts beside a code that aren't part of it ("NEW", "(new!)", "[NEW]").
static MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s*[\(\[]?\s*(new|updated|hot)\s*!?\s*[\)\]]?\s*$").unwrap());

/// The codes of the game named `name` (universe `universe`, place `place`): {codes: [{code, reward, works, added}],
/// sources: [{name, page}], caseSensitive}; codes empty when neither place has the game.
pub fn codes(name: &str, universe: i64, place: i64, ct: &Cancel) -> Res<Value> {
    let mut list: Vec<Code> = Vec::new();
    let mut sources: Vec<Value> = Vec::new();
    let mut case = false;
    match den(name, universe) {
        Ok(Some((page, found, cased))) => {
            sources.push(json!({ "name": "Roblox Den", "page": page }));
            case = cased;
            list = found;
        }
        Ok(None) => {}
        Err(why) => log::note(&format!("Game codes: Roblox Den didn't answer ({why}).")),
    }
    match wiki(name, place, ct) {
        Ok(Some((site, page, found))) => {
            let before = list.len();
            for c in found {
                if !list.iter().any(|o| o.code.eq_ignore_ascii_case(&c.code)) {
                    list.push(c);
                }
            }
            if list.len() > before || sources.is_empty() {
                sources.push(json!({ "name": wiki_name(&site), "page": page }));
            }
        }
        Ok(None) => {}
        Err(why) => log::note(&format!("Game codes: the wiki didn't answer ({}).", why.message())),
    }
    // Working ones first, the newest first in each.
    list.sort_by(|a, b| b.works.cmp(&a.works).then(b.added.cmp(&a.added)));
    Ok(json!({
        "codes": list.iter().map(|c| json!({ "code": c.code, "reward": c.reward, "works": c.works, "added": c.added })).collect::<Vec<_>>(),
        "sources": sources,
        "caseSensitive": case,
    }))
}

/// "https://blox-fruits.fandom.com" → "Blox Fruits Wiki".
fn wiki_name(site: &str) -> String {
    let host = site.trim_start_matches("https://").split('.').next().unwrap_or("");
    let words: Vec<String> = host.split('-').filter(|w| !w.is_empty()).map(|w| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    }).collect();
    format!("{} Wiki", words.join(" "))
}

/// Roblox Den's codes of the game whose universe is `universe`: (its page, the codes, whether they are case-sensitive),
/// None when its list hasn't the game.
fn den(name: &str, universe: i64) -> Result<Option<(String, Vec<Code>, bool)>, String> {
    if universe <= 0 {
        return Ok(None);
    }
    let clean = NOT_NAME.replace_all(&BRACKETS.replace_all(name, " "), " ").split_whitespace().collect::<Vec<_>>().join(" ");
    // (The whole name, then its first words: "Blox Fruits: Update 25" may be listed as "Blox Fruits".)
    let mut tries = vec![clean.clone()];
    let first: Vec<&str> = clean.split_whitespace().take(2).collect();
    if first.len() == 2 && first.join(" ") != clean {
        tries.push(first.join(" "));
    }
    let want = universe.to_string();
    for words in tries.iter().filter(|w| w.chars().count() >= 2) {
        let url = format!("https://robloxden.com/api/game-codes/search?searchTerm={}", crate::core::roblox_api::escape_data_string(words));
        let answer: Value = match HTTP.get(&url).set("Accept", "application/json").call() {
            // (Read whole, up to 64 MB: a word many games share is a big answer, "Anime" 9.8 MB, and into_string stops
            // at 10 MB; the owner's log, 2026-11-19: "response too big for into_string", and the game had no Codes tab.)
            Ok(res) => serde_json::from_reader(std::io::Read::take(res.into_reader(), 64 << 20)).map_err(|e| e.to_string())?,
            Err(ureq::Error::Status(code, _)) => return Err(format!("HTTP {code}")),
            Err(e) => return Err(e.to_string()),
        };
        let Some(game) = answer.as_array().into_iter().flatten().find(|g| g["game"]["universeId"].as_str() == Some(want.as_str()) || g["game"]["universeId"].as_i64() == Some(universe)) else {
            continue;
        };
        let slug = game["slug"].as_str().unwrap_or("");
        let page = format!("https://robloxden.com/game-codes/{slug}");
        let codes = game["gameCodes"]
            .as_array()
            .or_else(|| game["codes"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|c| {
                let code = c["code"].as_str()?.trim().to_string();
                if code.is_empty() || code.chars().count() > 60 {
                    return None;
                }
                // (Its rewards come in markdown or in HTML: "<strong>Garcello Skin</strong>, …".)
                let reward = self::words(&MD.replace_all(c["description"].as_str().unwrap_or(""), ""));
                let reward = LEAD.replace(&reward, "").trim().trim_end_matches('.').to_string();
                let reward = reward.chars().next().map(|f| f.to_uppercase().collect::<String>() + &reward[f.len_utf8()..]).unwrap_or_default();
                // ("check" set: Roblox Den has it down as likely gone; its own page counts only active codes without it,
                // measured on 7 games 2026-11-19.)
                Some(Code { code, reward: short(&reward), works: c["active"].as_bool().unwrap_or(false) && !c["check"].as_bool().unwrap_or(false), added: c["createdAt"].as_str().unwrap_or("").to_string() })
            })
            .collect::<Vec<_>>();
        return Ok(Some((page, codes, game["caseSensitive"].as_bool().unwrap_or(false))));
    }
    Ok(None)
}

/// A reward cut to a line or two.
fn short(text: &str) -> String {
    if text.chars().count() > 160 {
        format!("{}…", text.chars().take(159).collect::<String>().trim_end())
    } else {
        text.to_string()
    }
}

/// The game's wiki's codes: (the wiki, its Codes page, the codes), None when no wiki or no Codes page.
fn wiki(name: &str, place: i64, ct: &Cancel) -> Res<Option<(String, String, Vec<Code>)>> {
    let none = || Ok(None);
    let picked = {
        let p = prefs::lock();
        p.wikis.games.iter().find(|g| place > 0 && g.place == place && !g.site.is_empty()).map(|g| g.site.clone())
    };
    let site = match picked {
        Some(s) => s,
        // (Named after the game, or starting with its name: a wiki of something else with the name in it isn't taken.)
        None => match wiki_net::find(&NOT_NAME.replace_all(&BRACKETS.replace_all(name, " "), " ").split_whitespace().collect::<Vec<_>>().join(" "), ct)?
            .into_iter()
            .find(|f| f.r#match >= 2)
        {
            Some(f) => f.site,
            None => return none(),
        },
    };
    let ask = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_, _>>();
    let found = wiki_net::api(&site, &ask(&[("action", "query"), ("list", "search"), ("srsearch", "codes"), ("srnamespace", "0"), ("srlimit", "8")]), ct)?;
    let titles: Vec<String> =
        found["query"]["search"].as_array().into_iter().flatten().filter_map(|t| t["title"].as_str()).map(str::to_string).collect();
    let Some(title) = titles
        .iter()
        .find(|t| t.eq_ignore_ascii_case("codes"))
        .or_else(|| titles.iter().find(|t| t.to_lowercase().contains("codes") && !t.contains('/')))
        .cloned()
    else {
        return none();
    };
    let page = wiki_net::api(&site, &ask(&[("action", "parse"), ("page", &title), ("prop", "text"), ("disablelimitreport", "1")]), ct)?;
    let html = page["parse"]["text"].as_str().or_else(|| page["parse"]["text"]["*"].as_str()).unwrap_or("");
    let list = from_html(html);
    if list.is_empty() {
        return none();
    }
    let page = format!("{site}/wiki/{}", title.replace(' ', "_"));
    Ok(Some((site, page, list)))
}

/// The page's codes and their rewards: every table's, the code from its "Code" column (else its first), the reward from
/// its "Reward" column (else the next); expired when the table is under an "Expired" tab or heading.
pub fn from_html(html: &str) -> Vec<Code> {
    // What each table sits under: the last heading or tab before it.
    let mut marks: Vec<(usize, String)> = HEAD.captures_iter(html).map(|m| (m.get(0).map_or(0, |g| g.start()), words(&m[1]))).collect();
    let names: Vec<String> = TAB_NAME.captures_iter(html).map(|m| words(&m[1])).collect();
    marks.extend(TAB_PANEL.find_iter(html).zip(names).map(|(m, n)| (m.start(), n)));
    marks.extend(PANEL.find_iter(html).map(|m| (m.start(), PANEL_NAME.captures(m.as_str()).map(|c| words(&c[1])).unwrap_or_default())));
    marks.sort_by_key(|m| m.0);
    let mut out: Vec<Code> = Vec::new();
    for t in TABLE.captures_iter(html) {
        let at = t.get(0).map_or(0, |g| g.start());
        let under = marks.iter().rev().find(|m| m.0 < at).map(|m| m.1.as_str()).unwrap_or("");
        let works = !OLD.is_match(under);
        let (mut code_at, mut reward_at): (Option<usize>, Option<usize>) = (None, None);
        for row in ROW.captures_iter(&t[1]) {
            let cells: Vec<(bool, String)> = CELL.captures_iter(&row[1]).map(|c| (&c[1] == "h", words(&c[2]))).collect();
            if cells.is_empty() {
                continue;
            }
            if cells.iter().all(|c| c.0) {
                let low: Vec<String> = cells.iter().map(|c| c.1.to_lowercase()).collect();
                code_at = low.iter().position(|h| h.contains("code"));
                reward_at = low.iter().position(|h| h.contains("reward") || h.contains("prize") || h.contains("gives") || h.contains("item"));
                // A table of something else (no Code column under a header): skipped.
                if code_at.is_none() {
                    code_at = Some(usize::MAX);
                }
                continue;
            }
            if code_at == Some(usize::MAX) {
                continue;
            }
            let c = code_at.unwrap_or_else(|| cells.iter().position(|c| !c.1.is_empty()).unwrap_or(0));
            let r = reward_at.unwrap_or(c + 1);
            let Some(code) = cells.get(c).map(|x| MARK.replace(&x.1, "").trim().to_string()) else { continue };
            if !CODE.is_match(&code) || out.iter().any(|o| o.code == code) {
                continue;
            }
            let reward = short(&cells.get(r).map(|x| x.1.clone()).unwrap_or_default());
            out.push(Code { code, reward, works, added: String::new() });
            if out.len() >= MOST {
                return out;
            }
        }
    }
    out
}

/// A bit of the page's HTML as plain words.
fn words(html: &str) -> String {
    let text = TAG.replace_all(html, " ");
    let text = crate::val::entities(&text);
    let text = text.replace('[', " [").replace("[ ]", "").replace("[]", "");
    SPACE.replace_all(text.trim(), " ").replace(" ,", ",").replace(" .", ".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_only() {
        let html = r##"<div class="tabber wds-tabber"><ul class="wds-tabs"><li><div class="wds-tabs__tab-label"><a href="#">Working Codes</a></div></li><li><div class="wds-tabs__tab-label"><a href="#">Expired Codes</a></div></li></ul><div class="wds-tab__content wds-is-current"><table><tr><th><svg>x</svg></th><th>Code</th><th>Reward</th></tr><tr><td><input></td><td><code>EASTEREXP</code> NEW</td><td>20 minutes of 2x Experience</td></tr></table></div><div class="wds-tab__content"><table><tr><th>Code</th><th>Reward</th></tr><tr><td>OLDONE</td><td>gone</td></tr></table></div></div>"##;
        let got: Vec<(String, bool)> = from_html(html).into_iter().map(|c| (c.code, c.works)).collect();
        assert_eq!(got, vec![("EASTEREXP".to_string(), true), ("OLDONE".to_string(), false)]);
    }
}
