using System.Buffers.Binary;
using System.Globalization;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace RobloxManager;

/// <summary>An account's authenticator key as the vault keeps it: the shared secret and how codes are made from it.</summary>
public sealed class TotpKey
{
    /// <summary>Base32 (A–Z, 2–7), upper case, without spaces or padding.</summary>
    public string Secret { get; set; } = "";
    public int Digits { get; set; } = 6;
    /// <summary>Seconds each code lasts.</summary>
    public int Period { get; set; } = 30;
    /// <summary>"SHA1" (Roblox's, and most sites'), "SHA256" or "SHA512".</summary>
    public string Algorithm { get; set; } = "SHA1";
    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Extra { get; set; }
}

/// <summary>Why a pasted key was refused; the app words it for the person.</summary>
public enum TotpProblem { None, NotAKey, BadCharacter, TooShort, CutOff, NotTimeBased, NoSecret, Unsupported }

/// <summary>
/// Time-based one-time codes (RFC 6238), what an authenticator app shows: an HMAC of the number of 30-second steps
/// since 1970, cut down to 6 digits. Only arithmetic: nothing here touches the network, the disk or the log.
/// </summary>
public static class Totp
{
    const string Alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    /// <summary>80 bits, the shortest secret sites hand out (RFC 4226 asks for 128 or more): anything shorter was cut off.</summary>
    const int MinSecretChars = 16;
    const int MaxSecretChars = 256;

    /// <summary>
    /// Reads a key the way people paste it: the base32 secret (any case, grouped with spaces or dashes as sites show
    /// it) or an otpauth://totp/… link (a QR code's text). A typo base32 can't hold is refused rather than kept, since
    /// it would only ever make wrong codes. A wrong letter that is still base32 can't be told apart: the edit form
    /// shows the code it makes, to compare with the authenticator app.
    /// </summary>
    /// <param name="detail">The character refused (<see cref="TotpProblem.BadCharacter"/>) or the setting (<see cref="TotpProblem.Unsupported"/>).</param>
    public static TotpKey? Parse(string? input, out TotpProblem problem, out string detail)
    {
        detail = "";
        string text = (input ?? "").Trim();
        var key = new TotpKey();
        string secret = text;
        if (text.StartsWith("otpauth:", StringComparison.OrdinalIgnoreCase))
        {
            if (!Uri.TryCreate(text, UriKind.Absolute, out var uri))
                return Fail(TotpProblem.NotAKey, out problem);
            if (uri.Host.Equals("hotp", StringComparison.OrdinalIgnoreCase))
                return Fail(TotpProblem.NotTimeBased, out problem);
            if (!uri.Host.Equals("totp", StringComparison.OrdinalIgnoreCase))
                return Fail(TotpProblem.NotAKey, out problem);
            var query = Query(uri.Query);
            if (query.GetValueOrDefault("secret") is not { Length: > 0 } fromLink)
                return Fail(TotpProblem.NoSecret, out problem);
            secret = fromLink;
            if (query.TryGetValue("digits", out string? digits))
            {
                if (!int.TryParse(digits, NumberStyles.None, CultureInfo.InvariantCulture, out int n) || n is < 6 or > 8)
                    return Unsupported($"digits={digits}", out problem, out detail);
                key.Digits = n;
            }
            if (query.TryGetValue("period", out string? period))
            {
                if (!int.TryParse(period, NumberStyles.None, CultureInfo.InvariantCulture, out int s) || s is < 10 or > 300)
                    return Unsupported($"period={period}", out problem, out detail);
                key.Period = s;
            }
            if (query.TryGetValue("algorithm", out string? algorithm))
            {
                string name = algorithm.Replace("-", "").ToUpperInvariant();
                if (name is not ("SHA1" or "SHA256" or "SHA512"))
                    return Unsupported($"algorithm={algorithm}", out problem, out detail);
                key.Algorithm = name;
            }
        }

        // Sites group the key in fours with spaces (or dashes), and base32 may end in "=" padding.
        secret = new string(secret.Where(c => !char.IsWhiteSpace(c) && c != '-').ToArray()).TrimEnd('=').ToUpperInvariant();
        if (secret.Length == 0)
            return Fail(TotpProblem.NotAKey, out problem);
        foreach (char c in secret)
        {
            // "0", "1" and "8" are the usual slips (for O, I or L, and B): base32 leaves them out for that reason.
            if (!Alphabet.Contains(c))
            {
                detail = c.ToString();
                return Fail(TotpProblem.BadCharacter, out problem);
            }
        }
        if (secret.Length > MaxSecretChars)
            return Fail(TotpProblem.NotAKey, out problem);
        if (secret.Length < MinSecretChars)
            return Fail(TotpProblem.TooShort, out problem);
        // Base32 lengths that hold no whole number of bytes: a character went missing (or one too many).
        if (secret.Length % 8 is 1 or 3 or 6)
            return Fail(TotpProblem.CutOff, out problem);
        key.Secret = secret;
        problem = TotpProblem.None;
        return key;
    }

    static TotpKey? Fail(TotpProblem why, out TotpProblem problem)
    {
        problem = why;
        return null;
    }

    static TotpKey? Unsupported(string setting, out TotpProblem problem, out string detail)
    {
        detail = setting.Length > 40 ? setting[..40] + "…" : setting;
        problem = TotpProblem.Unsupported;
        return null;
    }

    /// <summary>The query of an otpauth link, names in lower case; the first of a name repeated wins.</summary>
    static Dictionary<string, string> Query(string query)
    {
        var values = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (string pair in query.TrimStart('?').Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            int eq = pair.IndexOf('=');
            string name = Uri.UnescapeDataString((eq < 0 ? pair : pair[..eq]).Replace('+', ' ')).ToLowerInvariant();
            string value = eq < 0 ? "" : Uri.UnescapeDataString(pair[(eq + 1)..].Replace('+', ' '));
            values.TryAdd(name, value);
        }
        return values;
    }

    /// <summary>The code an authenticator app with this key shows at <paramref name="at"/>.</summary>
    public static string Code(TotpKey key, DateTimeOffset at)
    {
        byte[] secret = Base32(key.Secret);
        try
        {
            return CodeAt(secret, at.ToUnixTimeSeconds() / PeriodOf(key), DigitsOf(key), key.Algorithm);
        }
        finally
        {
            CryptographicOperations.ZeroMemory(secret);
        }
    }

    /// <summary>Seconds until the code at <paramref name="at"/> changes (1 to the period).</summary>
    public static int SecondsLeft(TotpKey key, DateTimeOffset at) => PeriodOf(key) - (int)(at.ToUnixTimeSeconds() % PeriodOf(key));

    /// <summary>
    /// The key's period and length as Parse allows them (10-300 s, 6-8 digits): a vault edited by hand or written by
    /// another tool may hold anything, and a period of 0 would end in a division by zero.
    /// </summary>
    static int PeriodOf(TotpKey key) => key.Period is >= 10 and <= 300 ? key.Period : 30;

    static int DigitsOf(TotpKey key) => key.Digits is >= 6 and <= 8 ? key.Digits : 6;

    /// <summary>RFC 4226's HOTP for one counter value (RFC 6238 feeds it the time step): dynamic truncation, then the last digits.</summary>
    public static string CodeAt(byte[] secret, long step, int digits, string algorithm)
    {
        Span<byte> counter = stackalloc byte[8];
        BinaryPrimitives.WriteInt64BigEndian(counter, step);
        byte[] hash = algorithm switch
        {
            "SHA256" => HMACSHA256.HashData(secret, counter),
            "SHA512" => HMACSHA512.HashData(secret, counter),
            _ => HMACSHA1.HashData(secret, counter),
        };
        int offset = hash[^1] & 0x0F;
        int binary = (hash[offset] & 0x7F) << 24 | hash[offset + 1] << 16 | hash[offset + 2] << 8 | hash[offset + 3];
        int modulo = digits switch { 7 => 10_000_000, 8 => 100_000_000, _ => 1_000_000 };
        return (binary % modulo).ToString(CultureInfo.InvariantCulture).PadLeft(digits, '0');
    }

    /// <summary>Base32 (RFC 4648) to bytes; leftover bits at the end are dropped, as authenticator apps do.</summary>
    static byte[] Base32(string text)
    {
        var bytes = new byte[text.Length * 5 / 8];
        int buffer = 0, bits = 0, index = 0;
        foreach (char c in text)
        {
            buffer = buffer << 5 | Alphabet.IndexOf(c);
            bits += 5;
            if (bits >= 8)
            {
                bits -= 8;
                bytes[index++] = (byte)(buffer >> bits);
                buffer &= (1 << bits) - 1;
            }
        }
        return bytes;
    }
}
