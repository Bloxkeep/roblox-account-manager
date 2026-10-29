using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// How many play a game at each hour of the day (this PC's time), for its page's "When it is quiet": while Bloxkeep is
/// open it notes the player count Roblox gives for the games in use (see AppController.GamePage.cs), each hour's figure
/// an average that leans on the newest (so a game's new season shows within days). Only Roblox's public figures, kept
/// in the data folder (game-hours.json), for the 40 games noted last.
/// </summary>
public static class GameHours
{
    const int MaxGames = 40;
    /// <summary>A new figure's weight in its hour's average.</summary>
    const double NewWeight = 0.3;

    sealed class Game
    {
        public double?[] Hours { get; set; } = new double?[24];
        public DateTime Noted { get; set; }
    }

    static Dictionary<long, Game>? _games;
    static string FilePath => Path.Combine(Vault.DataDir, "game-hours.json");

    static Dictionary<long, Game> Games()
    {
        if (_games is not null)
            return _games;
        try
        {
            _games = File.Exists(FilePath) ? JsonSerializer.Deserialize<Dictionary<long, Game>>(File.ReadAllText(FilePath)) : null;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException)
        {
            _games = null;
        }
        _games ??= new();
        // A file from another version, or edited by hand: each game with its 24 hours, no other figure.
        foreach (var (id, g) in _games.ToList())
        {
            if (g?.Hours is not { Length: 24 })
                _games.Remove(id);
        }
        return _games;
    }

    /// <summary>The players <paramref name="playing"/> a game had at <paramref name="at"/> (local time is taken from it).</summary>
    public static void Note(long universeId, long playing, DateTime at)
    {
        if (universeId <= 0 || playing <= 0)
            return;
        var games = Games();
        if (!games.TryGetValue(universeId, out var game))
        {
            if (games.Count >= MaxGames)
                games.Remove(games.MinBy(g => g.Value.Noted).Key);
            games[universeId] = game = new Game();
        }
        int hour = at.ToLocalTime().Hour;
        game.Hours[hour] = game.Hours[hour] is { } was ? was + NewWeight * (playing - was) : playing;
        game.Noted = at;
    }

    /// <summary>The 24 hours' figures (null: never noted at that hour), or null for a game never noted.</summary>
    public static long?[]? Of(long universeId) =>
        Games().TryGetValue(universeId, out var g) ? g.Hours.Select(h => h is { } v ? (long?)Math.Round(v) : null).ToArray() : null;

    public static void Save()
    {
        if (_games is null)
            return;
        try
        {
            Directory.CreateDirectory(Vault.DataDir);
            DurableFile.WriteText(FilePath, JsonSerializer.Serialize(_games));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
    }

    /// <summary>Forgets what was read (the checks use another data folder).</summary>
    internal static void Reset() => _games = null;
}
