using System.Text.Json;

namespace RobloxManager;

/// <summary>The game box's lists: recent, pinned, favourites, popular, and search.</summary>
public sealed partial class AppController
{
    static readonly TimeSpan GameListLifetime = TimeSpan.FromMinutes(10);
    readonly Dictionary<long, string> _iconByUniverse = new();
    (List<GameSummary> Games, DateTime At)? _favoriteGames, _popularGames;

    /// <summary>Icons not known yet, in one request per 100 games.</summary>
    async Task FetchIconsAsync(IEnumerable<GameSummary> games)
    {
        if (_iconByUniverse.Count > MaxCachedGames)
            _iconByUniverse.Clear(); // a link per game ever shown; the ones on screen are asked for again
        var missing = games.Select(g => g.UniverseId).Where(id => id > 0 && !_iconByUniverse.ContainsKey(id)).Distinct().ToList();
        if (missing.Count == 0)
            return;
        try
        {
            foreach (var (id, url) in await RobloxApi.GetGameIconsAsync(missing))
                _iconByUniverse[id] = url;
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            // Rows still work without icons.
        }
    }

    /// <param name="fetchIcons">False for search results: they show at once, and the page asks for the icons after.</param>
    async Task<List<object>> GameRowsAsync(IEnumerable<GameSummary> games, bool fetchIcons = true)
    {
        var list = games.GroupBy(g => g.PlaceId).Select(g => g.First()).ToList();
        if (fetchIcons)
            await FetchIconsAsync(list);
        var pinned = _vault?.Data.Settings.PinnedGames.Select(p => p.PlaceId).ToHashSet() ?? [];
        return list.Select(g => (object)new
        {
            g.PlaceId,
            g.UniverseId,
            g.Name,
            g.Playing,
            iconUrl = _iconByUniverse.GetValueOrDefault(g.UniverseId),
            pinned = pinned.Contains(g.PlaceId),
        }).ToList();
    }

    async Task<object?> SearchGamesAsync(string query)
    {
        query = query.Trim();
        if (query.Length < 2)
            return Array.Empty<object>();
        return await GameRowsAsync((await RobloxApi.SearchGamesAsync(query)).Take(12), fetchIcons: false);
    }

    async Task<object?> GameIconsAsync(JsonElement a)
    {
        var ids = a.TryGetProperty("universeIds", out var arr) && arr.ValueKind == JsonValueKind.Array
            ? arr.EnumerateArray().Select(x => x.TryGetInt64(out long v) ? v : 0).Where(v => v > 0).Distinct().Take(50).ToList()
            : [];
        if (_iconByUniverse.Count > MaxCachedGames)
            _iconByUniverse.Clear(); // a link per game ever shown; the ones on screen are asked for again
        var missing = ids.Where(id => !_iconByUniverse.ContainsKey(id)).ToList();
        if (missing.Count > 0)
        {
            try
            {
                foreach (var (id, url) in await RobloxApi.GetGameIconsAsync(missing))
                    _iconByUniverse[id] = url;
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                // The rows keep their placeholder.
            }
        }
        return ids.Where(_iconByUniverse.ContainsKey).ToDictionary(id => id.ToString(), id => _iconByUniverse[id]);
    }

    /// <summary>Recent and pinned games plus, cached for a while, the accounts' Roblox favourites and the popular chart.</summary>
    async Task<object?> GameListsAsync()
    {
        var vault = RequireVault();
        async Task<List<GameSummary>> Favorites()
        {
            var lists = await Task.WhenAll(vault.Data.Accounts.Where(a => a.UserId > 0).Take(5).Select(async account =>
            {
                try
                {
                    return await RobloxApi.GetFavoriteGamesAsync(account.UserId);
                }
                catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                {
                    return [];
                }
            }));
            return lists.SelectMany(l => l).GroupBy(g => g.PlaceId).Select(g => g.First()).Take(30).ToList();
        }
        async Task<List<GameSummary>> Popular()
        {
            try
            {
                return (await RobloxApi.GetPopularGamesAsync(AnyGlobalCookie())).Take(30).ToList();
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                return [];
            }
        }
        var now = DateTime.UtcNow;
        var favTask = _favoriteGames is { } fav && now - fav.At <= GameListLifetime ? Task.FromResult(fav.Games) : Favorites();
        var popTask = _popularGames is { } pop && now - pop.At <= GameListLifetime ? Task.FromResult(pop.Games) : Popular();
        await Task.WhenAll(favTask, popTask);
        _favoriteGames = (favTask.Result, _favoriteGames is { } f && f.Games == favTask.Result ? f.At : now);
        _popularGames = (popTask.Result, _popularGames is { } p && p.Games == popTask.Result ? p.At : now);

        var s = vault.Data.Settings;
        var recent = s.RecentGames.Select(g => new GameSummary(g.PlaceId, g.UniverseId, g.Name.Length > 0 ? g.Name : $"Place {g.PlaceId}", 0)).ToList();
        var pinned = s.PinnedGames.Select(g => new GameSummary(g.PlaceId, g.UniverseId, g.Name, 0)).ToList();
        await FetchIconsAsync([.. recent, .. pinned, .. favTask.Result, .. popTask.Result]);
        return new
        {
            recent = await GameRowsAsync(recent),
            pinned = await GameRowsAsync(pinned),
            favorites = await GameRowsAsync(favTask.Result),
            popular = await GameRowsAsync(popTask.Result),
        };
    }

    object TogglePin(long placeId, long universeId, string name)
    {
        var pinned = RequireVault().Data.Settings.PinnedGames;
        bool nowPinned = pinned.RemoveAll(g => g.PlaceId == placeId) == 0;
        if (nowPinned && placeId > 0)
        {
            pinned.Insert(0, new RecentGame { PlaceId = placeId, UniverseId = universeId, Name = name.Trim() });
            if (pinned.Count > 30)
                pinned.RemoveRange(30, pinned.Count - 30);
        }
        Persist();
        PushState();
        return nowPinned;
    }

    object? RemoveRecent(long placeId)
    {
        RequireVault().Data.Settings.RecentGames.RemoveAll(g => g.PlaceId == placeId);
        Persist();
        PushState();
        return null;
    }
}
