// The interface's languages and how each writes numbers, times and dates. English and Vietnamese came first; Spanish,
// Portuguese (Brazil), Indonesian, French and German were added on 2026-11-24 (the owner). Their words are in
// assets/lang/<code>.json; the names of days and months and the way times are written follow Roblox's own
// (RobloxPlayer's content/configs/DateTimeLocaleConfigs: es-mx, pt-br, fr-fr, de-de), Indonesian follows CLDR.
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Lang {
    /// "en", "vi", "es", "pt", "id", "fr", "de": what prefs keep.
    pub code: &'static str,
    /// Its own name, as the language list shows it.
    pub name: &'static str,
    /// "Language" in it, for the search (palette).
    pub word: &'static str,
    /// 1,234 / 1.234 / 1 234.
    pub group: char,
    /// 0.5 / 0,5.
    pub decimal: char,
    /// 2:30 PM ("AM", "PM" here), else None: 14:30.
    pub meridiem: Option<(&'static str, &'static str)>,
    /// Between hours and minutes: ":" (Indonesian writes 14.30).
    pub time_sep: char,
    /// Day before month (24/12), else month first (12/24).
    pub day_first: bool,
    /// Between day and month: "/" or "." (German: 24.12.).
    pub date_sep: char,
    pub days_short: [&'static str; 7],
    pub days_long: [&'static str; 7],
    pub months: [&'static str; 12],
    pub months_short: [&'static str; 12],
    /// A short span's units: seconds, minutes, hours, as "{n}…" (Vietnamese writes them its own way: time::span).
    pub units: (&'static str, &'static str, &'static str),
    /// A day, short, after a number of them ("2d 5h", "2 ngày 5 giờ").
    pub day_abbr: &'static str,
    /// Whether a count takes the "one" form (key + "1"): French and Portuguese say 0 in it too.
    pub one_zero: bool,
}

pub const ALL: &[Lang] = &[
    Lang {
        code: "en",
        name: "English",
        word: "Language",
        group: ',',
        decimal: '.',
        meridiem: Some(("AM", "PM")),
        time_sep: ':',
        day_first: false,
        date_sep: '/',
        days_short: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        days_long: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
        months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
        months_short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
        units: ("s", "m", "h"),
        day_abbr: "d",
        one_zero: false,
    },
    Lang {
        code: "vi",
        name: "Tiếng Việt",
        word: "Ngôn ngữ",
        group: '.',
        decimal: ',',
        meridiem: None,
        time_sep: ':',
        day_first: true,
        date_sep: '/',
        days_short: ["CN", "T2", "T3", "T4", "T5", "T6", "T7"],
        days_long: ["Chủ Nhật", "Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy"],
        months: ["tháng 1", "tháng 2", "tháng 3", "tháng 4", "tháng 5", "tháng 6", "tháng 7", "tháng 8", "tháng 9", "tháng 10", "tháng 11", "tháng 12"],
        months_short: ["Th1", "Th2", "Th3", "Th4", "Th5", "Th6", "Th7", "Th8", "Th9", "Th10", "Th11", "Th12"],
        units: (" giây", "p", " giờ"),
        day_abbr: " ngày",
        one_zero: false,
    },
    Lang {
        code: "es",
        name: "Español",
        word: "Idioma",
        group: ',',
        decimal: '.',
        meridiem: Some(("a.m.", "p.m.")),
        time_sep: ':',
        day_first: true,
        date_sep: '/',
        days_short: ["dom.", "lun.", "mar.", "mié.", "jue.", "vie.", "sáb."],
        days_long: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
        months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
        months_short: ["ene.", "feb.", "mar.", "abr.", "may.", "jun.", "jul.", "ago.", "sep.", "oct.", "nov.", "dic."],
        units: (" s", " min", " h"),
        day_abbr: " d",
        one_zero: false,
    },
    Lang {
        code: "pt",
        name: "Português (Brasil)",
        word: "Idioma",
        group: '.',
        decimal: ',',
        meridiem: None,
        time_sep: ':',
        day_first: true,
        date_sep: '/',
        days_short: ["dom.", "seg.", "ter.", "qua.", "qui.", "sex.", "sáb."],
        days_long: ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
        months: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
        months_short: ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."],
        units: (" s", " min", " h"),
        day_abbr: " d",
        one_zero: true,
    },
    Lang {
        code: "id",
        name: "Bahasa Indonesia",
        word: "Bahasa",
        group: '.',
        decimal: ',',
        meridiem: None,
        time_sep: '.',
        day_first: true,
        date_sep: '/',
        days_short: ["Min", "Sen", "Sel", "Rab", "Kam", "Jum", "Sab"],
        days_long: ["Minggu", "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu"],
        months: ["Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September", "Oktober", "November", "Desember"],
        months_short: ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des"],
        units: (" dtk", " mnt", " j"),
        day_abbr: " hr",
        one_zero: false,
    },
    Lang {
        code: "fr",
        name: "Français",
        word: "Langue",
        group: '\u{a0}',
        decimal: ',',
        meridiem: None,
        time_sep: ':',
        day_first: true,
        date_sep: '/',
        days_short: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
        days_long: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
        months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
        months_short: ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
        units: ("\u{a0}s", "\u{a0}min", "\u{a0}h"),
        day_abbr: "\u{a0}j",
        one_zero: true,
    },
    Lang {
        code: "de",
        name: "Deutsch",
        word: "Sprache",
        group: '.',
        decimal: ',',
        meridiem: None,
        time_sep: ':',
        day_first: true,
        date_sep: '.',
        days_short: ["So.", "Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa."],
        days_long: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
        months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
        months_short: ["Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sep.", "Okt.", "Nov.", "Dez."],
        units: (" s", " Min.", " Std."),
        day_abbr: " T.",
        one_zero: false,
    },
];

/// The language shown (an index into ALL).
static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// The language with this code; None for one not here.
pub fn by_code(code: &str) -> Option<&'static Lang> {
    let code = code.trim().to_ascii_lowercase();
    // ("pt-br", "es-419": the part before the dash.)
    let code = code.split(['-', '_']).next().unwrap_or("");
    ALL.iter().find(|l| l.code == code)
}

/// A code kept in prefs or sent by the core, as one of ALL's ("en" for one not known).
pub fn known(code: &str) -> &'static str {
    by_code(code).map_or("en", |l| l.code)
}

pub fn set(code: &str) {
    let i = ALL.iter().position(|l| l.code == known(code)).unwrap_or(0);
    CURRENT.store(i, Ordering::Relaxed);
}

/// The language shown now.
/// The list to pick from (Settings › Appearance, the lock screen, Discord): each by its own name, A to Z as language
/// lists go (English, Bahasa Indonesia, Deutsch, …).
pub fn picker_items() -> Vec<(&'static str, String, bool)> {
    let mut all: Vec<&Lang> = ALL.iter().collect();
    all.sort_by_key(|l| l.name.to_lowercase());
    all.into_iter().map(|l| (l.code, l.name.to_string(), false)).collect()
}

pub fn cur() -> &'static Lang {
    &ALL[CURRENT.load(Ordering::Relaxed).min(ALL.len() - 1)]
}

impl Lang {
    /// Digits grouped by thousands this language's way: 1,234,567 / 1.234.567.
    pub fn thousands(&self, n: i64) -> String {
        let digits = n.unsigned_abs().to_string();
        let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
        if n < 0 {
            out.push('-');
        }
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push(self.group);
            }
            out.push(ch);
        }
        out
    }

    /// A number written with a point ("12.5", as Rust formats it) given this language's decimal mark.
    pub fn decimal(&self, text: &str) -> String {
        if self.decimal == '.' { text.to_string() } else { text.replace('.', &self.decimal.to_string()) }
    }

    /// How old, short, from days: "3d", "5mo", "2y" in English; "3 ngày", "5 tháng"; "5 meses", "1 año".
    pub fn age(&self, days: i64) -> String {
        let (n, unit) = if days < 30 { (days.max(1), 0) } else if days < 365 { (days / 30, 1) } else { (days / 365, 2) };
        let one = n == 1;
        let word = match (self.code, unit) {
            ("en", 0) => "d",
            ("en", 1) => "mo",
            ("en", _) => "y",
            ("vi", 0) => " ngày",
            ("vi", 1) => " tháng",
            ("vi", _) => " năm",
            ("es", 0) => " d",
            ("es", 1) => if one { " mes" } else { " meses" },
            ("es", _) => if one { " año" } else { " años" },
            ("pt", 0) => " d",
            ("pt", 1) => if one { " mês" } else { " meses" },
            ("pt", _) => if one { " ano" } else { " anos" },
            ("id", 0) => " hr",
            ("id", 1) => " bln",
            ("id", _) => " thn",
            ("fr", 0) => "\u{a0}j",
            ("fr", 1) => "\u{a0}mois",
            ("fr", _) => if one { "\u{a0}an" } else { "\u{a0}ans" },
            (_, 0) => " T.",
            (_, 1) => " Mon.",
            _ => " J.",
        };
        format!("{n}{word}")
    }

    /// Whether a count takes the "one" form.
    pub fn is_one(&self, n: i64) -> bool {
        n == 1 || (self.one_zero && n == 0)
    }

    /// An hour and minute of the day: 2:30 PM, 14:30, 14.30.
    pub fn hm(&self, h: i64, m: i64) -> String {
        let (h, m) = (h.rem_euclid(24), m.rem_euclid(60));
        match self.meridiem {
            Some((am, pm)) => format!("{}{}{m:02} {}", if h % 12 == 0 { 12 } else { h % 12 }, self.time_sep, if h < 12 { am } else { pm }),
            None => format!("{h:02}{}{m:02}", self.time_sep),
        }
    }

    /// A whole hour, for a chart's marks: 2 PM, 14:00.
    pub fn hour(&self, h: i64) -> String {
        match self.meridiem {
            Some((am, pm)) => format!("{} {}", if h.rem_euclid(12) == 0 { 12 } else { h.rem_euclid(12) }, if h.rem_euclid(24) < 12 { am } else { pm }),
            None => format!("{:02}{}00", h.rem_euclid(24), self.time_sep),
        }
    }

    /// A day and month: 12/24, 24/12, 24.12.
    pub fn dm(&self, d: i64, m: i64) -> String {
        match (self.day_first, self.date_sep) {
            (false, s) => format!("{m}{s}{d}"),
            (true, '.') => format!("{d}.{m}."),
            (true, s) => format!("{d}{s}{m}"),
        }
    }

    /// A whole date in numbers: 12/24/2026, 24/12/2026, 24.12.2026.
    pub fn dmy(&self, d: i64, m: i64, y: i64) -> String {
        let s = self.date_sep;
        if self.day_first { format!("{d:02}{s}{m:02}{s}{y}") } else { format!("{m:02}{s}{d:02}{s}{y}") }
    }

    /// A date in words: December 24, 2026 / 24 de diciembre de 2026 / 24. Dezember 2026 / 24 tháng 12, 2026.
    pub fn date_long(&self, d: i64, m: i64, y: i64) -> String {
        let month = self.months[(m - 1).clamp(0, 11) as usize];
        match self.code {
            "en" => format!("{month} {d}, {y}"),
            "vi" => format!("{d} {month}, {y}"),
            "es" | "pt" => format!("{d} de {month} de {y}"),
            "de" => format!("{d}. {month} {y}"),
            _ => format!("{d} {month} {y}"),
        }
    }

    /// A date in words without the year: December 24 / 24 de diciembre / 24. Dezember.
    pub fn day_month(&self, d: i64, m: i64) -> String {
        let month = self.months[(m - 1).clamp(0, 11) as usize];
        match self.code {
            "en" => format!("{month} {d}"),
            "es" | "pt" => format!("{d} de {month}"),
            "de" => format!("{d}. {month}"),
            _ => format!("{d} {month}"),
        }
    }

    /// A short date with the month's short name: Dec 24, 2026 / 24 dic. 2026 / 24. Dez. 2026.
    pub fn date_short(&self, d: i64, m: i64, y: Option<i64>) -> String {
        if self.code == "vi" {
            return match y {
                Some(y) => format!("{d:02}/{m:02}/{y}"),
                None => format!("{d:02}/{m:02}"),
            };
        }
        let month = self.months_short[(m - 1).clamp(0, 11) as usize];
        let base = match self.code {
            "en" => format!("{month} {d}"),
            "de" => format!("{d}. {month}"),
            _ => format!("{d} {month}"),
        };
        match (y, self.code) {
            (Some(y), "en") => format!("{base}, {y}"),
            (Some(y), _) => format!("{base} {y}"),
            (None, _) => base,
        }
    }

    /// A month and year, for a chart's marks: Dec '26 / dic. 26 / 12/2026.
    pub fn month_year(&self, m: i64, y: i64) -> String {
        if self.code == "vi" {
            return format!("{m}/{y}");
        }
        let month = self.months_short[(m - 1).clamp(0, 11) as usize];
        if self.code == "en" { format!("{month} '{:02}", y % 100) } else { format!("{month} {:02}", y % 100) }
    }
}
