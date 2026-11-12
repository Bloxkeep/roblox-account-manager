// Tests/Bloxkeep.Tests/TotpSuite.cs: the sign-in codes (totp.rs) against the published vectors: RFC 4226's table
// (counter 0 to 9) and RFC 6238 Appendix B (8 digits, SHA1, SHA256 and SHA512), the seconds left on a code, every way a
// pasted key is refused, and a vault edited by hand to a period or length that would divide by zero. Only arithmetic.
use super::Checks;
use crate::core::base::Stamp;
use crate::core::totp::{self, TotpKey, TotpProblem};

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// RFC 4648 base32 without padding, as the sites hand keys out.
fn base32(bytes: &[u8]) -> String {
    let mut text = String::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for &b in bytes {
        buffer = (buffer << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            text.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
            buffer &= (1 << bits) - 1;
        }
    }
    if bits > 0 {
        text.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    text
}

fn key_of(ascii: &str, algorithm: &str) -> TotpKey {
    TotpKey { secret: base32(ascii.as_bytes()), digits: 8, period: 30, algorithm: algorithm.into(), ..Default::default() }
}

fn at(unix: i64) -> Stamp {
    Stamp::from_unix_secs(unix)
}

fn parse(text: &str) -> (Option<TotpKey>, TotpProblem, String) {
    totp::parse(Some(text))
}

pub async fn run(c: &mut Checks, _dir: &str) {
    // RFC 4226 Appendix D: the key "12345678901234567890", six digits, counters 0 to 9.
    let sha1_key = b"12345678901234567890";
    let hotp = ["755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583", "399871", "520489"];
    let got: Vec<String> = (0..10).map(|n| totp::code_at(sha1_key, n, 6, "SHA1")).collect();
    c.check("RFC 4226 Appendix D: the ten counter values", got == hotp, got.join(","));

    // RFC 6238 Appendix B: 8 digits, 30 s steps; the SHA256 key is 32 bytes and the SHA512 key 64.
    let k1 = key_of("12345678901234567890", "SHA1");
    let k256 = key_of("12345678901234567890123456789012", "SHA256");
    let k512 = key_of("1234567890123456789012345678901234567890123456789012345678901234", "SHA512");
    let table: [(i64, &str, &str, &str); 6] = [
        (59, "94287082", "46119246", "90693936"),
        (1111111109, "07081804", "68084774", "25091201"),
        (1111111111, "14050471", "67062674", "99943326"),
        (1234567890, "89005924", "91819424", "93441116"),
        (2000000000, "69279037", "90698825", "38618901"),
        (20000000000, "65353130", "77737706", "47863826"),
    ];
    for (time, a, b, d) in table {
        let made = [totp::code(&k1, at(time)), totp::code(&k256, at(time)), totp::code(&k512, at(time))];
        c.check(&format!("RFC 6238 Appendix B at T={time}: SHA1, SHA256 and SHA512"), made == [a, b, d], made.join(" "));
    }
    let (p, problem, _) = parse(&base32(b"1234567890123456789012345678901234567890123456789012345678901234"));
    c.check("the SHA512 key (64 bytes, 103 letters) is one Parse takes", p.is_some() && problem == TotpProblem::None, problem);

    // Six digits are the last six of the eight (the modulo), as the authenticator apps show them.
    let six = TotpKey { secret: k1.secret.clone(), ..Default::default() };
    c.check("six digits: the last six of the eight (287082)", totp::code(&six, at(59)) == "287082", totp::code(&six, at(59)));
    let seven = TotpKey { secret: k1.secret.clone(), digits: 7, ..Default::default() };
    c.check("seven digits: the last seven (4287082)", totp::code(&seven, at(59)) == "4287082", totp::code(&seven, at(59)));
    c.check(
        "the code is the same all through its 30 seconds and changes at the next",
        totp::code(&six, at(60)) == totp::code(&six, at(89)) && totp::code(&six, at(89)) != totp::code(&six, at(90)),
        (),
    );
    let sixty = TotpKey { secret: k1.secret.clone(), period: 60, ..Default::default() };
    c.check(
        "a 60 s key keeps its code for 60 s",
        totp::code(&sixty, at(120)) == totp::code(&sixty, at(179)) && totp::code(&sixty, at(179)) != totp::code(&sixty, at(180)),
        (),
    );

    // Seconds left: 30 at the start of a step, 1 at its last second, 30 again at the next.
    let left = |unix: i64, key: &TotpKey| totp::seconds_left(key, at(unix));
    c.check(
        "seconds left: 30 at second 0, 1 at second 29, 30 at second 30 of the minute",
        left(60, &six) == 30 && left(89, &six) == 1 && left(90, &six) == 30,
        format!("{} {} {}", left(60, &six), left(89, &six), left(90, &six)),
    );
    c.check("seconds left: always 1 to the period", (0..120).all(|s| (1..=30).contains(&left(1_000_000_020 + s, &six))), ());
    c.check(
        "seconds left on a 60 s key counts to 60",
        left(120, &sixty) == 60 && left(179, &sixty) == 1,
        format!("{} {}", left(120, &sixty), left(179, &sixty)),
    );

    // A vault edited by hand: a period of 0 would divide by zero; a length outside 6 to 8 would make no code.
    let usual = totp::code(&six, at(1_234_567_890));
    for (period, digits) in [(0, 6), (-30, 6), (5, 6), (301, 6), (30, 5), (30, 0), (30, 9), (30, 100), (0, 0)] {
        let odd = TotpKey { secret: six.secret.clone(), period, digits, ..Default::default() };
        let outcome = std::panic::catch_unwind(|| (totp::code(&odd, at(1_234_567_890)), totp::seconds_left(&odd, at(1_234_567_890))));
        c.check(
            &format!("a vault with period {period} and {digits} digits: falls back to 30 s and 6 digits, no exception"),
            outcome.as_ref().is_ok_and(|(code, l)| *code == usual && (1..=30).contains(l)),
            outcome.as_ref().map(|(code, l)| format!("{code} {l}")).unwrap_or_else(|_| "panicked".into()),
        );
    }

    // Parse: how people paste a key.
    let plain = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
    let (parsed, problem, detail) = parse(plain);
    c.check(
        "a base32 key is taken as it is (SHA1, 6 digits, 30 s)",
        parsed.as_ref().is_some_and(|k| k.secret == plain && k.digits == 6 && k.period == 30 && k.algorithm == "SHA1") && problem == TotpProblem::None,
        format!("{problem:?} {detail}"),
    );
    let groups = |sep: &str| (0..8).map(|i| &plain[i * 4..i * 4 + 4]).collect::<Vec<_>>().join(sep);
    for (what, text) in [
        ("lower case", plain.to_lowercase()),
        ("in groups of four with spaces", groups(" ")),
        ("in groups with dashes", groups("-")),
        ("with = padding", format!("{plain}====")),
        ("with spaces round it and a tab or new line in it", format!("  {}\t\n{} \r\n", &plain[..16], &plain[16..])),
    ] {
        let (k, pr, _) = parse(&text);
        c.check(&format!("a key {what} is the same key"), k.is_some_and(|k| k.secret == plain) && pr == TotpProblem::None, pr);
    }
    for slip in ['0', '1', '8'] {
        let text = format!("GEZDGNBV{slip}Y3TQOJQGEZDGNBVGY3TQOJQ");
        let (bad, pr, why) = parse(&text);
        c.check(
            &format!("a \"{slip}\" in the key: BadCharacter, and says which"),
            bad.is_none() && pr == TotpProblem::BadCharacter && why == slip.to_string(),
            format!("{pr:?} \"{why}\""),
        );
    }
    let (empty, problem, _) = parse("");
    c.check("nothing pasted: NotAKey", empty.is_none() && problem == TotpProblem::NotAKey, problem);
    let (k, problem, _) = parse(" - - ");
    c.check("only spaces and dashes: NotAKey", k.is_none() && problem == TotpProblem::NotAKey, problem);
    let (k, problem, _) = totp::parse(None);
    c.check("null: NotAKey, no exception", k.is_none() && problem == TotpProblem::NotAKey, problem);

    // Length: under 16 letters was cut off at the front or back; 1, 3 and 6 over a multiple of 8 hold no whole byte.
    for length in 1..=15usize {
        let text = format!("{:A<length$}", &plain[..length.min(32)]);
        let (k, problem, _) = parse(&text);
        c.check(&format!("{length} letters: TooShort"), k.is_none() && problem == TotpProblem::TooShort, problem);
    }
    for length in [17usize, 19, 22, 25, 27, 30, 33, 35] {
        let (k, problem, _) = parse(&"A".repeat(length));
        c.check(&format!("{length} letters (length mod 8 is {}): CutOff", length % 8), k.is_none() && problem == TotpProblem::CutOff, problem);
    }
    for length in [16usize, 18, 20, 21, 23, 24, 26, 28, 29, 31, 32, 52, 103, 256] {
        let (k, problem, _) = parse(&"A".repeat(length));
        c.check(&format!("{length} letters (length mod 8 is {}): taken", length % 8), k.is_some() && problem == TotpProblem::None, problem);
    }
    let (k, problem, _) = parse(&"A".repeat(257));
    c.check("257 letters: NotAKey", k.is_none() && problem == TotpProblem::NotAKey, problem);

    // otpauth:// links.
    let (link, problem, _) = parse(&format!("otpauth://totp/Roblox:me?secret={plain}&issuer=Roblox&digits=8&period=60&algorithm=SHA-256"));
    c.check(
        "an otpauth://totp link: the secret and its digits, period and algorithm",
        link.as_ref().is_some_and(|k| k.digits == 8 && k.period == 60 && k.algorithm == "SHA256" && k.secret == plain) && problem == TotpProblem::None,
        problem,
    );
    let (plain_link, problem, _) = parse(&format!("OTPAUTH://TOTP/x?SECRET={}", plain.to_lowercase()));
    c.check(
        "…names and case don't matter, the defaults stay",
        plain_link.as_ref().is_some_and(|k| k.digits == 6 && k.period == 30 && k.algorithm == "SHA1" && k.secret == plain),
        problem,
    );
    let (k, problem, _) = parse(&format!("otpauth://hotp/x?secret={plain}&counter=1"));
    c.check("an otpauth://hotp link: NotTimeBased", k.is_none() && problem == TotpProblem::NotTimeBased, problem);
    let (k, problem, _) = parse(&format!("otpauth://steam/x?secret={plain}"));
    c.check("another otpauth host: NotAKey", k.is_none() && problem == TotpProblem::NotAKey, problem);
    let (k, problem, _) = parse("otpauth://totp/x?issuer=Roblox");
    c.check("a link without a secret: NoSecret", k.is_none() && problem == TotpProblem::NoSecret, problem);
    let (k, problem, _) = parse("otpauth://totp/x?secret=");
    c.check("a link with an empty secret: NoSecret", k.is_none() && problem == TotpProblem::NoSecret, problem);
    for (setting, text) in [
        ("digits=9", format!("otpauth://totp/x?secret={plain}&digits=9")),
        ("digits=5", format!("otpauth://totp/x?secret={plain}&digits=5")),
        ("digits=six", format!("otpauth://totp/x?secret={plain}&digits=six")),
        ("period=5", format!("otpauth://totp/x?secret={plain}&period=5")),
        ("period=301", format!("otpauth://totp/x?secret={plain}&period=301")),
        ("algorithm=MD5", format!("otpauth://totp/x?secret={plain}&algorithm=MD5")),
    ] {
        let (refused, problem, named) = parse(&text);
        c.check(
            &format!("{setting}: Unsupported, naming the setting"),
            refused.is_none() && problem == TotpProblem::Unsupported && named == setting,
            format!("{problem:?} \"{named}\""),
        );
    }
    let (long1, problem, long_detail) = parse(&format!("otpauth://totp/x?secret={plain}&algorithm={}", "M".repeat(80)));
    let units = long_detail.encode_utf16().count();
    c.check(
        "a long setting is named in 40 characters and an ellipsis",
        long1.is_none() && problem == TotpProblem::Unsupported && units == 41 && long_detail.ends_with('…'),
        format!("{problem:?} {units}"),
    );
    let (k, problem, _) = parse("otpauth://totp/x?secret=0&digits=9");
    c.check("a setting is refused before a bad secret is looked at", k.is_none() && problem == TotpProblem::Unsupported, problem);
    let ten = parse(&format!("otpauth://totp/x?secret={plain}&period=10")).0;
    let three_hundred = parse(&format!("otpauth://totp/x?secret={plain}&period=300")).0;
    c.check(
        "period=10 and period=300 are the ends that are taken",
        ten.is_some_and(|k| k.period == 10) && three_hundred.is_some_and(|k| k.period == 300),
        (),
    );

    // What Parse makes works with Code: the whole way from a pasted link to the RFC's code.
    let pasted = format!("otpauth://totp/x?secret={}&digits=8", base32(sha1_key));
    let via_link = parse(&pasted).0;
    c.check("a pasted link makes the RFC's code at T=59", via_link.is_some_and(|k| totp::code(&k, at(59)) == "94287082"), ());
}
