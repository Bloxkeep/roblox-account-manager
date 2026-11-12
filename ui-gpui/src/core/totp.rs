// Totp.cs: time-based one-time codes (RFC 6238) from an account's authenticator key.
use serde::{Deserialize, Serialize};

use crate::core::base::{Extra, Stamp};
use crate::core::roblox_api::unescape_data_string;

/// An account's authenticator key as the vault keeps it: the shared secret and how codes are made from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct TotpKey {
    /// Base32 (A–Z, 2–7), upper case, without spaces or padding.
    pub secret: String,
    pub digits: i32,
    /// Seconds each code lasts.
    pub period: i32,
    /// "SHA1" (Roblox's, and most sites'), "SHA256" or "SHA512".
    pub algorithm: String,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for TotpKey {
    fn default() -> Self {
        TotpKey { secret: String::new(), digits: 6, period: 30, algorithm: "SHA1".into(), extra: Extra::new() }
    }
}

/// Why a pasted key was refused; the app words it for the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum TotpProblem {
    #[default]
    None,
    NotAKey,
    BadCharacter,
    TooShort,
    CutOff,
    NotTimeBased,
    NoSecret,
    Unsupported,
}

const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
/// 80 bits, the shortest secret sites hand out (RFC 4226 asks for 128 or more): anything shorter was cut off.
const MIN_SECRET_CHARS: usize = 16;
const MAX_SECRET_CHARS: usize = 256;

/// Reads a key as people paste it: base32 (any case, spaces or dashes) or an otpauth://totp/… link. A typo
/// base32 can't hold is refused; one that is still base32 can't be told, so the edit form shows the code it makes.
/// Returns the key, why it was refused, and the detail: the character refused (BadCharacter) or the setting (Unsupported).
pub fn parse(input: Option<&str>) -> (Option<TotpKey>, TotpProblem, String) {
    let text = input.unwrap_or("").trim();
    let mut key = TotpKey::default();
    let mut secret = text.to_string();
    if text.get(..8).is_some_and(|p| p.eq_ignore_ascii_case("otpauth:")) {
        // PORT: Uri.TryCreate's absolute-URI rules are .NET's own; the url crate's (WHATWG) stand in.
        let Ok(uri) = url::Url::parse(text) else { return fail(TotpProblem::NotAKey) };
        let host = uri.host_str().unwrap_or("");
        if host.eq_ignore_ascii_case("hotp") {
            return fail(TotpProblem::NotTimeBased);
        }
        if !host.eq_ignore_ascii_case("totp") {
            return fail(TotpProblem::NotAKey);
        }
        let query = query(uri.query().unwrap_or(""));
        let get = |name: &str| query.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
        let Some(from_link) = get("secret").filter(|s| !s.is_empty()) else { return fail(TotpProblem::NoSecret) };
        secret = from_link;
        if let Some(digits) = get("digits") {
            match number_none_style(&digits).filter(|n| (6..=8).contains(n)) {
                Some(n) => key.digits = n,
                None => return unsupported(&format!("digits={digits}")),
            }
        }
        if let Some(period) = get("period") {
            match number_none_style(&period).filter(|s| (10..=300).contains(s)) {
                Some(s) => key.period = s,
                None => return unsupported(&format!("period={period}")),
            }
        }
        if let Some(algorithm) = get("algorithm") {
            let name = upper_invariant(&algorithm.replace('-', ""));
            if !matches!(name.as_str(), "SHA1" | "SHA256" | "SHA512") {
                return unsupported(&format!("algorithm={algorithm}"));
            }
            key.algorithm = name;
        }
    }

    // Sites group the key in fours with spaces (or dashes), and base32 may end in "=" padding.
    let secret: String = secret.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    let secret = upper_invariant(secret.trim_end_matches('='));
    if secret.is_empty() {
        return fail(TotpProblem::NotAKey);
    }
    for c in secret.chars() {
        // "0", "1" and "8" are the usual slips (for O, I or L, and B): base32 leaves them out for that reason.
        if !ALPHABET.contains(c) {
            return (None, TotpProblem::BadCharacter, c.to_string());
        }
    }
    if secret.len() > MAX_SECRET_CHARS {
        return fail(TotpProblem::NotAKey);
    }
    if secret.len() < MIN_SECRET_CHARS {
        return fail(TotpProblem::TooShort);
    }
    // Base32 lengths that hold no whole number of bytes: a character went missing (or one too many).
    if matches!(secret.len() % 8, 1 | 3 | 6) {
        return fail(TotpProblem::CutOff);
    }
    key.secret = secret;
    (Some(key), TotpProblem::None, String::new())
}

/// ToUpperInvariant: a character whose upper case is more than one (ß) stays as it is.
fn upper_invariant(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut up = c.to_uppercase();
            match (up.next(), up.next()) {
                (Some(u), None) => u,
                _ => c,
            }
        })
        .collect()
}

/// int.TryParse with NumberStyles.None: digits only.
fn number_none_style(s: &str) -> Option<i32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn fail(why: TotpProblem) -> (Option<TotpKey>, TotpProblem, String) {
    (None, why, String::new())
}

fn unsupported(setting: &str) -> (Option<TotpKey>, TotpProblem, String) {
    let units: Vec<u16> = setting.encode_utf16().collect();
    let detail = if units.len() > 40 { format!("{}…", String::from_utf16_lossy(&units[..40])) } else { setting.to_string() };
    (None, TotpProblem::Unsupported, detail)
}

/// The query of an otpauth link, names in lower case; the first of a name repeated wins.
fn query(query: &str) -> Vec<(String, String)> {
    let mut values: Vec<(String, String)> = Vec::new();
    for pair in query.trim_start_matches('?').split('&').filter(|p| !p.is_empty()) {
        let eq = pair.find('=');
        let name = unescape_data_string(&eq.map_or(pair, |e| &pair[..e]).replace('+', " ")).to_lowercase();
        let value = eq.map_or(String::new(), |e| unescape_data_string(&pair[e + 1..].replace('+', " ")));
        if !values.iter().any(|(n, _)| *n == name) {
            values.push((name, value));
        }
    }
    values
}

/// The code an authenticator app with this key shows at `at`.
pub fn code(key: &TotpKey, at: Stamp) -> String {
    let mut secret = base32(&key.secret);
    let result = code_at(&secret, at.unix_secs() / period_of(key) as i64, digits_of(key), &key.algorithm);
    zero_memory(&mut secret);
    result
}

/// Seconds until the code at `at` changes (1 to the period).
pub fn seconds_left(key: &TotpKey, at: Stamp) -> i32 {
    period_of(key) - (at.unix_secs() % period_of(key) as i64) as i32
}

/// The period and length as parse allows (10-300 s, 6-8 digits): a vault edited by hand may hold anything, and
/// a period of 0 would divide by zero.
fn period_of(key: &TotpKey) -> i32 {
    if (10..=300).contains(&key.period) { key.period } else { 30 }
}

fn digits_of(key: &TotpKey) -> i32 {
    if (6..=8).contains(&key.digits) { key.digits } else { 6 }
}

/// RFC 4226's HOTP for one counter value (RFC 6238 feeds it the time step): dynamic truncation, then the last digits.
pub fn code_at(secret: &[u8], step: i64, digits: i32, algorithm: &str) -> String {
    let counter = step.to_be_bytes();
    let hash = match algorithm {
        "SHA256" => hmac(64, |d| sha2_digest::<sha2::Sha256>(d), secret, &counter),
        "SHA512" => hmac(128, |d| sha2_digest::<sha2::Sha512>(d), secret, &counter),
        _ => hmac(64, |d| sha1_smol::Sha1::from(d).digest().bytes().to_vec(), secret, &counter),
    };
    let offset = (hash[hash.len() - 1] & 0x0F) as usize;
    let binary = ((hash[offset] & 0x7F) as i32) << 24 | (hash[offset + 1] as i32) << 16 | (hash[offset + 2] as i32) << 8 | hash[offset + 3] as i32;
    let modulo = match digits {
        7 => 10_000_000,
        8 => 100_000_000,
        _ => 1_000_000,
    };
    format!("{:0>width$}", binary % modulo, width = digits.max(0) as usize)
}

fn sha2_digest<D: sha2::Digest>(data: &[u8]) -> Vec<u8> {
    D::digest(data).to_vec()
}

/// HMAC (RFC 2104) over a hash with the given block size, as HMACSHA1/256/512.HashData.
fn hmac(block: usize, hash: impl Fn(&[u8]) -> Vec<u8>, key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut k = if key.len() > block { hash(key) } else { key.to_vec() };
    k.resize(block, 0);
    let mut inner: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    inner.extend_from_slice(message);
    let mut inner_hash = hash(&inner);
    let mut outer: Vec<u8> = k.iter().map(|b| b ^ 0x5C).collect();
    outer.extend_from_slice(&inner_hash);
    let result = hash(&outer);
    zero_memory(&mut k);
    zero_memory(&mut inner);
    zero_memory(&mut outer);
    zero_memory(&mut inner_hash);
    result
}

/// CryptographicOperations.ZeroMemory: writes the compiler doesn't drop.
fn zero_memory(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        // SAFETY: `b` is a valid, exclusive reference to one byte.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

/// Base32 (RFC 4648) to bytes; leftover bits at the end are dropped, as authenticator apps do.
fn base32(text: &str) -> Vec<u8> {
    // Length in UTF-16 units, as the C# counts it.
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut bytes = vec![0u8; units.len() * 5 / 8];
    let (mut buffer, mut bits, mut index) = (0i32, 0i32, 0usize);
    for c in units {
        // A character outside the alphabet is -1 here, as in the C# (only a vault edited by hand can hold one).
        let value = char::from_u32(c as u32).and_then(|c| ALPHABET.find(c)).map_or(-1, |i| i as i32);
        buffer = buffer.wrapping_shl(5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            bytes[index] = (buffer >> bits) as u8;
            index += 1;
            buffer &= (1 << bits) - 1;
        }
    }
    bytes
}
