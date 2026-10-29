namespace RobloxManager;

/// <summary>The game box: what a link, Place ID or name stands for, and the games kept (recent, pinned, saved).</summary>
public sealed partial class AppController
{
    static readonly TimeSpan GameRetry = TimeSpan.FromMinutes(2);
    /// <summary>The longest a place Roblox keeps answering without (unpublished, private) waits before it is asked again.</summary>
    static readonly TimeSpan MaxGameRetry = TimeSpan.FromHours(1);
    const int MaxCachedGames = 1000;
    /// <summary>Lookups in a row that Roblox answered without the place: each one doubles the wait before the next.</summary>
    readonly Dictionary<long, int> _gameMisses = new();

    /// <summary>Whether the cache can answer for a place; <paramref name="fresh"/> wants a live player count.</summary>
    bool HasGame(long placeId, bool fresh) =>
        _games.TryGetValue(placeId, out var cached)
        && ((cached is not null && !fresh) || DateTime.UtcNow - _gameFetchedAt.GetValueOrDefault(placeId) < GameRetryFor(placeId, cached, fresh));

    /// <summary>How long a lookup holds: two minutes; for a place Roblox answered without (asked by every status check while a
    /// client is in it) 2, 4, 8 … up to 60 minutes. A lookup the person asked for (fresh) waits two minutes at most.</summary>
    TimeSpan GameRetryFor(long placeId, GameInfo? cached, bool fresh = false) =>
        !fresh && cached is null && _gameMisses.TryGetValue(placeId, out int misses)
            ? TimeSpan.FromTicks(Math.Min(MaxGameRetry.Ticks, GameRetry.Ticks << Math.Min(misses - 1, 6)))
            : GameRetry;

    /// <summary>Fills the game cache in one batch; places whose universe isn't known from presence are looked up first.</summary>
    async Task LoadGamesAsync(IEnumerable<(long PlaceId, long UniverseId)> places, bool fresh = false)
    {
        var wanted = places.Where(p => p.PlaceId > 0 && !HasGame(p.PlaceId, fresh)).DistinctBy(p => p.PlaceId).ToList();
        if (wanted.Count == 0)
            return;
        var now = DateTime.UtcNow;
        if (_games.Count > MaxCachedGames)
        {
            foreach (long old in _games.Keys.OrderBy(id => _gameFetchedAt.GetValueOrDefault(id)).Take(_games.Count - MaxCachedGames / 2).ToList())
            {
                _games.Remove(old);
                _gameFetchedAt.Remove(old);
                _gameMisses.Remove(old);
            }
        }
        foreach (var (placeId, _) in wanted)
            _gameFetchedAt[placeId] = now;
        try
        {
            // Roblox has no batch lookup from place to universe, so the unknown ones are asked a few at a time.
            var universeOf = new Dictionary<long, long>();
            using (var gate = new SemaphoreSlim(4))
            {
                var lookups = await Task.WhenAll(wanted.Select(async w =>
                {
                    long known = w.UniverseId > 0 ? w.UniverseId : _games.GetValueOrDefault(w.PlaceId)?.UniverseId ?? 0;
                    if (known > 0)
                        return (w.PlaceId, known);
                    await gate.WaitAsync();
                    try
                    {
                        return (w.PlaceId, await RobloxApi.GetUniverseIdAsync(w.PlaceId));
                    }
                    finally
                    {
                        gate.Release();
                    }
                }));
                foreach (var (placeId, universeId) in lookups)
                    universeOf[placeId] = universeId;
            }
            var games = await RobloxApi.GetGamesAsync(universeOf.Values, AnyGlobalCookie());
            foreach (var (placeId, universeId) in universeOf)
            {
                // A missing answer (a request failed) keeps what was known before instead of forgetting the name.
                _games[placeId] = games.TryGetValue(universeId, out var info)
                    ? info with { PlaceId = placeId, IconUrl = info.IconUrl ?? _games.GetValueOrDefault(placeId)?.IconUrl }
                    : _games.GetValueOrDefault(placeId);
                if (_games[placeId] is null)
                    _gameMisses[placeId] = _gameMisses.GetValueOrDefault(placeId) + 1;
                else
                    _gameMisses.Remove(placeId);
            }
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            foreach (var (placeId, _) in wanted)
                _games.TryAdd(placeId, null);
        }
    }

    /// <summary>A sign-in for roblox.com's own services (a Roblox VNG one only works there), for calls any account can make.</summary>
    string? AnyGlobalCookie() =>
        _vault?.Data.Accounts.FirstOrDefault(a => a.CookieValid != false && a.Cookie.Length > 0 && a.Realm == Realm.Global)?.Cookie;

    async Task<GameInfo?> GetGameInfoAsync(long placeId, bool fresh = false)
    {
        await LoadGamesAsync([(placeId, 0)], fresh);
        return _games.GetValueOrDefault(placeId);
    }

    // Roblox gives a few server lists a minute (RobloxApi.ServerListLeft): switching order back and forth reuses what's loaded.
    static readonly TimeSpan ServerListLifetime = TimeSpan.FromSeconds(60);
    readonly Dictionary<(long, bool, string), (List<ServerInfo> Servers, string? Next, DateTime At)> _serverPages = new();

    /// <summary>A page of a game's public servers, kept a minute. left: how many more Roblox would give now (-1: unknown). When
    /// Roblox says to wait, busy with its seconds (retryIn); the window retries by itself then.</summary>
    async Task<object?> ServersAsync(long placeId, bool fewestFirst, string? cursor)
    {
        if (placeId <= 0)
            throw new UserError(L.T("Invalid Place ID."));
        var key = (placeId, fewestFirst, cursor ?? "");
        if (_serverPages.TryGetValue(key, out var kept) && DateTime.UtcNow - kept.At < ServerListLifetime)
            return new { servers = kept.Servers, next = kept.Next, busy = false, left = RobloxApi.ServerListLeft };
        try
        {
            var (servers, next) = await RobloxApi.GetServersAsync(placeId, fewestFirst, cursor);
            if (_serverPages.Count > 40)
                _serverPages.Clear();
            _serverPages[key] = (servers, next, DateTime.UtcNow);
            return new { servers, next, busy = false, left = RobloxApi.ServerListLeft };
        }
        catch (RobloxApiException ex) when (ex.RateLimited)
        {
            int retryIn = (int)Math.Ceiling((ex.RetryAfter ?? TimeSpan.FromSeconds(20)).TotalSeconds);
            return new { servers = Array.Empty<ServerInfo>(), next = (string?)null, busy = true, retryIn, left = 0 };
        }
    }

    static readonly TimeSpan FriendServerLifetime = TimeSpan.FromSeconds(45);
    /// <summary>A friend found in a game: who, where, and which of your accounts saw them (named when shown, so hiding names applies).</summary>
    sealed record FriendFound(long UserId, string Name, string DisplayName, string? AvatarUrl, string JobId, long PlaceId, Guid Via);
    /// <summary>Kept for a moment per game and per accounts asked (another selection asks other accounts).</summary>
    readonly Dictionary<(long PlaceId, string Viewers), (List<FriendFound> Friends, int Asked, DateTime At)> _friendServers = new();
    /// <summary>Who each account's friends are changes rarely; where they are is asked afresh.</summary>
    static readonly TimeSpan FriendIdsLifetime = TimeSpan.FromMinutes(10);
    readonly Dictionary<long, (List<long> Ids, DateTime At)> _friendIds = new();

    async Task<List<long>> FriendIdsAsync(long userId)
    {
        if (_friendIds.TryGetValue(userId, out var kept) && DateTime.UtcNow - kept.At < FriendIdsLifetime)
            return kept.Ids;
        var ids = await RobloxApi.GetFriendIdsAsync(userId);
        _friendIds[userId] = (ids, DateTime.UtcNow);
        return ids;
    }

    /// <summary>Friends of your accounts playing this game, with their server. Each account asks with its own cookie: Roblox gives
    /// the server only when that friend's privacy lets them join. At most 3 accounts (selected first) to keep requests few.</summary>
    async Task<object?> FriendServersAsync(long placeId, Guid[] preferred)
    {
        var vault = RequireVault();
        if (placeId <= 0)
            throw new UserError(L.T("Invalid Place ID."));
        var mine = vault.Data.Accounts.Select(a => a.UserId).ToHashSet();
        var viewers = vault.Data.Accounts
            .Where(a => a.UserId > 0 && a.CookieValid != false && a.Cookie.Length > 0 && a.Realm == Realm.Global)
            .OrderBy(a => preferred.Contains(a.Id) ? 0 : 1)
            .Take(3)
            .ToList();
        var key = (placeId, string.Join(",", viewers.Select(v => v.Id)));
        if (_friendServers.TryGetValue(key, out var kept) && DateTime.UtcNow - kept.At < FriendServerLifetime)
            return FriendServersDto(kept.Friends, kept.Asked);
        var found = new Dictionary<long, (string JobId, long PlaceId, Guid Via)>();
        await Task.WhenAll(viewers.Select(async viewer =>
        {
            try
            {
                var friends = (await FriendIdsAsync(viewer.UserId)).Where(id => !mine.Contains(id)).ToList();
                if (friends.Count == 0)
                    return;
                foreach (var (id, p) in await RobloxApi.GetPresenceAsync(viewer.Cookie, friends))
                {
                    if (p.Type == 2 && p.JobId is { Length: > 0 } job && (p.PlaceId == placeId || p.RootPlaceId == placeId))
                    {
                        lock (found)
                            found.TryAdd(id, (job, p.PlaceId ?? placeId, viewer.Id));
                    }
                }
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                // That account's friends are just left out.
            }
        }));
        var ids = found.Keys.ToList();
        // Names and pictures are extras: without them the friends still show (by id) and can be joined.
        Dictionary<long, RobloxUser> users = [];
        Dictionary<long, string> faces = [];
        try
        {
            if (ids.Count > 0)
            {
                var named = RobloxApi.GetUsersAsync(ids);
                var pictured = RobloxApi.GetHeadshotUrlsAsync(ids);
                users = await named;
                faces = await pictured;
            }
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
        }
        var friends = found
            // The server belongs to this place, which can be another place of the game than the one asked.
            .Select(f => new FriendFound(f.Key, users.GetValueOrDefault(f.Key)?.Name ?? "", users.GetValueOrDefault(f.Key)?.DisplayName ?? "",
                faces.GetValueOrDefault(f.Key), f.Value.JobId, f.Value.PlaceId, f.Value.Via))
            .OrderBy(f => f.DisplayName, StringComparer.CurrentCultureIgnoreCase)
            .ToList();
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));
        if (_friendServers.Count > 20)
            _friendServers.Clear();
        _friendServers[key] = (friends, viewers.Count, DateTime.UtcNow);
        return FriendServersDto(friends, viewers.Count);
    }

    /// <summary>The friends as the page gets them, with the account that saw each named as it reads now.</summary>
    object FriendServersDto(List<FriendFound> friends, int asked) => new
    {
        asked,
        friends = friends.Select(f => new
        {
            userId = f.UserId,
            name = f.Name,
            displayName = f.DisplayName,
            avatarUrl = f.AvatarUrl,
            jobId = f.JobId,
            placeId = f.PlaceId,
            via = FindAccount(f.Via) is { } account ? NameText(account) : "",
        }).ToList(),
    };

    /// <summary>A private server found for this game, with the accounts that can join it (each asked with its own cookie).</summary>
    sealed record PrivateFound(RobloxApi.PrivateServer Server, List<Guid> Via);
    readonly Dictionary<(long PlaceId, string Viewers), (List<PrivateFound> Servers, int Asked, bool Failed, DateTime At)> _privateServers = new();

    /// <summary>The game's private servers your accounts can join, for Browse servers. Roblox answers each account with its own
    /// list, so the selected accounts are asked (up to 6; none selected, 3 others), and each server lists who can join it.</summary>
    async Task<object?> PrivateServersAsync(long placeId, Guid[] selected)
    {
        var vault = RequireVault();
        if (placeId <= 0)
            throw new UserError(L.T("Invalid Place ID."));
        var usable = vault.Data.Accounts
            .Where(a => a.UserId > 0 && a.CookieValid != false && a.Cookie.Length > 0 && a.Realm == Realm.Global)
            .ToList();
        var viewers = usable.Where(a => selected.Contains(a.Id)).Take(6).ToList();
        if (viewers.Count == 0)
            viewers = usable.Take(3).ToList();
        var key = (placeId, string.Join(",", viewers.Select(v => v.Id)));
        if (_privateServers.TryGetValue(key, out var kept) && DateTime.UtcNow - kept.At < FriendServerLifetime)
            return PrivateServersDto(kept.Servers, kept.Asked, kept.Failed);
        var found = new Dictionary<string, PrivateFound>(StringComparer.OrdinalIgnoreCase);
        int failed = 0;
        await Task.WhenAll(viewers.Select(async viewer =>
        {
            try
            {
                foreach (var s in await RobloxApi.GetPrivateServersAsync(viewer.Cookie, placeId))
                {
                    lock (found)
                    {
                        if (found.TryGetValue(s.AccessCode, out var f))
                            f.Via.Add(viewer.Id);
                        else
                            found[s.AccessCode] = new PrivateFound(s, [viewer.Id]);
                    }
                }
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                Interlocked.Increment(ref failed); // that account's servers are just left out
            }
        }));
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));
        // Those every account asked can join first, then the fuller ones (people are playing there).
        var servers = found.Values
            .OrderByDescending(f => f.Via.Count)
            .ThenByDescending(f => f.Server.Playing)
            .ThenBy(f => f.Server.Name, StringComparer.CurrentCultureIgnoreCase)
            .ToList();
        bool allFailed = viewers.Count > 0 && failed == viewers.Count;
        if (_privateServers.Count > 20)
            _privateServers.Clear();
        if (!allFailed)
            _privateServers[key] = (servers, viewers.Count, false, DateTime.UtcNow);
        return PrivateServersDto(servers, viewers.Count, allFailed);
    }

    object PrivateServersDto(List<PrivateFound> servers, int asked, bool failed) => new
    {
        asked,
        failed,
        servers = servers.Select(f => new
        {
            id = f.Server.Id,
            name = f.Server.Name,
            owner = f.Server.OwnerDisplayName.Length > 0 ? f.Server.OwnerDisplayName : f.Server.OwnerName,
            ownerName = f.Server.OwnerName,
            accessCode = f.Server.AccessCode,
            playing = f.Server.Playing,
            maxPlayers = f.Server.MaxPlayers,
            via = f.Via.Select(id => FindAccount(id) is { } account ? NameText(account) : "").Where(n => n.Length > 0).ToList(),
            viaIds = f.Via.Select(id => id.ToString()).ToList(),
        }).ToList(),
    };

    async Task<object?> ResolveGameAsync(string input)
    {
        if (!GameTarget.TryParse(input, "", out var target, out string error))
            return new { ok = false, error };
        if (target!.ShareCode is not null)
            return new { ok = true, kind = "share" };
        var info = await GetGameInfoAsync(target.PlaceId, fresh: true);
        string kind = target.IsPrivate ? "private" : target.JobId is not null ? "server" : "game";
        return new
        {
            ok = true,
            kind,
            placeId = target.PlaceId,
            name = info?.Name,
            creator = info?.Creator,
            playing = info?.Playing,
            iconUrl = info?.IconUrl,
        };
    }

    void RememberGame(long placeId)
    {
        var recent = RequireVault().Data.Settings.RecentGames;
        recent.RemoveAll(g => g.PlaceId == placeId);
        var info = _games.GetValueOrDefault(placeId);
        recent.Insert(0, new RecentGame { PlaceId = placeId, UniverseId = info?.UniverseId ?? 0, Name = info?.Name ?? "" });
        if (recent.Count > 8)
            recent.RemoveRange(8, recent.Count - 8);
    }

    async Task LoadRecentGamesAsync()
    {
        var vault = _vault;
        if (vault is null)
            return;
        bool changed = false;
        await LoadGamesAsync(vault.Data.Settings.RecentGames.Select(g => (g.PlaceId, g.UniverseId)).ToList());
        foreach (var game in vault.Data.Settings.RecentGames)
        {
            if (_games.GetValueOrDefault(game.PlaceId) is not { Name.Length: > 0 } info)
                continue;
            if (info.Name != game.Name || game.UniverseId != info.UniverseId)
            {
                game.Name = info.Name;
                game.UniverseId = info.UniverseId;
                changed = true;
            }
        }
        if (_vault != vault)
            return;
        if (changed)
            Persist();
        PushState();
    }
}
