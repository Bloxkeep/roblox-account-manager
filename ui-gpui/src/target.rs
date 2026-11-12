// Where a game box points: what the person typed or pasted, read as a place, a server of it, a private server or a
// share link. It is the C# app's GameTarget.TryParse (RobloxManager/GameTarget.cs), moved here rule for rule: the app
// still reads the same words its own way when it launches, so the two must agree. They are held to that by one table
// of cases (Tests/game-target-cases.json) that both run: the C# suite "game-target" and the test at this file's end.

/// Where to send an account: a place, and with it a server (job), a private server (by its link's code, or by its
/// access code) or a share link's code.
#[derive(Debug, Default, PartialEq)]
pub struct Target {
    pub place: i64,
    pub job: Option<String>,
    pub link: Option<String>,
    pub share: Option<String>,
    pub access: Option<String>,
}

impl Target {
    /// A private server, by its link or by its access code.
    pub fn is_private(&self) -> bool {
        self.link.is_some() || self.access.is_some()
    }
}

/// Why what was typed isn't a place to go to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Why {
    Empty,
    Words,
    ShareKind,
    Access,
    NoPlace,
    Place,
    Job,
}

impl Why {
    /// Its words, in assets/words.json (the English ones are the C# app's own, to the letter).
    pub fn key(self) -> &'static str {
        match self {
            Why::Empty => "gt.empty",
            Why::Words => "gt.words",
            Why::ShareKind => "gt.shareKind",
            Why::Access => "gt.access",
            Why::NoPlace => "gt.noPlace",
            Why::Place => "gt.place",
            Why::Job => "gt.job",
        }
    }
}

/// Accepts a place id, a roblox.com/games/… link (privateServerLinkCode, gameInstanceId), a
/// roblox.com/share?code=…&type=Server link (also roblox://navigation/share_links, or wrapped in ro.blox.com), or a
/// roblox:// deep link with placeId, also with accessCode.
pub fn parse(place_input: &str, job_input: &str) -> Result<Target, Why> {
    parse_at(place_input, job_input, 0)
}

fn has(text: &str, part: &str) -> bool {
    text.to_ascii_lowercase().contains(part)
}

fn unescape(s: &str) -> String {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy().into_owned()
}

/// The name=value pairs of a link: after ? or &, right after "//" ("roblox://placeId=…") or at the very start. The
/// first of a name is the one kept, and names are compared without regard to case. (The C# app's pattern,
/// `(?:[?&]|//|^)([^=&#\s/?]+)=([^&#\s]*)`, walked by hand: at each place its marks are tried in that order.)
fn pairs(input: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = input.chars().collect();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut leads: Vec<usize> = Vec::new();
        if chars[i] == '?' || chars[i] == '&' {
            leads.push(1);
        }
        if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
            leads.push(2);
        }
        if i == 0 {
            leads.push(0);
        }
        match leads.into_iter().find_map(|lead| pair_at(&chars, i + lead)) {
            Some((name, value, end)) => {
                push(&mut out, name, value);
                i = end.max(i + 1);
            }
            None => i += 1,
        }
    }
    out
}

/// A pair whose name begins at `from`: the name, the value and where the value ends.
fn pair_at(chars: &[char], from: usize) -> Option<(String, String, usize)> {
    let mut j = from;
    while j < chars.len() && !matches!(chars[j], '=' | '&' | '#' | '/' | '?') && !chars[j].is_whitespace() {
        j += 1;
    }
    if j == from || j >= chars.len() || chars[j] != '=' {
        return None;
    }
    let mut k = j + 1;
    while k < chars.len() && !matches!(chars[k], '&' | '#') && !chars[k].is_whitespace() {
        k += 1;
    }
    Some((chars[from..j].iter().collect(), chars[j + 1..k].iter().collect(), k))
}

fn push(out: &mut Vec<(String, String)>, name: String, value: String) {
    let name = unescape(&name);
    if !out.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
        out.push((name, unescape(&value)));
    }
}

fn find<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

/// The digits after "/games/", when the link has them (0 when they are too many to be a number).
fn games_path(input: &str) -> Option<i64> {
    let low = input.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = low[from..].find("/games/") {
        let start = from + at + "/games/".len();
        let digits: String = low[start..].chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() {
            return Some(digits.parse().unwrap_or(0));
        }
        from = start;
    }
    None
}

/// A GUID as .NET reads one (32 hex digits; dashed 8-4-4-4-12; that in braces or brackets), written the one way Roblox
/// and its logs write it: lower case, dashed.
fn guid(text: &str) -> Option<String> {
    let t = text.trim();
    let inner = if (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('(') && t.ends_with(')')) { &t[1..t.len() - 1] } else { t };
    let hex: String = if inner.len() == 36 {
        let b = inner.as_bytes();
        if b[8] != b'-' || b[13] != b'-' || b[18] != b'-' || b[23] != b'-' {
            return None;
        }
        inner.chars().filter(|c| *c != '-').collect()
    } else if inner.len() == 32 && inner.len() == t.len() {
        inner.to_string()
    } else {
        return None;
    };
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let h = hex.to_ascii_lowercase();
    Some(format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32]))
}

fn some(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn parse_at(place_input: &str, job_input: &str, depth: u8) -> Result<Target, Why> {
    let input = place_input.trim();
    let job = job_input.trim();
    if input.is_empty() {
        return Err(Why::Empty);
    }
    let mut place: i64 = 0;
    let (mut link, mut share, mut job_id, mut access): (Option<String>, Option<String>, Option<String>, Option<String>) = (None, None, None, None);
    match input.parse::<i64>() {
        Ok(n) if n <= 0 => return Err(Why::Place),
        Ok(n) => place = n,
        Err(_) => {
            // Words, not a link (a game typed by name and not picked from the search): say what to do instead.
            if !input.contains('/') && !input.contains('?') && !has(input, "roblox") {
                return Err(Why::Words);
            }
            let query = pairs(input);
            // A ro.blox.com link from the Roblox app wraps the real link in af_web_dp (or af_dp); read that one.
            if depth < 2 {
                if let Some(wrapped) = find(&query, "af_web_dp").or_else(|| find(&query, "af_dp")) {
                    if has(wrapped, "roblox") {
                        return parse_at(wrapped, job_input, depth + 1);
                    }
                }
            }
            if let Some(n) = games_path(input) {
                place = n;
            } else if let Some(p) = find(&query, "placeId") {
                place = p.trim().parse().unwrap_or(0);
            }
            let is_share = has(input, "roblox.com/share") || has(input, "share_links");
            if let (true, Some(code)) = (is_share, find(&query, "code")) {
                if find(&query, "type").is_some_and(|t| !t.eq_ignore_ascii_case("Server")) {
                    return Err(Why::ShareKind);
                }
                share = Some(code.to_string());
            }
            if let Some(code) = find(&query, "privateServerLinkCode").or_else(|| find(&query, "linkCode")) {
                link = Some(code.to_string());
            }
            if let Some(id) = find(&query, "gameInstanceId").or_else(|| find(&query, "jobId")) {
                job_id = Some(id.to_string());
            }
            if let Some(code) = find(&query, "accessCode").filter(|c| !c.trim().is_empty()) {
                access = Some(guid(code).ok_or(Why::Access)?);
            }
            if place <= 0 && share.is_none() {
                return Err(Why::NoPlace);
            }
        }
    }
    if !job.is_empty() {
        job_id = Some(job.to_string());
    }
    if let Some(id) = job_id.as_deref().filter(|j| !j.is_empty()) {
        job_id = Some(guid(id).ok_or(Why::Job)?);
    }
    Ok(Target { place, job: some(job_id), link: some(link), share, access })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// The table both this and the C# app are held to (Tests/game-target-cases.json; the C# suite is "game-target").
    #[test]
    fn agrees_with_the_app() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Tests/game-target-cases.json");
        let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&path).expect("the cases file")).expect("the cases as JSON");
        let words: Value = serde_json::from_str(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/words.json")).unwrap()).unwrap();
        assert!(cases.len() >= 30, "the table lost its cases");
        let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        for case in &cases {
            let input = text(case, "in").unwrap_or_default();
            let got = parse(&input, &text(case, "job").unwrap_or_default());
            match (case.get("ok").and_then(Value::as_bool).unwrap_or(false), got) {
                (true, Ok(t)) => {
                    let want = Target { place: case.get("place").and_then(Value::as_i64).unwrap_or(0), job: text(case, "jobId"), link: text(case, "link"), share: text(case, "share"), access: text(case, "access") };
                    assert_eq!(t, want, "for {input:?}");
                }
                (false, Err(why)) => {
                    // The same sentence the app says, in English.
                    let said = words["en"][why.key()].as_str().unwrap_or_default();
                    assert_eq!(Some(said.to_string()), text(case, "error"), "for {input:?}");
                }
                (want, got) => panic!("for {input:?}: the app {} it, this gave {got:?}", if want { "takes" } else { "refuses" }),
            }
        }
    }
}
