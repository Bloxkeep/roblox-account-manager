namespace RobloxManager;

/// <summary>
/// Settings › Discord › Show on my Discord profile (<see cref="Prefs.DiscordPresence"/>, off by default): the profile
/// shows "Playing Bloxkeep" with how many Roblox windows are open and which game, never an account's name, and a link to
/// the website. Updated every few seconds through the Discord app on this PC (<see cref="DiscordPresence"/>); turning it
/// off or quitting takes it away.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Bloxkeep's application on Discord's developer portal (created 2026-10-20, with the logo as its icon), whose name and icon the profile shows.</summary>
    internal const string DiscordApplicationId = "1561788269933498408";
    const string WebsiteUrl = "https://bloxkeep.github.io/roblox-account-manager/";
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

    /// <summary>
    /// In a game: its name and icon (Bloxkeep's logo at the corner), how many windows, and a button to its page. With
    /// several games each takes its turn, one per update (<see cref="DiscordPresenceInterval"/>), so the profile moves
    /// through them. Otherwise how many windows are open, or that Bloxkeep is managing accounts.
    /// </summary>
    DiscordPresence.Activity PresenceActivity()
    {
        int windows = RobloxCount;
        var games = _running.Values.Select(PlayingPlace).OfType<long>()
            .Select(id => _games.GetValueOrDefault(id)).OfType<GameInfo>().Where(g => g.Name.Length > 0)
            .GroupBy(g => g.Name).Select(g => g.First()).ToList();
        string open = L.P(windows, "1 Roblox window open", "{0} Roblox windows open", windows);
        long since = new DateTimeOffset(DateTime.SpecifyKind(_started, DateTimeKind.Utc)).ToUnixTimeSeconds();
        if (games.Count == 0)
            return new(windows == 0 ? L.T("Managing Roblox accounts") : open, null, since, L.T("Get Bloxkeep"), WebsiteUrl);
        var shown = games[_presenceTurn++ % games.Count];
        return new(shown.Name, games.Count > 1 ? L.F("{0} · {1} games", open, games.Count) : open, since, L.T("Get Bloxkeep"), WebsiteUrl)
        {
            Picture = shown.IconUrl is { } icon && icon.StartsWith("https://", StringComparison.Ordinal) ? icon : null,
            PictureText = shown.Name,
            FirstLabel = L.T("View game"),
            FirstUrl = $"https://www.roblox.com/games/{shown.PlaceId}",
        };
    }

    /// <summary>Which game the profile shows next when there are several.</summary>
    int _presenceTurn;

    /// <summary>The game a window is playing: none once it left for Roblox's home screen, lost its connection or was signed
    /// out (the log keeps the last place then; seen 2026-10-20). Without a log yet, the one it was opened with.</summary>
    long? PlayingPlace((int Pid, PendingLaunch Launch) run) =>
        _logs.StateOf(run.Pid) is { } log ? log.Status is "ingame" or "joining" ? log.PlaceId : null : run.Launch.Target?.PlaceId;

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
