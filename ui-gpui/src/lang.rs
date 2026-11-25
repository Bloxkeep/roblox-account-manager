// The interface's languages and how each writes numbers, times and dates. English and Vietnamese came first; Spanish,
// Portuguese (Brazil), Indonesian, French, German and Russian were added on 2026-11-24 (the owner), then Japanese,
// Korean, Turkish, Polish, Thai and Lao the same day, Filipino and Malay after them. Their words are in
// assets/lang/<code>.json; the names of days and months and the way times are written follow Roblox's own
// (RobloxPlayer's content/configs/DateTimeLocaleConfigs: es-mx, pt-br, fr-fr, de-de, ru-ru, ja-jp, ko-kr); Roblox has
// none for Indonesian, Turkish, Polish, Thai, Lao, Filipino and Malay, which follow Windows' own (CultureInfo: CLDR,
// fil-PH and ms-MY).
//
// Counts work as ICU's MessageFormat plurals do. A word for a count (i18n.rs tv/tn) is looked for in this order:
//   "key1"      exactly one, in its own words when a language has them ("View the window", "Once"; =1 in ICU);
//   "key#one", "key#few", "key#many"
//               the CLDR form the count takes in the language (`Plural`: Russian 21 "#one", 3 "#few", 5 "#many";
//               French and Portuguese 0 "#one");
//   "key"       the rest (English's plural), and any form a language has no words for.
// The app's own sentences (core/l.rs) do the same: the English one sentence for exactly one, and the English plural
// sentence's forms ({"one": …, "few": …, "many": …, "other": …}) for the rest.
use std::sync::atomic::{AtomicUsize, Ordering};

/// A count's form, as CLDR names them (only the ones Bloxkeep's languages use: no zero, two).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plural {
    One,
    Few,
    Many,
    Other,
}

impl Plural {
    /// What a word's key gets for this form ("acc.sel" → "acc.sel#few"); Other is the key itself.
    pub fn suffix(self) -> &'static str {
        match self {
            Plural::One => "#one",
            Plural::Few => "#few",
            Plural::Many => "#many",
            Plural::Other => "",
        }
    }

    /// CLDR's name, as a sentence's forms are kept in assets/lang/<code>.json's "core" ({"one": …, "few": …}).
    pub fn name(self) -> &'static str {
        match self {
            Plural::One => "one",
            Plural::Few => "few",
            Plural::Many => "many",
            Plural::Other => "other",
        }
    }
}

/// CLDR's rules, by family (whole numbers only: Bloxkeep counts things).
pub mod rule {
    use super::Plural;
    /// No forms (Vietnamese, Indonesian, Japanese, Korean, Thai, Lao; Turkish says "5 hesap", one word after a count).
    pub fn none(_: i64) -> Plural {
        Plural::Other
    }
    /// 1 alone (English, Spanish, German).
    pub fn one(n: i64) -> Plural {
        if n.unsigned_abs() == 1 { Plural::One } else { Plural::Other }
    }
    /// 0 and 1 (French, Portuguese).
    pub fn zero_one(n: i64) -> Plural {
        if n.unsigned_abs() <= 1 { Plural::One } else { Plural::Other }
    }
    /// Polish: 1 alone one; 2–4, 22–24 (not 12–14) few; the rest many (0, 5–21, 25–31).
    pub fn polish(n: i64) -> Plural {
        let n = n.unsigned_abs();
        let (ten, hundred) = (n % 10, n % 100);
        if n == 1 {
            Plural::One
        } else if (2..=4).contains(&ten) && !(12..=14).contains(&hundred) {
            Plural::Few
        } else {
            Plural::Many
        }
    }
    /// East Slavic (Russian): 1, 21, 101 one; 2–4, 22–24 few; 0, 5–20, 11–14, 25–30 many.
    pub fn slavic(n: i64) -> Plural {
        let n = n.unsigned_abs();
        let (ten, hundred) = (n % 10, n % 100);
        if ten == 1 && hundred != 11 {
            Plural::One
        } else if (2..=4).contains(&ten) && !(12..=14).contains(&hundred) {
            Plural::Few
        } else {
            Plural::Many
        }
    }
}

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
    /// After a day and month written alone: "." in German (24.12.), else nothing (24.12, 24/12).
    pub dm_end: &'static str,
    pub days_short: [&'static str; 7],
    pub days_long: [&'static str; 7],
    pub months: [&'static str; 12],
    /// A month's name inside a date ("24 декабря": Russian says it "of December"); the same as `months` elsewhere.
    pub months_of: [&'static str; 12],
    pub months_short: [&'static str; 12],
    /// A short month inside a date ("24 мая"); the same as `months_short` elsewhere.
    pub months_short_of: [&'static str; 12],
    /// After a year in a date: " г." in Russian (24 декабря 2026 г.), else nothing.
    pub year_end: &'static str,
    /// A short span's units: seconds, minutes, hours, as "{n}…" (Vietnamese writes them its own way: time::span).
    pub units: (&'static str, &'static str, &'static str),
    /// A day, short, after a number of them ("2d 5h", "2 ngày 5 giờ").
    pub day_abbr: &'static str,
    /// Which form a count takes.
    pub plural: fn(i64) -> Plural,
    /// How old, short (age()): days, months, years, each as (one form, few form, the rest).
    pub ages: [Ages; 3],
    /// Years added to a year as shown: 543 in Thai (the Buddhist era, as Windows writes Thai dates: 2026 is 2569),
    /// else 0.
    pub era: i64,
    /// Sizes' units: bytes, KB, MB, GB (French writes o/Ko/Mo/Go, Russian Б/КБ/МБ/ГБ): size(), mb().
    pub sizes: [&'static str; 4],
}

/// A unit's words by form: one, few, the rest (most languages say the same each time: `same`).
pub type Ages = (&'static str, &'static str, &'static str);

const fn same(w: &'static str) -> Ages {
    (w, w, w)
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
        dm_end: "",
        days_short: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        days_long: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
        months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
        months_of: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
        months_short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
        months_short_of: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
        year_end: "",
        units: ("s", "m", "h"),
        day_abbr: "d",
        plural: rule::one,
        ages: [same("d"), same("mo"), same("y")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
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
        dm_end: "",
        days_short: ["CN", "T2", "T3", "T4", "T5", "T6", "T7"],
        days_long: ["Chủ Nhật", "Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy"],
        months: ["tháng 1", "tháng 2", "tháng 3", "tháng 4", "tháng 5", "tháng 6", "tháng 7", "tháng 8", "tháng 9", "tháng 10", "tháng 11", "tháng 12"],
        months_of: ["tháng 1", "tháng 2", "tháng 3", "tháng 4", "tháng 5", "tháng 6", "tháng 7", "tháng 8", "tháng 9", "tháng 10", "tháng 11", "tháng 12"],
        months_short: ["Th1", "Th2", "Th3", "Th4", "Th5", "Th6", "Th7", "Th8", "Th9", "Th10", "Th11", "Th12"],
        months_short_of: ["Th1", "Th2", "Th3", "Th4", "Th5", "Th6", "Th7", "Th8", "Th9", "Th10", "Th11", "Th12"],
        year_end: "",
        units: (" giây", "p", " giờ"),
        day_abbr: " ngày",
        plural: rule::none,
        ages: [same(" ngày"), same(" tháng"), same(" năm")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
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
        dm_end: "",
        days_short: ["dom.", "lun.", "mar.", "mié.", "jue.", "vie.", "sáb."],
        days_long: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
        months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
        months_of: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
        months_short: ["ene.", "feb.", "mar.", "abr.", "may.", "jun.", "jul.", "ago.", "sep.", "oct.", "nov.", "dic."],
        months_short_of: ["ene.", "feb.", "mar.", "abr.", "may.", "jun.", "jul.", "ago.", "sep.", "oct.", "nov.", "dic."],
        year_end: "",
        units: (" s", " min", " h"),
        day_abbr: " d",
        plural: rule::one,
        ages: [same(" d"), (" mes", " meses", " meses"), (" año", " años", " años")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
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
        dm_end: "",
        days_short: ["dom.", "seg.", "ter.", "qua.", "qui.", "sex.", "sáb."],
        days_long: ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
        months: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
        months_of: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
        months_short: ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."],
        months_short_of: ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."],
        year_end: "",
        units: (" s", " min", " h"),
        day_abbr: " d",
        plural: rule::zero_one,
        ages: [same(" d"), (" mês", " meses", " meses"), (" ano", " anos", " anos")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
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
        dm_end: "",
        days_short: ["Min", "Sen", "Sel", "Rab", "Kam", "Jum", "Sab"],
        days_long: ["Minggu", "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu"],
        months: ["Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September", "Oktober", "November", "Desember"],
        months_of: ["Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September", "Oktober", "November", "Desember"],
        months_short: ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des"],
        months_short_of: ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des"],
        year_end: "",
        units: (" dtk", " mnt", " j"),
        day_abbr: " hr",
        plural: rule::none,
        ages: [same(" hr"), same(" bln"), same(" thn")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
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
        dm_end: "",
        days_short: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
        days_long: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
        months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
        months_of: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
        months_short: ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
        months_short_of: ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
        year_end: "",
        units: ("\u{a0}s", "\u{a0}min", "\u{a0}h"),
        day_abbr: "\u{a0}j",
        plural: rule::zero_one,
        ages: [same("\u{a0}j"), same("\u{a0}mois"), ("\u{a0}an", "\u{a0}ans", "\u{a0}ans")],
        era: 0,
        sizes: ["o", "Ko", "Mo", "Go"],
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
        dm_end: ".",
        days_short: ["So.", "Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa."],
        days_long: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
        months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
        months_of: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
        months_short: ["Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sep.", "Okt.", "Nov.", "Dez."],
        months_short_of: ["Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sep.", "Okt.", "Nov.", "Dez."],
        year_end: "",
        units: (" s", " Min.", " Std."),
        day_abbr: " T.",
        plural: rule::one,
        ages: [same(" T."), same(" Mon."), same(" J.")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
    },
    Lang {
        code: "ru",
        name: "Русский",
        word: "Язык",
        group: '\u{a0}',
        decimal: ',',
        meridiem: None,
        time_sep: ':',
        day_first: true,
        date_sep: '.',
        dm_end: "",
        days_short: ["вс", "пн", "вт", "ср", "чт", "пт", "сб"],
        days_long: ["воскресенье", "понедельник", "вторник", "среда", "четверг", "пятница", "суббота"],
        months: ["январь", "февраль", "март", "апрель", "май", "июнь", "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь"],
        months_of: ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"],
        months_short: ["янв.", "февр.", "март", "апр.", "май", "июнь", "июль", "авг.", "сент.", "окт.", "нояб.", "дек."],
        months_short_of: ["янв.", "февр.", "мар.", "апр.", "мая", "июн.", "июл.", "авг.", "сент.", "окт.", "нояб.", "дек."],
        year_end: "\u{a0}г.",
        units: ("\u{a0}с", "\u{a0}мин", "\u{a0}ч"),
        day_abbr: "\u{a0}д",
        plural: rule::slavic,
        ages: [same("\u{a0}д"), same("\u{a0}мес."), ("\u{a0}г.", "\u{a0}г.", "\u{a0}л.")],
        era: 0,
        sizes: ["Б", "КБ", "МБ", "ГБ"],
    },
    Lang {
        code: "ja",
        name: "日本語",
        word: "言語",
        group: ',',
        decimal: '.',
        meridiem: None,
        time_sep: ':',
        day_first: false,
        date_sep: '/',
        dm_end: "",
        days_short: ["日", "月", "火", "水", "木", "金", "土"],
        days_long: ["日曜日", "月曜日", "火曜日", "水曜日", "木曜日", "金曜日", "土曜日"],
        months: ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
        months_of: ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
        months_short: ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
        months_short_of: ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
        year_end: "",
        units: ("秒", "分", "時間"),
        day_abbr: "日",
        plural: rule::none,
        ages: [same("日"), same("か月"), same("年")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
    },
    Lang {
        code: "ko",
        name: "한국어",
        word: "언어",
        group: ',',
        decimal: '.',
        meridiem: None,
        time_sep: ':',
        day_first: false,
        date_sep: '/',
        dm_end: "",
        days_short: ["일", "월", "화", "수", "목", "금", "토"],
        days_long: ["일요일", "월요일", "화요일", "수요일", "목요일", "금요일", "토요일"],
        months: ["1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월"],
        months_of: ["1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월"],
        months_short: ["1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월"],
        months_short_of: ["1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월"],
        year_end: "",
        units: ("초", "분", "시간"),
        day_abbr: "일",
        plural: rule::none,
        ages: [same("일"), same("개월"), same("년")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
    },
    Lang {
        code: "tr",
        name: "Türkçe",
        word: "Dil",
        group: '.',
        decimal: ',',
        meridiem: None,
        time_sep: ':',
        day_first: true,
        date_sep: '.',
        dm_end: "",
        days_short: ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"],
        days_long: ["Pazar", "Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi"],
        months: ["Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık"],
        months_of: ["Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık"],
        months_short: ["Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara"],
        months_short_of: ["Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara"],
        year_end: "",
        units: (" sn", " dk", " sa"),
        day_abbr: " g",
        plural: rule::none,
        ages: [same(" g"), same(" ay"), same(" yıl")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
    },
    Lang {
        code: "pl",
        name: "Polski",
        word: "Język",
        group: '\u{a0}',
        decimal: ',',
        meridiem: None,
        time_sep: ':',
        day_first: true,
        date_sep: '.',
        dm_end: "",
        days_short: ["niedz.", "pon.", "wt.", "śr.", "czw.", "pt.", "sob."],
        days_long: ["niedziela", "poniedziałek", "wtorek", "środa", "czwartek", "piątek", "sobota"],
        months: ["styczeń", "luty", "marzec", "kwiecień", "maj", "czerwiec", "lipiec", "sierpień", "wrzesień", "październik", "listopad", "grudzień"],
        months_of: ["stycznia", "lutego", "marca", "kwietnia", "maja", "czerwca", "lipca", "sierpnia", "września", "października", "listopada", "grudnia"],
        months_short: ["sty", "lut", "mar", "kwi", "maj", "cze", "lip", "sie", "wrz", "paź", "lis", "gru"],
        months_short_of: ["sty", "lut", "mar", "kwi", "maj", "cze", "lip", "sie", "wrz", "paź", "lis", "gru"],
        year_end: "",
        units: ("\u{a0}s", "\u{a0}min", "\u{a0}godz."),
        day_abbr: "\u{a0}d",
        plural: rule::polish,
        ages: [same("\u{a0}d"), same("\u{a0}mies."), ("\u{a0}rok", "\u{a0}lata", "\u{a0}lat")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
    },
    Lang {
        code: "th",
        name: "ไทย",
        word: "ภาษา",
        group: ',',
        decimal: '.',
        meridiem: None,
        time_sep: ':',
        day_first: true,
        date_sep: '/',
        dm_end: "",
        days_short: ["อา.", "จ.", "อ.", "พ.", "พฤ.", "ศ.", "ส."],
        days_long: ["วันอาทิตย์", "วันจันทร์", "วันอังคาร", "วันพุธ", "วันพฤหัสบดี", "วันศุกร์", "วันเสาร์"],
        months: ["มกราคม", "กุมภาพันธ์", "มีนาคม", "เมษายน", "พฤษภาคม", "มิถุนายน", "กรกฎาคม", "สิงหาคม", "กันยายน", "ตุลาคม", "พฤศจิกายน", "ธันวาคม"],
        months_of: ["มกราคม", "กุมภาพันธ์", "มีนาคม", "เมษายน", "พฤษภาคม", "มิถุนายน", "กรกฎาคม", "สิงหาคม", "กันยายน", "ตุลาคม", "พฤศจิกายน", "ธันวาคม"],
        months_short: ["ม.ค.", "ก.พ.", "มี.ค.", "เม.ย.", "พ.ค.", "มิ.ย.", "ก.ค.", "ส.ค.", "ก.ย.", "ต.ค.", "พ.ย.", "ธ.ค."],
        months_short_of: ["ม.ค.", "ก.พ.", "มี.ค.", "เม.ย.", "พ.ค.", "มิ.ย.", "ก.ค.", "ส.ค.", "ก.ย.", "ต.ค.", "พ.ย.", "ธ.ค."],
        year_end: "",
        units: (" วิ", " นาที", " ชม."),
        day_abbr: " วัน",
        plural: rule::none,
        ages: [same(" วัน"), same(" เดือน"), same(" ปี")],
        era: 543,
        sizes: ["B", "KB", "MB", "GB"],
    },
    Lang {
        code: "lo",
        name: "ລາວ",
        word: "ພາສາ",
        group: '.',
        decimal: ',',
        meridiem: None,
        time_sep: ':',
        day_first: true,
        date_sep: '/',
        dm_end: "",
        days_short: ["ອາທິດ", "ຈັນ", "ອັງຄານ", "ພຸດ", "ພະຫັດ", "ສຸກ", "ເສົາ"],
        days_long: ["ວັນອາທິດ", "ວັນຈັນ", "ວັນອັງຄານ", "ວັນພຸດ", "ວັນພະຫັດ", "ວັນສຸກ", "ວັນເສົາ"],
        months: ["ມັງກອນ", "ກຸມພາ", "ມີນາ", "ເມສາ", "ພຶດສະພາ", "ມິຖຸນາ", "ກໍລະກົດ", "ສິງຫາ", "ກັນຍາ", "ຕຸລາ", "ພະຈິກ", "ທັນວາ"],
        months_of: ["ມັງກອນ", "ກຸມພາ", "ມີນາ", "ເມສາ", "ພຶດສະພາ", "ມິຖຸນາ", "ກໍລະກົດ", "ສິງຫາ", "ກັນຍາ", "ຕຸລາ", "ພະຈິກ", "ທັນວາ"],
        months_short: ["ມ.ກ.", "ກ.ພ.", "ມ.ນ.", "ມ.ສ.", "ພ.ພ.", "ມິ.ຖ.", "ກ.ລ.", "ສ.ຫ.", "ກ.ຍ.", "ຕ.ລ.", "ພ.ຈ.", "ທ.ວ."],
        months_short_of: ["ມ.ກ.", "ກ.ພ.", "ມ.ນ.", "ມ.ສ.", "ພ.ພ.", "ມິ.ຖ.", "ກ.ລ.", "ສ.ຫ.", "ກ.ຍ.", "ຕ.ລ.", "ພ.ຈ.", "ທ.ວ."],
        year_end: "",
        units: (" ວິ", " ນທ", " ຊມ"),
        day_abbr: " ມື້",
        plural: rule::none,
        ages: [same(" ມື້"), same(" ເດືອນ"), same(" ປີ")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
    },
    Lang {
        code: "fil",
        name: "Filipino",
        word: "Wika",
        group: ',',
        decimal: '.',
        meridiem: Some(("AM", "PM")),
        time_sep: ':',
        day_first: false,
        date_sep: '/',
        dm_end: "",
        days_short: ["Lin", "Lun", "Mar", "Miy", "Huw", "Biy", "Sab"],
        days_long: ["Linggo", "Lunes", "Martes", "Miyerkules", "Huwebes", "Biyernes", "Sabado"],
        months: ["Enero", "Pebrero", "Marso", "Abril", "Mayo", "Hunyo", "Hulyo", "Agosto", "Setyembre", "Oktubre", "Nobyembre", "Disyembre"],
        months_of: ["Enero", "Pebrero", "Marso", "Abril", "Mayo", "Hunyo", "Hulyo", "Agosto", "Setyembre", "Oktubre", "Nobyembre", "Disyembre"],
        months_short: ["Ene", "Peb", "Mar", "Abr", "May", "Hun", "Hul", "Ago", "Set", "Okt", "Nob", "Dis"],
        months_short_of: ["Ene", "Peb", "Mar", "Abr", "May", "Hun", "Hul", "Ago", "Set", "Okt", "Nob", "Dis"],
        year_end: "",
        units: (" seg", " min", " oras"),
        day_abbr: " araw",
        // (Filipino says "1 account", "5 account": one word after any count.)
        plural: rule::none,
        ages: [same(" araw"), same(" buwan"), same(" taon")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
    },
    Lang {
        code: "ms",
        name: "Bahasa Melayu",
        word: "Bahasa",
        group: ',',
        decimal: '.',
        meridiem: Some(("PG", "PTG")),
        time_sep: ':',
        day_first: true,
        date_sep: '/',
        dm_end: "",
        days_short: ["Ahd", "Isn", "Sel", "Rab", "Kha", "Jum", "Sab"],
        days_long: ["Ahad", "Isnin", "Selasa", "Rabu", "Khamis", "Jumaat", "Sabtu"],
        months: ["Januari", "Februari", "Mac", "April", "Mei", "Jun", "Julai", "Ogos", "September", "Oktober", "November", "Disember"],
        months_of: ["Januari", "Februari", "Mac", "April", "Mei", "Jun", "Julai", "Ogos", "September", "Oktober", "November", "Disember"],
        months_short: ["Jan", "Feb", "Mac", "Apr", "Mei", "Jun", "Jul", "Ogo", "Sep", "Okt", "Nov", "Dis"],
        months_short_of: ["Jan", "Feb", "Mac", "Apr", "Mei", "Jun", "Jul", "Ogo", "Sep", "Okt", "Nov", "Dis"],
        year_end: "",
        units: (" s", " min", " j"),
        day_abbr: " h",
        plural: rule::none,
        ages: [same(" hari"), same(" bln"), same(" thn")],
        era: 0,
        sizes: ["B", "KB", "MB", "GB"],
    },
];

/// The language shown (an index into ALL).
static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// The language with this code; None for one not here.
pub fn by_code(code: &str) -> Option<&'static Lang> {
    let code = code.trim().to_ascii_lowercase().replace('_', "-");
    // ("pt-br", "es-419": the part before the dash.)
    let code = code.split('-').next().unwrap_or("");
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

    /// An amount of `mb` megabytes, the app's one way of writing RAM: "512 MB", then GB with at most one decimal
    /// ("1.7 GB", "2 GB"), in the language's decimal mark and units ("1,7 ГБ", "1,7 Go").
    pub fn mb(&self, mb: f64) -> String {
        if mb >= 1024. {
            let gb = (mb / 1024. * 10.).round() / 10.;
            let n = if gb.fract() == 0. { format!("{gb:.0}") } else { format!("{gb:.1}") };
            format!("{}\u{a0}{}", self.decimal(&n), self.sizes[3])
        } else {
            format!("{:.0}\u{a0}{}", mb.max(0.), self.sizes[2])
        }
    }

    /// A file's size from bytes: "820 B", "3.4 KB" (a decimal under 10 KB), "56 KB", "1.2 MB".
    pub fn size(&self, bytes: f64) -> String {
        let (n, unit) = if bytes < 1024. {
            (format!("{bytes:.0}"), 0)
        } else if bytes < 10_240. {
            (format!("{:.1}", bytes / 1024.), 1)
        } else if bytes < 1_048_576. {
            (format!("{:.0}", bytes / 1024.), 1)
        } else {
            (format!("{:.1}", bytes / 1_048_576.), 2)
        };
        format!("{}\u{a0}{}", self.decimal(&n), self.sizes[unit])
    }

    /// How old, short, from days: "3d", "5mo", "2y" in English; "3 ngày", "5 tháng"; "5 meses", "1 año".
    pub fn age(&self, days: i64) -> String {
        let (n, unit) = if days < 30 { (days.max(1), 0) } else if days < 365 { (days / 30, 1) } else { (days / 365, 2) };
        let (one, few, rest) = self.ages[unit];
        let word = match self.form(n) {
            Plural::One => one,
            Plural::Few => few,
            _ => rest,
        };
        format!("{n}{word}")
    }

    /// The form a count takes (CLDR).
    pub fn form(&self, n: i64) -> Plural {
        (self.plural)(n)
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

    /// Japanese or Korean: dates go year, month, day with a unit after each.
    pub fn east_asian(&self) -> bool {
        matches!(self.code, "ja" | "ko")
    }

    /// A day and month: 12/24, 24/12, 24.12.
    pub fn dm(&self, d: i64, m: i64) -> String {
        match (self.day_first, self.date_sep) {
            (false, s) => format!("{m}{s}{d}"),
            (true, s) => format!("{d}{s}{m}{}", self.dm_end),
        }
    }

    /// A whole date in numbers: 12/24/2026, 24/12/2026, 24.12.2026.
    pub fn dmy(&self, d: i64, m: i64, y: i64) -> String {
        let (s, y) = (self.date_sep, y + self.era);
        if self.east_asian() {
            return format!("{y}{s}{m:02}{s}{d:02}");
        }
        if self.day_first { format!("{d:02}{s}{m:02}{s}{y}") } else { format!("{m:02}{s}{d:02}{s}{y}") }
    }

    /// A date in words: December 24, 2026 / 24 de diciembre de 2026 / 24. Dezember 2026 / 24 tháng 12, 2026 /
    /// 24 декабря 2026 г. / 2026年12月24日 / 2026년 12월 24일.
    pub fn date_long(&self, d: i64, m: i64, y: i64) -> String {
        let (month, y) = (self.months_of[(m - 1).clamp(0, 11) as usize], format!("{}{}", y + self.era, self.year_end));
        match self.code {
            "en" => format!("{month} {d}, {y}"),
            "vi" => format!("{d} {month}, {y}"),
            "ja" => format!("{y}年{m}月{d}日"),
            "ko" => format!("{y}년 {m}월 {d}일"),
            "es" | "pt" => format!("{d} de {month} de {y}"),
            "de" => format!("{d}. {month} {y}"),
            _ => format!("{d} {month} {y}"),
        }
    }

    /// A date in words without the year: December 24 / 24 de diciembre / 24. Dezember / 24 декабря.
    pub fn day_month(&self, d: i64, m: i64) -> String {
        let month = self.months_of[(m - 1).clamp(0, 11) as usize];
        match self.code {
            "en" => format!("{month} {d}"),
            "ja" => format!("{m}月{d}日"),
            "ko" => format!("{m}월 {d}일"),
            "es" | "pt" => format!("{d} de {month}"),
            "de" => format!("{d}. {month}"),
            _ => format!("{d} {month}"),
        }
    }

    /// A short date with the month's short name: Dec 24, 2026 / 24 dic. 2026 / 24. Dez. 2026.
    pub fn date_short(&self, d: i64, m: i64, y: Option<i64>) -> String {
        let y = y.map(|y| y + self.era);
        if self.code == "vi" {
            return match y {
                Some(y) => format!("{d:02}/{m:02}/{y}"),
                None => format!("{d:02}/{m:02}"),
            };
        }
        if self.east_asian() {
            return match y {
                Some(y) => format!("{y}/{m}/{d}"),
                None => format!("{m}/{d}"),
            };
        }
        let month = self.months_short_of[(m - 1).clamp(0, 11) as usize];
        let base = match self.code {
            "en" => format!("{month} {d}"),
            "de" => format!("{d}. {month}"),
            _ => format!("{d} {month}"),
        };
        match (y, self.code) {
            (Some(y), "en") => format!("{base}, {y}"),
            (Some(y), _) => format!("{base} {y}{}", self.year_end),
            (None, _) => base,
        }
    }

    /// A month and year, for a chart's marks: Dec '26 / dic. 26 / 12/2026.
    pub fn month_year(&self, m: i64, y: i64) -> String {
        let y = y + self.era;
        if self.code == "vi" {
            return format!("{m}/{y}");
        }
        if self.east_asian() {
            return format!("{y}/{m}");
        }
        let month = self.months_short[(m - 1).clamp(0, 11) as usize];
        if self.code == "en" { format!("{month} '{:02}", y % 100) } else { format!("{month} {:02}", y % 100) }
    }
}
