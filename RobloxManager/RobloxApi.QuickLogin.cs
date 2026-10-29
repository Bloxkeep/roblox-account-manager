using System.Globalization;
using System.Net;
using System.Text.Json;

namespace RobloxManager;

/// <summary>Quick Login: signing an account in with a code approved on a device already signed in.</summary>
public static partial class RobloxApi
{
    static string QuickLoginBase(string realm) => Realm.Api(realm, "apis") + "/auth-token-service/v1/login/";

    /// <summary>
    /// Starts a Quick Login: Roblox hands out a short code (and a QR picture of it) that the user confirms on a device
    /// already signed in to Roblox. <see cref="QuickLoginCode.PrivateKey"/> proves we created the code; keep it in memory only.
    /// </summary>
    public static async Task<QuickLoginCode> CreateQuickLoginAsync(string realm, CancellationToken ct = default)
    {
        using var res = await PostWithCsrfAsync(QuickLoginBase(realm) + "create", "", "{}", null, ct, realm);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        if (!res.IsSuccessStatusCode)
            throw Refused("Quick Login code", res.StatusCode, body);
        using var doc = JsonDocument.Parse(body);
        var root = doc.RootElement;
        string code = root.GetProperty("code").GetString() ?? "";
        string key = root.GetProperty("privateKey").GetString() ?? "";
        if (code.Length == 0 || key.Length == 0)
            throw new RobloxApiException(L.T("Roblox didn't return a sign-in code."));
        // Roblox writes the expiry in UTC without a zone; codes last about five minutes.
        var expires = root.TryGetProperty("expirationTime", out var e) && DateTime.TryParse(e.GetString(), CultureInfo.InvariantCulture,
            DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal, out var t) ? t : DateTime.UtcNow.AddMinutes(5);
        string? image = root.TryGetProperty("imagePath", out var i) ? i.GetString() : null;
        return new QuickLoginCode(realm, code, key, expires, image);
    }

    /// <summary>The QR picture of a Quick Login code as a data: URL (the page may only show images from Roblox's CDN).</summary>
    public static async Task<string?> GetQuickLoginQrAsync(QuickLoginCode login, CancellationToken ct = default)
    {
        if (login.ImagePath is not { Length: > 0 } path || !path.StartsWith('/'))
            return null;
        using var req = Request(HttpMethod.Get, Realm.Api(login.Realm, "apis") + "/auth-token-service" + path, null);
        using var res = await Http.SendAsync(req, ct);
        if (!res.IsSuccessStatusCode || res.Content.Headers.ContentType?.MediaType != "image/png")
            return null;
        return "data:image/png;base64," + Convert.ToBase64String(await res.Content.ReadAsByteArrayAsync(ct));
    }

    /// <returns>Created, UserLinked (someone typed the code; the name is theirs), Validated, Cancelled or Expired.</returns>
    public static async Task<QuickLoginStatus> GetQuickLoginStatusAsync(QuickLoginCode login, CancellationToken ct = default)
    {
        string json = JsonSerializer.Serialize(new { code = login.Code, privateKey = login.PrivateKey });
        using var res = await PostWithCsrfAsync(QuickLoginBase(login.Realm) + "status", "", json, null, ct, login.Realm);
        string body = await res.Content.ReadAsStringAsync(ct);
        // An expired or unknown code is answered with CodeInvalid / CodeNotFound.
        if (body.Contains("\"CodeInvalid\"") || body.Contains("\"CodeNotFound\""))
            return new QuickLoginStatus("Expired", null, null);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        if (!res.IsSuccessStatusCode)
            throw Refused("Quick Login status", res.StatusCode, body);
        using var doc = JsonDocument.Parse(body);
        var root = doc.RootElement;
        string? Text(string name) => root.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
        return new QuickLoginStatus(Text("status") ?? "Created", Text("accountName"), Text("accountPictureUrl"));
    }

    /// <summary>Trades a confirmed code for the account's session cookie, exactly as roblox.com does after Quick Login.</summary>
    public static async Task<string> FinishQuickLoginAsync(QuickLoginCode login, CancellationToken ct = default)
    {
        string json = JsonSerializer.Serialize(new { ctype = "AuthToken", cvalue = login.Code, password = login.PrivateKey });
        using var res = await PostWithCsrfAsync(Realm.Api(login.Realm, "auth") + "/v2/login", "", json, null, ct, login.Realm);
        string body = await res.Content.ReadAsStringAsync(ct);
        // The browser sign-in is roblox.com only, so Roblox VNG is sent to the password sign-in.
        if (Header(res, "rblx-challenge-id") is not null)
            throw new UserError(login.Realm == Realm.Vng
                ? L.T("Roblox wants an extra check for this sign-in. Sign in with the username and password instead.")
                : L.T("Roblox wants an extra check for this sign-in. Use Sign in with browser instead."));
        string? cookie = res.Headers.TryGetValues("Set-Cookie", out var values)
            ? values.Where(v => v.StartsWith(CookieName + "=", StringComparison.Ordinal)).Select(NormalizeCookie).FirstOrDefault(v => v.Length > 0)
            : null;
        if (res.IsSuccessStatusCode && cookie is not null)
            return cookie;
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        throw Refused("Quick Login finish", res.StatusCode, body);
    }

    /// <summary>Withdraws a code nobody used, so it can't be confirmed after the window closes.</summary>
    public static async Task CancelQuickLoginAsync(QuickLoginCode login)
    {
        try
        {
            using var res = await PostWithCsrfAsync(QuickLoginBase(login.Realm) + "cancel", "", JsonSerializer.Serialize(new { code = login.Code }), null, default, login.Realm);
        }
        catch (Exception ex) when (IsExpectedError(ex))
        {
            // The code expires on its own a few minutes later.
        }
    }
}
