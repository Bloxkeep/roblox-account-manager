// What every part of the core stands on, as the C# app's parts stood on .NET's own: an id (Guid), a moment in time as
// the vault writes it (DateTime → Stamp), why something failed (UserError and RobloxApiException → Fail), a way to call
// work off (CancellationToken → Cancel). The core is the C# app (RobloxManager/*.cs) moved here file by file: a file
// `Foo.cs` is `core/foo.rs`, `AppController.Foo.cs` is `core/app/foo.rs`, `Forms/Foo.cs` is `core/forms/foo.rs`, each
// with the same parts under the same names (snake_case), so one can be read beside the other.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// C#'s Guid: written "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" in the vault, as there. `Guid::nil()` is Guid.Empty.
pub use uuid::Uuid as Guid;

/// Guid.NewGuid().
pub fn new_guid() -> Guid {
    Guid::new_v4()
}

/// A vault's JSON value this version doesn't know (JsonElement).
pub type Json = serde_json::Value;
/// The fields of a vault object this version doesn't know ([JsonExtensionData]): read and written back untouched.
pub type Extra = serde_json::Map<String, Json>;

/// Why something failed.
#[derive(Debug, Clone)]
pub enum Fail {
    /// Said to the person as it is (C#'s UserError): already in their language.
    User(String),
    /// Roblox refused, or couldn't be reached (C#'s RobloxApiException, flag for flag): `message` is for the person.
    Roblox {
        message: String,
        /// Roblox answered 429: wait a little and try again.
        rate_limited: bool,
        cookie_invalid: bool,
        /// Roblox answered 5xx (down or busy): worth trying again later, unlike a refusal (400, 403).
        server_error: bool,
        /// Roblox refused a password sign-in (wrong password, a locked account…): that attempt can only start over.
        attempt_over: bool,
        /// With `rate_limited`: how long until Roblox takes requests again, when it said.
        retry_after: Option<std::time::Duration>,
        /// Roblox wouldn't give the account a sign-in ticket (4xx, not 401/429): its roblox.com says why.
        sign_in_refused: bool,
        /// Roblox holds the account until a moderation notice is accepted on roblox.com (403, "User is moderated").
        moderated: bool,
    },
    /// Anything else (a file, bad JSON, Windows): logged, never shown as it is.
    Other(String),
}

pub type Res<T> = Result<T, Fail>;

impl Fail {
    pub fn user(text: impl Into<String>) -> Fail {
        Fail::User(text.into())
    }
    pub fn other(text: impl std::fmt::Display) -> Fail {
        Fail::Other(text.to_string())
    }
    /// The sentence of it (C#'s Exception.Message).
    pub fn message(&self) -> &str {
        match self {
            Fail::User(m) | Fail::Other(m) => m,
            Fail::Roblox { message, .. } => message,
        }
    }
    pub fn rate_limited(&self) -> bool {
        matches!(self, Fail::Roblox { rate_limited: true, .. })
    }
    pub fn cookie_invalid(&self) -> bool {
        matches!(self, Fail::Roblox { cookie_invalid: true, .. })
    }
    pub fn server_error(&self) -> bool {
        matches!(self, Fail::Roblox { server_error: true, .. })
    }
    pub fn attempt_over(&self) -> bool {
        matches!(self, Fail::Roblox { attempt_over: true, .. })
    }
    pub fn sign_in_refused(&self) -> bool {
        matches!(self, Fail::Roblox { sign_in_refused: true, .. })
    }
    pub fn moderated(&self) -> bool {
        matches!(self, Fail::Roblox { moderated: true, .. })
    }
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Fail::Roblox { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Fail {}

impl From<std::io::Error> for Fail {
    fn from(e: std::io::Error) -> Fail {
        Fail::Other(e.to_string())
    }
}

impl From<serde_json::Error> for Fail {
    fn from(e: serde_json::Error) -> Fail {
        Fail::Other(e.to_string())
    }
}

/// A busy flag set for as long as this lives: cleared when dropped, so a future dropped mid-await or a worker that
/// panicked can't leave it set for good.
pub struct Busy<'a>(&'a std::cell::Cell<bool>);

impl<'a> Busy<'a> {
    pub fn set(flag: &'a std::cell::Cell<bool>) -> Busy<'a> {
        flag.set(true);
        Busy(flag)
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Runs `f` when dropped (C#'s `finally`): also when the task holding it is dropped mid-await or a worker panicked.
pub struct OnDrop<F: FnOnce()>(Option<F>);

pub fn on_drop<F: FnOnce()>(f: F) -> OnDrop<F> {
    OnDrop(Some(f))
}

impl<F: FnOnce()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

/// Stands for a secret (a password, cookie, key, webhook link) in a hand-written Debug, so `{:?}` in a log or a panic
/// never carries it.
pub struct Redacted;

impl std::fmt::Debug for Redacted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// C#'s `text.Substring(0, max)`, counted in UTF-16 units as C# (and Discord, ntfy, the bot) count; the whole text when
/// shorter. A cut through a surrogate pair stops one unit early: half an emoji can't be held, and from_utf16_lossy
/// would make it U+FFFD.
pub fn cut_utf16(text: &str, max: usize) -> &str {
    let mut units = 0;
    for (i, c) in text.char_indices() {
        units += c.len_utf16();
        if units > max {
            return &text[..i];
        }
    }
    text
}

/// Environment.GetFolderPath(LocalApplicationData): %LOCALAPPDATA%, else the known folder Windows keeps for it, so a
/// process started without that variable still finds the data instead of a folder relative to wherever it runs.
pub fn local_app_data() -> std::path::PathBuf {
    if let Some(local) = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()) {
        return local.into();
    }
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath};
    let mut path: windows_sys::core::PWSTR = std::ptr::null_mut();
    // SAFETY: FOLDERID_LocalAppData is a valid known-folder id; `path` receives a CoTaskMemAlloc'd string freed below.
    let hr = unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, std::ptr::null_mut(), &mut path) };
    let mut text = std::ffi::OsString::new();
    if hr >= 0 && !path.is_null() {
        use std::os::windows::ffi::OsStringExt;
        // SAFETY: `path` is a NUL-terminated wide string from the call above.
        text = unsafe {
            let mut n = 0usize;
            while *path.add(n) != 0 {
                n += 1;
            }
            std::ffi::OsString::from_wide(std::slice::from_raw_parts(path, n))
        };
    }
    if !path.is_null() {
        // SAFETY: freed once, as SHGetKnownFolderPath asks (even when it failed).
        unsafe { CoTaskMemFree(path as *const std::ffi::c_void) };
    }
    text.into()
}

/// C#'s CancellationToken and its source in one: whoever holds a copy can call the work off, and the work asks.
#[derive(Clone, Default, Debug)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    /// Whether this is the very same token as `other` (C#: comparing two CancellationTokenSource references).
    pub fn same(&self, other: &Cancel) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    /// ThrowIfCancellationRequested: an Err the caller passes up with `?`.
    pub fn check(&self) -> Res<()> {
        if self.is_cancelled() { Err(Fail::Other("cancelled".into())) } else { Ok(()) }
    }
    /// Sleeps up to `d`, waking early (false) when called off.
    pub fn sleep(&self, d: std::time::Duration) -> bool {
        let until = std::time::Instant::now() + d;
        while std::time::Instant::now() < until {
            if self.is_cancelled() {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(25).min(until.saturating_duration_since(std::time::Instant::now())));
        }
        !self.is_cancelled()
    }
}

/// How a moment was written (C#'s DateTimeKind): it decides how it is written back, never which moment it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// "…Z".
    Utc,
    /// "…+07:00": minutes ahead of UTC.
    Local(i32),
    /// No zone written: this PC's time, as .NET reads it.
    Unspecified,
}

/// C#'s DateTime as the vault holds it: a moment (100 ns ticks since 1970-01-01 UTC) and how it was written. Two
/// stamps compare by their moment alone.
#[derive(Clone, Copy, Debug)]
pub struct Stamp {
    ticks: i64,
    kind: Kind,
}

const TICKS_PER_SEC: i64 = 10_000_000;

/// The parts of a moment on this PC's clock (weekday: 0 Sunday … 6 Saturday, as .NET's DayOfWeek).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Civil {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub weekday: u32,
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

/// This PC's minutes ahead of UTC, now.
fn local_offset_minutes() -> i32 {
    (crate::time::utc_offset_secs() / 60) as i32
}

/// This PC's minutes ahead of UTC at a moment (seconds since 1970): that date's own summer time, as .NET's DateTime
/// takes it, not today's.
fn offset_at(unix_secs: i64) -> i32 {
    zone_offset(unix_secs, false)
}

/// The same for a time on this PC's clock (its seconds since 1970 read as if UTC). A time skipped or repeated at a
/// clock change gets the offset Windows picks for it.
fn offset_for_local(local_secs: i64) -> i32 {
    zone_offset(local_secs, true)
}

#[cfg(windows)]
fn zone_offset(secs: i64, wall_clock: bool) -> i32 {
    use std::{
        cell::RefCell,
        collections::HashMap,
        time::{Duration, Instant},
    };
    // Statistics asks hundreds of times a frame. Clocks change on a quarter hour, so one answer serves each 15
    // minutes; all are forgotten after a minute (the PC's time zone may have changed).
    thread_local! {
        static CACHE: RefCell<(Option<Instant>, HashMap<(i64, bool), i32>)> = RefCell::new((None, HashMap::new()));
    }
    let key = (secs.div_euclid(900), wall_clock);
    let hit = CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.0.is_none_or(|at| at.elapsed() > Duration::from_secs(60)) || c.1.len() > 8192 {
            *c = (Some(Instant::now()), HashMap::new());
        }
        c.1.get(&key).copied()
    });
    if let Some(m) = hit {
        return m;
    }
    let m = windows_offset(key.0 * 900, wall_clock).unwrap_or_else(local_offset_minutes);
    CACHE.with(|c| c.borrow_mut().1.insert(key, m));
    m
}

#[cfg(not(windows))]
fn zone_offset(_secs: i64, _wall_clock: bool) -> i32 {
    local_offset_minutes()
}

/// Windows' answer for one moment (`wall_clock`: `secs` is this PC's clock); None outside SYSTEMTIME's years.
#[cfg(windows)]
fn windows_offset(secs: i64, wall_clock: bool) -> Option<i32> {
    use windows_sys::Win32::{
        Foundation::SYSTEMTIME,
        System::Time::{DYNAMIC_TIME_ZONE_INFORMATION, GetDynamicTimeZoneInformation, SystemTimeToTzSpecificLocalTimeEx, TzSpecificLocalTimeToSystemTimeEx},
    };
    let (days, rest) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let (y, m, d) = civil_from_days(days);
    if !(1601..=30827).contains(&y) {
        return None;
    }
    let from = SYSTEMTIME {
        wYear: y as u16,
        wMonth: m as u16,
        wDayOfWeek: 0,
        wDay: d as u16,
        wHour: (rest / 3600) as u16,
        wMinute: (rest % 3600 / 60) as u16,
        wSecond: (rest % 60) as u16,
        wMilliseconds: 0,
    };
    // SAFETY: plain structs Windows reads and fills in.
    let mut tz: DYNAMIC_TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetDynamicTimeZoneInformation(&mut tz) } == u32::MAX {
        return None;
    }
    let mut to: SYSTEMTIME = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        if wall_clock {
            TzSpecificLocalTimeToSystemTimeEx(&tz, &from, &mut to)
        } else {
            SystemTimeToTzSpecificLocalTimeEx(&tz, &from, &mut to)
        }
    };
    if ok == 0 {
        return None;
    }
    let to_secs = days_from_civil(to.wYear as i64, to.wMonth as i64, to.wDay as i64) * 86400 + to.wHour as i64 * 3600 + to.wMinute as i64 * 60 + to.wSecond as i64;
    Some(((if wall_clock { secs - to_secs } else { to_secs - secs }) / 60) as i32)
}

impl Stamp {
    /// DateTime.Now.
    pub fn now() -> Stamp {
        Stamp { ticks: Stamp::utc_now().ticks, kind: Kind::Local(local_offset_minutes()) }
    }
    /// DateTime.UtcNow.
    pub fn utc_now() -> Stamp {
        let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        Stamp { ticks: d.as_secs() as i64 * TICKS_PER_SEC + (d.subsec_nanos() / 100) as i64, kind: Kind::Utc }
    }
    pub fn from_unix_secs(secs: i64) -> Stamp {
        Stamp { ticks: secs * TICKS_PER_SEC, kind: Kind::Utc }
    }
    pub fn from_unix_ms(ms: i64) -> Stamp {
        Stamp { ticks: ms * 10_000, kind: Kind::Utc }
    }
    /// A moment by this PC's clock (new DateTime(y, m, d, h, min, s), local).
    pub fn from_local(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Stamp {
        let wall = days_from_civil(year as i64, month as i64, day as i64) * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64;
        let off = offset_for_local(wall);
        Stamp { ticks: (wall - off as i64 * 60) * TICKS_PER_SEC, kind: Kind::Local(off) }
    }
    pub fn kind(self) -> Kind {
        self.kind
    }
    pub fn unix_secs(self) -> i64 {
        self.ticks.div_euclid(TICKS_PER_SEC)
    }
    pub fn unix_ms(self) -> i64 {
        self.ticks.div_euclid(10_000)
    }
    /// ToUniversalTime(): the same moment, written as UTC.
    pub fn to_utc(self) -> Stamp {
        Stamp { ticks: self.ticks, kind: Kind::Utc }
    }
    /// The moment `secs` later (earlier when negative): AddSeconds, AddMinutes(m) is `plus_secs(m * 60.)`…
    pub fn plus_secs(self, secs: f64) -> Stamp {
        Stamp { ticks: self.ticks + (secs * TICKS_PER_SEC as f64).round() as i64, kind: self.kind }
    }
    /// `(self - earlier).TotalSeconds`: below zero when `earlier` is in fact later.
    pub fn secs_since(self, earlier: Stamp) -> f64 {
        (self.ticks - earlier.ticks) as f64 / TICKS_PER_SEC as f64
    }
    /// `(DateTime.UtcNow - self).TotalSeconds`.
    pub fn age_secs(self) -> f64 {
        Stamp::utc_now().secs_since(self)
    }
    /// Its parts on this PC's clock.
    pub fn local(self) -> Civil {
        self.civil(offset_at(self.unix_secs()))
    }
    /// Its parts in UTC.
    pub fn utc(self) -> Civil {
        self.civil(0)
    }
    fn civil(self, offset_minutes: i32) -> Civil {
        let secs = self.unix_secs() + offset_minutes as i64 * 60;
        let (days, rest) = (secs.div_euclid(86400), secs.rem_euclid(86400));
        let (y, m, d) = civil_from_days(days);
        Civil { year: y as i32, month: m as u32, day: d as u32, hour: (rest / 3600) as u32, minute: (rest % 3600 / 60) as u32, second: (rest % 60) as u32, weekday: (days + 4).rem_euclid(7) as u32 }
    }
    /// As .NET's System.Text.Json writes it: "2026-11-11T10:00:00.1234567+07:00", the fraction left out when zero and
    /// without its trailing zeros, "Z" for UTC, no zone for a moment that had none.
    pub fn to_iso(self) -> String {
        let offset = match self.kind {
            Kind::Utc => 0,
            Kind::Local(m) => m,
            Kind::Unspecified => offset_at(self.unix_secs()),
        };
        let c = self.civil(offset);
        let mut out = format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", c.year, c.month, c.day, c.hour, c.minute, c.second);
        let frac = self.ticks.rem_euclid(TICKS_PER_SEC);
        if frac != 0 {
            let digits = format!("{frac:07}");
            out.push('.');
            out.push_str(digits.trim_end_matches('0'));
        }
        match self.kind {
            Kind::Utc => out.push('Z'),
            Kind::Local(m) => out.push_str(&format!("{}{:02}:{:02}", if m < 0 { '-' } else { '+' }, m.abs() / 60, m.abs() % 60)),
            Kind::Unspecified => {}
        }
        out
    }
    /// Reads what `to_iso` (or .NET) wrote; None for anything else.
    pub fn parse(text: &str) -> Option<Stamp> {
        let t = text.trim();
        let (date, rest) = t.split_once(['T', ' '])?;
        let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
        let (y, m, day) = (d.next()??, d.next()??, d.next()??);
        let (clock, kind) = if let Some(c) = rest.strip_suffix(['Z', 'z']) {
            (c, Kind::Utc)
        } else if let Some(i) = rest.rfind(['+', '-']) {
            let (c, zone) = rest.split_at(i);
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let mut z = zone[1..].split(':').map(|p| p.parse::<i32>().ok());
            (c, Kind::Local(sign * (z.next()?? * 60 + z.next().flatten().unwrap_or(0))))
        } else {
            (rest, Kind::Unspecified)
        };
        let (hms, frac) = clock.split_once('.').unwrap_or((clock, ""));
        let mut c = hms.split(':').map(|p| p.parse::<i64>().ok());
        let (h, min, s) = (c.next()??, c.next()??, c.next().flatten().unwrap_or(0));
        let frac_ticks = if frac.is_empty() { 0 } else { format!("{frac:0<7}").get(..7)?.parse::<i64>().ok()? };
        let offset = match kind {
            Kind::Utc => 0,
            Kind::Local(m) => m,
            Kind::Unspecified => offset_for_local(days_from_civil(y, m, day) * 86400 + h * 3600 + min * 60 + s),
        };
        let secs = days_from_civil(y, m, day) * 86400 + h * 3600 + min * 60 + s - offset as i64 * 60;
        Some(Stamp { ticks: secs * TICKS_PER_SEC + frac_ticks, kind })
    }
}

impl PartialEq for Stamp {
    fn eq(&self, other: &Stamp) -> bool {
        self.ticks == other.ticks
    }
}
impl Eq for Stamp {}
impl PartialOrd for Stamp {
    fn partial_cmp(&self, other: &Stamp) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Stamp {
    fn cmp(&self, other: &Stamp) -> std::cmp::Ordering {
        self.ticks.cmp(&other.ticks)
    }
}
impl std::hash::Hash for Stamp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ticks.hash(state);
    }
}

impl Serialize for Stamp {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_iso())
    }
}

impl<'de> Deserialize<'de> for Stamp {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Stamp, D::Error> {
        let text = String::deserialize(d)?;
        Stamp::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("not a date: {text}")))
    }
}
