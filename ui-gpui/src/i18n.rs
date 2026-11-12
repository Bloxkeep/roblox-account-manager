// The interface's words: assets/i18n.json, made from wwwroot/i18n.js (tools/i18n.mjs), the same words the web page has.
// English is the default and the fallback; {name} placeholders are filled from the vars. Some words carry the page's
// little markup (<b>, <span class="sep">): `t` drops it, `rich` (rich.rs) draws it.
use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct I18n {
    en: HashMap<String, String>,
    vi: HashMap<String, String>,
    #[serde(skip)]
    pub vietnamese: bool,
}

impl I18n {
    pub fn load(json: &str) -> Self {
        serde_json::from_str(json).unwrap_or_default()
    }

    /// Words made for 1.1 (assets/words.json), on top of the page's: they go into i18n.js when the old page goes.
    pub fn add(&mut self, json: &str) {
        if let Ok(more) = serde_json::from_str::<I18n>(json) {
            self.en.extend(more.en);
            self.vi.extend(more.vi);
        }
    }

    pub fn set_language(&mut self, lang: &str) {
        self.vietnamese = lang == "vi";
        // (Times and dates follow: AM/PM and month/day in English, as the web page wrote them.)
        crate::time::set_english(!self.vietnamese);
    }

    fn raw(&self, key: &str) -> Option<&str> {
        let own = if self.vietnamese { self.vi.get(key) } else { None };
        own.or_else(|| self.en.get(key)).map(String::as_str)
    }

    /// Whether a language has words for the key.
    pub fn has(&self, key: &str) -> bool {
        self.raw(key).is_some()
    }

    /// The words for a key with their markup, placeholders filled; the key itself when no language has it.
    pub fn marked(&self, key: &str, vars: &[(&str, &str)]) -> String {
        let mut s = self.raw(key).unwrap_or(key).to_string();
        for (k, v) in vars {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }

    /// The words for a key in a language picked for it (Discord's messages may be in another one), as plain text.
    pub fn t_in(&self, vietnamese: bool, key: &str) -> String {
        let own = if vietnamese { self.vi.get(key) } else { None };
        plain(own.or_else(|| self.en.get(key)).map(String::as_str).unwrap_or(key))
    }

    /// The words for a key, as plain text.
    pub fn t(&self, key: &str) -> String {
        plain(&self.marked(key, &[]))
    }

    /// The words for a key as plain text, placeholders filled. The markup goes from the words before the values go
    /// in: a value is someone's text (a name, a note, a game) and keeps its "<" and "&" as typed.
    ///
    /// A count of one ({n} is 1) takes the key's "1" form when the language has it, as [`I18n::tn`] does: "1 account
    /// selected", not "1 accounts selected".
    pub fn tv(&self, key: &str, vars: &[(&str, &str)]) -> String {
        if vars.iter().any(|(k, v)| *k == "n" && *v == "1") {
            let one = format!("{key}1");
            let has_one = if self.vietnamese { self.vi.contains_key(&one) } else { self.en.contains_key(&one) };
            if has_one {
                return Self::fill(plain(self.raw(&one).unwrap_or(&one)), vars);
            }
        }
        Self::fill(plain(self.raw(key).unwrap_or(key)), vars)
    }

    fn fill(mut s: String, vars: &[(&str, &str)]) -> String {
        for (k, v) in vars {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }

    /// The "1" form (key + "1") when n is 1 and the chosen language has it, else the plural; {n} filled.
    pub fn tn(&self, key: &str, n: i64, vars: &[(&str, &str)]) -> String {
        let one = format!("{key}1");
        let has_one = if self.vietnamese { self.vi.contains_key(&one) } else { self.en.contains_key(&one) };
        let k = if n == 1 && has_one { one.as_str() } else { key };
        let n = n.to_string();
        let mut all = vec![("n", n.as_str())];
        all.extend_from_slice(vars);
        Self::fill(plain(self.raw(k).unwrap_or(k)), &all)
    }
}

/// Drops the page's markup: tags gone, entities the page uses turned back into characters.
pub fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", "\u{a0}")
}
