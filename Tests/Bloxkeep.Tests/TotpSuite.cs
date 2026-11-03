using System.Text;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The sign-in codes (Totp.cs) against the published vectors: RFC 4226's table (counter 0 to 9) and RFC 6238 Appendix B (8
/// digits, SHA1, SHA256 and SHA512), the seconds left on a code, every way a pasted key is refused, and a vault edited by
/// hand to a period or length that would divide by zero. Only arithmetic: no data folder, no network.
/// </summary>
static class TotpSuite
{
    const string Alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    public static Task Run(Checks c, string dir)
    {
        // RFC 4226 Appendix D: the key "12345678901234567890", six digits, counters 0 to 9.
        byte[] sha1Key = Encoding.ASCII.GetBytes("12345678901234567890");
        string[] hotp = ["755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583", "399871", "520489"];
        var got = Enumerable.Range(0, 10).Select(n => Totp.CodeAt(sha1Key, n, 6, "SHA1")).ToArray();
        c.Check("RFC 4226 Appendix D: the ten counter values", got.SequenceEqual(hotp), string.Join(",", got));

        // RFC 6238 Appendix B: 8 digits, 30 s steps; the SHA256 key is 32 bytes and the SHA512 key 64, the SHA1 key's 20 repeated.
        var keys = new Dictionary<string, TotpKey>
        {
            ["SHA1"] = KeyOf("12345678901234567890", "SHA1"),
            ["SHA256"] = KeyOf("12345678901234567890123456789012", "SHA256"),
            ["SHA512"] = KeyOf("1234567890123456789012345678901234567890123456789012345678901234", "SHA512"),
        };
        (long Time, string Sha1, string Sha256, string Sha512)[] table =
        [
            (59, "94287082", "46119246", "90693936"),
            (1111111109, "07081804", "68084774", "25091201"),
            (1111111111, "14050471", "67062674", "99943326"),
            (1234567890, "89005924", "91819424", "93441116"),
            (2000000000, "69279037", "90698825", "38618901"),
            (20000000000, "65353130", "77737706", "47863826"),
        ];
        foreach (var row in table)
        {
            var at = DateTimeOffset.FromUnixTimeSeconds(row.Time);
            string[] made = [Totp.Code(keys["SHA1"], at), Totp.Code(keys["SHA256"], at), Totp.Code(keys["SHA512"], at)];
            c.Check($"RFC 6238 Appendix B at T={row.Time}: SHA1, SHA256 and SHA512", made.SequenceEqual([row.Sha1, row.Sha256, row.Sha512]), string.Join(" ", made));
        }
        c.Check("the SHA512 key (64 bytes, 103 letters) is one Parse takes", Totp.Parse(Base32(Encoding.ASCII.GetBytes("1234567890123456789012345678901234567890123456789012345678901234")), out var p, out _) is { } && p == TotpProblem.None);

        // Six digits are the last six of the eight (the modulo), as the authenticator apps show them.
        var six = new TotpKey { Secret = keys["SHA1"].Secret };
        c.Check("six digits: the last six of the eight (287082)", Totp.Code(six, DateTimeOffset.FromUnixTimeSeconds(59)) == "287082");
        var seven = new TotpKey { Secret = keys["SHA1"].Secret, Digits = 7 };
        c.Check("seven digits: the last seven (4287082)", Totp.Code(seven, DateTimeOffset.FromUnixTimeSeconds(59)) == "4287082");
        c.Check("the code is the same all through its 30 seconds and changes at the next",
            Totp.Code(six, DateTimeOffset.FromUnixTimeSeconds(60)) == Totp.Code(six, DateTimeOffset.FromUnixTimeSeconds(89))
            && Totp.Code(six, DateTimeOffset.FromUnixTimeSeconds(89)) != Totp.Code(six, DateTimeOffset.FromUnixTimeSeconds(90)));
        var sixty = new TotpKey { Secret = keys["SHA1"].Secret, Period = 60 };
        c.Check("a 60 s key keeps its code for 60 s", Totp.Code(sixty, DateTimeOffset.FromUnixTimeSeconds(120)) == Totp.Code(sixty, DateTimeOffset.FromUnixTimeSeconds(179))
            && Totp.Code(sixty, DateTimeOffset.FromUnixTimeSeconds(179)) != Totp.Code(sixty, DateTimeOffset.FromUnixTimeSeconds(180)));

        // Seconds left: 30 at the start of a step, 1 at its last second, 30 again at the next.
        int Left(long unix, TotpKey? key = null) => Totp.SecondsLeft(key ?? six, DateTimeOffset.FromUnixTimeSeconds(unix));
        c.Check("seconds left: 30 at second 0, 1 at second 29, 30 at second 30 of the minute", Left(60) == 30 && Left(89) == 1 && Left(90) == 30, $"{Left(60)} {Left(89)} {Left(90)}");
        c.Check("seconds left: always 1 to the period", Enumerable.Range(0, 120).All(s => Left(1_000_000_020 + s) is >= 1 and <= 30));
        c.Check("seconds left on a 60 s key counts to 60", Left(120, sixty) == 60 && Left(179, sixty) == 1, $"{Left(120, sixty)} {Left(179, sixty)}");

        // A vault edited by hand: a period of 0 would divide by zero; a length outside 6 to 8 would make no code.
        var usual = Totp.Code(six, DateTimeOffset.FromUnixTimeSeconds(1_234_567_890));
        foreach (var (period, digits) in new[] { (0, 6), (-30, 6), (5, 6), (301, 6), (30, 5), (30, 0), (30, 9), (30, 100), (0, 0) })
        {
            var odd = new TotpKey { Secret = six.Secret, Period = period, Digits = digits };
            string? code = null;
            int left = 0;
            Exception? thrown = null;
            try
            {
                code = Totp.Code(odd, DateTimeOffset.FromUnixTimeSeconds(1_234_567_890));
                left = Totp.SecondsLeft(odd, DateTimeOffset.FromUnixTimeSeconds(1_234_567_890));
            }
            catch (Exception ex)
            {
                thrown = ex;
            }
            c.Check($"a vault with period {period} and {digits} digits: falls back to 30 s and 6 digits, no exception",
                thrown is null && code == usual && left is >= 1 and <= 30, thrown?.GetType().Name ?? $"{code} {left}");
        }

        // Parse: how people paste a key.
        string plain = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        var parsed = Totp.Parse(plain, out var problem, out var detail);
        c.Check("a base32 key is taken as it is (SHA1, 6 digits, 30 s)", parsed is { Secret: var s0, Digits: 6, Period: 30, Algorithm: "SHA1" } && s0 == plain && problem == TotpProblem.None, $"{problem} {detail}");
        foreach (var (what, text) in new[]
        {
            ("lower case", plain.ToLowerInvariant()),
            ("in groups of four with spaces", string.Join(" ", Enumerable.Range(0, 8).Select(i => plain.Substring(i * 4, 4)))),
            ("in groups with dashes", string.Join("-", Enumerable.Range(0, 8).Select(i => plain.Substring(i * 4, 4)))),
            ("with = padding", plain + "===="),
            ("with spaces round it and a tab or new line in it", "  " + plain[..16] + "\t\n" + plain[16..] + " \r\n"),
        })
        {
            c.Check($"a key {what} is the same key", Totp.Parse(text, out var pr, out _) is { } k && k.Secret == plain && pr == TotpProblem.None);
        }
        foreach (char slip in "018")
        {
            string text = "GEZDGNBV" + slip + "Y3TQOJQGEZDGNBVGY3TQOJQ";
            var bad = Totp.Parse(text, out var pr, out var why);
            c.Check($"a \"{slip}\" in the key: BadCharacter, and says which", bad is null && pr == TotpProblem.BadCharacter && why == slip.ToString(), $"{pr} \"{why}\"");
        }
        var empty = Totp.Parse("", out problem, out _);
        c.Check("nothing pasted: NotAKey", empty is null && problem == TotpProblem.NotAKey, problem);
        c.Check("only spaces and dashes: NotAKey", Totp.Parse(" - - ", out problem, out _) is null && problem == TotpProblem.NotAKey, problem);
        c.Check("null: NotAKey, no exception", Totp.Parse(null, out problem, out _) is null && problem == TotpProblem.NotAKey, problem);

        // Length: under 16 letters was cut off at the front or back; 1, 3 and 6 over a multiple of 8 hold no whole byte.
        foreach (int length in Enumerable.Range(1, 15))
            c.Check($"{length} letters: TooShort", Totp.Parse(plain[..Math.Min(length, 32)].PadRight(length, 'A'), out problem, out _) is null && problem == TotpProblem.TooShort, problem);
        foreach (int length in new[] { 17, 19, 22, 25, 27, 30, 33, 35 })
            c.Check($"{length} letters (length mod 8 is {length % 8}): CutOff", Totp.Parse(new string('A', length), out problem, out _) is null && problem == TotpProblem.CutOff, problem);
        foreach (int length in new[] { 16, 18, 20, 21, 23, 24, 26, 28, 29, 31, 32, 52, 103, 256 })
            c.Check($"{length} letters (length mod 8 is {length % 8}): taken", Totp.Parse(new string('A', length), out problem, out _) is { } && problem == TotpProblem.None, problem);
        c.Check("257 letters: NotAKey", Totp.Parse(new string('A', 257), out problem, out _) is null && problem == TotpProblem.NotAKey, problem);

        // otpauth:// links.
        var link = Totp.Parse($"otpauth://totp/Roblox:me?secret={plain}&issuer=Roblox&digits=8&period=60&algorithm=SHA-256", out problem, out _);
        c.Check("an otpauth://totp link: the secret and its digits, period and algorithm", link is { Digits: 8, Period: 60, Algorithm: "SHA256" } && link.Secret == plain && problem == TotpProblem.None, problem);
        var plainLink = Totp.Parse($"OTPAUTH://TOTP/x?SECRET={plain.ToLowerInvariant()}", out problem, out _);
        c.Check("…names and case don't matter, the defaults stay", plainLink is { Digits: 6, Period: 30, Algorithm: "SHA1" } && plainLink.Secret == plain, problem);
        c.Check("an otpauth://hotp link: NotTimeBased", Totp.Parse($"otpauth://hotp/x?secret={plain}&counter=1", out problem, out _) is null && problem == TotpProblem.NotTimeBased, problem);
        c.Check("another otpauth host: NotAKey", Totp.Parse($"otpauth://steam/x?secret={plain}", out problem, out _) is null && problem == TotpProblem.NotAKey, problem);
        c.Check("a link without a secret: NoSecret", Totp.Parse("otpauth://totp/x?issuer=Roblox", out problem, out _) is null && problem == TotpProblem.NoSecret, problem);
        c.Check("a link with an empty secret: NoSecret", Totp.Parse("otpauth://totp/x?secret=", out problem, out _) is null && problem == TotpProblem.NoSecret, problem);
        foreach (var (setting, text) in new[]
        {
            ("digits=9", $"otpauth://totp/x?secret={plain}&digits=9"),
            ("digits=5", $"otpauth://totp/x?secret={plain}&digits=5"),
            ("digits=six", $"otpauth://totp/x?secret={plain}&digits=six"),
            ("period=5", $"otpauth://totp/x?secret={plain}&period=5"),
            ("period=301", $"otpauth://totp/x?secret={plain}&period=301"),
            ("algorithm=MD5", $"otpauth://totp/x?secret={plain}&algorithm=MD5"),
        })
        {
            var refused = Totp.Parse(text, out problem, out var named);
            c.Check($"{setting}: Unsupported, naming the setting", refused is null && problem == TotpProblem.Unsupported && named == setting, $"{problem} \"{named}\"");
        }
        var long1 = Totp.Parse($"otpauth://totp/x?secret={plain}&algorithm={new string('M', 80)}", out problem, out var longDetail);
        c.Check("a long setting is named in 40 characters and an ellipsis", long1 is null && problem == TotpProblem.Unsupported && longDetail.Length == 41 && longDetail.EndsWith('…'), $"{problem} {longDetail.Length}");
        c.Check("a setting is refused before a bad secret is looked at", Totp.Parse("otpauth://totp/x?secret=0&digits=9", out problem, out _) is null && problem == TotpProblem.Unsupported, problem);
        c.Check("period=10 and period=300 are the ends that are taken", Totp.Parse($"otpauth://totp/x?secret={plain}&period=10", out _, out _) is { Period: 10 } && Totp.Parse($"otpauth://totp/x?secret={plain}&period=300", out _, out _) is { Period: 300 });

        // What Parse makes works with Code: the whole way from a pasted link to the RFC's code.
        string pasted = $"otpauth://totp/x?secret={Base32(sha1Key)}&digits=8";
        c.Check("a pasted link makes the RFC's code at T=59", Totp.Parse(pasted, out _, out _) is { } viaLink && Totp.Code(viaLink, DateTimeOffset.FromUnixTimeSeconds(59)) == "94287082");
        return Task.CompletedTask;
    }

    static TotpKey KeyOf(string ascii, string algorithm) => new() { Secret = Base32(Encoding.ASCII.GetBytes(ascii)), Digits = 8, Period = 30, Algorithm = algorithm };

    /// <summary>RFC 4648 base32 without padding, as the sites hand keys out.</summary>
    static string Base32(byte[] bytes)
    {
        var text = new StringBuilder();
        int buffer = 0, bits = 0;
        foreach (byte b in bytes)
        {
            buffer = buffer << 8 | b;
            bits += 8;
            while (bits >= 5)
            {
                bits -= 5;
                text.Append(Alphabet[buffer >> bits & 31]);
                buffer &= (1 << bits) - 1;
            }
        }
        if (bits > 0)
            text.Append(Alphabet[buffer << (5 - bits) & 31]);
        return text.ToString();
    }
}
