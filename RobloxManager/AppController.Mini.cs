namespace RobloxManager;

/// <summary>
/// What the mini window (<see cref="MiniForm"/>) shows and does: the clients open now, taken from what the process
/// check already tracks (no snapshot of its own), and the same close actions the page has.
/// </summary>
public sealed partial class AppController
{
    /// <summary>One row of the mini window: a client open on this PC and the account it was opened for.</summary>
    public sealed record MiniClient(Guid Id, string Label, string Game, int Pid, long RamMb);

    /// <summary>RAM per client as of the last process check (every 1.5–5 s).</summary>
    Dictionary<int, long> _clientRam = new();

    /// <summary>The window shows or hides the mini window (null toggles) and says whether it is open afterwards.</summary>
    public Func<bool?, bool>? SetMiniOpen { get; init; }

    void NoteClientMemory(List<RobloxProcesses.PlayerProcess> players) =>
        _clientRam = players.ToDictionary(p => p.Pid, p => p.WorkingSet);

    /// <summary>Every Roblox client on this PC as of the last process check, Bloxkeep's or not.</summary>
    public int RobloxCount => _clientRam.Count;

    /// <summary>The clients Bloxkeep opened that are still running, in the launch order (as the page and monitor list them); empty while locked.</summary>
    public List<MiniClient> RunningClients()
    {
        var list = new List<MiniClient>();
        if (_vault is not { } vault)
            return list;
        foreach (var (id, info) in _running)
        {
            if (FindAccount(id) is not { } account)
                continue;
            string game = Connection(account, info.Pid, info.Launch) switch
            {
                "menu" => L.T("No game"),
                "disconnected" => L.T("Disconnected"),
                _ => SessionGameName(info.Pid, info.Launch),
            };
            list.Add(new MiniClient(id, NameText(account), game, info.Pid, _clientRam.GetValueOrDefault(info.Pid) / MB));
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

    /// <summary>Closes one account's client, like the page's "Close Roblox".</summary>
    public void CloseClient(Guid id) => CloseAccounts([id], force: false);

    /// <summary>Closes every Roblox on this PC, like the page's "Close all"; the number of clients closed.</summary>
    public int CloseAllClients() => (int)CloseAll(force: false);

    /// <summary>The page's mini-window button; the window pushes the new state once it has shown or hidden it.</summary>
    object? ToggleMini()
    {
        SetMiniOpen?.Invoke(null);
        return null;
    }
}
