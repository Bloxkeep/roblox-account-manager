namespace RobloxManager;

/// <summary>
/// Settings › Discord › Show on my Discord profile (<see cref="Prefs.DiscordPresence"/>, off until turned on): the
/// person's Discord profile shows "Playing Bloxkeep" with how many Roblox windows are open and which game, never an
/// account's name, and a link to Bloxkeep's website. Kept up to date every few seconds through the Discord app on this
/// PC (<see cref="DiscordPresence"/>); turned off, or Bloxkeep quitting, takes it away.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Bloxkeep's application on Discord's developer portal, whose name and icon the profile shows; empty until it exists.</summary>
    internal const string DiscordApplicationId = "";
    const string WebsiteUrl = "https://minhhoang0122.github.io/Bloxkeep/";
    static readonly TimeSpan DiscordPresenceInterval = TimeSpan.FromSeconds(15);

    readonly DiscordPresence _discordPresence = new();
    DateTime _nextDiscordPresence;

    void TickDiscordPresence(DateTime now)
    {
        if (now < _nextDiscordPresence)
            return;
        _nextDiscordPresence = now + DiscordPresenceInterval;
        bool on = Prefs.DiscordPresence && DiscordApplicationId.Length > 0;
        _ = _discordPresence.SetAsync(on ? PresenceActivity() : null, DiscordApplicationId);
    }

    /// <summary>What the profile says now: the windows open, and the game they are in (or how many games).</summary>
    DiscordPresence.Activity PresenceActivity()
    {
        int windows = RobloxCount;
        var games = _running.Values.Select(r => _logs.StateOf(r.Pid)?.PlaceId ?? r.Launch.Target?.PlaceId).OfType<long>()
            .Select(id => _games.GetValueOrDefault(id)?.Name).OfType<string>().Where(n => n.Length > 0).Distinct().ToList();
        string details = windows == 0 ? L.T("Managing Roblox accounts")
            : L.P(windows, "1 Roblox window open", "{0} Roblox windows open", windows);
        string? state = games.Count == 1 ? games[0] : games.Count > 1 ? L.F("{0} games", games.Count) : null;
        long since = new DateTimeOffset(DateTime.SpecifyKind(_started, DateTimeKind.Utc)).ToUnixTimeSeconds();
        return new(details, state, since, L.T("Get Bloxkeep"), WebsiteUrl);
    }

    object? SetDiscordPresence(bool on)
    {
        Prefs.DiscordPresence = on;
        Prefs.Save();
        _nextDiscordPresence = DateTime.MinValue; // at once, not at the next round
        TickDiscordPresence(DateTime.UtcNow);
        PushState();
        return null;
    }
}
