namespace RobloxManager;

/// <summary>What the mini window (<see cref="MiniWindow"/>) shows and does: the clients open now, from what the process
/// check already tracks (no snapshot of its own), and the page's close actions.</summary>
public sealed partial class AppController
{
    /// <param name="State">"game" (in a game, or getting in), "menu" (Roblox's home screen) or "disconnected".</param>
    public sealed record MiniClient(Guid Id, string Label, string Game, int Pid, long RamMb, string State = "game");

    /// <summary>RAM per client as of the last process check (every 1.5–5 s).</summary>
    Dictionary<int, long> _clientRam = new();

    /// <summary>The window shows or hides the mini window (null toggles) and says whether it is open afterwards.</summary>
    public Func<bool?, bool>? SetMiniOpen { get; init; }

    /// <summary>Each client's own memory (commit) as of the last process check: what Settings' "Restart a heavy window" reads.</summary>
    Dictionary<int, long> _clientCommit = new();

    void NoteClientMemory(List<RobloxProcesses.PlayerProcess> players)
    {
        _clientRam = players.ToDictionary(p => p.Pid, p => p.Ram);
        _clientCommit = players.ToDictionary(p => p.Pid, p => p.Private > 0 ? p.Private : p.Ram);
    }

    /// <summary>Every Roblox client on this PC as of the last process check, Bloxkeep's or not.</summary>
    public int RobloxCount => _clientRam.Count;

    /// <summary>The clients Bloxkeep opened that are still running, in the launch order (as the page and monitor list them); empty while locked (the screen too).</summary>
    public List<MiniClient> RunningClients()
    {
        var list = new List<MiniClient>();
        if (Open is not { } vault)
            return list;
        foreach (var (id, info) in _running)
        {
            if (FindAccount(id) is not { } account)
                continue;
            string state = Connection(account, info.Pid, info.Launch) switch { "menu" => "menu", "disconnected" => "disconnected", _ => "game" };
            string game = state switch
            {
                "menu" => L.T("No game"),
                "disconnected" => L.T("Disconnected"),
                _ => SessionGameName(info.Pid, info.Launch),
            };
            list.Add(new MiniClient(id, NameText(account), game, info.Pid, _clientRam.GetValueOrDefault(info.Pid) / MB, state));
        }
        var rank = EffectiveLaunchOrder(vault).Select((id, i) => (id, i)).ToDictionary(p => p.id, p => p.i);
        return list.OrderBy(c => rank.GetValueOrDefault(c.Id, int.MaxValue)).ToList();
    }

    /// <summary>The game's name the way the page shows it (see <see cref="SessionGameDto"/>), as text.</summary>
    string SessionGameName(int pid, PendingLaunch launch)
    {
        long? logged = _logs.StateOf(pid)?.PlaceId, opened = launch.Target?.PlaceId;
        bool loggedNamed = logged is long l && _games.GetValueOrDefault(l)?.Name is { Length: > 0 };
        long? placeId = loggedNamed || opened is null ? logged ?? opened : opened;
        if (placeId is not long id)
            return L.T("No game");
        return _games.GetValueOrDefault(id)?.Name is { Length: > 0 } name ? name : L.F("Place {0}", id);
    }

    /// <summary>The mini window's page: the clients (empty while locked), every Roblox on the PC, whether "Close all" asks
    /// first, the theme and the language.</summary>
    internal object MiniMessage() => new
    {
        unlocked = Unlocked,
        clients = (Unlocked ? RunningClients() : []).Select(c => new { id = c.Id, name = c.Label, game = c.Game, pid = c.Pid, ramMb = c.RamMb, state = c.State }),
        robloxCount = Unlocked ? RobloxCount : 0,
        askCloseAll = !Prefs.SkipConfirm.Contains("closeAll"),
        theme = Themes.Current.Id,
        customTheme = CustomThemeDto(),
        language = L.Language,
    };

    /// <summary>The mini window's Show: the account's window brought forward in its turn.</summary>
    public void ShowClient(Guid id)
    {
        if (Unlocked && _running.TryGetValue(id, out var info))
            RobloxProcesses.FocusInTurn(info.Pid);
        else
            Log.Note($"Mini window: Show did nothing for account {id.ToString()[..8]} ({(Unlocked ? "its Roblox isn't open" : "locked")}).");
    }

    public void CloseClient(Guid id)
    {
        if (Unlocked)
            CloseAccounts([id], force: false);
        else
            Log.Note("Mini window: Close did nothing (locked).");
    }

    /// <summary>Closes every Roblox on this PC, like the page's "Close all"; the number of clients closed. Not while locked,
    /// the lock screen too (a click on the mini window's button just before it came up).</summary>
    public int CloseAllClients()
    {
        if (!Unlocked)
        {
            Log.Note("Mini window: Close all did nothing (locked).");
            return 0;
        }
        return (int)CloseAll(force: false);
    }

    /// <summary>The page's mini-window button; the window pushes the new state once it has shown or hidden it.</summary>
    object? ToggleMini()
    {
        if (SetMiniOpen is null)
            Log.Note("Mini window: the page's button did nothing (no window to show it).");
        SetMiniOpen?.Invoke(null);
        return null;
    }
}
