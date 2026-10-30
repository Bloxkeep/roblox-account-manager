using System.Globalization;
using System.Net;
using System.Text;
using System.Text.Json;

namespace RobloxManager;

/// <summary>Public lookups: users, pictures, games, servers and presence.</summary>
public static partial class RobloxApi
{
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

    /// <summary>The user with this username (not display name); null when nobody has it or it is banned (Roblox answers
    /// an empty "data" list for an unknown name, checked 2026-10-14).</summary>
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

    /// <summary>The public profile's "isBanned" (users.roblox.com/v1/users/{id}, no cookie; confirmed 2026-10-14). Null
    /// when it couldn't tell: no such user, or an answer without the field.</summary>
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

    /// <summary>Many games' details, keyed by universe, three requests per 50 games. Since 2026 games.roblox.com hides titles
    /// from anonymous callers, so names come from develop.roblox.com and the player count needs a cookie. Games without
    /// a name are left out (and asked again later).</summary>
    public static async Task<Dictionary<long, GameInfo>> GetGamesAsync(IEnumerable<long> universeIds, string? cookie = null, CancellationToken ct = default)
    {
        var result = new Dictionary<long, GameInfo>();
        foreach (var chunk in universeIds.Where(id => id > 0).Distinct().Chunk(50))
        {
            var details = GetUniverseDetailsAsync(chunk, ct);
            var counts = GetPlayerCountsAsync(chunk, cookie, ct);
            var icons = GetGameIconsAsync(chunk, ct);
            try
            {
                await Task.WhenAll(details, counts, icons);
            }
            catch (Exception ex) when (details.IsCompletedSuccessfully && !ct.IsCancellationRequested && IsExpectedError(ex))
            {
                // The names are what counts: a player count or icon that timed out or came back garbled is left blank instead
                // of losing the names too, which the game list then remembered as unknown for 2 minutes (review, 2026-10-30).
            }
            foreach (var (id, (name, creator, rootPlaceId)) in details.Result)
            {
                result[id] = new GameInfo(rootPlaceId, id, name, creator,
                    counts.IsCompletedSuccessfully ? counts.Result.GetValueOrDefault(id) : 0,
                    icons.IsCompletedSuccessfully ? icons.Result.GetValueOrDefault(id) : null);
            }
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

    /// <summary>
    /// A game's page (AppController.GamePage.cs): its name, maker and description from develop.roblox.com (shown to
    /// anyone), its players, visits, favourites, genre and last update from games.roblox.com (blank without a cookie
    /// since 2026; the universe's own "updated" is used then), and its likes from the votes (shown to anyone). Checked
    /// against Blox Fruits on 2026-10-29. Null when Roblox doesn't know the universe.
    /// </summary>
    public static async Task<GameDetailsInfo?> GetGameDetailsAsync(long universeId, string? cookie, CancellationToken ct = default)
    {
        var universe = GetJsonAsync($"https://develop.roblox.com/v1/universes/multiget?ids={universeId}", ct);
        var game = GetJsonAsync($"https://games.roblox.com/v1/games?universeIds={universeId}", ct, cookie);
        var votes = GetJsonAsync($"https://games.roblox.com/v1/games/votes?universeIds={universeId}", ct);
        try
        {
            await Task.WhenAll(universe, game, votes);
        }
        catch (HttpRequestException) when (universe.IsCompletedSuccessfully)
        {
            // Only the universe is needed; the figures are shown as unknown.
        }
        static JsonElement? First(Task<JsonDocument?> doc) =>
            doc.IsCompletedSuccessfully && doc.Result?.RootElement is { } root && root.TryGetProperty("data", out var data)
            && data.ValueKind == JsonValueKind.Array && data.GetArrayLength() > 0 ? data[0] : null;
        static string? Text(JsonElement? e, string name) =>
            e is { } o && o.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String && v.GetString() is { Length: > 0 } s ? s : null;
        static long Num(JsonElement? e, string name) =>
            e is { } o && o.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetInt64() : 0;
        static DateTime? When(JsonElement? e, string name) =>
            Text(e, name) is { } s && DateTime.TryParse(s, CultureInfo.InvariantCulture, DateTimeStyles.AdjustToUniversal | DateTimeStyles.AssumeUniversal, out var d)
            && d.Year > 2000 ? d : null;
        try
        {
            var u = First(universe);
            if (u is null || Text(u, "name") is not { } name)
                return null;
            // Without a cookie games.roblox.com answers with id 0 and blanks for everything.
            var g = First(game) is { } gg && Num(gg, "id") == universeId ? gg : (JsonElement?)null;
            var v = First(votes);
            long up = Num(v, "upVotes"), down = Num(v, "downVotes");
            return new GameDetailsInfo(name, Text(u, "creatorName") ?? "", Text(u, "description"), Text(g, "genre"),
                Num(g, "playing"), Num(g, "visits"), Num(g, "favoritedCount"),
                up + down > 0 ? (int)Math.Round(100.0 * up / (up + down)) : null,
                When(u, "created"), When(g, "updated") ?? When(u, "updated"), Num(u, "rootPlaceId"));
        }
        finally
        {
            foreach (var doc in new[] { universe, game, votes })
            {
                if (doc.IsCompletedSuccessfully)
                    doc.Result?.Dispose();
            }
        }
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

    // Roblox allows one connection only a few server lists a minute ("x-ratelimit-limit: 3, 3;w=60", October 2026),
    // whatever the game. What it said is kept so a request it would refuse isn't sent.
    static readonly object ServerListGate = new();
    static int _serverListLeft = -1;
    static DateTime _serverListReset;

    /// <summary>How many more server lists Roblox takes before its minute is up (-1: not known yet).</summary>
    public static int ServerListLeft
    {
        get { lock (ServerListGate) return DateTime.UtcNow >= _serverListReset ? -1 : _serverListLeft; }
    }

    /// <summary>A page of a game's public servers, busiest or emptiest first, without full ones.</summary>
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
        // A failed answer is said, not read as "no servers": the server window kept that empty list a minute after a one-off
        // 500, and a preset told "no server has room" (review, 2026-10-30).
        if (!res.IsSuccessStatusCode)
            throw new RobloxApiException(L.T("Couldn't load the server list. Try again in a moment."), serverError: (int)res.StatusCode >= 500);
        using var doc = JsonDocument.Parse(await res.Content.ReadAsStringAsync(ct));
        string? next = doc.RootElement.TryGetProperty("nextPageCursor", out var c) && c.ValueKind == JsonValueKind.String ? c.GetString() : null;
        return (ReadServers(doc.RootElement), next);
    }

    /// <summary>The page's servers without empty ones: a server whose last player left is closing, and joining it ends on
    /// "This experience has ended…". Seen 2026-10-17: the emptiest-first list sometimes started with one that had no
    /// player count but still a ping, the very server a party of 5 was sent to.</summary>
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

    /// <summary>Online / in-game status. With a cookie Roblox also reveals the game for that account and for anyone whose
    /// privacy settings allow it.</summary>
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
