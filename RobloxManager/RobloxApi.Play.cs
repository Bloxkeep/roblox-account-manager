using System.Globalization;
using System.Net;
using System.Text.Json;

namespace RobloxManager;

/// <summary>What opening Roblox for an account needs: its sign-in ticket, whether it may play a game, its private servers, its Robux.</summary>
public static partial class RobloxApi
{
    /// <summary>Since Sep 2026 roblox.com posts a client assertion as the authentication-ticket request's body; the older
    /// empty body is the fallback if the endpoint is unavailable.</summary>
    static async Task<string?> GetClientAssertionAsync(string cookie, CancellationToken ct, string realm = Realm.Global)
    {
        string url = Realm.Api(realm, "auth") + "/v1/client-assertion/";
        try
        {
            using (var req = Request(HttpMethod.Get, url, cookie, fromWebsite: true, realm: realm))
            using (var res = await Http.SendAsync(req, ct))
            {
                string body = (await res.Content.ReadAsStringAsync(ct)).Trim();
                if (res.IsSuccessStatusCode && body.StartsWith('{'))
                    return body;
                if (res.StatusCode != HttpStatusCode.MethodNotAllowed)
                    return null;
            }
            using var post = await PostWithCsrfAsync(url, cookie, "{}", null, ct, realm);
            string postBody = (await post.Content.ReadAsStringAsync(ct)).Trim();
            return post.IsSuccessStatusCode && postBody.StartsWith('{') ? postBody : null;
        }
        catch (HttpRequestException)
        {
            return null;
        }
    }

    /// <summary>The account's Robux via its own cookie (economy …/v1/users/{id}/currency; 401 without a sign-in, checked on
    /// both hosts 2026-10-19). Null when Roblox didn't say; 429 throws, so a round stops there.</summary>
    public static async Task<long?> GetRobuxAsync(string cookie, string realm, long userId, CancellationToken ct = default)
    {
        realm = Realm.Normalize(realm);
        using var req = Request(HttpMethod.Get, $"{Realm.Api(realm, "economy")}/v1/users/{userId.ToString(CultureInfo.InvariantCulture)}/currency", cookie,
            realm: realm);
        using var res = await Http.SendAsync(req, ct);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw RateLimitedBy(res);
        if (!res.IsSuccessStatusCode)
            return null;
        return ReadRobux(await res.Content.ReadAsStringAsync(ct));
    }

    /// <summary>The number in a currency answer, or null for anything else.</summary>
    internal static long? ReadRobux(string body)
    {
        try
        {
            using var doc = JsonDocument.Parse(body);
            return doc.RootElement.ValueKind == JsonValueKind.Object && doc.RootElement.TryGetProperty("robux", out var robux)
                   && robux.TryGetInt64(out long n) && n >= 0 ? n : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    /// <summary>One-time ticket that signs the Roblox client in as this account, from the account's own realm. VNG's client
    /// redeemed roblox.com tickets at auth.robloxapp.vnggames.com (seen 2026-10-19): one sign-in service, two names.</summary>
    public static async Task<string> GetAuthTicketAsync(string cookie, CancellationToken ct = default, string realm = Realm.Global)
    {
        realm = Realm.Normalize(realm);
        string body = await GetClientAssertionAsync(cookie, ct, realm) ?? "{}";
        // Rate-limited: waited out as Roblox says (its reset, up to a minute) and asked again, twice at most; a launch of
        // many accounts lost the ones refused instead (RoALT waits and asks again too).
        for (int attempt = 0; ; attempt++)
        {
            using var res = await PostWithCsrfAsync(Realm.Api(realm, "auth") + "/v1/authentication-ticket/", cookie, body,
                req => req.Headers.TryAddWithoutValidation("RBXAuthenticationNegotiation", "1"), ct, realm);
            if (res.StatusCode == HttpStatusCode.TooManyRequests && attempt < MaxTicketWaits && RateLimitWait(res) is { } wait && wait <= MaxTicketWait)
            {
                Log.Note($"Sign-in ticket: rate-limited, asked again in {(int)wait.TotalSeconds} s.");
                await Task.Delay(wait, ct);
                continue;
            }
            return await TicketFrom(res, realm, ct);
        }
    }

    const int MaxTicketWaits = 2;
    static readonly TimeSpan MaxTicketWait = TimeSpan.FromMinutes(1);

    static async Task<string> TicketFrom(HttpResponseMessage res, string realm, CancellationToken ct)
    {
        if (Header(res, "rbx-authentication-ticket") is { Length: > 0 } ticket)
            return ticket;
        if (res.StatusCode == HttpStatusCode.Unauthorized)
            throw new RobloxApiException(L.T("This account was signed out — sign in to it again."), cookieInvalid: true);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw RateLimitedBy(res);
        string text = await res.Content.ReadAsStringAsync(ct);
        Log.Note($"Sign-in ticket{(realm == Realm.Vng ? " (VNG)" : "")}: HTTP {(int)res.StatusCode}, errors {Errors(text).Summary}");
        // A 5xx is Roblox's problem; anything else (a ban, a warning to read, a challenge) won't change by retrying, and
        // roblox.com shows the account why.
        if ((int)res.StatusCode >= 500)
            throw new RobloxApiException(L.F("Couldn't get a sign-in ticket (HTTP {0}). Try again in a moment.", (int)res.StatusCode),
                serverError: true);
        if (IsModerated(res.StatusCode, text))
            throw new RobloxApiException(L.T("Roblox has a notice on this account to accept first. Open its roblox.com and accept it.")) { SignInRefused = true, Moderated = true };
        throw new RobloxApiException(L.F("Roblox refused to sign this account in (HTTP {0}). Open its roblox.com to see why.",
            (int)res.StatusCode)) { SignInRefused = true };
    }

    /// <summary>Whether the account may play the game (universe), as Roblox's client asks before joining
    /// (multiget-playability-status). Null when Roblox gave no answer for that game.</summary>
    public static async Task<Playability?> GetPlayabilityAsync(string cookie, string realm, long universeId, CancellationToken ct = default)
    {
        realm = Realm.Normalize(realm);
        using var doc = await GetJsonAsync(Realm.Api(realm, "games") + "/v1/games/multiget-playability-status?universeIds="
            + universeId.ToString(CultureInfo.InvariantCulture), ct, cookie);
        if (doc is null || doc.RootElement.ValueKind != JsonValueKind.Array)
            return null;
        foreach (var e in doc.RootElement.EnumerateArray())
        {
            if (e.ValueKind != JsonValueKind.Object
                || e.TryGetProperty("universeId", out var id) && id.ValueKind == JsonValueKind.Number && id.GetInt64() != universeId)
                continue;
            // The documented number, or the name Roblox's live answer gives (seen 2026-10-21: "GuestProhibited").
            int? status = e.TryGetProperty("playabilityStatus", out var s) ? s.ValueKind switch
            {
                JsonValueKind.Number when s.TryGetInt32(out int n) => n,
                JsonValueKind.String => Playability.StatusOf(s.GetString()),
                _ => null,
            } : null;
            bool playable = e.TryGetProperty("isPlayable", out var p) ? p.ValueKind == JsonValueKind.True : status == 1;
            string? text = e.TryGetProperty("unplayableDisplayText", out var w) && w.ValueKind == JsonValueKind.String
                && w.GetString() is { Length: > 0 } said ? said.Trim() : null;
            return new Playability(playable, status, text);
        }
        return null;
    }

    /// <summary>A private server the account owns or was invited to. The access code is what the website's Join button
    /// sends (RequestPrivateGame&amp;accessCode=…); JobId is its running instance, if any.</summary>
    public sealed record PrivateServer(long Id, string Name, long OwnerId, string OwnerName, string OwnerDisplayName,
        string AccessCode, string? JobId, int Playing, int MaxPlayers);

    /// <summary>The game's private servers this account can join (GET /v1/games/{placeId}/private-servers; nextPageCursor is
    /// sent back as exclusiveStartKeyCursor). At most three pages; 429 and a lost sign-in throw.</summary>
    public static async Task<List<PrivateServer>> GetPrivateServersAsync(string cookie, long placeId, CancellationToken ct = default)
    {
        var list = new List<PrivateServer>();
        string? cursor = null;
        for (int page = 0; page < 3; page++)
        {
            string url = $"https://games.roblox.com/v1/games/{placeId.ToString(CultureInfo.InvariantCulture)}/private-servers"
                + (cursor is null ? "" : "?exclusiveStartKeyCursor=" + Uri.EscapeDataString(cursor));
            using var req = Request(HttpMethod.Get, url, cookie);
            using var res = await Http.SendAsync(req, ct);
            if (res.StatusCode == HttpStatusCode.TooManyRequests)
                throw RateLimitedBy(res);
            if (res.StatusCode == HttpStatusCode.Unauthorized)
                throw new RobloxApiException(L.T("This account was signed out — sign in to it again."), cookieInvalid: true);
            if (!res.IsSuccessStatusCode)
                throw new RobloxApiException(L.F("Roblox didn't answer (HTTP {0}).", (int)res.StatusCode), serverError: (int)res.StatusCode >= 500);
            list.AddRange(ReadPrivateServers(await res.Content.ReadAsStringAsync(ct), out cursor));
            if (string.IsNullOrEmpty(cursor))
                break;
        }
        return list;
    }

    /// <summary>The servers in one page of that answer (those without an access code can't be joined and are left out).</summary>
    internal static List<PrivateServer> ReadPrivateServers(string body, out string? next)
    {
        next = null;
        var list = new List<PrivateServer>();
        try
        {
            using var doc = JsonDocument.Parse(body);
            var root = doc.RootElement;
            if (root.ValueKind != JsonValueKind.Object)
                return list;
            if (root.TryGetProperty("nextPageCursor", out var n) && n.ValueKind == JsonValueKind.String)
                next = n.GetString();
            if (!root.TryGetProperty("data", out var data) || data.ValueKind != JsonValueKind.Array)
                return list;
            static string Text(JsonElement e, string name) => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";
            static long Number(JsonElement e, string name) => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt64(out long x) ? x : 0;
            foreach (var s in data.EnumerateArray())
            {
                if (s.ValueKind != JsonValueKind.Object || Text(s, "accessCode") is not { Length: > 0 } code)
                    continue;
                var owner = s.TryGetProperty("owner", out var o) && o.ValueKind == JsonValueKind.Object ? o : default;
                bool hasOwner = owner.ValueKind == JsonValueKind.Object;
                list.Add(new PrivateServer(Number(s, "vipServerId"), Text(s, "name"),
                    hasOwner ? Number(owner, "id") : 0, hasOwner ? Text(owner, "name") : "", hasOwner ? Text(owner, "displayName") : "",
                    code, Text(s, "id") is { Length: > 0 } job ? job : null, (int)Number(s, "playing"), (int)Number(s, "maxPlayers")));
            }
        }
        catch (JsonException)
        {
        }
        return list;
    }

    /// <summary>Where a share link leads: the place, a private server's link code, an invite's server (while the invite is
    /// valid), a link's launch data. Expired links still join, as on the website.</summary>
    public sealed record ShareTarget(long PlaceId, string? LinkCode = null, string? JobId = null, string? LaunchData = null, bool Expired = false);

    /// <summary>Each share-link type's part of resolve-link's answer (the website's names; one is filled, by type).</summary>
    static readonly (string Type, string Field)[] ShareFields =
    [
        ("Server", "privateServerInviteData"), ("ExperienceInvite", "experienceInviteData"),
        ("NotificationExperienceInvite", "notificationExperienceInviteData"), ("ScreenshotInvite", "screenshotInviteData"),
        ("ExperienceDetails", "experienceDetailsInviteData"), ("ExperienceAffiliate", "experienceAffiliateData"),
        ("ExperienceEvent", "experienceEventData"),
    ];

    /// <summary>Roblox's limit on a link's launch data; longer is left out.</summary>
    const int MaxLaunchData = 200;

    /// <summary>
    /// Reads a roblox.com share link with the account's sign-in: a private server's (type=Server, the only kind until
    /// 2026-11-14), a friend's invite, a game's, an event's or a screenshot's (sharelinks/v1/resolve-link), or a "?v=v2" one
    /// (deeplinks/v2/resolve). RoALT reads them the same way.
    /// </summary>
    public static async Task<ShareTarget> ResolveShareAsync(string cookie, string shareCode, string? shareType, CancellationToken ct = default)
    {
        bool v2 = shareType == GameTarget.ShareV2;
        string type = shareType ?? "Server";
        string json = v2 ? JsonSerializer.Serialize(new { linkId = shareCode }) : JsonSerializer.Serialize(new { linkId = shareCode, linkType = type });
        using var res = await PostWithCsrfAsync(v2 ? "https://apis.roblox.com/deeplinks/v2/resolve" : "https://apis.roblox.com/sharelinks/v1/resolve-link",
            cookie, json, null, ct);
        string body = await res.Content.ReadAsStringAsync(ct);
        if (res.StatusCode == HttpStatusCode.Unauthorized)
            throw new RobloxApiException(L.T("This account was signed out — sign in to it again."), cookieInvalid: true);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw RateLimitedBy(res);
        if (!res.IsSuccessStatusCode)
        {
            Log.Note($"Share link ({type}): HTTP {(int)res.StatusCode}, errors {Errors(body).Summary}");
            throw new RobloxApiException(L.F("Couldn't open the share link (HTTP {0}). Check the link, or try again in a moment.", (int)res.StatusCode));
        }
        var invalid = new RobloxApiException(L.T("The share link is invalid or has expired."));
        using var doc = JsonDocument.Parse(body);
        var root = doc.RootElement;
        if (root.ValueKind != JsonValueKind.Object)
            throw invalid;
        if (v2)
        {
            string? status = ShareText(root, "status");
            if (Is(status, "Invalid") || ShareText(root, "linkType") != "Experience" || ShareNum(root, "targetId") is not { } target)
                throw invalid;
            string? custom = root.TryGetProperty("customData", out var c) && c.ValueKind == JsonValueKind.String ? c.GetString() : null;
            return new ShareTarget(target, LaunchData: LaunchDataOf(custom), Expired: Is(status, "Expired"));
        }
        foreach (var (kind, field) in ShareFields)
        {
            if (!root.TryGetProperty(field, out var d) || d.ValueKind != JsonValueKind.Object)
                continue;
            string? status = ShareText(d, "status");
            if (Is(status, "Invalid"))
                throw invalid;
            long place = ShareNum(d, "placeId") ?? (ShareNum(d, "universeId") is { } universe
                ? (await GetUniverseDetailsAsync([universe], ct)).GetValueOrDefault(universe).RootPlaceId : 0);
            if (place <= 0)
                throw invalid;
            bool expired = Is(status, "Expired");
            if (kind == "Server")
                return ShareText(d, "linkCode") is { } linkCode ? new ShareTarget(place, LinkCode: linkCode, Expired: expired) : throw invalid;
            // An invite joins the friend's server only while it is valid, as the website does; else the game.
            if (kind.EndsWith("ExperienceInvite", StringComparison.Ordinal) && (status is null || Is(status, "Valid"))
                && Guid.TryParse(ShareText(d, "instanceId"), out var job))
                return new ShareTarget(place, JobId: job.ToString("D"), Expired: expired);
            string? launch = ShareText(d, "launchData")
                ?? (d.TryGetProperty("joinData", out var join) && join.ValueKind == JsonValueKind.Object ? ShareText(join, "launchData") : null);
            return new ShareTarget(place, LaunchData: LaunchDataOf(launch), Expired: expired);
        }
        throw invalid;

        static bool Is(string? status, string what) => string.Equals(status, what, StringComparison.OrdinalIgnoreCase);
    }

    static string? ShareText(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String && v.GetString() is { Length: > 0 } s ? s : null;

    static long? ShareNum(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt64(out long n) && n > 0 ? n : null;

    static string? LaunchDataOf(string? data)
    {
        if (data is not { Length: > MaxLaunchData })
            return data;
        Log.Note($"Share link: its launch data ({data.Length} characters) is over Roblox's {MaxLaunchData} and was left out.");
        return null;
    }
}
