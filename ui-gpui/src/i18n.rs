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

    /// Whether the language shown has its own words for a key ("1" forms: a language without them uses the plural).
    fn has_own(&self, key: &str) -> bool {
        if self.lang == "en" { self.en.contains_key(key) } else { self.own(self.lang, key).is_some() }
    }

    /// Whether `n` takes a key's "1" form in the language shown.
    fn one(&self, n: i64) -> bool {
        crate::lang::by_code(self.lang).is_some_and(|l| l.is_one(n))
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
    /// A count of one ({n} is 1; 0 too in French and Portuguese) takes the key's "1" form when the language has it, as
    /// [`I18n::tn`] does: "1 account selected", not "1 accounts selected".
    pub fn tv(&self, key: &str, vars: &[(&str, &str)]) -> String {
        if vars.iter().any(|(k, v)| *k == "n" && v.parse::<i64>().is_ok_and(|n| self.one(n))) {
            let one = format!("{key}1");
            if self.has_own(&one) {
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
        let k = if self.one(n) && self.has_own(&one) { one.as_str() } else { key };
        let n = crate::lang::cur().thousands(n);
        let mut all = vec![("n", n.as_str())];
        all.extend_from_slice(vars);
        Self::fill(plain(self.raw(k).unwrap_or(k)), &all)
    }
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
