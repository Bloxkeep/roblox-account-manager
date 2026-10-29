namespace RobloxManager;

/// <summary>
/// A game's page (ui/src/GamePage.tsx, opened from its name under the game box): Roblox's figures for it, your accounts
/// in it over the last 7 days (from Statistics' stays), the hours it is quiet (GameHours, noted every half hour while
/// Bloxkeep is open), its wiki (a web search) and its roblox.com page.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan GameHoursEvery = TimeSpan.FromMinutes(30);
    /// <summary>The games whose player count is noted: those being played, then the newest in the game box's list.</summary>
    const int GameHoursRecent = 3;
    /// <summary>Roblox's disconnect codes that are a kick: by the game (267), for idling 20 minutes (278).</summary>
    static readonly int[] KickCodes = [267, 278];
    const int IdleKickCode = 278;

    DateTime _gameHoursAt;
    bool _gameHoursBusy;

    async Task<object?> GameDetailsAsync(long placeId)
    {
        RequireVault();
        if (placeId <= 0)
            throw new UserError(L.T("Invalid Place ID."));
        long universe = _games.GetValueOrDefault(placeId)?.UniverseId ?? 0;
        if (universe == 0)
            universe = await RobloxApi.GetUniverseIdAsync(placeId);
        var details = universe > 0 ? await RobloxApi.GetGameDetailsAsync(universe, AnyGlobalCookie()) : null;
        if (details is null)
            throw new UserError(L.T("Roblox has no game with this Place ID."));
        if (details.Playing > 0)
        {
            GameHours.Note(universe, details.Playing, DateTime.UtcNow);
            GameHours.Save();
        }
        string? icon = null;
        try
        {
            icon = (await RobloxApi.GetGameIconsAsync([universe])).GetValueOrDefault(universe);
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            // The page shows a plain tile instead.
        }
        return new
        {
            placeId, name = details.Name, creator = details.Creator, genre = details.Genre, iconUrl = icon, description = details.Description,
            playing = details.Playing, visits = details.Visits, favorites = details.Favorites, likes = details.LikesPercent,
            created = details.Created, updated = details.Updated,
            accounts = GameAccounts(placeId, universe, DateTime.UtcNow),
            hours = GameHours.Of(universe),
        };
    }

    /// <summary>
    /// The accounts that played this game in the 7 days before <paramref name="now"/> (any of its places), most played
    /// first: how long, kicks (by the game or for idling) and other drops, and whether it's in it now or banned from it.
    /// </summary>
    internal List<object> GameAccounts(long placeId, long universe, DateTime now)
    {
        var vault = RequireVault();
        var since = now.AddDays(-7);
        // A stay begun before its place was looked up has no universe: its place tells, from another stay or the cache.
        var universeOf = vault.Data.PlaySessions.Where(s => s.UniverseId > 0).GroupBy(s => s.PlaceId).ToDictionary(g => g.Key, g => g.Last().UniverseId);
        bool InGame(PlaySession s) => s.PlaceId == placeId
            || universe > 0 && (s.UniverseId > 0 ? s.UniverseId : universeOf.GetValueOrDefault(s.PlaceId) is > 0 and var u ? u : _games.GetValueOrDefault(s.PlaceId)?.UniverseId ?? 0) == universe;
        var rows = new List<(Guid Id, string Name, double Seconds, int Kicks, bool Idle, int Drops, bool Now, DateTime Last)>();
        foreach (var group in vault.Data.PlaySessions.Where(InGame).Where(s => (s.EndedAt ?? now) > since).GroupBy(s => s.AccountId))
        {
            double seconds = group.Sum(s => Math.Max(0, ((s.EndedAt ?? now) - (s.StartedAt > since ? s.StartedAt : since)).TotalSeconds));
            var ended = group.Where(s => s.EndReason == "disconnected" && s.EndedAt > since).ToList();
            int kicks = ended.Count(s => s.ReasonCode is { } c && KickCodes.Contains(c));
            var account = FindAccount(group.Key);
            // Hide account names applies here too; a deleted account goes by the name it had.
            string name = account is not null ? Name(account).ToString() : Prefs.HideNames ? "–" : group.Last().AccountLabel ?? "–";
            rows.Add((group.Key, name, seconds, kicks, ended.Any(s => s.ReasonCode == IdleKickCode), ended.Count - kicks,
                group.Any(s => s.EndedAt is null), group.Max(s => s.EndedAt ?? now)));
        }
        return rows.OrderByDescending(r => r.Seconds).Select(r => (object)new
        {
            id = r.Id, name = r.Name, minutes = (int)Math.Round(r.Seconds / 60), kicks = r.Kicks, kickWhy = r.Idle ? "AFK" : null, drops = r.Drops,
            now = r.Now ? "ingame" : FindAccount(r.Id) is { } a && GameBanOf(a, placeId) is not null ? "banned" : "",
            lastPlayed = r.Now ? (DateTime?)null : r.Last,
        }).ToList();
    }

    /// <summary>Its wiki: a web search for it, since each game's wiki lives on a site of its own.</summary>
    static object? OpenGameWiki(string name) =>
        name.Trim().Length == 0 ? null : OpenUrl("https://www.google.com/search?q=" + Uri.EscapeDataString(name.Trim() + " Roblox wiki"));

    static object? OpenGamePage(long placeId) =>
        placeId <= 0 ? throw new UserError(L.T("Invalid Place ID.")) : OpenUrl($"https://www.roblox.com/games/{placeId}");

    /// <summary>Every half hour: the player counts of the games being played and the newest few in the game box's list.</summary>
    void NoteGameHoursIfDue(DateTime now)
    {
        if (_gameHoursBusy || now - _gameHoursAt < GameHoursEvery || _vault is null || AnyGlobalCookie() is not { } cookie)
            return;
        _gameHoursAt = now;
        var universes = _vault.Data.PlaySessions.Where(s => s.EndedAt is null && s.UniverseId > 0).Select(s => s.UniverseId)
            .Concat(_vault.Data.Settings.RecentGames.Take(GameHoursRecent).Select(g => g.UniverseId)).Where(id => id > 0).Distinct().ToList();
        if (universes.Count == 0)
            return;
        _gameHoursBusy = true;
        _ = NoteGameHoursAsync(universes, cookie);
    }

    async Task NoteGameHoursAsync(List<long> universes, string cookie)
    {
        try
        {
            var games = await RobloxApi.GetGamesAsync(universes, cookie);
            var at = DateTime.UtcNow;
            foreach (var (id, game) in games)
                GameHours.Note(id, game.Playing, at);
            GameHours.Save();
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            // Asked again in half an hour.
        }
        finally
        {
            _gameHoursBusy = false;
        }
    }
}
