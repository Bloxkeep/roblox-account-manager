// Reading the app's JSON state: a field, or nothing when it isn't there or is of another kind (bad data never crashes
// the window). One set for every page (they each had their own copy: 23 of `s`, 2026-11-17).
use serde_json::Value;

/// A text field ("" when missing).
pub fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// HTML's character references read: the named ones Roblox and the wikis use (`&amp;`, `&nbsp;`…) and every numbered
/// one (`&#224;`, `&#xE0;`). Read twice at most, as Roblox's own text comes escaped twice at times (a private server's
/// name in its messages read "&#224;" on screen, 2026-11-20). One reader for every page: each had its own short list
/// of names, and none read the numbered ones.
pub fn entities(s: &str) -> String {
    fn once(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(at) = rest.find('&') {
            out.push_str(&rest[..at]);
            rest = &rest[at..];
            let end = rest[1..].find(|c: char| !(c.is_ascii_alphanumeric() || c == '#')).map(|e| e + 1).filter(|e| rest[*e..].starts_with(';'));
            let read = end.and_then(|e| {
                let name = &rest[1..e];
                let ch = match name {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => name
                        .strip_prefix("#x")
                        .or_else(|| name.strip_prefix("#X"))
                        .map(|h| u32::from_str_radix(h, 16).ok())
                        .unwrap_or_else(|| name.strip_prefix('#').and_then(|d| d.parse::<u32>().ok()))
                        .and_then(char::from_u32)
                        .map(|c| if c == '\u{a0}' { ' ' } else { c }),
                };
                ch.map(|c| (c, e + 1))
            });
            match read {
                Some((c, len)) => {
                    out.push(c);
                    rest = &rest[len..];
                }
                None => {
                    out.push('&');
                    rest = &rest[1..];
                }
            }
        }
        out.push_str(rest);
        out
    }
    let first = once(s);
    if first.contains('&') { once(&first) } else { first }
}

/// A name while names are hidden (Settings › Appearance): its first letter and "…", as the accounts' are shown.
pub fn first_only(name: &str) -> String {
    name.trim().chars().next().map(|c| format!("{c}…")).unwrap_or_else(|| "…".into())
}

/// A whole-number field (0 when missing).
pub fn int(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

/// A yes/no field (no when missing).
pub fn flag(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// A list field (empty when missing).
pub fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// A text field as an owned string ("" when missing).
pub fn string(v: &Value, k: &str) -> String {
    s(v, k).to_string()
}
