using System.Globalization;
using System.Net;
using System.Text;
using System.Text.Json;

namespace RobloxManager;

public sealed record RobloxUser(long Id, string Name, string DisplayName);

public sealed record GameInfo(long PlaceId, long UniverseId, string Name, string Creator, long Playing, string? IconUrl);

public sealed record GameSummary(long PlaceId, long UniverseId, string Name, long Playing);

/// <summary>
/// Which Roblox an account signs in to. Roblox VNG (Vietnam) serves the same API under robloxapp.vnggames.com, and its
/// accounts only work there.
/// </summary>
public static class Realm
{
    public const string Global = "";
    public const string Vng = "vng";

    public static string Normalize(string? realm) => realm == Vng ? Vng : Global;

    /// <summary>The API host for a service, e.g. Api(Vng, "auth") = https://auth.robloxapp.vnggames.com.</summary>
    public static string Api(string realm, string service) =>
        realm == Vng ? $"https://{service}.robloxapp.vnggames.com" : $"https://{service}.roblox.com";

    public static string Website(string realm) => realm == Vng ? "https://www.robloxapp.vnggames.com" : "https://www.roblox.com";
}

/// <summary>A Quick Login code. <see cref="PrivateKey"/> is a secret: it never leaves the app's memory.</summary>
public sealed record QuickLoginCode(string Realm, string Code, string PrivateKey, DateTime ExpiresUtc, string? ImagePath);

public sealed record QuickLoginStatus(string Status, string? AccountName, string? PictureUrl);

/// <summary>A public server of a game, as listed on the game's page.</summary>
public sealed record ServerInfo(string JobId, int Playing, int MaxPlayers, int Ping, double Fps);

/// <param name="Type">0 offline, 1 online (website/app), 2 in game, 3 in Studio, 4 invisible.</param>
public sealed record Presence(int Type, string? Location, long? PlaceId, long? RootPlaceId, string? JobId, DateTime? LastOnline, long? UniverseId);

public sealed class RobloxApiException(string message, bool cookieInvalid = false, bool rateLimited = false) : Exception(message)
{
    public bool CookieInvalid { get; } = cookieInvalid;

    /// <summary>Roblox answered 429: wait a little and try again.</summary>
    public bool RateLimited { get; } = rateLimited;
}

/// <summary>
/// Talks only to official roblox.com endpoints. The cookie is attached per request and never stored by the HTTP stack.
/// </summary>
public static class RobloxApi
{
    const string UserAgent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
    const string CookieName = ".ROBLOSECURITY";

    static readonly HttpClient Http = new(new SocketsHttpHandler
    {
        UseCookies = false,
        AllowAutoRedirect = false,
        AutomaticDecompression = DecompressionMethods.All,
        PooledConnectionLifetime = TimeSpan.FromMinutes(5),
    })
    {
        Timeout = TimeSpan.FromSeconds(20),
    };

    /// <summary>Accepts a bare value, "name=value", or a whole Cookie header pasted from DevTools.</summary>
    public static string NormalizeCookie(string raw)
    {
        string c = raw.Trim().Trim('"', '\'').Trim();
        int i = c.IndexOf(CookieName + "=", StringComparison.OrdinalIgnoreCase);
        if (i >= 0)
            c = c[(i + CookieName.Length + 1)..];
        int semi = c.IndexOf(';');
        if (semi >= 0)
            c = c[..semi];
        return c.Trim();
    }

    public static bool IsExpectedError(Exception ex) =>
        ex is HttpRequestException or TaskCanceledException or JsonException or RobloxApiException
            or KeyNotFoundException or InvalidOperationException;

    static HttpRequestMessage Request(HttpMethod method, string url, string? cookie, bool fromWebsite = false, string realm = Realm.Global)
    {
        var req = new HttpRequestMessage(method, url);
        req.Headers.TryAddWithoutValidation("User-Agent", UserAgent);
        req.Headers.TryAddWithoutValidation("Accept", "application/json, text/plain, */*");
        if (!string.IsNullOrEmpty(cookie))
            req.Headers.TryAddWithoutValidation("Cookie", $"{CookieName}={cookie}");
        if (fromWebsite)
        {
            req.Headers.TryAddWithoutValidation("Origin", Realm.Website(realm));
            req.Headers.TryAddWithoutValidation("Referer", Realm.Website(realm) + "/");
        }
        return req;
    }

    static string? Header(HttpResponseMessage res, string name) =>
        res.Headers.TryGetValues(name, out var values) ? values.FirstOrDefault() : null;

    static string Shorten(string s) => s.Length > 300 ? s[..300] + "…" : s;

    /// <returns>null when Roblox rejects the cookie (expired, logged out, or invalid).</returns>
    public static async Task<RobloxUser?> GetAuthenticatedUserAsync(string cookie, string realm = Realm.Global, CancellationToken ct = default)
    {
        using var req = Request(HttpMethod.Get, Realm.Api(realm, "users") + "/v1/users/authenticated", cookie);
        using var res = await Http.SendAsync(req, ct);
        if (res.StatusCode == HttpStatusCode.Unauthorized)
            return null;
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (!res.IsSuccessStatusCode)
            throw new RobloxApiException(L.F("Roblox returned HTTP {0}: {1}", (int)res.StatusCode, Shorten(body)));
        using var doc = JsonDocument.Parse(body);
        var root = doc.RootElement;
        return new RobloxUser(
            root.GetProperty("id").GetInt64(),
            root.GetProperty("name").GetString() ?? "",
            root.GetProperty("displayName").GetString() ?? "");
    }

    /// <summary>
    /// POST with Roblox's CSRF handshake: a request without a valid token is answered with 403 and a fresh
    /// x-csrf-token header, which is then replayed.
    /// </summary>
    static async Task<HttpResponseMessage> PostWithCsrfAsync(string url, string cookie, string jsonBody,
        Action<HttpRequestMessage>? configure, CancellationToken ct, string realm = Realm.Global)
    {
        string? token = null;
        for (int attempt = 0; ; attempt++)
        {
            HttpResponseMessage res;
            using (var req = Request(HttpMethod.Post, url, cookie, fromWebsite: true, realm))
            {
                if (token is not null)
                    req.Headers.TryAddWithoutValidation("x-csrf-token", token);
                configure?.Invoke(req);
                req.Content = new StringContent(jsonBody, Encoding.UTF8, "application/json");
                res = await Http.SendAsync(req, ct);
            }
            string? fresh = Header(res, "x-csrf-token");
            if (res.StatusCode == HttpStatusCode.Forbidden && !string.IsNullOrEmpty(fresh) && fresh != token && attempt < 2)
            {
                token = fresh;
                res.Dispose();
                continue;
            }
            return res;
        }
    }

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
            throw new RobloxApiException(L.F("Roblox returned HTTP {0}: {1}", (int)res.StatusCode, Shorten(body)));
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
            throw new RobloxApiException(L.F("Roblox returned HTTP {0}: {1}", (int)res.StatusCode, Shorten(body)));
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
        if (Header(res, "rblx-challenge-id") is not null)
            throw new UserError(L.T("Roblox wants an extra check for this sign-in. Use Sign in with browser instead."));
        string? cookie = res.Headers.TryGetValues("Set-Cookie", out var values)
            ? values.Where(v => v.StartsWith(CookieName + "=", StringComparison.Ordinal)).Select(NormalizeCookie).FirstOrDefault(v => v.Length > 0)
            : null;
        if (res.IsSuccessStatusCode && cookie is not null)
            return cookie;
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        throw new RobloxApiException(L.F("Roblox returned HTTP {0}: {1}", (int)res.StatusCode, Shorten(body)));
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

    /// <summary>
    /// Since Sep 2026 roblox.com first fetches a client assertion and posts it as the body of the
    /// authentication-ticket request. Falls back to the older empty body if the endpoint is unavailable.
    /// </summary>
    static async Task<string?> GetClientAssertionAsync(string cookie, CancellationToken ct)
    {
        const string url = "https://auth.roblox.com/v1/client-assertion/";
        try
        {
            using (var req = Request(HttpMethod.Get, url, cookie, fromWebsite: true))
            using (var res = await Http.SendAsync(req, ct))
            {
                string body = (await res.Content.ReadAsStringAsync(ct)).Trim();
                if (res.IsSuccessStatusCode && body.StartsWith('{'))
                    return body;
                if (res.StatusCode != HttpStatusCode.MethodNotAllowed)
                    return null;
            }
            using var post = await PostWithCsrfAsync(url, cookie, "{}", null, ct);
            string postBody = (await post.Content.ReadAsStringAsync(ct)).Trim();
            return post.IsSuccessStatusCode && postBody.StartsWith('{') ? postBody : null;
        }
        catch (HttpRequestException)
        {
            return null;
        }
    }

    /// <summary>
    /// The outcome of a username+password sign-in. Either a session cookie (done), a two-step prompt the user must
    /// answer, or a reason it can't continue here (captcha, which needs the browser).
    /// </summary>
    public sealed record LoginResult(string? Cookie, TwoStepPrompt? TwoStep, string? Blocked);

    /// <summary>What's needed to finish a two-step sign-in: the user picks Email or Authenticator and types the code.</summary>
    public sealed record TwoStepPrompt(string Realm, long UserId, string ChallengeId, string LoginChallengeId, string Username, string Password);

    static string? CookieFrom(HttpResponseMessage res) =>
        res.Headers.TryGetValues("Set-Cookie", out var values)
            ? values.Where(v => v.StartsWith(CookieName + "=", StringComparison.Ordinal)).Select(NormalizeCookie).FirstOrDefault(v => v.Length > 0)
            : null;

    /// <summary>
    /// Signs in with the account's own username and password, exactly as the Roblox client does: the values go only to
    /// Roblox's (or VNG's) official auth host over HTTPS and are never stored or logged by the app.
    /// </summary>
    public static async Task<LoginResult> LoginWithPasswordAsync(string username, string password, string realm, CancellationToken ct = default)
    {
        string json = JsonSerializer.Serialize(new { ctype = "Username", cvalue = username, password });
        using var res = await PostWithCsrfAsync(Realm.Api(realm, "auth") + "/v2/login", "", json, null, ct, realm);
        if (CookieFrom(res) is { } cookie && res.IsSuccessStatusCode)
            return new LoginResult(cookie, null, null);

        // A challenge: either two-step verification (we can prompt) or a captcha (only the browser can show it).
        if (Header(res, "rblx-challenge-id") is { Length: > 0 } challengeId)
        {
            string type = Header(res, "rblx-challenge-type") ?? "";
            if (type.Equals("twostepverification", StringComparison.OrdinalIgnoreCase)
                && DecodeMetadata(Header(res, "rblx-challenge-metadata")) is { } meta
                && meta.TryGetProperty("challengeId", out var cid) && cid.GetString() is { Length: > 0 } twoStepId
                && meta.TryGetProperty("userId", out var uid))
            {
                long userId = uid.ValueKind == JsonValueKind.Number ? uid.GetInt64() : long.TryParse(uid.GetString(), out var n) ? n : 0;
                return new LoginResult(null, new TwoStepPrompt(realm, userId, twoStepId, challengeId, username, password), null);
            }
            return new LoginResult(null, null, "captcha");
        }

        string body = await res.Content.ReadAsStringAsync(ct);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Too many sign-in attempts. Wait a few minutes and try again."), rateLimited: true);
        if (res.StatusCode == HttpStatusCode.Forbidden && body.Contains("Incorrect username or password"))
            throw new UserError(L.T("Incorrect username or password."));
        throw new RobloxApiException(L.F("Roblox returned HTTP {0}: {1}", (int)res.StatusCode, Shorten(body)));
    }

    static JsonElement? DecodeMetadata(string? base64)
    {
        if (string.IsNullOrEmpty(base64))
            return null;
        try
        {
            using var doc = JsonDocument.Parse(Encoding.UTF8.GetString(Convert.FromBase64String(base64)));
            return doc.RootElement.Clone();
        }
        catch (Exception ex) when (ex is FormatException or JsonException)
        {
            return null;
        }
    }

    /// <summary>
    /// Answers a two-step prompt: verifies the code with the chosen method, then retries the sign-in carrying the
    /// verification so Roblox hands back the session.
    /// </summary>
    public static async Task<string> VerifyTwoStepAsync(TwoStepPrompt prompt, string method, string code, CancellationToken ct = default)
    {
        string mediaType = method == "email" ? "email" : "authenticator";
        string verifyUrl = $"{Realm.Api(prompt.Realm, "twostepverification")}/v1/users/{prompt.UserId}/challenges/{mediaType}/verify";
        string verifyBody = JsonSerializer.Serialize(new { challengeId = prompt.ChallengeId, actionType = "Login", code });
        string? token;
        using (var res = await PostWithCsrfAsync(verifyUrl, "", verifyBody, null, ct, prompt.Realm))
        {
            string body = await res.Content.ReadAsStringAsync(ct);
            if (res.StatusCode == HttpStatusCode.TooManyRequests)
                throw new RobloxApiException(L.T("Too many sign-in attempts. Wait a few minutes and try again."), rateLimited: true);
            if (!res.IsSuccessStatusCode)
                throw new UserError(body.Contains("Invalid") || body.Contains("incorrect")
                    ? L.T("That verification code is wrong or expired.")
                    : L.F("Roblox returned HTTP {0}: {1}", (int)res.StatusCode, Shorten(body)));
            using var doc = JsonDocument.Parse(body);
            token = doc.RootElement.TryGetProperty("verificationToken", out var v) ? v.GetString() : null;
        }
        if (string.IsNullOrEmpty(token))
            throw new RobloxApiException(L.T("Roblox didn't return a verification token."));

        // Retry the login, now carrying the solved two-step challenge.
        string meta = Convert.ToBase64String(Encoding.UTF8.GetBytes(JsonSerializer.Serialize(
            new { verificationToken = token, rememberDevice = false, challengeId = prompt.ChallengeId, actionType = "Login" })));
        string json = JsonSerializer.Serialize(new { ctype = "Username", cvalue = prompt.Username, password = prompt.Password });
        using var login = await PostWithCsrfAsync(Realm.Api(prompt.Realm, "auth") + "/v2/login", "", json, req =>
        {
            req.Headers.TryAddWithoutValidation("rblx-challenge-id", prompt.LoginChallengeId);
            req.Headers.TryAddWithoutValidation("rblx-challenge-type", "twostepverification");
            req.Headers.TryAddWithoutValidation("rblx-challenge-metadata", meta);
        }, ct, prompt.Realm);
        string loginBody = await login.Content.ReadAsStringAsync(ct);
        if (CookieFrom(login) is { } cookie && login.IsSuccessStatusCode)
            return cookie;
        throw new RobloxApiException(L.F("Roblox returned HTTP {0}: {1}", (int)login.StatusCode, Shorten(loginBody)));
    }

    /// <summary>One-time ticket that lets the Roblox client sign in as this account.</summary>
    public static async Task<string> GetAuthTicketAsync(string cookie, CancellationToken ct = default)
    {
        string body = await GetClientAssertionAsync(cookie, ct) ?? "{}";
        using var res = await PostWithCsrfAsync("https://auth.roblox.com/v1/authentication-ticket/", cookie, body,
            req => req.Headers.TryAddWithoutValidation("RBXAuthenticationNegotiation", "1"), ct);

        if (Header(res, "rbx-authentication-ticket") is { Length: > 0 } ticket)
            return ticket;
        if (res.StatusCode == HttpStatusCode.Unauthorized)
            throw new RobloxApiException(L.T("This account was signed out — sign in to it again."), cookieInvalid: true);
        string text = await res.Content.ReadAsStringAsync(ct);
        throw new RobloxApiException(L.F("Couldn't get a sign-in ticket (HTTP {0}). {1}", (int)res.StatusCode, Shorten(text)));
    }

    /// <summary>Turns a "roblox.com/share?code=…&amp;type=Server" link into the place and private-server link code.</summary>
    public static async Task<(long PlaceId, string LinkCode)> ResolvePrivateServerShareAsync(string cookie, string shareCode, CancellationToken ct = default)
    {
        string json = JsonSerializer.Serialize(new { linkId = shareCode, linkType = "Server" });
        using var res = await PostWithCsrfAsync("https://apis.roblox.com/sharelinks/v1/resolve-link", cookie, json, null, ct);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (!res.IsSuccessStatusCode)
            throw new RobloxApiException(L.F("Couldn't open the private server link (HTTP {0}). {1}", (int)res.StatusCode, Shorten(body)));

        using var doc = JsonDocument.Parse(body);
        if (doc.RootElement.TryGetProperty("privateServerInviteData", out var data) && data.ValueKind == JsonValueKind.Object
            && data.TryGetProperty("placeId", out var place) && place.ValueKind == JsonValueKind.Number
            && data.TryGetProperty("linkCode", out var link) && link.GetString() is { Length: > 0 } linkCode)
            return (place.GetInt64(), linkCode);
        throw new RobloxApiException(L.T("The private server link is invalid or has expired."));
    }

    static async Task<JsonDocument?> GetJsonAsync(string url, CancellationToken ct, string? cookie = null)
    {
        using var req = Request(HttpMethod.Get, url, cookie);
        using var res = await Http.SendAsync(req, ct);
        return res.IsSuccessStatusCode ? JsonDocument.Parse(await res.Content.ReadAsStringAsync(ct)) : null;
    }

    static bool IsRobloxCdn(string? url) =>
        Uri.TryCreate(url, UriKind.Absolute, out var uri) && uri.Scheme == Uri.UriSchemeHttps
        && uri.Host.EndsWith(".rbxcdn.com", StringComparison.OrdinalIgnoreCase);

    /// <summary>The Windows client Roblox currently serves, i.e. what a launcher updates to (no cookie involved).</summary>
    public static async Task<RobloxInstalls.ClientVersion?> GetLatestClientVersionAsync(CancellationToken ct = default)
    {
        using var doc = await GetJsonAsync("https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer", ct);
        if (doc is null)
            return null;
        string? Text(string name) => doc.RootElement.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
        return Text("version") is { Length: > 0 } version ? new RobloxInstalls.ClientVersion(version, Text("clientVersionUpload") ?? "") : null;
    }

    /// <summary>Public avatar headshot URLs (no cookie involved); missing ones are left out.</summary>
    public static async Task<Dictionary<long, string>> GetHeadshotUrlsAsync(IEnumerable<long> userIds, CancellationToken ct = default)
    {
        var result = new Dictionary<long, string>();
        foreach (var chunk in userIds.Where(id => id > 0).Distinct().Chunk(100))
        {
            using var doc = await GetJsonAsync(
                $"https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={string.Join(',', chunk)}&size=150x150&format=Png&isCircular=false", ct);
            if (doc is null)
                continue;
            foreach (var item in doc.RootElement.GetProperty("data").EnumerateArray())
            {
                if (item.GetProperty("state").GetString() == "Completed" && item.GetProperty("imageUrl").GetString() is { } url && IsRobloxCdn(url))
                    result[item.GetProperty("targetId").GetInt64()] = url;
            }
        }
        return result;
    }

    /// <summary>The universe (experience) a place belongs to, or 0 when Roblox doesn't know the place.</summary>
    public static async Task<long> GetUniverseIdAsync(long placeId, CancellationToken ct = default)
    {
        using var doc = await GetJsonAsync($"https://apis.roblox.com/universes/v1/places/{placeId}/universe", ct);
        return doc is not null && doc.RootElement.TryGetProperty("universeId", out var u) && u.ValueKind == JsonValueKind.Number ? u.GetInt64() : 0;
    }

    /// <summary>Game details for previews: name, creator, live player count and icon.</summary>
    public static async Task<GameInfo?> GetGameInfoAsync(long placeId, string? cookie = null, CancellationToken ct = default)
    {
        long universeId = await GetUniverseIdAsync(placeId, ct);
        if (universeId == 0)
            return null;
        var games = await GetGamesAsync([universeId], cookie, ct);
        return games.TryGetValue(universeId, out var info) ? info with { PlaceId = placeId } : null;
    }

    /// <summary>
    /// Details of many games at once, keyed by universe: three requests per 50 games, sent together. Since 2026
    /// games.roblox.com hides titles from anonymous callers, so names come from develop.roblox.com and the player
    /// count needs a cookie. Games without a name in the answer are left out (and asked again later).
    /// </summary>
    public static async Task<Dictionary<long, GameInfo>> GetGamesAsync(IEnumerable<long> universeIds, string? cookie = null, CancellationToken ct = default)
    {
        var result = new Dictionary<long, GameInfo>();
        foreach (var chunk in universeIds.Where(id => id > 0).Distinct().Chunk(50))
        {
            var details = GetUniverseDetailsAsync(chunk, ct);
            var counts = GetPlayerCountsAsync(chunk, cookie, ct);
            var icons = GetGameIconsAsync(chunk, ct);
            await Task.WhenAll(details, counts, icons);
            foreach (var (id, (name, creator, rootPlaceId)) in details.Result)
                result[id] = new GameInfo(rootPlaceId, id, name, creator, counts.Result.GetValueOrDefault(id), icons.Result.GetValueOrDefault(id));
        }
        return result;
    }

    static async Task<Dictionary<long, (string Name, string Creator, long RootPlaceId)>> GetUniverseDetailsAsync(long[] universeIds, CancellationToken ct)
    {
        var result = new Dictionary<long, (string, string, long)>();
        using var doc = await GetJsonAsync($"https://develop.roblox.com/v1/universes/multiget?{string.Join('&', universeIds.Select(id => $"ids={id}"))}", ct);
        if (doc?.RootElement.TryGetProperty("data", out var data) != true || data.ValueKind != JsonValueKind.Array)
            return result;
        foreach (var item in data.EnumerateArray())
        {
            string Text(string name) => item.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";
            long Num(string name) => item.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetInt64() : 0;
            if (Num("id") > 0 && Text("name").Length > 0)
                result[Num("id")] = (Text("name"), Text("creatorName"), Num("rootPlaceId"));
        }
        return result;
    }

    static async Task<Dictionary<long, long>> GetPlayerCountsAsync(long[] universeIds, string? cookie, CancellationToken ct)
    {
        var result = new Dictionary<long, long>();
        using var doc = await GetJsonAsync($"https://games.roblox.com/v1/games?universeIds={string.Join(',', universeIds)}", ct, cookie);
        if (doc?.RootElement.TryGetProperty("data", out var data) != true || data.ValueKind != JsonValueKind.Array)
            return result;
        foreach (var item in data.EnumerateArray())
        {
            if (item.TryGetProperty("id", out var id) && id.ValueKind == JsonValueKind.Number
                && item.TryGetProperty("playing", out var p) && p.ValueKind == JsonValueKind.Number)
                result[id.GetInt64()] = p.GetInt64();
        }
        return result;
    }

    /// <summary>Roblox's own game search, the same one the website's search box uses.</summary>
    public static async Task<List<GameSummary>> SearchGamesAsync(string query, CancellationToken ct = default)
    {
        string url = $"https://apis.roblox.com/search-api/omni-search?searchQuery={Uri.EscapeDataString(query)}&sessionId={Guid.NewGuid()}&pageType=all";
        using var doc = await GetJsonAsync(url, ct);
        var result = new List<GameSummary>();
        if (doc?.RootElement.TryGetProperty("searchResults", out var groups) != true || groups.ValueKind != JsonValueKind.Array)
            return result;
        foreach (var group in groups.EnumerateArray())
        {
            if (group.TryGetProperty("contentGroupType", out var type) && type.GetString() == "Game"
                && group.TryGetProperty("contents", out var contents) && contents.ValueKind == JsonValueKind.Array)
            {
                foreach (var game in contents.EnumerateArray())
                {
                    if (ReadGame(game, "universeId", "rootPlaceId") is { } summary)
                        result.Add(summary);
                }
            }
        }
        return result;
    }

    /// <summary>Games an account marked as favourite on Roblox (public list).</summary>
    public static async Task<List<GameSummary>> GetFavoriteGamesAsync(long userId, CancellationToken ct = default)
    {
        using var doc = await GetJsonAsync($"https://games.roblox.com/v2/users/{userId}/favorite/games?limit=25&sortOrder=Desc", ct);
        var result = new List<GameSummary>();
        if (doc?.RootElement.TryGetProperty("data", out var data) != true || data.ValueKind != JsonValueKind.Array)
            return result;
        foreach (var game in data.EnumerateArray())
        {
            long placeId = game.TryGetProperty("rootPlace", out var root) && root.ValueKind == JsonValueKind.Object
                           && root.TryGetProperty("id", out var rid) && rid.TryGetInt64(out long p) ? p : 0;
            long universeId = game.TryGetProperty("id", out var uid) && uid.TryGetInt64(out long u) ? u : 0;
            string name = game.TryGetProperty("name", out var n) ? n.GetString() ?? "" : "";
            if (placeId > 0 && name.Length > 0)
                result.Add(new GameSummary(placeId, universeId, name, 0));
        }
        return result;
    }

    /// <summary>The first game list on Roblox's home/charts page. Roblox only fills it for signed-in requests.</summary>
    public static async Task<List<GameSummary>> GetPopularGamesAsync(string? cookie, CancellationToken ct = default)
    {
        string session = Guid.NewGuid().ToString();
        string token = "";
        for (int page = 0; page < 4; page++)
        {
            string url = $"https://apis.roblox.com/explore-api/v1/get-sorts?sessionId={session}&device=computer&country=all"
                         + (token.Length > 0 ? $"&sortsPageToken={Uri.EscapeDataString(token)}" : "");
            using var doc = await GetJsonAsync(url, ct, cookie);
            if (doc is null || !doc.RootElement.TryGetProperty("sorts", out var sorts) || sorts.ValueKind != JsonValueKind.Array)
                break;
            foreach (var sort in sorts.EnumerateArray())
            {
                if (!sort.TryGetProperty("games", out var games) || games.ValueKind != JsonValueKind.Array || games.GetArrayLength() == 0)
                    continue;
                var result = games.EnumerateArray().Select(g => ReadGame(g, "universeId", "rootPlaceId")).OfType<GameSummary>().ToList();
                if (result.Count > 0)
                    return result;
            }
            token = doc.RootElement.TryGetProperty("nextSortsPageToken", out var next) ? next.GetString() ?? "" : "";
            if (token.Length == 0)
                break;
        }
        return [];
    }

    static GameSummary? ReadGame(JsonElement game, string universeField, string placeField)
    {
        long Num(string name) => game.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt64(out long n) ? n : 0;
        string name = game.TryGetProperty("name", out var nv) && nv.ValueKind == JsonValueKind.String ? nv.GetString() ?? "" : "";
        long placeId = Num(placeField);
        return placeId > 0 && name.Length > 0 ? new GameSummary(placeId, Num(universeField), name, Num("playerCount")) : null;
    }

    /// <summary>
    /// A page of a game's public servers (the "Servers" tab of the game's page), busiest or emptiest first, without
    /// full ones. <paramref name="cursor"/> continues a previous page.
    /// </summary>
    public static async Task<(List<ServerInfo> Servers, string? Next)> GetServersAsync(long placeId, bool fewestFirst, string? cursor, CancellationToken ct = default)
    {
        string url = $"https://games.roblox.com/v1/games/{placeId}/servers/Public?sortOrder={(fewestFirst ? "Asc" : "Desc")}&limit=50&excludeFullGames=true"
                     + (string.IsNullOrEmpty(cursor) ? "" : "&cursor=" + Uri.EscapeDataString(cursor));
        using var req = Request(HttpMethod.Get, url, null);
        using var res = await Http.SendAsync(req, ct);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        var servers = new List<ServerInfo>();
        if (!res.IsSuccessStatusCode)
            return (servers, null);
        using var doc = JsonDocument.Parse(await res.Content.ReadAsStringAsync(ct));
        if (doc.RootElement.TryGetProperty("data", out var data) && data.ValueKind == JsonValueKind.Array)
        {
            foreach (var s in data.EnumerateArray())
            {
                int Int(string name) => s.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt32(out int n) ? n : 0;
                double Num(string name) => s.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetDouble() : 0;
                if (s.TryGetProperty("id", out var id) && id.GetString() is { Length: > 0 } jobId)
                    servers.Add(new ServerInfo(jobId, Int("playing"), Int("maxPlayers"), (int)Math.Round(Num("ping")), Math.Round(Num("fps"))));
            }
        }
        string? next = doc.RootElement.TryGetProperty("nextPageCursor", out var c) && c.ValueKind == JsonValueKind.String ? c.GetString() : null;
        return (servers, next);
    }

    /// <summary>Square game icons for many games at once.</summary>
    public static async Task<Dictionary<long, string>> GetGameIconsAsync(IEnumerable<long> universeIds, CancellationToken ct = default)
    {
        var result = new Dictionary<long, string>();
        foreach (var chunk in universeIds.Where(id => id > 0).Distinct().Chunk(100))
        {
            using var doc = await GetJsonAsync(
                $"https://thumbnails.roblox.com/v1/games/icons?universeIds={string.Join(',', chunk)}&returnPolicy=PlaceHolder&size=150x150&format=Png&isCircular=false", ct);
            if (doc?.RootElement.TryGetProperty("data", out var data) != true || data.ValueKind != JsonValueKind.Array)
                continue;
            foreach (var item in data.EnumerateArray())
            {
                if (item.GetProperty("imageUrl").GetString() is { } url && IsRobloxCdn(url))
                    result[item.GetProperty("targetId").GetInt64()] = url;
            }
        }
        return result;
    }

    /// <summary>
    /// Online / in-game status. With a cookie, Roblox also reveals the game for that account and for anyone
    /// whose privacy settings allow it.
    /// </summary>
    public static async Task<Dictionary<long, Presence>> GetPresenceAsync(string? cookie, IReadOnlyCollection<long> userIds, string realm = Realm.Global, CancellationToken ct = default)
    {
        var result = new Dictionary<long, Presence>();
        foreach (var chunk in userIds.Where(id => id > 0).Distinct().Chunk(100))
        {
            string json = JsonSerializer.Serialize(new { userIds = chunk });
            using var res = await PostWithCsrfAsync(Realm.Api(realm, "presence") + "/v1/presence/users", cookie ?? "", json, null, ct, realm);
            if (res.StatusCode == HttpStatusCode.TooManyRequests)
                throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
            if (!res.IsSuccessStatusCode)
                continue;
            using var doc = JsonDocument.Parse(await res.Content.ReadAsStringAsync(ct));
            foreach (var p in doc.RootElement.GetProperty("userPresences").EnumerateArray())
            {
                long? Num(string name) => p.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetInt64() : null;
                string? Text(string name) => p.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
                DateTime? lastOnline = Text("lastOnline") is { } s && DateTime.TryParse(s, null, System.Globalization.DateTimeStyles.RoundtripKind, out var d) ? d : null;
                result[p.GetProperty("userId").GetInt64()] = new Presence(
                    (int)(Num("userPresenceType") ?? 0), Text("lastLocation"), Num("placeId"), Num("rootPlaceId"), Text("gameId"), lastOnline, Num("universeId"));
            }
        }
        return result;
    }
}
