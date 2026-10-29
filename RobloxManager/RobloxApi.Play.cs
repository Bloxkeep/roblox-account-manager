using System.Globalization;
using System.Net;
using System.Text.Json;

namespace RobloxManager;

/// <summary>What opening Roblox for an account needs: its sign-in ticket, whether it may play a game, its private servers, its Robux.</summary>
public static partial class RobloxApi
{
    /// <summary>
    /// Since Sep 2026 roblox.com first fetches a client assertion and posts it as the body of the
    /// authentication-ticket request. Falls back to the older empty body if the endpoint is unavailable.
    /// </summary>
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

    /// <summary>
    /// The account's Robux, asked with its own cookie (economy …/v1/users/{id}/currency answers {"robux": n} to the
    /// account itself; without a sign-in it answers 401, checked on both hosts on 2026-10-19). Null when Roblox didn't
    /// say: signed out, a refusal, or an answer in another shape. Too many requests (429) throws, so a round stops there.
    /// </summary>
    public static async Task<long?> GetRobuxAsync(string cookie, string realm, long userId, CancellationToken ct = default)
    {
        realm = Realm.Normalize(realm);
        using var req = Request(HttpMethod.Get, $"{Realm.Api(realm, "economy")}/v1/users/{userId.ToString(CultureInfo.InvariantCulture)}/currency", cookie,
            realm: realm);
        using var res = await Http.SendAsync(req, ct);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
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

    /// <summary>
    /// One-time ticket that lets the Roblox client sign in as this account, from the account's own Roblox (a Roblox VNG
    /// account's from Roblox VNG's sign-in service). Roblox VNG's client redeemed tickets from roblox.com at
    /// auth.robloxapp.vnggames.com (seen 2026-10-19), so the two look like one sign-in service under two names.
    /// </summary>
    public static async Task<string> GetAuthTicketAsync(string cookie, CancellationToken ct = default, string realm = Realm.Global)
    {
        realm = Realm.Normalize(realm);
        string body = await GetClientAssertionAsync(cookie, ct, realm) ?? "{}";
        using var res = await PostWithCsrfAsync(Realm.Api(realm, "auth") + "/v1/authentication-ticket/", cookie, body,
            req => req.Headers.TryAddWithoutValidation("RBXAuthenticationNegotiation", "1"), ct, realm);

        if (Header(res, "rbx-authentication-ticket") is { Length: > 0 } ticket)
            return ticket;
        if (res.StatusCode == HttpStatusCode.Unauthorized)
            throw new RobloxApiException(L.T("This account was signed out — sign in to it again."), cookieInvalid: true);
        if (res.StatusCode == HttpStatusCode.TooManyRequests)
            throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
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

    /// <summary>
    /// Whether Roblox lets the account play the game (universe), as its own client asks before joining: GET games'
    /// /v1/games/multiget-playability-status with the account's cookie. Null when Roblox gave no answer for that game.
    /// </summary>
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

    /// <summary>
    /// A private server the account can join: one it owns or was invited to. Its access code is what the website's Join
    /// button sends (PlaceLauncher RequestPrivateGame&amp;accessCode=…); JobId is its running instance, if any.
    /// </summary>
    public sealed record PrivateServer(long Id, string Name, long OwnerId, string OwnerName, string OwnerDisplayName,
        string AccessCode, string? JobId, int Playing, int MaxPlayers);

    /// <summary>
    /// The game's private servers this account can join, as the Servers tab of its Roblox page lists them: games'
    /// GET /v1/games/{placeId}/private-servers with the account's cookie (Roblox's API reference, creator-docs
    /// openapi.json: data[] of id, name, vipServerId, accessCode, owner, playing, maxPlayers; nextPageCursor, asked
    /// again as exclusiveStartKeyCursor). At most three pages. Too many requests (429) and a lost sign-in throw.
    /// </summary>
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
                throw new RobloxApiException(L.T("Roblox is rate-limiting requests; try again shortly."), rateLimited: true);
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
}
