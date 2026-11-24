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
    plurals(c);
}

/// Counts take each language's CLDR form (lang.rs): Russian's one / few / many, French's 0 in the one form, and a word
/// falls back to its key when a language has no words for a form.
fn plurals(c: &mut Checks) {
    use crate::lang::{Plural::*, by_code};
    let form = |code: &str, n: i64| by_code(code).map(|l| l.form(n));
    let ru = [(0, Many), (1, One), (2, Few), (4, Few), (5, Many), (11, Many), (12, Many), (14, Many), (21, One), (22, Few), (25, Many), (101, One), (111, Many), (1004, Few)];
    let wrong: Vec<i64> = ru.iter().filter(|(n, f)| form("ru", *n) != Some(*f)).map(|(n, _)| *n).collect();
    c.check("Russian counts: 1, 21, 101 one; 2-4, 22 few; 0, 5-20, 111 many", wrong.is_empty(), &wrong);
    let pl = [(0, Many), (1, One), (2, Few), (4, Few), (5, Many), (12, Many), (14, Many), (21, Many), (22, Few), (24, Few), (25, Many), (101, Many), (112, Many), (1002, Few)];
    let wrong: Vec<i64> = pl.iter().filter(|(n, f)| form("pl", *n) != Some(*f)).map(|(n, _)| *n).collect();
    c.check("Polish counts: 1 alone one; 2-4, 22-24 few; 0, 5-21, 12-14, 101 many", wrong.is_empty(), &wrong);
    c.check("French and Portuguese say 0 and 1 in the one form, 2 not", [form("fr", 0), form("fr", 1), form("pt", 0)] == [Some(One); 3] && form("fr", 2) == Some(Other), "");
    c.check("English says 1 alone in the one form, Vietnamese has one form", form("en", 1) == Some(One) && form("en", 0) == Some(Other) && form("vi", 1) == Some(Other), "");
    let age = |code: &str, days: i64| by_code(code).map(|l| l.age(days)).unwrap_or_default();
    c.check(
        "how old, by form: 1 г. / 3 г. / 5 л., 1 mes / 2 meses",
        age("ru", 365) == "1\u{a0}г." && age("ru", 3 * 365) == "3\u{a0}г." && age("ru", 5 * 365) == "5\u{a0}л." && age("es", 30) == "1 mes" && age("es", 60) == "2 meses",
        [age("ru", 365), age("ru", 5 * 365), age("es", 60)],
    );
    let ru = by_code("ru");
    c.check(
        "Russian dates: 24 декабря 2026 г., 24.12, 1 мая",
        ru.is_some_and(|l| l.date_long(24, 12, 2026) == "24 декабря 2026\u{a0}г." && l.dm(24, 12) == "24.12" && l.day_month(1, 5) == "1 мая"),
        ru.map(|l| l.date_long(24, 12, 2026)),
    );
    c.check("German keeps its 24.12.", by_code("de").is_some_and(|l| l.dm(24, 12) == "24.12."), "");
    let mut words = crate::i18n::I18n::load(
        r##"{"en": {"x": "{n} things", "x1": "1 thing"},
            "ru": {"x": "{n} вещей", "x1": "одна вещь", "x#one": "{n} вещь", "x#few": "{n} вещи"},
            "fr": {"x": "{n} choses", "x1": "une chose", "x#one": "{n} chose"},
            "de": {"x": "{n} Dinge"}}"##,
    );
    words.set_language("ru");
    let said = [words.tn("x", 1, &[]), words.tn("x", 3, &[]), words.tn("x", 11, &[]), words.tn("x", 21, &[]), words.tn("x", 1234, &[]), words.tn("x", 1235, &[]), words.tv("x", &[("n", "22")]), words.tv("x", &[("n", "2.5")])];
    c.check(
        "a word in Russian: its own words for exactly one, else its form for the count ({n} grouped too); a value that is no count takes the key",
        said == ["одна вещь", "3 вещи", "11 вещей", "21 вещь", "1\u{a0}234 вещи", "1\u{a0}235 вещей", "22 вещи", "2.5 вещей"],
        &said,
    );
    words.set_language("fr");
    let said = [words.tn("x", 0, &[]), words.tn("x", 1, &[]), words.tn("x", 2, &[])];
    c.check("French: 0 chose (its one form), une chose, 2 choses", said == ["0 chose", "une chose", "2 choses"], &said);
    words.set_language("de");
    c.check("a language without a one form says the key for 1 (never English's)", words.tn("x", 1, &[]) == "1 Dinge", words.tn("x", 1, &[]));
    words.set_language("en");
    c.check("English: 1 thing, 0 things", words.tn("x", 1, &[]) == "1 thing" && words.tn("x", 0, &[]) == "0 things", words.tn("x", 0, &[]));
    let sentence = |n| crate::core::l::counted_in("{0} account didn't open.", "{0} accounts didn't open.", n, "ru");
    let said = [sentence(1), sentence(3), sentence(5), sentence(21)];
    c.check(
        "the core's Russian sentences take their form by the count (assets/lang/ru.json)",
        said == ["{0} аккаунт не открылся.", "{0} аккаунта не открылись.", "{0} аккаунтов не открылись.", "{0} аккаунт не открылся."],
        &said,
    );
    // RAM and file sizes in each language's units and decimal mark (lang.rs › mb, size).
    let sizes = |code: &str| by_code(code).map(|l| [l.mb(512.), l.mb(1700.), l.mb(2048.), l.size(3500.), l.size(1_300_000.)]).unwrap_or_default();
    let said = [sizes("en"), sizes("ru"), sizes("fr")];
    c.check(
        "sizes by language: 1.7 GB, 1,7 ГБ, 1,7 Go; 2 GB without a decimal",
        said == [
            ["512\u{a0}MB", "1.7\u{a0}GB", "2\u{a0}GB", "3.4\u{a0}KB", "1.2\u{a0}MB"].map(String::from),
            ["512\u{a0}МБ", "1,7\u{a0}ГБ", "2\u{a0}ГБ", "3,4\u{a0}КБ", "1,2\u{a0}МБ"].map(String::from),
            ["512\u{a0}Mo", "1,7\u{a0}Go", "2\u{a0}Go", "3,4\u{a0}Ko", "1,2\u{a0}Mo"].map(String::from),
        ],
        &said,
    );
}
