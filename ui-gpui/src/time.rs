// Times as the app sends them (ISO 8601, UTC) and as the interface says them: a playing time that counts up
// (2:14:12), and how long ago (vừa xong, 5 phút trước, Hôm qua…).
use std::time::{SystemTime, UNIX_EPOCH};

use crate::i18n::I18n;

pub fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// "2026-11-09T07:12:30.123Z" or with "+07:00": seconds since 1970.
pub fn iso_secs(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |from: usize, len: usize| -> Option<i64> { s.get(from..from + len)?.parse().ok() };
    let (y, mo, d) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (h, mi, sec) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    // Days from 1970-01-01 (Howard Hinnant's days_from_civil).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let mut t = days * 86400 + h * 3600 + mi * 60 + sec;
    // An offset after the seconds (and their fraction).
    let mut i = 19;
    while i < b.len() && (b[i] == b'.' || b[i].is_ascii_digit()) {
        i += 1;
    }
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        // "+07:00" or "+0700".
        let minutes_at = if b.get(i + 3) == Some(&b':') { i + 4 } else { i + 3 };
        let off = num(i + 1, 2)? * 3600 + num(minutes_at, 2).unwrap_or(0) * 60;
        t -= if b[i] == b'+' { off } else { -off };
    }
    Some(t)
}

/// 2:14:12, 0:38:16.
pub fn clock(secs: i64) -> String {
    let s = secs.max(0);
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// A short span for a small place: "40s", "17m", "1h 20m" ("17p", "1g20" in Vietnamese, as people write it).
pub fn span(secs: i64) -> String {
    let s = secs.max(0);
    let (h, m) = (s / 3600, s / 60 % 60);
    match (s, english()) {
        (0..60, true) => format!("{s}s"),
        (0..60, false) => format!("{s} giây"),
        (60..3600, true) => format!("{m}m"),
        (60..3600, false) => format!("{m}p"),
        (_, true) if m == 0 => format!("{h}h"),
        (_, true) => format!("{h}h {m}m"),
        (_, false) if m == 0 => format!("{h} giờ"),
        (_, false) => format!("{h}g{m:02}"),
    }
}

/// How long ago, in the app's words.
pub fn ago(secs: i64, i18n: &I18n) -> String {
    let s = secs.max(0);
    match s {
        0..60 => i18n.t("time.now"),
        60..3600 => i18n.tn("time.min", s / 60, &[]),
        3600..86400 => i18n.tn("time.hour", s / 3600, &[]),
        86400..172800 => i18n.t("alog.yesterday"),
        _ => i18n.tn("time.day", s / 86400, &[]),
    }
}

/// The PC's clock ahead of UTC, in seconds (+25200 in Vietnam), summer time included.
pub fn utc_offset_secs() -> i64 {
    // Asked hundreds of times a frame by Statistics: Windows is read at most once a second.
    thread_local! {
        static LAST: std::cell::Cell<Option<(std::time::Instant, i64)>> = const { std::cell::Cell::new(None) };
    }
    if let Some((at, v)) = LAST.with(|l| l.get())
        && at.elapsed() < std::time::Duration::from_secs(1)
    {
        return v;
    }
    let v = read_utc_offset();
    LAST.with(|l| l.set(Some((std::time::Instant::now(), v))));
    v
}

fn read_utc_offset() -> i64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
        // SAFETY: the struct is plain data Windows fills in.
        let mut tz: TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
        let id = unsafe { GetTimeZoneInformation(&mut tz) };
        let bias = tz.Bias + if id == 2 { tz.DaylightBias } else if id == 1 { tz.StandardBias } else { 0 };
        -(bias as i64) * 60
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// Seconds since 1970 as ISO 8601 in UTC: 2026-11-09T07:12:30Z.
pub fn iso(secs: i64) -> String {
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rest / 3_600, rest % 3_600 / 60, rest % 60)
}

/// Whether times and dates are written the English way (the interface's language; i18n.rs sets it).
static ENGLISH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_english(on: bool) {
    ENGLISH.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn english() -> bool {
    ENGLISH.load(std::sync::atomic::Ordering::Relaxed)
}

/// An hour and minute of the day as the interface writes it: "7:30 AM", "12:00 PM" in English (the web page's
/// toLocaleTimeString), "07:30" in Vietnamese.
pub fn hm(h: i64, m: i64) -> String {
    let (h, m) = (h.rem_euclid(24), m.rem_euclid(60));
    if english() {
        let twelve = if h % 12 == 0 { 12 } else { h % 12 };
        format!("{twelve}:{m:02} {}", if h < 12 { "AM" } else { "PM" })
    } else {
        format!("{h:02}:{m:02}")
    }
}

/// "07:30" or "7:30" (a preset's time as kept) the interface's way; the words as they are when they aren't a time.
pub fn hm_text(time: &str) -> String {
    match time.split_once(':').map(|(h, m)| (h.trim().parse::<i64>(), m.trim().parse::<i64>())) {
        Some((Ok(h), Ok(m))) => hm(h, m),
        _ => time.to_string(),
    }
}

/// A day of a month: "12/24" in English (month first, as the web page wrote it), "24/12" in Vietnamese.
pub fn dm(d: i64, m: i64) -> String {
    if english() { format!("{m}/{d}") } else { format!("{d}/{m}") }
}

/// A moment as the day and the time on this PC's clock: 10/11 14:32 (in English 11/10 2:32 PM).
pub fn day_time(secs: i64) -> String {
    let t = secs + utc_offset_secs();
    let (days, rest) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    // The day of the month and the month from the days since 1970 (the civil calendar's arithmetic).
    let z = days + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if english() {
        format!("{m:02}/{d:02} {}", hm(rest / 3_600, rest % 3_600 / 60))
    } else {
        format!("{d:02}/{m:02} {:02}:{:02}", rest / 3_600, rest % 3_600 / 60)
    }
}
