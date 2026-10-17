using System.Collections.Concurrent;
using System.Globalization;
using System.Net;
using System.Net.Sockets;
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

public sealed class RobloxApiException(string message, bool cookieInvalid = false, bool rateLimited = false, bool serverError = false,
    bool attemptOver = false) : Exception(message)
{
    public bool CookieInvalid { get; } = cookieInvalid;

    /// <summary>Roblox refused a password sign-in (wrong password, a locked account…): that attempt can't go on, only start over.</summary>
    public bool AttemptOver { get; } = attemptOver;

    /// <summary>Roblox answered 429: wait a little and try again.</summary>
    public bool RateLimited { get; } = rateLimited;

    /// <summary>With <see cref="RateLimited"/>: how long until Roblox takes requests again, when it said.</summary>
    public TimeSpan? RetryAfter { get; init; }

    /// <summary>Roblox answered 5xx (down or busy): worth trying again later, unlike a refusal (400, 403).</summary>
    public bool ServerError { get; } = serverError;
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
        // Connect over IPv4 only. An auth ticket is bound to the IP that requested it; the Roblox client redeems it
        // over IPv4, so if this app fetched it over IPv6 the IPs wouldn't match and the redeem would be rejected.
        ConnectCallback = Ipv4ConnectAsync,
    })
    {
        Timeout = TimeSpan.FromSeconds(20),
    };

    static async ValueTask<Stream> Ipv4ConnectAsync(SocketsHttpConnectionContext ctx, CancellationToken ct)
    {
        var entry = await Dns.GetHostAddressesAsync(ctx.DnsEndPoint.Host, AddressFamily.InterNetwork, ct);
        var socket = new Socket(AddressFamily.InterNetwork, SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
        try
        {
            await socket.ConnectAsync(entry, ctx.DnsEndPoint.Port, ct);
            return new NetworkStream(socket, ownsSocket: true);
        }
        catch
        {
            socket.Dispose();
            throw;
        }
    }

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

    /// <summary>Roblox's error codes, and for the log its error messages: never the rest of a reply (it may name the account).</summary>
    static (List<int> Codes, string Summary) Errors(string body)
    {
        var codes = new List<int>();
        var parts = new List<string>();
        if (DecodeJson(body) is { ValueKind: JsonValueKind.Object } root && root.TryGetProperty("errors", out var errors) && errors.ValueKind == JsonValueKind.Array)
        {
            foreach (var e in errors.EnumerateArray().Where(e => e.ValueKind == JsonValueKind.Object))
            {
                int code = e.TryGetProperty("code", out var c) && c.ValueKind == JsonValueKind.Number && c.TryGetInt32(out int n) ? n : -1;
                string message = e.TryGetProperty("message", out var m) && m.ValueKind == JsonValueKind.String ? m.GetString() ?? "" : "";
                codes.Add(code);
                parts.Add($"{code} '{(message.Length > 120 ? message[..120] + "…" : message)}'");
            }
        }
        return (codes, parts.Count > 0 ? string.Join(", ", parts) : $"none listed ({body.Length} chars)");
    }

    /// <summary>A refusal from Roblox: a short sentence for the person; what Roblox said goes to the log.</summary>
    static RobloxApiException Refused(string what, HttpStatusCode status, string body, bool cookieInvalid = false)
    {
        int code = (int)status;
        Log.Note($"{what}: HTTP {code}, errors {Errors(body).Summary}");
        return new RobloxApiException(code >= 500
                ? L.F("Roblox isn't working properly right now (HTTP {0}). Try again in a moment.", code)
                : L.F("Roblox refused the request (HTTP {0}). Try again in a moment.", code),
            cookieInvalid: cookieInvalid, serverError: code >= 500);
    }

    /// <summary>
    /// Roblox refused a password sign-in (at /v2/login, or a check answered on the way): its error code in plain
    /// words. The attempt is over either way, so the person starts again from the username and password. Codes as
    /// listed in auth.roblox.com's documentation of /v2/login; 16 to 28 are the ones roblox.com's own login page knows
    /// (screen time, security questions, blocked, account-switch limits, passkey-only).
    /// </summary>
    static RobloxApiException LoginRefused(string step, HttpStatusCode status, string body)
    {
        var (codes, summary) = Errors(body);
        Log.Note($"Sign-in refused at {step}: HTTP {(int)status}, errors {summary}");
        bool Has(params int[] wanted) => codes.Any(wanted.Contains);
        bool tooMany = status == HttpStatusCode.TooManyRequests || Has(7, 15);
        bool server = (int)status >= 500;
        string message =
            tooMany ? L.T("Too many sign-in attempts. Wait a few minutes and try again.")
            : Has(1) || body.Contains("Incorrect username or password") ? L.T("Incorrect username or password.")
            : Has(4) ? L.T("Roblox has locked this account to protect it. Reset its password on roblox.com (Forgot password) and sign in again, or sign in with your phone.")
            : Has(3) ? L.T("Enter the username and password.")
            : Has(6) ? L.T("Roblox reports a problem with this account (it may be restricted or banned). Check it on roblox.com.")
            : Has(5, 8, 9, 10, 12, 14, 16, 18, 22, 23, 24, 25, 26, 27, 28) ? L.T("Roblox doesn't allow a password sign-in for this account here. Sign in with your phone or the browser instead.")
            : Has(2) ? L.T("Roblox wants a captcha Bloxkeep didn't get. Try again, or sign in with your phone or the browser.")
            : server ? L.F("Roblox isn't working properly right now (HTTP {0}). Try again in a moment.", (int)status)
            : L.F("Roblox didn't accept the sign-in (HTTP {0}). Try again, or sign in with your phone or the browser.", (int)status);
        return new RobloxApiException(message, rateLimited: tooMany, serverError: server, attemptOver: true);
    }

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
            throw Refused("Signed-in user", res.StatusCode, body);
        using var doc = JsonDocument.Parse(body);
        var root = doc.RootElement;
        return new RobloxUser(
            root.GetProperty("id").GetInt64(),
            root.GetProperty("name").GetString() ?? "",
            root.GetProperty("displayName").GetString() ?? "");
    }

    /// <summary>Last CSRF token per signed-in session (keyed by a hash of the cookie, never the cookie itself).</summary>
    static readonly ConcurrentDictionary<(string Realm, int Session), string> CsrfTokens = new();

    /// <summary>
    /// POST with Roblox's CSRF handshake: a request without a valid token is answered with 403 and a fresh
    /// x-csrf-token header, which is then replayed, once. Only that refusal is replayed: any other 403 (a wrong
    /// password, a locked account, a check) is Roblox's answer, and repeating it could send a password twice.
    /// </summary>
    static async Task<HttpResponseMessage> PostWithCsrfAsync(string url, string cookie, string jsonBody,
        Action<HttpRequestMessage>? configure, CancellationToken ct, string realm = Realm.Global)
    {
        // The token stays valid for a while, so it is reused for the same session instead of paying the 403 each time.
        var session = (realm, cookie.GetHashCode());
        string? token = cookie.Length > 0 ? CsrfTokens.GetValueOrDefault(session) : null;
        for (int attempt = 0; ; attempt++)
        {
            HttpResponseMessage res;
            using (var req = Request(HttpMethod.Post, url, cookie, fromWebsite: true, realm))
            {
                if (token is not null)
                    req.Headers.TryAddWithoutValidation("x-csrf-token", token);
                configure?.Invoke(req);
                req.Content = new StringContent(jsonBody, Encoding.UTF8, "application/json");
                // Cancelled meanwhile (a sign-in called off while its check was being answered): nothing more goes out.
                // The same exception a cancelled request throws, so callers see no difference.
                if (ct.IsCancellationRequested)
                    throw new TaskCanceledException("The request was canceled.", null, ct);
                res = await Http.SendAsync(req, ct);
            }
            string? fresh = Header(res, "x-csrf-token");
            try
            {
                // The body is buffered, so reading it here leaves it for the caller.
                if (attempt == 0 && res.StatusCode == HttpStatusCode.Forbidden && !string.IsNullOrEmpty(fresh) && fresh != token
                    && IsCsrfRefusal(res.StatusCode, fresh, token, Header(res, "rblx-challenge-id"), await res.Content.ReadAsStringAsync(ct)))
                {
                    token = fresh;
                    if (cookie.Length > 0)
                        CsrfTokens[session] = fresh;
                    res.Dispose();
                    continue;
                }
            }
            catch
            {
                res.Dispose(); // reading the refusal failed (cancelled, cut off): the answer is let go here
                throw;
            }
            return res;
        }
    }

    /// <summary>
    /// Roblox's CSRF refusal and nothing else: 403, a new x-csrf-token, no challenge, no error code but 0, and the
    /// "Token Validation Failed" message — which may be missing only when no token was sent at all: then the request
    /// can't have got past the token check to the password.
    /// </summary>
    internal static bool IsCsrfRefusal(HttpStatusCode status, string? fresh, string? sent, string? challengeId, string body)
    {
        if (status != HttpStatusCode.Forbidden || string.IsNullOrEmpty(fresh) || fresh == sent || !string.IsNullOrEmpty(challengeId))
            return false;
        return Errors(body).Codes.All(c => c == 0)
            && (sent is null || body.Contains("Token Validation Failed", StringComparison.OrdinalIgnoreCase));
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
    /// The outcome of a username+password sign-in. Either a session cookie (done), a two-step prompt or a captcha the
    /// user must answer, or a check this app can't show (<see cref="Blocked"/>: what to tell the person, which ends the attempt).
    /// </summary>
    public sealed record LoginResult(string? Cookie, TwoStepPrompt? TwoStep, CaptchaPrompt? Captcha = null, string? Blocked = null,
        ProofOfWorkPrompt? ProofOfWork = null, HumanCaptchaPrompt? HumanCaptcha = null);

    /// <summary>
    /// Roblox's "captchav2": HUMAN Security's press-and-hold check. The puzzle is fetched from apis.roblox.com/v2/captcha
    /// with <see cref="CaptchaChallengeId"/> and solved in <see cref="CaptchaForm"/>, which hands back a redemption token.
    /// </summary>
    /// <param name="Metadata">The challenge's own metadata, sent back with the token filled in.</param>
    public sealed record HumanCaptchaPrompt(string Realm, string LoginChallengeId, string CaptchaChallengeId, string Metadata,
        string Username, string Password);

    /// <summary>
    /// A proof-of-work check: Roblox hands out a puzzle that takes the computer a few seconds to solve, which roblox.com
    /// solves in the background. No input from the user is needed.
    /// </summary>
    /// <param name="Metadata">The challenge's own metadata (session, request path…); it goes back to Roblox as-is, with the token filled in.</param>
    public sealed record ProofOfWorkPrompt(string Realm, string LoginChallengeId, string SessionId, string Metadata, string Username, string Password);

    /// <summary>What's needed to finish a two-step sign-in: the user picks where the code comes from and types it.</summary>
    /// <param name="MediaType">
    /// Set when Roblox answered the sign-in itself with "twoStepVerificationData" (a 200 with no cookie) instead of a
    /// challenge header: the code was already sent that way (e.g. "Email"), and the sign-in is finished through
    /// /v3/users/{id}/two-step-verification/login with the ticket rather than by repeating the login.
    /// </param>
    public sealed record TwoStepPrompt(string Realm, long UserId, string ChallengeId, string LoginChallengeId, string Username, string Password,
        string? MediaType = null, string ActionType = "Login")
    {
        /// <summary>Where this account can get a code (authenticator, email, sms), the one Roblox prefers first.</summary>
        public string[] Methods { get; init; } = ["authenticator", "email"];

        /// <summary>
        /// Set once Roblox accepted the code: a retry after finishing failed on the way (offline) reuses it instead of
        /// sending the spent code again.
        /// </summary>
        public string? VerificationToken { get; set; }
    }

    /// <summary>The 2-step methods Bloxkeep can answer: a code typed by the user, checked at challenges/{method}/verify.</summary>
    static readonly string[] CodeMethods = ["authenticator", "email", "sms"];

    /// <summary>
    /// Roblox's name for a 2-step method, from either spelling it uses: the name ("Email") or its number in
    /// TwoStepVerificationMediaType (Email 0, SMS 1, Authenticator 2, RecoveryCode 3, SecurityKey 4, CrossDevice 5,
    /// Password 6, Passkey 7).
    /// </summary>
    static string MediaName(JsonElement e) => e.ValueKind switch
    {
        JsonValueKind.String => e.GetString() ?? "",
        JsonValueKind.Number when e.TryGetInt32(out int n) && n >= 0 && n < MediaTypes.Length => MediaTypes[n],
        _ => "",
    };

    static readonly string[] MediaTypes = ["Email", "SMS", "Authenticator", "RecoveryCode", "SecurityKey", "CrossDevice", "Password", "Passkey"];

    /// <returns>The path name of a method answered with a typed code, or null for one Bloxkeep can't do (security key…).</returns>
    static string? CodeMethod(string mediaType) => CodeMethods.FirstOrDefault(m => m.Equals(mediaType, StringComparison.OrdinalIgnoreCase));

    /// <summary>
    /// A captcha Roblox put in front of a sign-in. The puzzle itself is Arkose Labs' FunCaptcha (Roblox's captcha
    /// provider, see apis.roblox.com/captcha/v1/metadata); <see cref="Blob"/> ties it to this sign-in attempt.
    /// </summary>
    /// <param name="ChallengeType">"captcha", or "captchav2" when Roblox asks for one after another check.</param>
    /// <param name="Metadata">The challenge's own metadata, sent back with the token filled in.</param>
    public sealed record CaptchaPrompt(string Realm, string LoginChallengeId, string UnifiedCaptchaId, string Blob, string ActionType,
        string Username, string Password, string ChallengeType = "captcha", string Metadata = "{}");

    static string? CookieFrom(HttpResponseMessage res) =>
        res.Headers.TryGetValues("Set-Cookie", out var values)
            ? values.Where(v => v.StartsWith(CookieName + "=", StringComparison.Ordinal)).Select(NormalizeCookie).FirstOrDefault(v => v.Length > 0)
            : null;

    /// <summary>
    /// Signs in with the account's own username and password, exactly as the Roblox client does: the values go only to
    /// Roblox's (or VNG's) official auth host over HTTPS and are never stored or logged by the app.
    /// </summary>
    public static Task<LoginResult> LoginWithPasswordAsync(string username, string password, string realm, CancellationToken ct = default) =>
        SendLoginAsync(username, password, realm, null, ct);

    /// <summary>POST /v2/login, optionally carrying a solved challenge, and sorts out what Roblox answered.</summary>
    static async Task<LoginResult> SendLoginAsync(string username, string password, string realm, Action<HttpRequestMessage>? configure,
        CancellationToken ct)
    {
        string json = JsonSerializer.Serialize(new { ctype = "Username", cvalue = username, password });
        using var res = await PostWithCsrfAsync(Realm.Api(realm, "auth") + "/v2/login", "", json, configure, ct, realm);
        if (CookieFrom(res) is { } cookie && res.IsSuccessStatusCode)
            return new LoginResult(cookie, null);

        string step = configure is null ? "login" : "login after a check";
        if (Header(res, "rblx-challenge-id") is { Length: > 0 } challengeId)
            return ParseChallenge(realm, challengeId, Header(res, "rblx-challenge-type") ?? "",
                DecodeMetadata(Header(res, "rblx-challenge-metadata")), username, password, step, res);

        string body = await res.Content.ReadAsStringAsync(ct);
        if (res.IsSuccessStatusCode)
        {
            if (TwoStepFromBody(realm, body, username, password) is { } twoStep)
                return new LoginResult(null, twoStep);
            var root = DecodeJson(body);
            // Accepted, but Roblox wants an ID check first (done only in its own apps and site).
            if (Field(root, "identityVerificationLoginTicket") is { Length: > 0 })
                return new LoginResult(null, null, Blocked: CantShow(realm, "identity verification"));
            if (root is { ValueKind: JsonValueKind.Object } r && r.TryGetProperty("isBanned", out var banned) && banned.ValueKind == JsonValueKind.True)
                throw new RobloxApiException(L.T("Roblox reports a problem with this account (it may be restricted or banned). Check it on roblox.com."), attemptOver: true);
            return new LoginResult(null, null, Blocked: Unsupported(step, realm, res, root));
        }
        throw LoginRefused(step, res.StatusCode, body);
    }

    /// <summary>A string field of a JSON object, or null.</summary>
    static string? Field(JsonElement? obj, string name) =>
        obj is { ValueKind: JsonValueKind.Object } o && o.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    /// <summary>A check Roblox's site or app does that Bloxkeep can't (yet); <paramref name="kind"/> names it.</summary>
    static string CantShow(string realm, string kind)
    {
        Log.Note($"Sign-in check from Roblox{(realm == Realm.Vng ? " VNG" : "")} that Bloxkeep can't show: {kind}");
        return L.F("Roblox asked for a check Bloxkeep can't show yet ({0}). Try again later, or sign in with your phone.", kind);
    }

    /// <summary>
    /// A sign-in answer that matches no shape Bloxkeep knows. Writes one line to error.log with only its layout (the
    /// step, the HTTP status, the names of Roblox's challenge headers, field names and value kinds, error codes),
    /// never a value: no password, cookie, ticket, token, name or id. Returns what to tell the person.
    /// </summary>
    static string Unsupported(string step, string realm, HttpResponseMessage? res, JsonElement? json, string? type = null)
    {
        var headers = res is null ? [] : res.Headers.Select(h => h.Key).Where(k => k.StartsWith("rblx-", StringComparison.OrdinalIgnoreCase)).ToList();
        var codes = json is { ValueKind: JsonValueKind.Object } o && o.TryGetProperty("errors", out var errors) && errors.ValueKind == JsonValueKind.Array
            ? errors.EnumerateArray().Select(e => e.ValueKind == JsonValueKind.Object && e.TryGetProperty("code", out var c) ? c.GetRawText() : "?").ToList()
            : [];
        Log.Note($"Sign-in answer not understood{(realm == Realm.Vng ? " (VNG)" : "")}: step '{step}'"
            + (res is null ? "" : $", HTTP {(int)res.StatusCode}")
            + (type is null ? "" : $", type '{SafeName(type)}'")
            + $", headers [{string.Join(", ", headers)}]"
            + $", body {(json is { } j ? Shape(j) : "not JSON")}"
            + (codes.Count > 0 ? $", errors [{string.Join(", ", codes)}]" : ""));
        return L.T("Roblox asked for a sign-in check Bloxkeep doesn't support yet. Sign in with your phone or the browser instead. The details were saved in error.log (Settings › Backup & data › Open folder).");
    }

    /// <summary>A name from Roblox (a check's type, a field) as it may go to the log: short and plain, else "#".</summary>
    static string SafeName(string name) =>
        name.Length == 0 ? "" : name.Length <= 40 && !name.All(char.IsAsciiDigit) && name.All(c => char.IsAsciiLetterOrDigit(c) || c is '-' or '_' or '.')
            ? name : "#";

    /// <summary>Checks Roblox's site knows but that only it (or the Roblox app) can show.</summary>
    static readonly string[] BrowserOnlyChecks =
    [
        "securityquestions", "security-questions", "reauthentication", "rostile", "privateaccesstoken", "deviceintegrity",
        "proofofspace", "phoneverification", "emailverification", "biometric", "chef", "turnstile",
    ];

    /// <summary>Turns a challenge Roblox put in front of the sign-in into what the user (or the app) has to answer.</summary>
    /// <param name="res">The answer the challenge came in, when it came in headers (for the log of an unknown one).</param>
    static LoginResult ParseChallenge(string realm, string challengeId, string type, JsonElement? meta, string username, string password,
        string step, HttpResponseMessage? res = null)
    {
        string? Text(string name) => Field(meta, name);
        bool Is(params string[] kinds) => kinds.Any(k => type.Equals(k, StringComparison.OrdinalIgnoreCase));
        if (Known() is { } known)
        {
            // Only the kind of check and the layout of its fields: enough to follow a change on Roblox's side.
            Log.Note($"Sign-in check from Roblox{(realm == Realm.Vng ? " VNG" : "")}: type '{SafeName(type)}', fields "
                + (meta is { } shape ? Shape(shape) : "none"));
            return known;
        }
        // An unknown check, or a known one in a shape that changed: one line with its layout, for a later version.
        return new LoginResult(null, null, Blocked: Unsupported(step, realm, res, meta, type));

        LoginResult? Known()
        {
            if (Is("twostepverification")
                && Text("challengeId") is { Length: > 0 } twoStepId
                && meta!.Value.TryGetProperty("userId", out var uid)
                && (uid.ValueKind == JsonValueKind.Number ? uid.TryGetInt64(out long id) ? id : 0
                    : uid.ValueKind == JsonValueKind.String && long.TryParse(uid.GetString(), out var n) ? n : 0) is > 0 and var userId)
            {
                return new LoginResult(null, new TwoStepPrompt(realm, userId, twoStepId, challengeId, username, password,
                    ActionType: Text("actionType") is { Length: > 0 } action ? action : "Login"));
            }
            if (Is("captcha", "captchav2")
                && Text("unifiedCaptchaId") is { Length: > 0 } captchaId
                && Text("dataExchangeBlob") is { Length: > 0 } blob)
            {
                return new LoginResult(null, null, new CaptchaPrompt(realm, challengeId, captchaId, blob, Text("actionType") ?? "Login",
                    username, password, type, meta!.Value.GetRawText()));
            }
            if (Is("captchav2") && Text("challengeId") is { Length: > 0 } humanId)
                return new LoginResult(null, null, HumanCaptcha: new HumanCaptchaPrompt(realm, challengeId, humanId, meta!.Value.GetRawText(), username, password));
            if (Is("proofofwork") && Text("sessionId") is { Length: > 0 } powSession)
                return new LoginResult(null, null, ProofOfWork: new ProofOfWorkPrompt(realm, challengeId, powSession, meta!.Value.GetRawText(), username, password));
            // The account has to turn on 2-step (or an authenticator) before it may sign in: roblox.com sends it to settings.
            if (Is("forcetwostepverification", "forceauthenticator"))
                return new LoginResult(null, null, Blocked: L.T("Roblox wants this account to turn on 2-step verification before it signs in. Sign in on roblox.com or in the Roblox app, turn it on there, then try again, or sign in with your phone."));
            if (Is("blocksession", "denied"))
                return new LoginResult(null, null, Blocked: L.T("Roblox blocked this sign-in. Sign in with your phone or the browser instead."));
            if (Is(BrowserOnlyChecks))
                return new LoginResult(null, null, Blocked: L.F("Roblox asked for a check Bloxkeep can't show yet ({0}). Try again later, or sign in with your phone.", SafeName(type)));
            return null;
        }
    }

    /// <summary>
    /// The other way Roblox asks for 2-step: a 200 answer whose body carries the user and "twoStepVerificationData"
    /// ({ mediaType, ticket }) — seen after a solved captcha. Null when the body isn't that.
    /// </summary>
    static TwoStepPrompt? TwoStepFromBody(string realm, string body, string username, string password)
    {
        try
        {
            using var doc = JsonDocument.Parse(body);
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object
                || !root.TryGetProperty("twoStepVerificationData", out var data) || data.ValueKind != JsonValueKind.Object
                || !data.TryGetProperty("ticket", out var ticket) || ticket.GetString() is not { Length: > 0 } ticketId
                || !root.TryGetProperty("user", out var user) || !user.TryGetProperty("id", out var id) || !id.TryGetInt64(out long userId))
                return null;
            string media = data.TryGetProperty("mediaType", out var m) && MediaName(m) is { Length: > 0 } kind ? kind : "Email";
            Log.Note($"Sign-in check from Roblox{(realm == Realm.Vng ? " VNG" : "")}: two-step in the answer, by {SafeName(media)}");
            return new TwoStepPrompt(realm, userId, ticketId, "", username, password, media);
        }
        catch (Exception ex) when (ex is JsonException or InvalidOperationException)
        {
            // A field of another kind than expected (GetString on a number…): not this shape.
            return null;
        }
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

    /// <summary>The FunCaptcha key Roblox (or VNG) uses for sign-ins, from its public captcha settings.</summary>
    public static async Task<string> GetLoginCaptchaKeyAsync(string realm, CancellationToken ct = default)
    {
        using var req = Request(HttpMethod.Get, Realm.Api(realm, "apis") + "/captcha/v1/metadata", null);
        using var res = await Http.SendAsync(req, ct);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (!res.IsSuccessStatusCode)
            throw Refused("Captcha settings", res.StatusCode, body);
        using var doc = JsonDocument.Parse(body);
        if (doc.RootElement.TryGetProperty("funCaptchaPublicKeys", out var keys)
            && keys.TryGetProperty("ACTION_TYPE_WEB_LOGIN", out var key) && key.GetString() is { Length: > 0 } value)
            return value;
        throw new RobloxApiException(L.T("Roblox didn't say which captcha to show."));
    }

    /// <summary>
    /// Hands Roblox the token from a solved captcha, then retries the sign-in carrying it, as roblox.com does. The
    /// answer can still be a two-step prompt (or, rarely, another captcha).
    /// </summary>
    public static Task<LoginResult> FinishCaptchaLoginAsync(CaptchaPrompt prompt, string captchaToken, CancellationToken ct = default) =>
        ContinueLoginAsync(prompt.Realm, prompt.LoginChallengeId, prompt.ChallengeType,
            prompt.ChallengeType == "captcha"
                ? JsonSerializer.Serialize(new { unifiedCaptchaId = prompt.UnifiedCaptchaId, captchaToken, actionType = prompt.ActionType })
                : WithField(prompt.Metadata, "captchaToken", captchaToken),
            prompt.Username, prompt.Password, ct);

    /// <summary>Carries on with the sign-in once the press-and-hold check handed back its redemption token.</summary>
    public static Task<LoginResult> FinishHumanCaptchaLoginAsync(HumanCaptchaPrompt prompt, string redemptionToken, CancellationToken ct = default) =>
        ContinueLoginAsync(prompt.Realm, prompt.LoginChallengeId, "captchav2", WithField(prompt.Metadata, "redemptionToken", redemptionToken),
            prompt.Username, prompt.Password, ct);

    /// <summary>
    /// Solves a proof-of-work check the way roblox.com does in the background: fetch the puzzle, compute the answer,
    /// trade it for a redemption token, then carry on with the sign-in.
    /// </summary>
    public static async Task<LoginResult> FinishProofOfWorkLoginAsync(ProofOfWorkPrompt prompt, CancellationToken ct = default)
    {
        string service = Realm.Api(prompt.Realm, "apis") + "/proof-of-work-service/v1/pow-puzzle";
        string puzzleType, artifacts;
        using (var req = Request(HttpMethod.Get, $"{service}?sessionID={Uri.EscapeDataString(prompt.SessionId)}", null))
        using (var res = await Http.SendAsync(req, ct))
        {
            string body = await res.Content.ReadAsStringAsync(ct);
            if (!res.IsSuccessStatusCode)
                throw Refused("Proof-of-work puzzle", res.StatusCode, body);
            using var doc = JsonDocument.Parse(body);
            var root = doc.RootElement;
            puzzleType = !root.TryGetProperty("puzzleType", out var p) ? ""
                : p.ValueKind == JsonValueKind.String ? p.GetString() ?? "" : p.GetRawText();
            artifacts = root.TryGetProperty("artifacts", out var a) ? (a.ValueKind == JsonValueKind.String ? a.GetString() ?? "" : a.GetRawText()) : "";
            Log.Note($"Proof-of-work puzzle: type '{SafeName(puzzleType)}', fields {string.Join(", ", root.EnumerateObject().Select(x => x.Name))}"
                + (DecodeJson(artifacts) is { ValueKind: JsonValueKind.Object } art ? $", artifacts {string.Join(", ", art.EnumerateObject().Select(x => x.Name))}" : ""));
        }

        string solution = await Task.Run(() => SolvePuzzle(puzzleType, artifacts, ct), ct);

        string redemptionToken;
        using (var res = await PostWithCsrfAsync(service, "", JsonSerializer.Serialize(new { sessionID = prompt.SessionId, solution }), null, ct, prompt.Realm))
        {
            string body = await res.Content.ReadAsStringAsync(ct);
            if (!res.IsSuccessStatusCode)
                throw Refused("Proof-of-work answer", res.StatusCode, body);
            using var doc = JsonDocument.Parse(body);
            var root = doc.RootElement;
            if (root.TryGetProperty("answerCorrect", out var ok) && ok.ValueKind == JsonValueKind.False)
                throw new RobloxApiException(L.T("Roblox didn't accept the sign-in check. Please try again."));
            redemptionToken = root.TryGetProperty("redemptionToken", out var r) ? r.GetString() ?? "" : "";
            if (redemptionToken.Length == 0)
            {
                Log.Note($"Proof-of-work answer reply fields: {string.Join(", ", root.EnumerateObject().Select(x => x.Name))}");
                throw new RobloxApiException(L.T("Roblox didn't accept the sign-in check. Please try again."));
            }
        }

        return await ContinueLoginAsync(prompt.Realm, prompt.LoginChallengeId, "proofofwork",
            WithField(prompt.Metadata, "redemptionToken", redemptionToken), prompt.Username, prompt.Password, ct);
    }

    /// <summary>
    /// Roblox's time-lock puzzle: square A modulo N, T times in a row. It can't be sped up by running in parallel, which
    /// is the point: it costs every sign-in a few seconds of one CPU core.
    /// </summary>
    static string SolvePuzzle(string puzzleType, string artifacts, CancellationToken ct)
    {
        // Roblox names the type as a number: 1 is the time-lock puzzle (seen Oct 2026, with artifacts N, A, T).
        if (!(puzzleType == "1" || puzzleType.Equals("TimeLockPuzzle", StringComparison.OrdinalIgnoreCase))
            || DecodeJson(artifacts) is not { ValueKind: JsonValueKind.Object } art
            || Number(art, "N") is not { } n || Number(art, "A") is not { } x || Number(art, "T") is not { } t)
            throw new UserError(L.F("Roblox asked for a check Bloxkeep can't show yet ({0}). Try again later, or sign in with your phone.",
                SafeName(puzzleType) is { Length: > 0 } kind && kind != "#" ? kind : "proofofwork"));
        for (System.Numerics.BigInteger i = 0; i < t; i++)
        {
            if ((i & 0xFFF) == 0)
                ct.ThrowIfCancellationRequested();
            x = System.Numerics.BigInteger.ModPow(x, 2, n);
        }
        return x.ToString(CultureInfo.InvariantCulture);

        static System.Numerics.BigInteger? Number(JsonElement obj, string name)
        {
            var prop = obj.EnumerateObject().FirstOrDefault(p => p.Name.Equals(name, StringComparison.OrdinalIgnoreCase));
            string? text = prop.Value.ValueKind switch
            {
                JsonValueKind.String => prop.Value.GetString(),
                JsonValueKind.Number => prop.Value.GetRawText(),
                _ => null,
            };
            return System.Numerics.BigInteger.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out var v) ? v : null;
        }
    }

    /// <summary>
    /// The layout of a JSON value for the log: field names and value kinds (text by its length), never the values.
    /// A string holding JSON (or base64 JSON) is opened up too. Three levels at most; a field name that looks like data
    /// (a number such as a user id, or free text) is written as "#".
    /// </summary>
    static string Shape(JsonElement e, int depth = 0)
    {
        if (depth >= 3 && e.ValueKind is JsonValueKind.Object or JsonValueKind.Array)
            return "…";
        switch (e.ValueKind)
        {
            case JsonValueKind.Object:
                return "{" + string.Join(", ", e.EnumerateObject().Select(p => $"{SafeName(p.Name)}: {Shape(p.Value, depth + 1)}")) + "}";
            case JsonValueKind.Array:
                return $"[{e.GetArrayLength()} x {(e.GetArrayLength() > 0 ? Shape(e[0], depth + 1) : "-")}]";
            case JsonValueKind.String:
                string s = e.GetString() ?? "";
                if ((DecodeJson(s) ?? (s.Length > 0 && s.Length % 4 == 0 ? DecodeMetadata(s) : null)) is { ValueKind: JsonValueKind.Object or JsonValueKind.Array } inner)
                    return "json" + Shape(inner, depth + 1);
                return $"text({s.Length})";
            case JsonValueKind.True or JsonValueKind.False:
                return "bool";
            default:
                return e.ValueKind.ToString().ToLowerInvariant();
        }
    }

    /// <summary>The JSON object with one string field set (added or replaced); every other field is kept as it was.</summary>
    static string WithField(string json, string name, string value)
    {
        var obj = System.Text.Json.Nodes.JsonNode.Parse(json) as System.Text.Json.Nodes.JsonObject ?? new System.Text.Json.Nodes.JsonObject();
        obj[name] = value;
        return obj.ToJsonString();
    }

    static JsonElement? DecodeJson(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            return doc.RootElement.Clone();
        }
        catch (JsonException)
        {
            return null;
        }
    }

    /// <summary>Tells Roblox a challenge was answered, then retries the sign-in carrying the answer, as roblox.com does.</summary>
    static async Task<LoginResult> ContinueLoginAsync(string realm, string challengeId, string type, string metadataJson,
        string username, string password, CancellationToken ct)
    {
        string json = JsonSerializer.Serialize(new { challengeId, challengeType = type, challengeMetadata = metadataJson });
        string retryId = challengeId, retryType = type, retryMeta = metadataJson;
        using (var res = await PostWithCsrfAsync(Realm.Api(realm, "apis") + "/challenge/v1/continue", "", json, null, ct, realm))
        {
            if (res.IsSuccessStatusCode)
            {
                // Roblox answers with the challenge the retried request must carry; use it when it's there.
                string reply = await res.Content.ReadAsStringAsync(ct);
                var answer = DecodeJson(reply);
                // roblox.com reads challengeType from every reply ("" when no check is left); without it, the answer is new.
                if (answer is not { ValueKind: JsonValueKind.Object } r || Field(r, "challengeType") is null)
                    return new LoginResult(null, null, Blocked: Unsupported($"{SafeName(type)} continue", realm, res, answer));
                string? Text(string name) => Field(r, name);
                Log.Note($"Challenge continue ({SafeName(type)}) reply: fields {string.Join(", ", r.EnumerateObject().Select(x => SafeName(x.Name)))}, type '{SafeName(Text("challengeType")!)}'");
                // Roblox can chain checks (e.g. a captcha after proof-of-work): answer the next one before retrying.
                if (Text("challengeType") is { Length: > 0 } next && !next.Equals(type, StringComparison.OrdinalIgnoreCase))
                {
                    string raw = Text("challengeMetadata") ?? "{}";
                    var nextMeta = DecodeJson(raw) ?? DecodeMetadata(raw);
                    return ParseChallenge(realm, Text("challengeId") is { Length: > 0 } nextId ? nextId : challengeId, next, nextMeta, username, password,
                        $"{SafeName(type)} continue");
                }
                if (Text("challengeId") is { Length: > 0 } id) retryId = id;
                if (Text("challengeType") is { Length: > 0 } t) retryType = t;
                if (Text("challengeMetadata") is { Length: > 0 } m) retryMeta = m;
            }
            else
            {
                // The answered check is spent: this attempt can only start over.
                string body = await res.Content.ReadAsStringAsync(ct);
                throw LoginRefused($"the {type} check", res.StatusCode, body);
            }
        }
        string meta = Convert.ToBase64String(Encoding.UTF8.GetBytes(retryMeta));
        try
        {
            return await SendLoginAsync(username, password, realm, req =>
            {
                req.Headers.TryAddWithoutValidation("rblx-challenge-id", retryId);
                req.Headers.TryAddWithoutValidation("rblx-challenge-type", retryType);
                req.Headers.TryAddWithoutValidation("rblx-challenge-metadata", meta);
            }, ct);
        }
        catch (RobloxApiException ex)
        {
            Log.Note($"Sign-in retry after {type} refused{(ex.RateLimited ? " (rate-limited)" : "")}."); // its message may carry Roblox's reply
            throw;
        }
    }

    static string TwoStepUrl(TwoStepPrompt prompt, string path) =>
        $"{Realm.Api(prompt.Realm, "twostepverification")}/v1/users/{prompt.UserId}/{path}";

    /// <summary>
    /// Learns which 2-step methods the account has, as roblox.com does before it shows the code screen (no cookie is
    /// needed: the challenge id proves the sign-in). Only methods answered with a typed code are kept, Roblox's
    /// preferred one first. When none of them is left (a security key, a passkey or approval on another device),
    /// the sign-in can't go on here.
    /// </summary>
    public static async Task<LoginResult> WithTwoStepMethodsAsync(TwoStepPrompt prompt, CancellationToken ct = default)
    {
        List<string> enabled = [];
        string primary = "";
        try
        {
            string url = TwoStepUrl(prompt, "configuration")
                + $"?challengeId={Uri.EscapeDataString(prompt.ChallengeId)}&actionType={Uri.EscapeDataString(prompt.ActionType)}";
            using var req = Request(HttpMethod.Get, url, null, fromWebsite: true, prompt.Realm);
            using var res = await Http.SendAsync(req, ct);
            string body = await res.Content.ReadAsStringAsync(ct);
            var config = DecodeJson(body);
            if (res.IsSuccessStatusCode && config is { ValueKind: JsonValueKind.Object } c
                && c.TryGetProperty("methods", out var methods) && methods.ValueKind == JsonValueKind.Array)
            {
                primary = c.TryGetProperty("primaryMediaType", out var p) ? MediaName(p) : "";
                enabled = methods.EnumerateArray()
                    .Where(m => m.ValueKind == JsonValueKind.Object && m.TryGetProperty("enabled", out var on) && on.ValueKind == JsonValueKind.True
                        && m.TryGetProperty("mediaType", out _))
                    .Select(m => MediaName(m.GetProperty("mediaType"))).Where(m => m.Length > 0).ToList();
                Log.Note($"Two-step methods: {string.Join(", ", enabled.Select(SafeName))}, preferred {SafeName(primary)}");
            }
            else
            {
                // Not needed to go on: the code screen then offers the usual methods.
                Log.Note($"Two-step methods unknown: HTTP {(int)res.StatusCode}, body {(config is { } j ? Shape(j) : "not JSON")}");
            }
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException && !ct.IsCancellationRequested)
        {
            Log.Note($"Two-step methods unknown: {ex.GetType().Name}");
        }
        return PickTwoStepMethods(prompt, enabled, primary);
    }

    /// <param name="enabled">The account's enabled methods as Roblox names them; empty when unknown.</param>
    static LoginResult PickTwoStepMethods(TwoStepPrompt prompt, List<string> enabled, string primary)
    {
        // Unknown: the method the answer named, then the ones most accounts have (unless it named one without a code).
        if (enabled.Count == 0)
        {
            if (prompt.MediaType is { } sent)
                enabled.Add(sent);
            if (prompt.MediaType is null || CodeMethod(prompt.MediaType) is not null)
                enabled.AddRange(["Authenticator", "Email"]);
        }
        var usable = enabled.Select(CodeMethod).OfType<string>().Distinct().ToList();
        if (usable.Count == 0)
        {
            Log.Note($"Two-step by {string.Join(", ", enabled.Select(SafeName))} only: not a typed code.");
            return new LoginResult(null, null, Blocked: L.T("This account confirms sign-ins with a security key, a passkey or another device, which Bloxkeep can't do. Sign in with your phone or the browser instead."));
        }
        // The method the code was already sent by (or Roblox's preferred one) comes first.
        string? first = CodeMethod(prompt.MediaType ?? "") ?? CodeMethod(primary);
        if (first is not null && usable.Remove(first))
            usable.Insert(0, first);
        return new LoginResult(null, prompt with { Methods = [.. usable] });
    }

    /// <summary>Has Roblox send a (new) code by email or SMS; the answer is just whether it was sent.</summary>
    public static async Task SendTwoStepCodeAsync(TwoStepPrompt prompt, string method, CancellationToken ct = default)
    {
        string body = JsonSerializer.Serialize(new { challengeId = prompt.ChallengeId, actionType = prompt.ActionType });
        using var res = await PostWithCsrfAsync(TwoStepUrl(prompt, $"challenges/{method}/send-code"), "", body, null, ct, prompt.Realm);
        if (res.IsSuccessStatusCode)
            return;
        throw TwoStepRefused("Two-step code", res.StatusCode, await res.Content.ReadAsStringAsync(ct));
    }

    /// <summary>
    /// A refusal from twostepverification.roblox.com, by its documented codes: 10 a wrong code (typed again), 18 an
    /// authenticator code used already, 1 a challenge that expired (the attempt is over), 5 or 429 too many tries.
    /// </summary>
    static Exception TwoStepRefused(string what, HttpStatusCode status, string body)
    {
        var (codes, _) = Errors(body);
        if (status == HttpStatusCode.TooManyRequests || codes.Contains(5))
            return new RobloxApiException(L.T("Too many sign-in attempts. Wait a few minutes and try again."), rateLimited: true);
        if (codes.Contains(10))
            return new UserError(L.T("That verification code is wrong or expired."));
        if (codes.Contains(18))
            return new UserError(L.T("That code was used already. Wait for the next one in the authenticator app."));
        if (codes.Contains(1))
        {
            Log.Note($"{what}: HTTP {(int)status}, the challenge expired");
            return new RobloxApiException(L.T("This sign-in has expired. Sign in again."), attemptOver: true);
        }
        return Refused(what, status, body);
    }

    /// <summary>
    /// Answers a two-step prompt: verifies the code with the chosen method, then finishes the sign-in the way it was
    /// asked for. Asked in the login's answer ("twoStepVerificationData"): through /v3/users/{id}/two-step-verification/login
    /// with the ticket. Asked in a challenge header: the answer goes to /challenge/v1/continue and the login is
    /// repeated carrying it, as roblox.com does; that may still end in another check.
    /// </summary>
    public static async Task<LoginResult> VerifyTwoStepAsync(TwoStepPrompt prompt, string method, string code, CancellationToken ct = default)
    {
        string token;
        if (prompt.VerificationToken is { Length: > 0 } accepted)
        {
            token = accepted;
        }
        else
        {
            string verifyBody = JsonSerializer.Serialize(new { challengeId = prompt.ChallengeId, actionType = prompt.ActionType, code });
            using (var res = await PostWithCsrfAsync(TwoStepUrl(prompt, $"challenges/{method}/verify"), "", verifyBody, null, ct, prompt.Realm))
            {
                string body = await res.Content.ReadAsStringAsync(ct);
                if (!res.IsSuccessStatusCode)
                    throw TwoStepRefused("Two-step verification", res.StatusCode, body);
                var answer = DecodeJson(body);
                if (Field(answer, "verificationToken") is not { Length: > 0 } verified)
                    throw new RobloxApiException(Unsupported("two-step verify", prompt.Realm, res, answer), attemptOver: true);
                token = verified;
            }
            prompt.VerificationToken = token;
        }

        if (prompt.MediaType is null)
            return await ContinueLoginAsync(prompt.Realm, prompt.LoginChallengeId, "twostepverification",
                JsonSerializer.Serialize(new { verificationToken = token, rememberDevice = false, challengeId = prompt.ChallengeId, actionType = prompt.ActionType }),
                prompt.Username, prompt.Password, ct);

        string finish = JsonSerializer.Serialize(new { challengeId = prompt.ChallengeId, verificationToken = token, rememberDevice = false });
        using var done = await PostWithCsrfAsync($"{Realm.Api(prompt.Realm, "auth")}/v3/users/{prompt.UserId}/two-step-verification/login",
            "", finish, null, ct, prompt.Realm);
        string doneBody = await done.Content.ReadAsStringAsync(ct);
        if (CookieFrom(done) is { } signedIn && done.IsSuccessStatusCode)
            return new LoginResult(signedIn, null);
        if (done.IsSuccessStatusCode)
        {
            var answer = DecodeJson(doneBody);
            return new LoginResult(null, null, Blocked: Field(answer, "identityVerificationLoginTicket") is { Length: > 0 }
                ? CantShow(prompt.Realm, "identity verification")
                : Unsupported("two-step finish", prompt.Realm, done, answer));
        }
        // 400: the ticket (1, 5) or the verification (10) is no longer good; only a new sign-in helps.
        if (done.StatusCode == HttpStatusCode.BadRequest)
        {
            Log.Note($"Sign-in refused at login after two-step: HTTP 400, errors {Errors(doneBody).Summary}");
            throw new RobloxApiException(L.T("This sign-in has expired. Sign in again."), attemptOver: true);
        }
        throw LoginRefused("login after two-step", done.StatusCode, doneBody);
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
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        string text = await res.Content.ReadAsStringAsync(ct);
        Log.Note($"Sign-in ticket: HTTP {(int)res.StatusCode}, errors {Errors(text).Summary}");
        // A 5xx is Roblox's problem; anything else (a ban, a challenge, a bad request) won't change by retrying.
        throw new RobloxApiException(L.F("Couldn't get a sign-in ticket (HTTP {0}). Try again in a moment.", (int)res.StatusCode),
            serverError: (int)res.StatusCode >= 500);
    }

    /// <summary>Turns a "roblox.com/share?code=…&amp;type=Server" link into the place and private-server link code.</summary>
    public static async Task<(long PlaceId, string LinkCode)> ResolvePrivateServerShareAsync(string cookie, string shareCode, CancellationToken ct = default)
    {
        string json = JsonSerializer.Serialize(new { linkId = shareCode, linkType = "Server" });
        using var res = await PostWithCsrfAsync("https://apis.roblox.com/sharelinks/v1/resolve-link", cookie, json, null, ct);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (res.StatusCode == HttpStatusCode.Unauthorized)
            throw new RobloxApiException(L.T("This account was signed out — sign in to it again."), cookieInvalid: true);
        if (!res.IsSuccessStatusCode)
        {
            Log.Note($"Private server link: HTTP {(int)res.StatusCode}, errors {Errors(body).Summary}");
            throw new RobloxApiException(L.F("Couldn't open the private server link (HTTP {0}). Check the link, or try again in a moment.", (int)res.StatusCode));
        }

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

    /// <summary>A user's friends, ids only (Roblox leaves the names empty here; see <see cref="GetUsersAsync"/>).</summary>
    public static async Task<List<long>> GetFriendIdsAsync(long userId, CancellationToken ct = default)
    {
        using var doc = await GetJsonAsync($"https://friends.roblox.com/v1/users/{userId}/friends", ct);
        if (doc?.RootElement.TryGetProperty("data", out var data) != true || data.ValueKind != JsonValueKind.Array)
            return [];
        return data.EnumerateArray()
            .Select(f => f.TryGetProperty("id", out var id) && id.TryGetInt64(out long v) ? v : 0)
            .Where(id => id > 0)
            .ToList();
    }

    /// <summary>
    /// The user with this username (not display name), or null when nobody has it or the account is banned; no cookie
    /// involved. Roblox answers {"data":[{"requestedUsername","hasVerifiedBadge","id","name","displayName"}]}, an
    /// empty list for an unknown name (checked on 2026-10-14).
    /// </summary>
    public static async Task<RobloxUser?> GetUserByNameAsync(string username, CancellationToken ct = default)
    {
        using var req = Request(HttpMethod.Post, "https://users.roblox.com/v1/usernames/users", null);
        req.Content = new StringContent(JsonSerializer.Serialize(new { usernames = new[] { username }, excludeBannedUsers = true }), Encoding.UTF8, "application/json");
        using var res = await Http.SendAsync(req, ct);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        if (!res.IsSuccessStatusCode)
            throw Refused("Username lookup", res.StatusCode, body);
        using var doc = JsonDocument.Parse(body);
        foreach (var u in doc.RootElement.GetProperty("data").EnumerateArray())
        {
            long id = u.GetProperty("id").GetInt64();
            return new RobloxUser(id, u.GetProperty("name").GetString() ?? "", u.GetProperty("displayName").GetString() ?? "");
        }
        return null;
    }

    /// <summary>Names for many users at once, 100 per request (no cookie involved); banned users are left out.</summary>
    /// <param name="strict">A refused request throws (rate limit included) instead of leaving its users out: for a lookup
    /// where "not there" must mean that Roblox knows nobody by that id.</param>
    public static async Task<Dictionary<long, RobloxUser>> GetUsersAsync(IEnumerable<long> userIds, CancellationToken ct = default, bool strict = false)
    {
        var result = new Dictionary<long, RobloxUser>();
        foreach (var chunk in userIds.Where(id => id > 0).Distinct().Chunk(100))
        {
            using var req = Request(HttpMethod.Post, "https://users.roblox.com/v1/users", null);
            req.Content = new StringContent(JsonSerializer.Serialize(new { userIds = chunk, excludeBannedUsers = true }), Encoding.UTF8, "application/json");
            using var res = await Http.SendAsync(req, ct);
            if (strict && res.StatusCode == HttpStatusCode.TooManyRequests)
                throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
            if (strict && !res.IsSuccessStatusCode)
                throw Refused("User lookup", res.StatusCode, await res.Content.ReadAsStringAsync(ct));
            if (!res.IsSuccessStatusCode)
                continue;
            using var doc = JsonDocument.Parse(await res.Content.ReadAsStringAsync(ct));
            foreach (var u in doc.RootElement.GetProperty("data").EnumerateArray())
            {
                long id = u.GetProperty("id").GetInt64();
                result[id] = new RobloxUser(id, u.GetProperty("name").GetString() ?? "", u.GetProperty("displayName").GetString() ?? "");
            }
        }
        return result;
    }

    /// <summary>
    /// Whether Roblox reports the user as banned, from the public profile anyone can read (users.roblox.com/v1/users/{id},
    /// no cookie; its "isBanned" was confirmed on 2026-10-14). Null when it couldn't tell: no such user, or an answer
    /// without the field.
    /// </summary>
    public static async Task<bool?> GetBannedAsync(long userId, string realm = Realm.Global, CancellationToken ct = default)
    {
        using var req = Request(HttpMethod.Get, Realm.Api(realm, "users") + $"/v1/users/{userId}", null);
        using var res = await Http.SendAsync(req, ct);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
        if (!res.IsSuccessStatusCode)
            return null;
        using var doc = JsonDocument.Parse(await res.Content.ReadAsStringAsync(ct));
        return doc.RootElement.ValueKind == JsonValueKind.Object && doc.RootElement.TryGetProperty("isBanned", out var banned)
            && banned.ValueKind is JsonValueKind.True or JsonValueKind.False ? banned.GetBoolean() : null;
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

    // Roblox lets one internet connection load server lists only a few times a minute (its answers say
    // "x-ratelimit-limit: 3, 3;w=60", October 2026), whatever the game or order. What it said is kept, so a request
    // that would be refused isn't sent: it would only be refused, and the wait reads from Roblox's own clock.
    static readonly object ServerListGate = new();
    static int _serverListLeft = -1;
    static DateTime _serverListReset;

    /// <summary>How many more server lists Roblox takes before its minute is up (-1: not known yet).</summary>
    public static int ServerListLeft
    {
        get { lock (ServerListGate) return DateTime.UtcNow >= _serverListReset ? -1 : _serverListLeft; }
    }

    /// <summary>
    /// A page of a game's public servers (the "Servers" tab of the game's page), busiest or emptiest first, without
    /// full ones. <paramref name="cursor"/> continues a previous page.
    /// </summary>
    public static async Task<(List<ServerInfo> Servers, string? Next)> GetServersAsync(long placeId, bool fewestFirst, string? cursor, CancellationToken ct = default)
    {
        lock (ServerListGate)
        {
            var wait = _serverListReset - DateTime.UtcNow;
            if (_serverListLeft == 0 && wait > TimeSpan.Zero)
                throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true) { RetryAfter = wait };
        }
        string url = $"https://games.roblox.com/v1/games/{placeId}/servers/Public?sortOrder={(fewestFirst ? "Asc" : "Desc")}&limit=50&excludeFullGames=true"
                     + (string.IsNullOrEmpty(cursor) ? "" : "&cursor=" + Uri.EscapeDataString(cursor));
        using var req = Request(HttpMethod.Get, url, null);
        using var res = await Http.SendAsync(req, ct);
        int? left = FirstNumber(res, "x-ratelimit-remaining");
        int? reset = FirstNumber(res, "x-ratelimit-reset") ?? (int?)res.Headers.RetryAfter?.Delta?.TotalSeconds;
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
        {
            var wait = TimeSpan.FromSeconds(Math.Clamp(reset ?? 20, 1, 120));
            lock (ServerListGate)
            {
                _serverListLeft = 0;
                _serverListReset = DateTime.UtcNow + wait;
            }
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true) { RetryAfter = wait };
        }
        if (left is int n && reset is int r)
        {
            lock (ServerListGate)
            {
                _serverListLeft = n;
                _serverListReset = DateTime.UtcNow + TimeSpan.FromSeconds(Math.Clamp(r, 0, 120));
            }
        }
        if (!res.IsSuccessStatusCode)
            return ([], null);
        using var doc = JsonDocument.Parse(await res.Content.ReadAsStringAsync(ct));
        string? next = doc.RootElement.TryGetProperty("nextPageCursor", out var c) && c.ValueKind == JsonValueKind.String ? c.GetString() : null;
        return (ReadServers(doc.RootElement), next);
    }

    /// <summary>
    /// The servers of a page of the list, without empty ones: a server whose last player has left is closing, and
    /// joining it ends on "This experience has ended, or the server became unavailable…". Seen 2026-10-17 (Steal An
    /// Egg, 7 players a server): the emptiest-first list sometimes starts with one that has no player count but still a
    /// ping (its last player's, from moments before), the very server a party of 5 was sent to.
    /// </summary>
    internal static List<ServerInfo> ReadServers(JsonElement root)
    {
        var servers = new List<ServerInfo>();
        if (!root.TryGetProperty("data", out var data) || data.ValueKind != JsonValueKind.Array)
            return servers;
        foreach (var s in data.EnumerateArray())
        {
            int Int(string name) => s.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt32(out int n) ? n : 0;
            double Num(string name) => s.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetDouble() : 0;
            if (s.TryGetProperty("id", out var id) && id.GetString() is { Length: > 0 } jobId && Int("playing") > 0)
                servers.Add(new ServerInfo(jobId, Int("playing"), Int("maxPlayers"), (int)Math.Round(Num("ping")), Math.Round(Num("fps"))));
        }
        return servers;
    }

    /// <summary>The first number of a rate-limit header ("2" of "2", "3" of "3, 3;w=60"), or null.</summary>
    static int? FirstNumber(HttpResponseMessage res, string header) =>
        res.Headers.TryGetValues(header, out var values) && values.FirstOrDefault() is { } v
        && int.TryParse(v.Split(',')[0].Trim(), System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out int n) ? n : null;

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
        // 100 ids per request (Roblox's limit); the requests go out together.
        var pages = await Task.WhenAll(userIds.Where(id => id > 0).Distinct().Chunk(100).Select(async chunk =>
        {
            string json = JsonSerializer.Serialize(new { userIds = chunk });
            using var res = await PostWithCsrfAsync(Realm.Api(realm, "presence") + "/v1/presence/users", cookie ?? "", json, null, ct, realm);
            if (res.StatusCode == HttpStatusCode.TooManyRequests)
                throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
            // A failed page would read as "everyone offline / unknown"; fail the whole check so the last answer stays.
            // 401 with a cookie: that sign-in is dead (flagged, so the caller can mark it and ask with another).
            if (!res.IsSuccessStatusCode)
                throw new RobloxApiException(L.F("Roblox refused the request (HTTP {0}). Try again in a moment.", (int)res.StatusCode),
                    cookieInvalid: res.StatusCode == HttpStatusCode.Unauthorized && !string.IsNullOrEmpty(cookie));
            return await res.Content.ReadAsStringAsync(ct);
        }));
        foreach (string? page in pages)
        {
            if (page is null)
                continue;
            using var doc = JsonDocument.Parse(page);
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
