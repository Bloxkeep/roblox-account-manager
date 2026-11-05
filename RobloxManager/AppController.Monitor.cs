using System.Text.Json;

namespace RobloxManager;

/// <summary>The game monitor tab (<see cref="MonitorTab"/>): every Roblox window on this PC with its account.</summary>
public sealed partial class AppController
{
    /// <summary>The process check's snapshot is used while younger than this (its ticks come every 1.5 s, or 4 s with nothing running).</summary>
    static readonly TimeSpan MonitorSnapshotAge = TimeSpan.FromSeconds(4.5);

    /// <summary>AccountId is null for a window Bloxkeep didn't open. Status as <see cref="Connection"/> says it ("" when
    /// nothing is known); JobId the server it is in (the page marks windows sharing one); IconUrl the game's, once fetched.</summary>
    public sealed record MonitorClient(IntPtr Hwnd, int Pid, Guid? AccountId, string Label, string Game, long RamMb,
        string Status = "", string? JobId = null, string? IconUrl = null);

    /// <summary>One window per client: tracked accounts in launch order, then other clients by pid. The snapshot may be
    /// seconds old: a client gone since has no window, so no tile.</summary>
    public List<MonitorClient> MonitorClients()
    {
        var now = DateTime.UtcNow;
        var players = _lastPlayers is { } last && now >= last.At && now - last.At < MonitorSnapshotAge ? last.Players : RobloxProcesses.GetPlayers();
        var windows = RobloxProcesses.VisibleWindows(players.Select(p => p.Pid));
        var ram = players.ToDictionary(p => p.Pid, p => p.Ram / MB);
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
                string status = Connection(account, run.Pid, run.Launch);
                var log = _logs.StateOf(run.Pid);
                // A window cut off keeps its game: the page groups it with the others there, marked.
                long? place = status == "menu" ? null : MonitorPlace(log?.PlaceId, run.Launch.Target?.PlaceId);
                list.Add(new MonitorClient(hwnd, run.Pid, account.Id, NameText(account), MonitorGameName(place), ram.GetValueOrDefault(run.Pid),
                    status, status == "ingame" ? log?.JobId : null, MonitorIcon(place)));
            }
        }
        foreach (var (pid, handles) in windows.OrderBy(w => w.Key))
        {
            var hwnd = RobloxProcesses.MainWindowOf(handles);
            if (hwnd == IntPtr.Zero || !taken.Add(pid))
                continue;
            // Opened outside Bloxkeep: its log still says where it is (the logs of every client are followed).
            var log = _logs.StateOf(pid);
            long? place = log?.PlaceId;
            string status = log is null ? "" : LogConnection(log.Status);
            // Not in a game, it is "No game" like the accounts' windows, so the page puts them in one lane.
            list.Add(new MonitorClient(hwnd, pid, null, "Roblox", MonitorGameName(place), ram.GetValueOrDefault(pid),
                status, status == "ingame" ? log?.JobId : null, MonitorIcon(place)));
        }
        return list;
    }

    /// <summary>As the page shows it (<see cref="SessionGameDto"/>): the logged place once named, else the one opened.</summary>
    long? MonitorPlace(long? logged, long? opened)
    {
        bool loggedNamed = logged is long l && _games.GetValueOrDefault(l)?.Name is { Length: > 0 };
        return loggedNamed || opened is null ? logged ?? opened : opened;
    }

    string MonitorGameName(long? placeId)
    {
        if (placeId is not long id)
            return L.T("No game");
        return _games.GetValueOrDefault(id)?.Name is { Length: > 0 } name ? name : L.F("Place {0}", id);
    }

    string? MonitorIcon(long? placeId) => placeId is long id ? _games.GetValueOrDefault(id)?.IconUrl : null;

    public void CloseMonitorClient(MonitorClient client)
    {
        if (client.AccountId is { } id && _running.ContainsKey(id))
            CloseAccounts([id], force: false);
        else
            RobloxProcesses.Close(client.Pid, InGameNow(client.Pid));
    }

    /// <summary>A window's id on the page: its handle as a number.</summary>
    static string MonitorId(IntPtr hwnd) => hwnd.ToInt64().ToString(System.Globalization.CultureInfo.InvariantCulture);

    /// <summary>The monitor tab's windows to the page ("monitor"): name (Hide names holds), game, RAM, PID, minimised.</summary>
    internal void EmitMonitor(List<MonitorClient> clients, Func<IntPtr, bool> minimized)
    {
        if (Open is null)
            return;
        Emit("monitor", new
        {
            tiles = clients.Select(c => new
            {
                id = MonitorId(c.Hwnd), pid = c.Pid, name = c.Label, game = c.Game, ramMb = c.RamMb, ours = c.AccountId is not null, minimized = minimized(c.Hwnd),
                status = c.Status, jobId = c.JobId, iconUrl = c.IconUrl,
            }),
        });
    }

    /// <summary>The window asks its monitor tab for the windows again (after a click changed one).</summary>
    public Action? MonitorRefresh { get; init; }

    /// <summary>A tile's click, key or menu item: show, minimize, restore, close {id}; copyPid returns the PID copied.</summary>
    object? MonitorActCommand(JsonElement a)
    {
        RequireVault();
        string id = Str(a, "id");
        if (MonitorClients().FirstOrDefault(c => MonitorId(c.Hwnd) == id) is not { } client)
            return null; // gone meanwhile: every action survives that
        return MonitorAct(client, Str(a, "what"));
    }

    /// <summary>A tile's action on its window.</summary>
    object? MonitorAct(MonitorClient client, string what)
    {
        switch (what)
        {
            case "show":
                RobloxProcesses.FocusInTurn(client.Pid);
                break;
            // After a nudge, a picture or a macro's turn under way (as Show): the turn would put the window back as it
            // found it, undoing the change.
            case "minimize":
                InputTurn.RunWhenFree("monitor", () =>
                {
                    RobloxProcesses.Minimise(client.Hwnd);
                    MonitorRefresh?.Invoke();
                });
                return null;
            case "restore":
                InputTurn.RunWhenFree("monitor", () =>
                {
                    RobloxProcesses.RestoreQuietly(client.Hwnd);
                    MonitorRefresh?.Invoke();
                });
                return null;
            case "close":
                CloseMonitorClient(client);
                break;
            case "copyPid":
                Ui.CopyToClipboard(client.Pid.ToString(System.Globalization.CultureInfo.InvariantCulture), secret: false);
                return client.Pid;
        }
        MonitorRefresh?.Invoke();
        return null;
    }

    /// <summary>The monitor tab's area in CSS pixels, the page's pixel ratio, and where each tile's picture goes in it.</summary>
    public sealed record MonitorArea(double X, double Y, double Width, double Height, double Scale, IReadOnlyList<MonitorPicture> Pictures);

    /// <summary>A tile's picture: its window and its rectangle in CSS pixels (relative to the page, as the area is).</summary>
    public readonly record struct MonitorPicture(IntPtr Hwnd, double X, double Y, double Width, double Height);

    /// <summary>The window lays the live pictures over the page's monitor tab (see <see cref="MonitorTab"/>), or takes them away (null).</summary>
    public Action<MonitorArea?>? SetMonitorArea { get; init; }

    /// <summary>The monitor tab moved, or must not be covered ({ hidden: true }: another tab, a dialog, the window
    /// hidden). Locked: no tiles, the page shows the lock screen.</summary>
    object? MonitorAreaCommand(JsonElement a)
    {
        SetMonitorArea?.Invoke(Open is null || Bool(a, "hidden") ? null : ParseMonitorArea(a));
        return null;
    }

    const double MonitorMax = 100_000;

    /// <summary>Written so that NaN (a value missing) fails every test.</summary>
    static bool Sane(double x, double y, double w, double h) =>
        w >= 1 && h >= 1 && w < MonitorMax && h < MonitorMax && Math.Abs(x) < MonitorMax && Math.Abs(y) < MonitorMax;

    /// <summary>Nothing absurd or missing from the page reaches the window's layout; a picture that is, is left out.</summary>
    static MonitorArea? ParseMonitorArea(JsonElement a)
    {
        double x = Num(a, "x"), y = Num(a, "y"), w = Num(a, "w"), h = Num(a, "h"), scale = Num(a, "scale");
        if (!Sane(x, y, w, h) || !(scale >= .25 && scale <= 8))
            return null;
        var pictures = new List<MonitorPicture>();
        if (Has(a, "pictures", JsonValueKind.Array))
        {
            foreach (var p in a.GetProperty("pictures").EnumerateArray().Take(200))
            {
                if (p.ValueKind != JsonValueKind.Object || !long.TryParse(Str(p, "id"), System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out long hwnd) || hwnd == 0)
                    continue;
                double px = Num(p, "x"), py = Num(p, "y"), pw = Num(p, "w"), ph = Num(p, "h");
                if (Sane(px, py, pw, ph))
                    pictures.Add(new MonitorPicture(new IntPtr(hwnd), px, py, pw, ph));
            }
        }
        return new MonitorArea(x, y, w, h, scale, pictures);
    }

    static double Num(JsonElement a, string name) =>
        Has(a, name, JsonValueKind.Number) && a.GetProperty(name).TryGetDouble(out double v) ? v : double.NaN;
}
