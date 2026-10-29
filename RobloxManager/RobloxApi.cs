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

/// <summary>
/// Roblox's answer to whether an account can play a game (games' multiget-playability-status, which Roblox's own client
/// asks before joining): <see cref="Status"/> is its PlayabilityStatus (1 playable, 11 account restricted, 26 age check
/// required…), <see cref="Text"/> Roblox's own words for why not.
/// </summary>
public sealed record Playability(bool Playable, int? Status, string? Text)
{
    /// <summary>The names of Roblox's PlayabilityStatus numbers, as its games API documents them: its live answer gives the name.</summary>
    static readonly string[] Names =
    [
        "UnplayableOtherReason", "Playable", "GuestProhibited", "GameUnapproved", "IncorrectConfiguration", "UniverseRootPlaceIsPrivate",
        "InsufficientPermissionFriendsOnly", "InsufficientPermissionGroupOnly", "DeviceRestricted", "UnderReview", "PurchaseRequired",
        "AccountRestricted", "TemporarilyUnavailable", "PlaceHasNoPublishedVersion", "ComplianceBlocked",
        "ContextualPlayabilityRegionalAvailability", "ContextualPlayabilityRegionalCompliance",
        "ContextualPlayabilityAgeRecommendationParentalControls", "ContextualPlayabilityExperienceBlockedParentalControls",
        "ContextualPlayabilityAgeGated", "ContextualPlayabilityUnverifiedSeventeenPlusUser", "FiatPurchaseRequired",
        "FiatPurchaseDeviceRestricted", "ContextualPlayabilityUnrated", "ContextualPlayabilityAgeGatedByDescriptor",
        "ContextualPlayabilityGeneral", "ContextualPlayabilityAgeCheckRequired", "ContextualPlayabilityRequireParentApproval",
        "ContextualPlayabilityCoreGated", "ContextualPlayabilityTrustedFriendRequired", "PlusSubscriptionRequired",
        "ContextualPlayabilityPlaytestDisabled", "InsufficientPermissionEditorsOnly", "InsufficientPermissionPlayTestersOnly",
    ];

    public static int? StatusOf(string? name) => Array.IndexOf(Names, name) is >= 0 and var at ? at : null;
}

/// <summary>A Quick Login code. <see cref="PrivateKey"/> is a secret: it never leaves the app's memory.</summary>
public sealed record QuickLoginCode(string Realm, string Code, string PrivateKey, DateTime ExpiresUtc, string? ImagePath);

public sealed record QuickLoginStatus(string Status, string? AccountName, string? PictureUrl);

/// <summary>A public server of a game, as listed on the game's page.</summary>
public sealed record ServerInfo(string JobId, int Playing, int MaxPlayers, int Ping, double Fps);

/// <param name="Type">0 offline, 1 online (website/app), 2 in game, 3 in Studio, 4 invisible.</param>
public sealed record Presence(int Type, string? Location, long? PlaceId, long? RootPlaceId, string? JobId, DateTime? LastOnline, long? UniverseId);

/// <summary>
/// A sign-in cookie Roblox replaced: a request carried <see cref="Sent"/>, and Roblox's answer set <see cref="Renewed"/>
/// in its place. Both are secrets, so <see cref="ToString"/> leaves them out.
/// </summary>
public sealed record RenewedCookie(string Realm, string Sent, string Renewed)
{
    public override string ToString() => $"RenewedCookie ({(Realm == RobloxManager.Realm.Vng ? "Roblox VNG" : "Roblox")})";
}

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

    /// <summary>Roblox wouldn't give the account a sign-in ticket (4xx, not 401/429): its roblox.com says why.</summary>
    public bool SignInRefused { get; init; }

    /// <summary>
    /// Roblox holds the account until a moderation notice (a warning, a ban that ended) is accepted on its roblox.com:
    /// HTTP 403, "User is moderated" (seen 2026-10-29 for an account warned that day, on the sign-in check).
    /// </summary>
    public bool Moderated { get; init; }
}

/// <summary>
/// Talks only to official roblox.com endpoints. The cookie is attached per request and never stored by the HTTP stack;
/// a new one Roblox sets in an answer is queued for the app (<see cref="TryTakeRenewedCookie"/>), which saves it.
/// </summary>
public static partial class RobloxApi
{
    const string UserAgent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
    const string CookieName = ".ROBLOSECURITY";

    static readonly HttpClient Http = new(new CookieWatch(new SocketsHttpHandler
    {
        UseCookies = false,
        AllowAutoRedirect = false,
        AutomaticDecompression = DecompressionMethods.All,
        PooledConnectionLifetime = TimeSpan.FromMinutes(5),
        // Connect over IPv4 only. An auth ticket is bound to the IP that requested it; the Roblox client redeems it
        // over IPv4, so if this app fetched it over IPv6 the IPs wouldn't match and the redeem would be rejected.
        ConnectCallback = Ipv4ConnectAsync,
    }))
    {
        Timeout = TimeSpan.FromSeconds(20),
    };

    /// <summary>The checks' Roblox (Tests/Bloxkeep.Tests): when set, it answers every request instead of the network. The app never sets it.</summary>
    internal static Func<HttpRequestMessage, Task<HttpResponseMessage>>? StandIn { get; set; }

    /// <summary>
    /// Cookies Roblox renewed, waiting for the app to save them to their accounts (AppController.SignIns.cs). Roblox
    /// sends a new .ROBLOSECURITY in Set-Cookie every few hours to days, not on every request, and some months later
    /// refuses the one it replaced (devforum.roblox.com/t/4328913, announced 2026-02-03, enforced from 2026-05-01).
    /// </summary>
    static readonly ConcurrentQueue<RenewedCookie> Renewals = new();

    public static bool TryTakeRenewedCookie([System.Diagnostics.CodeAnalysis.NotNullWhen(true)] out RenewedCookie? renewed) =>
        Renewals.TryDequeue(out renewed);

    public static bool RenewedCookiesWaiting => !Renewals.IsEmpty;

    /// <summary>Every answer from Roblox passes here: one to a request that carried a sign-in cookie may set a new one.</summary>
    sealed class CookieWatch(HttpMessageHandler network) : DelegatingHandler(network)
    {
        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            var res = StandIn is { } standIn ? await standIn(request) : await base.SendAsync(request, ct);
            if (RenewedBy(request, res) is { } renewed)
                Renewals.Enqueue(renewed);
            return res;
        }
    }

    /// <summary>
    /// The cookie a Roblox answer sets in place of the one its request carried; null when no cookie went, none came
    /// back, the same one came back, or the answer deletes it (signing out sets an empty one, expired in 1970).
    /// </summary>
    internal static RenewedCookie? RenewedBy(HttpRequestMessage request, HttpResponseMessage res)
    {
        if (request.RequestUri is not { } uri || !res.Headers.TryGetValues("Set-Cookie", out var setCookies)
            || !request.Headers.TryGetValues("Cookie", out var cookies)
            || cookies.FirstOrDefault(c => c.Contains(CookieName + "=", StringComparison.Ordinal)) is not { } carried)
            return null;
        // Only Roblox's own hosts set its sign-in cookie (and every request here goes to them anyway).
        string? realm = uri.Host.EndsWith(".robloxapp.vnggames.com", StringComparison.OrdinalIgnoreCase) ? Realm.Vng
            : uri.Host.EndsWith(".roblox.com", StringComparison.OrdinalIgnoreCase) ? Realm.Global : null;
        string sent = NormalizeCookie(carried);
        // The last one wins, as in a browser.
        string? renewed = setCookies.Select(SetsCookie).LastOrDefault(v => v is not null);
        return realm is not null && sent.Length > 0 && renewed is not null && renewed != sent ? new RenewedCookie(realm, sent, renewed) : null;
    }

    /// <summary>The value a Set-Cookie header gives .ROBLOSECURITY; null for another cookie, an empty value, or one already expired.</summary>
    static string? SetsCookie(string header)
    {
        string[] parts = header.Split(';');
        int eq = parts[0].IndexOf('=');
        if (eq < 0 || parts[0][..eq].Trim() != CookieName)
            return null;
        foreach (string part in parts.Skip(1))
        {
            int at = part.IndexOf('=');
            if (at < 0)
                continue;
            string name = part[..at].Trim(), value = part[(at + 1)..].Trim();
            if (name.Equals("max-age", StringComparison.OrdinalIgnoreCase)
                && long.TryParse(value, NumberStyles.AllowLeadingSign, CultureInfo.InvariantCulture, out long age) && age <= 0)
                return null;
            if (name.Equals("expires", StringComparison.OrdinalIgnoreCase)
                && DateTime.TryParseExact(value, ["ddd, dd-MMM-yyyy HH:mm:ss 'GMT'", "ddd, dd MMM yyyy HH:mm:ss 'GMT'", "r"], CultureInfo.InvariantCulture,
                    DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal, out var expires)
                && expires <= DateTime.UtcNow)
                return null;
        }
        string cookie = parts[0][(eq + 1)..].Trim().Trim('"');
        return cookie.Length > 0 ? cookie : null;
    }

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
    /// <summary>A 403 whose error says the account is moderated (<see cref="RobloxApiException.Moderated"/>).</summary>
    static bool IsModerated(HttpStatusCode status, string body) =>
        status == HttpStatusCode.Forbidden && Errors(body).Summary.Contains("moderated", StringComparison.OrdinalIgnoreCase);

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
        if (IsModerated(status, body))
            return new RobloxApiException(L.T("Roblox has a notice on this account to accept first. Open its roblox.com and accept it.")) { Moderated = true };
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
    /// <summary>
    /// Roblox's answer to letting an account into a game (gamejoin's join-game, what a Roblox window asks when it joins):
    /// its HTTP status, its own status number, its message, and its statusData (raw, cut short), whatever that holds.
    /// The join ticket a successful answer holds is left unread.
    /// </summary>
    /// <param name="QueuePosition">The account's place in a full game's line, when Roblox gives one (its client shows "Position in queue is N").</param>
    public sealed record JoinAnswer(int Http, int? Status, string? Message, string? Data = null, int? QueuePosition = null)
    {
        /// <summary>Roblox's bot check holding this join (a 403 with its rblx-challenge-* headers), or null.</summary>
        public JoinChallenge? Challenge { get; init; }

        /// <summary>
        /// The server or game is full (status 6: Roblox's client says "Requested experience is full" for it, and its
        /// ConnectionError 518 is PlacelaunchGameFull, 512 + the status, as 524 is for 12).
        /// </summary>
        public bool Full => Status == 6;

        /// <summary>
        /// The server has ended (status 5: Roblox's ConnectionError 517 is PlacelaunchGameEnded, 512 + the status, as 518
        /// is PlacelaunchGameFull for 6; Roblox's API dump).
        /// </summary>
        public bool Ended => Status == 5 || ServerGone;

        /// <summary>
        /// The join named a server (a Job ID) and Roblox refused it without a ban: that server is gone, not the game closed
        /// to the account. Seen 2026-10-29: a party sent to its first account's server after that server ended with a
        /// system error ("This experience has ended, or the server became unavailable unexpectedly…", the window's
        /// PlacelaunchGameEnded) got status 12, "Unable to join Game 311", every time it was asked. Counted as ended, so
        /// the accounts go to another server instead of not being tried again. Set by the join check, which knows the target.
        /// </summary>
        public bool ServerGone { get; init; }

        /// <summary>
        /// Roblox refuses the account this game: one of the statuses its own client counts as a join error (its setting
        /// FStringGameJoinStatusConsideredError, "12;29;30;31;32;33;" in 0.744), not a wait. Asked again, it answers the
        /// same. A game's ban is one of them (see <see cref="Banned"/>).
        /// </summary>
        public bool Refused => !ServerGone && Status is 12 or (>= 29 and <= 33);

        /// <summary>The number in "Unable to join Game 211": what the window turns into its own error.</summary>
        public int? Code => Message is { } m && System.Text.RegularExpressions.Regex.Match(m, @"Unable to join Game (\d+)") is { Success: true } found
            && int.TryParse(found.Groups[1].Value, NumberStyles.None, CultureInfo.InvariantCulture, out int code) ? code : null;

        /// <summary>
        /// The game (or its moderators) banned the account. Seen 2026-10-21 for an account a game had banned (its window
        /// showed Join Error 600, "You were banned by this experience or its moderators"): status 12, "Unable to join
        /// Game 211". Words about a ban count too.
        /// </summary>
        public bool Banned => (Status == 12 && Code == 211)
            || Message is { } m && (m.Contains("banned", StringComparison.OrdinalIgnoreCase) || m.Contains("bị cấm", StringComparison.OrdinalIgnoreCase));

        /// <summary>
        /// The game's own words when Roblox gives them: after "Moderation message:", or a message or reason in statusData;
        /// null otherwise (the 211 answer carries none).
        /// </summary>
        public string? Reason
        {
            get
            {
                if (Message is { } m && m.IndexOf("Moderation message:", StringComparison.OrdinalIgnoreCase) is >= 0 and var at
                    && m[(at + 19)..].Trim() is { Length: > 0 } words)
                    return words;
                if (Data is not { } data || !data.TrimStart().StartsWith('{'))
                    return null;
                try
                {
                    using var doc = JsonDocument.Parse(data);
                    foreach (var p in doc.RootElement.EnumerateObject())
                        if (p.Value.ValueKind == JsonValueKind.String && p.Value.GetString() is { Length: > 0 } said
                            && (p.Name.Contains("message", StringComparison.OrdinalIgnoreCase) || p.Name.Contains("reason", StringComparison.OrdinalIgnoreCase)))
                            return said;
                }
                catch (JsonException)
                {
                }
                return null;
            }
        }
    }

    /// <summary>
    /// Asks Roblox to let the account into a game (a server of it, or that one server), only to read its answer: Bloxkeep
    /// asks after a window has waited a minute without getting in, when Roblox logs nothing about why.
    /// </summary>
    /// <param name="answered">A check answered (<see cref="ContinueChallengeAsync"/>): the join is asked again carrying it,
    /// as roblox.com retries a request once its check is passed.</param>
    public static async Task<JoinAnswer> JoinCheckAsync(string cookie, long placeId, string? jobId, string realm, CancellationToken ct = default,
        JoinChallenge? answered = null)
    {
        string url = Realm.Api(realm, "gamejoin") + (jobId is null ? "/v1/join-game" : "/v1/join-game-instance");
        object body = jobId is null
            ? new { placeId, isTeleport = false, gameJoinAttemptId = Guid.NewGuid().ToString() }
            : new { placeId, gameId = jobId, isTeleport = false, gameJoinAttemptId = Guid.NewGuid().ToString() };
        using var res = await PostWithCsrfAsync(url, cookie, JsonSerializer.Serialize(body), answered is null ? null : req =>
        {
            req.Headers.TryAddWithoutValidation("rblx-challenge-id", answered.Id);
            req.Headers.TryAddWithoutValidation("rblx-challenge-type", answered.Type);
            req.Headers.TryAddWithoutValidation("rblx-challenge-metadata", Convert.ToBase64String(Encoding.UTF8.GetBytes(answered.MetadataJson)));
        }, ct, realm);
        string text = await res.Content.ReadAsStringAsync(ct);
        // Roblox's bot check ("Challenge is required to authorize the request", 403): which check, and what it carries, by
        // name only (its values tie it to this account).
        JoinChallenge? challenge = null;
        if (Header(res, "rblx-challenge-id") is { Length: > 0 } challengeId)
        {
            var meta = DecodeMetadata(Header(res, "rblx-challenge-metadata"));
            challenge = new JoinChallenge(challengeId, Header(res, "rblx-challenge-type") ?? "", meta?.GetRawText() ?? "{}");
            // The action and the request it holds are Roblox's own names, nothing of the account's: said as they are.
            static string Plain(string? s) => s is { Length: > 0 and <= 60 } && s.All(c => char.IsAsciiLetterOrDigit(c) || c is '/' or '-' or '_' or '.') ? s : "#";
            Log.Note($"Join held for Roblox's check: type '{SafeName(challenge.Type)}', metadata fields "
                + (meta is { ValueKind: JsonValueKind.Object } m ? string.Join(", ", m.EnumerateObject().Select(x => SafeName(x.Name))) : "(none)")
                + $"; action '{Plain(challenge.Meta("actionType"))}', request {Plain(challenge.Meta("requestMethod"))} {Plain(challenge.Meta("requestPath"))}");
        }
        int? status = null, queue = null;
        string? message = null, data = null;
        if (text.TrimStart().StartsWith('{'))
        {
            using var doc = JsonDocument.Parse(text);
            var root = doc.RootElement;
            if (root.TryGetProperty("status", out var s) && s.ValueKind == JsonValueKind.Number)
                status = s.GetInt32();
            if (root.TryGetProperty("queuePosition", out var q) && q.ValueKind == JsonValueKind.Number && q.TryGetInt32(out int place) && place > 0)
                queue = place;
            if (root.TryGetProperty("message", out var m) && m.ValueKind == JsonValueKind.String)
                message = m.GetString();
            // A refusal of the request itself: {"errors":[{"code":…,"message":…}]}.
            else if (root.TryGetProperty("errors", out var errors) && errors.ValueKind == JsonValueKind.Array && errors.GetArrayLength() > 0
                     && errors[0].TryGetProperty("message", out var e) && e.ValueKind == JsonValueKind.String)
                message = e.GetString();
            // Never with a join ticket: only an answer that let the account in holds one, and that one is left unread.
            if (status != 2 && root.TryGetProperty("statusData", out var d) && d.ValueKind is JsonValueKind.Object or JsonValueKind.String)
                data = d.GetRawText() is { Length: > 400 } raw ? raw[..400] : d.GetRawText();
        }
        return new JoinAnswer((int)res.StatusCode, status, message, data, queue) { Challenge = challenge };
    }

    /// <summary>
    /// The check Roblox put on a join (its rblx-challenge-* headers): its id, its kind ("captcha", "captchav2",
    /// "proofofwork"…) and what it carries, as JSON.
    /// </summary>
    public sealed record JoinChallenge(string Id, string Type, string MetadataJson)
    {
        /// <summary>A text field of what the check carries, or null.</summary>
        public string? Meta(string name) => Field(DecodeJson(MetadataJson), name);
        /// <summary>What it carries with one field added (the answer: a redemption or captcha token).</summary>
        public string MetaWith(string name, string value) => WithField(MetadataJson, name, value);
    }

    /// <summary>
    /// Hands Roblox a check's answer with the account's own sign-in, as roblox.com does, and returns the check the
    /// retried request carries (Roblox echoes it, sometimes with a new id); a different kind of check after it comes
    /// back as that one, to answer in turn.
    /// </summary>
    public static async Task<JoinChallenge> ContinueChallengeAsync(string realm, string cookie, JoinChallenge answered, CancellationToken ct = default)
    {
        string json = JsonSerializer.Serialize(new { challengeId = answered.Id, challengeType = answered.Type, challengeMetadata = answered.MetadataJson });
        using var res = await PostWithCsrfAsync(Realm.Api(realm, "apis") + "/challenge/v1/continue", cookie, json, null, ct, realm);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (!res.IsSuccessStatusCode)
            throw Refused("Check answer", res.StatusCode, body);
        var reply = DecodeJson(body);
        Log.Note($"Check answer ({SafeName(answered.Type)}) reply: fields "
            + (reply is { ValueKind: JsonValueKind.Object } r ? string.Join(", ", r.EnumerateObject().Select(x => SafeName(x.Name))) : "(none)")
            + $", type '{SafeName(Field(reply, "challengeType") ?? "")}'");
        return new JoinChallenge(Field(reply, "challengeId") is { Length: > 0 } id ? id : answered.Id,
            Field(reply, "challengeType") is { Length: > 0 } type ? type : answered.Type,
            Field(reply, "challengeMetadata") is { Length: > 0 } meta ? meta : answered.MetadataJson);
    }

    /// <summary>The FunCaptcha key Roblox uses for <paramref name="actionType"/> (its captcha settings name each action), or null.</summary>
    public static async Task<string?> GetCaptchaKeyAsync(string realm, string actionType, CancellationToken ct = default)
    {
        using var req = Request(HttpMethod.Get, Realm.Api(realm, "apis") + "/captcha/v1/metadata", null);
        using var res = await Http.SendAsync(req, ct);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (!res.IsSuccessStatusCode)
            throw Refused("Captcha settings", res.StatusCode, body);
        using var doc = JsonDocument.Parse(body);
        if (!doc.RootElement.TryGetProperty("funCaptchaPublicKeys", out var keys) || keys.ValueKind != JsonValueKind.Object)
            return null;
        // Compared without the underscores: "WebLogin" is ACTION_TYPE_WEB_LOGIN, "Generic" ACTION_TYPE_GENERIC_CHALLENGE (a
        // name it starts). Roblox lists no key of a join's own (2026-10-27): a join's check, seen as "captcha" in the
        // client's log, takes the generic one when nothing else fits.
        static string Plain(string s) => new(s.Where(char.IsAsciiLetterOrDigit).Select(char.ToUpperInvariant).ToArray());
        string wanted = "ACTIONTYPE" + Plain(actionType);
        var all = keys.EnumerateObject().Where(k => k.Value.ValueKind == JsonValueKind.String).ToList();
        var key = all.FirstOrDefault(k => Plain(k.Name) == wanted) is { Value.ValueKind: JsonValueKind.String } exact ? exact
            : all.FirstOrDefault(k => Plain(k.Name).StartsWith(wanted, StringComparison.Ordinal)) is { Value.ValueKind: JsonValueKind.String } starts ? starts
            : all.FirstOrDefault(k => k.Name == "ACTION_TYPE_GENERIC_CHALLENGE");
        Log.Note($"Captcha key for '{SafeName(actionType)}': {(key.Value.ValueKind == JsonValueKind.String ? "found" : "none")} among "
            + string.Join(", ", keys.EnumerateObject().Select(k => SafeName(k.Name))));
        return key.Value.ValueKind == JsonValueKind.String ? key.Value.GetString() : null;
    }

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
}
