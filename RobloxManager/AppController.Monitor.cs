using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// What the game monitor tab (<see cref="MonitorTab"/>) shows and does: every Roblox window on this PC with the
/// account it was opened for, and the same show and close actions the page has.
/// </summary>
public sealed partial class AppController
{
    /// <summary>The process check's snapshot is used while younger than this (its ticks come every 1.5 s, or 4 s with nothing running).</summary>
    static readonly TimeSpan MonitorSnapshotAge = TimeSpan.FromSeconds(4.5);

    /// <summary>One tile of the monitor: a Roblox window, and the account (null for one Bloxkeep didn't open).</summary>
    public sealed record MonitorClient(IntPtr Hwnd, int Pid, Guid? AccountId, string Label, string Game, long RamMb);

    /// <summary>
    /// The Roblox windows open now, one per client: tracked accounts first in their launch order (then as listed),
    /// then any other client, by process id. The process check's last snapshot (a few seconds old at most: a client
    /// gone since has no window any more, so no tile) and one pass over the desktop's windows.
    /// </summary>
    public List<MonitorClient> MonitorClients()
    {
        var now = DateTime.UtcNow;
        var players = _lastPlayers is { } last && now >= last.At && now - last.At < MonitorSnapshotAge ? last.Players : RobloxProcesses.GetPlayers();
        var windows = RobloxProcesses.VisibleWindows(players.Select(p => p.Pid));
        var ram = players.ToDictionary(p => p.Pid, p => p.WorkingSet / MB);
        var list = new List<MonitorClient>();
        var taken = new HashSet<int>();
        if (_vault is { } vault)
        {
            var order = vault.Data.Settings.LaunchOrder;
            foreach (var account in vault.Data.Accounts.OrderBy(a => order.IndexOf(a.Id) is var i && i >= 0 ? i : int.MaxValue))
            {
                if (!_running.TryGetValue(account.Id, out var run) || !windows.TryGetValue(run.Pid, out var handles))
                    continue;
                var hwnd = RobloxProcesses.MainWindowOf(handles);
                if (hwnd == IntPtr.Zero || !taken.Add(run.Pid))
                    continue;
                string game = Connection(account, run.Pid, run.Launch) switch
                {
                    "menu" => L.T("No game"),
                    "disconnected" => L.T("Disconnected"),
                    _ => MonitorGameName(_logs.StateOf(run.Pid)?.PlaceId, run.Launch.Target?.PlaceId),
                };
                list.Add(new MonitorClient(hwnd, run.Pid, account.Id, NameText(account), game, ram.GetValueOrDefault(run.Pid)));
            }
        }
        foreach (var (pid, handles) in windows.OrderBy(w => w.Key))
        {
            var hwnd = RobloxProcesses.MainWindowOf(handles);
            if (hwnd == IntPtr.Zero || !taken.Add(pid))
                continue;
            // Opened outside Bloxkeep: its log still says where it is (the logs of every client are followed).
            long? place = _logs.StateOf(pid)?.PlaceId;
            list.Add(new MonitorClient(hwnd, pid, null, "Roblox", place is null ? "" : MonitorGameName(place, null), ram.GetValueOrDefault(pid)));
        }
        return list;
    }

    /// <summary>The game's name the way the page shows it (see <see cref="SessionGameDto"/>): the logged place once named, else the one opened.</summary>
    string MonitorGameName(long? logged, long? opened)
    {
        bool loggedNamed = logged is long l && _games.GetValueOrDefault(l)?.Name is { Length: > 0 };
        long? placeId = loggedNamed || opened is null ? logged ?? opened : opened;
        if (placeId is not long id)
            return L.T("No game");
        return _games.GetValueOrDefault(id)?.Name is { Length: > 0 } name ? name : L.F("Place {0}", id);
    }

    /// <summary>Closes a tile's client: like the page's "Close Roblox" for an account's client, else the window itself.</summary>
    public void CloseMonitorClient(MonitorClient client)
    {
        if (client.AccountId is { } id && _running.ContainsKey(id))
            CloseAccounts([id], force: false);
        else
            RobloxProcesses.Close(client.Pid, InGameNow(client.Pid));
    }

    /// <summary>Where the page's monitor tab wants the live pictures: its content area in CSS pixels, and the page's pixel ratio.</summary>
    public readonly record struct MonitorArea(double X, double Y, double Width, double Height, double Scale);

    /// <summary>The window lays the live pictures over the page's monitor tab (see <see cref="MonitorTab"/>), or takes them away (null).</summary>
    public Action<MonitorArea?>? SetMonitorArea { get; init; }

    /// <summary>
    /// The page's monitor tab moved, or must not be covered right now ({ hidden: true }: another tab, a dialog or
    /// menu, the window hidden). Locked, the tiles would have no accounts' names, and the page shows the lock screen.
    /// </summary>
    object? MonitorAreaCommand(JsonElement a)
    {
        SetMonitorArea?.Invoke(_vault is null || Bool(a, "hidden") ? null : ParseMonitorArea(a));
        return null;
    }

    /// <summary>The page's numbers are checked like any other input: nothing absurd (or missing) reaches the window's layout.</summary>
    static MonitorArea? ParseMonitorArea(JsonElement a)
    {
        double x = Num(a, "x"), y = Num(a, "y"), w = Num(a, "w"), h = Num(a, "h"), scale = Num(a, "scale");
        const double Max = 100_000;
        // Written so that NaN (a value missing) fails every test.
        if (!(w >= 1 && h >= 1 && w < Max && h < Max && Math.Abs(x) < Max && Math.Abs(y) < Max && scale >= .25 && scale <= 8))
            return null;
        return new MonitorArea(x, y, w, h, scale);
    }

    static double Num(JsonElement a, string name) =>
        Has(a, name, JsonValueKind.Number) && a.GetProperty(name).TryGetDouble(out double v) ? v : double.NaN;
}
