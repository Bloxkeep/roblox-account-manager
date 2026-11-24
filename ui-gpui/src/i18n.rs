// The interface's words: assets/i18n.json, made from wwwroot/i18n.js (tools/i18n.mjs), the same words the web page has.
// English is the default and the fallback; {name} placeholders are filled from the vars. Some words carry the page's
// little markup (<b>, <span class="sep">): `t` drops it, `rich` (rich.rs) draws it.
use std::{collections::HashMap, path::Path, sync::Arc};

type Words = HashMap<String, String>;

/// Every language's words by key (English the fallback), shared by the windows' copies.
#[derive(Default, Clone)]
pub struct I18n {
    en: Arc<Words>,
    others: Arc<HashMap<&'static str, Words>>,
    /// The language shown (lang.rs's code).
    pub lang: &'static str,
    /// Shown in Vietnamese: what only Vietnamese has (its own wiki words, its way of writing a span).
    pub vietnamese: bool,
}

impl I18n {
    /// The page's words ({"en": {…}, "vi": {…}}).
    pub fn load(json: &str) -> Self {
        let mut me = I18n { lang: "en", ..Default::default() };
        me.add(json);
        me
    }

    /// Every language's words from the app's assets folder: the page's (i18n.json), 1.1's (words.json) and the
    /// languages added later (lang/<code>.json's "words").
    pub fn from_assets(assets: &Path) -> Self {
        let mut me = Self::load(&std::fs::read_to_string(assets.join("i18n.json")).unwrap_or_default());
        me.add(&std::fs::read_to_string(assets.join("words.json")).unwrap_or_default());
        for l in crate::lang::ALL.iter().filter(|l| !matches!(l.code, "en" | "vi")) {
            let Ok(text) = std::fs::read_to_string(assets.join(format!("lang/{}.json", l.code))) else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            if let Some(words) = v.get("words").and_then(|w| serde_json::from_value::<Words>(w.clone()).ok()) {
                Arc::make_mut(&mut me.others).entry(l.code).or_default().extend(words);
            }
        }
        me
    }

    /// Each text in the language shown with its English one: Settings' search finds a row by either (app.js's data-en).
    pub fn en_by_own(&self) -> HashMap<String, String> {
        let Some(own) = self.others.get(self.lang) else { return HashMap::new() };
        own.iter().filter_map(|(k, v)| self.en.get(k).map(|e| (plain(v), plain(e)))).collect()
    }

    /// A key's English words, whatever the language shown.
    pub fn en(&self, key: &str) -> String {
        self.en.get(key).map(|e| plain(e)).unwrap_or_default()
    }

    /// More words ({"en": {…}, "vi": {…}, …}), on top of those here: 1.1's (assets/words.json) go into i18n.js when the
    /// old page goes.
    pub fn add(&mut self, json: &str) {
        let Ok(all) = serde_json::from_str::<HashMap<String, Words>>(json) else { return };
        for (code, words) in all {
            if code == "en" {
                Arc::make_mut(&mut self.en).extend(words);
            } else if let Some(l) = crate::lang::by_code(&code) {
                Arc::make_mut(&mut self.others).entry(l.code).or_default().extend(words);
            }
        }
    }

    pub fn set_language(&mut self, lang: &str) {
        self.lang = crate::lang::known(lang);
        self.vietnamese = self.lang == "vi";
        // (Numbers, times and dates follow: lang.rs.)
        crate::lang::set(self.lang);
    }

    /// The words of the language shown for a key, None when it has none (English's are used then).
    fn own(&self, lang: &str, key: &str) -> Option<&String> {
        if lang == "en" { None } else { self.others.get(lang).and_then(|w| w.get(key)) }
    }

    /// Whether the language shown has its own words for a key (a count's forms: a language without one uses the key).
    fn has_own(&self, key: &str) -> bool {
        if self.lang == "en" { self.en.contains_key(key) } else { self.own(self.lang, key).is_some() }
    }

    /// The key for a count of `n` in the language shown (lang.rs): "key1" for exactly one, else the count's form
    /// ("key#few"), when the language has words for it; else the key.
    fn counted(&self, key: &str, n: i64) -> String {
        if n == 1 {
            let k = format!("{key}1");
            if self.has_own(&k) {
                return k;
            }
        }
        let form = crate::lang::by_code(self.lang).map_or(crate::lang::Plural::Other, |l| l.form(n));
        let k = format!("{key}{}", form.suffix());
        if form != crate::lang::Plural::Other && self.has_own(&k) { k } else { key.to_string() }
    }

    /// What only a language added later has, with no English key (a country's name "country.US", a wiki line's
    /// word "wikiw.Damage"): None in English and Vietnamese, and when it has none.
    pub fn own_word(&self, key: &str) -> Option<&str> {
        if matches!(self.lang, "en" | "vi") { None } else { self.own(self.lang, key).map(String::as_str) }
    }

    pub fn raw(&self, key: &str) -> Option<&str> {
        self.own(self.lang, key).or_else(|| self.en.get(key)).map(String::as_str)
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
    pub fn t_in(&self, lang: &str, key: &str) -> String {
        plain(self.own(crate::lang::known(lang), key).or_else(|| self.en.get(key)).map(String::as_str).unwrap_or(key))
    }

    /// The words for a key, as plain text.
    pub fn t(&self, key: &str) -> String {
        plain(&self.marked(key, &[]))
    }

    /// The words for a key as plain text, placeholders filled. The markup goes from the words before the values go
    /// in: a value is someone's text (a name, a note, a game) and keeps its "<" and "&" as typed.
    ///
    /// A count in {n} takes its form's words, as [`I18n::tn`] does: "1 account selected", not "1 accounts selected";
    /// "2 аккаунта", "5 аккаунтов". {n} may come grouped ("1,234", "1 234").
    pub fn tv(&self, key: &str, vars: &[(&str, &str)]) -> String {
        let n = vars.iter().find(|(k, _)| *k == "n").and_then(|(_, v)| count_in(v));
        let k = match n {
            Some(n) => self.counted(key, n),
            None => key.to_string(),
        };
        Self::fill(plain(self.raw(&k).unwrap_or(&k)), vars)
    }

    fn fill(mut s: String, vars: &[(&str, &str)]) -> String {
        for (k, v) in vars {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }

    /// A count's words: its form's key in the language shown (lang.rs), else the key; {n} filled, grouped.
    pub fn tn(&self, key: &str, n: i64, vars: &[(&str, &str)]) -> String {
        let k = self.counted(key, n);
        let n = crate::lang::cur().thousands(n);
        let mut all = vec![("n", n.as_str())];
        all.extend_from_slice(vars);
        Self::fill(plain(self.raw(&k).unwrap_or(&k)), &all)
    }
}

/// A whole count written as a value ("5", "-2", "1,234", "1 234"), None for anything else ("2.5", "abc").
fn count_in(v: &str) -> Option<i64> {
    let v = v.trim();
    let (neg, body) = match v.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, v),
    };
    if !body.chars().all(|c| c.is_ascii_digit() || matches!(c, ',' | '.') || c.is_whitespace()) {
        return None;
    }
    // (Marks between digits only group them by three: "2.5" is no count.)
    let groups: Vec<&str> = body.split(|c: char| !c.is_ascii_digit()).collect();
    if groups.iter().any(|g| g.is_empty()) || groups.iter().skip(1).any(|g| g.len() != 3) || (groups.len() > 1 && groups[0].len() > 3) {
        return None;
    }
    let n: i64 = groups.concat().parse().ok()?;
    Some(if neg { -n } else { n })
}

/// Drops the page's markup: tags gone, entities the page uses turned back into characters.
pub fn plain(s: &str) -> String {
    // Most words have no markup: one copy, not a scan and four replaces (a page asks hundreds per frame).
    if !s.contains(['<', '&']) {
        return s.to_string();
    }
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
    // (Our own words keep a no-break space a no-break space.)
    crate::val::entities(&out.replace("&nbsp;", "\u{a0}"))
}
