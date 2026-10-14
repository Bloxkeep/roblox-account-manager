using System.Globalization;

namespace RobloxManager;

/// <summary>
/// Settings › Roblox updates: the Roblox builds seen on this PC (kept in prefs.json, not the vault, so it also works
/// while locked), and noticing when Roblox starts closing its other windows as soon as one more opens — what an
/// update that stops Multi-Roblox working looks like from here.
/// </summary>
public sealed partial class AppController
{
    const int MaxRobloxHistory = 30;
    /// <summary>A client that ends this soon after another one started may have been closed by it.</summary>
    static readonly TimeSpan NewClientWindow = TimeSpan.FromSeconds(20);
    /// <summary>Without the log line: two or more tracked clients ending this close together.</summary>
    static readonly TimeSpan CloseBurst = TimeSpan.FromSeconds(5);
    /// <summary>The notice waits this long after the first close, so it can count the windows closed with it.</summary>
    static readonly TimeSpan ClosedNoticeDelay = TimeSpan.FromSeconds(6);
    static readonly TimeSpan ClosedWarningShown = TimeSpan.FromHours(24);
    static readonly TimeSpan RecentRobloxUpdate = TimeSpan.FromHours(48);
    const string ReleaseNotesIndex = "https://create.roblox.com/docs/en-us/releases";

    /// <summary>Roblox clients running at the last tick, and when each one that started since then first appeared.</summary>
    readonly HashSet<int> _knownClients = new();
    readonly Dictionary<int, DateTime> _clientStarts = new();
    bool _clientsSeen;
    /// <summary>Accounts whose client Roblox closed: no auto-rejoin (it would close the next window again).</summary>
    readonly HashSet<Guid> _closedByRoblox = new();
    /// <summary>Tracked clients that ended right after another one started, without the log line saying why.</summary>
    readonly List<(Guid Account, DateTime At, bool Counted)> _suspectEnds = new();
    /// <summary>Windows counted for the notice about to go out.</summary>
    int _closedPending;
    DateTime? _closedNoticeDue;
    /// <summary>Two clients in a game together this long, none closed by Roblox since: Multi-Roblox works again.</summary>
    static readonly TimeSpan WorkingAgain = TimeSpan.FromSeconds(10);
    /// <summary>Since when two or more clients have been in a game at once (Multi-Roblox armed); null after a close by Roblox or under two.</summary>
    DateTime? _togetherSince;

    /// <summary>
    /// Notes the Roblox clients that appeared since the last tick: not the ones already running when Bloxkeep started,
    /// and not the windowless copy a client starts in the notification area as it closes (its parent is that client,
    /// running or just gone) — counted as a new window, two closing at once looked as if each had closed the other.
    /// </summary>
    /// <param name="parentOf">The process that started a client (0 when unknown); asked only for a client not seen before.</param>
    void NoteClientStarts(HashSet<int> pids, DateTime now, Func<int, int> parentOf)
    {
        foreach (int pid in pids)
        {
            if (_knownClients.Contains(pid) || !_clientsSeen)
                continue;
            int parent = parentOf(pid);
            if (parent == 0 || !(pids.Contains(parent) || _knownClients.Contains(parent)))
                _clientStarts[pid] = now;
        }
        _knownClients.Clear();
        _knownClients.UnionWith(pids);
        foreach (var (pid, at) in _clientStarts.ToList())
        {
            if (now - at > NewClientWindow * 3 || now < at)
                _clientStarts.Remove(pid);
        }
        _clientsSeen = true;
    }

    /// <summary>The windowless copy Roblox starts in the notification area when a window closes is not a new window.</summary>
    void ForgetClientStart(int pid) => _clientStarts.Remove(pid);

    /// <summary>
    /// A tracked client ended (read before auto-rejoin decides): did Roblox close it because another client opened?
    /// Its log saying so, with another client started in the last 20 s, is enough. Without that line, two or more
    /// tracked clients ending within 5 s of each other right after a new client started. Closes Bloxkeep made itself
    /// (Close, a rejoin restart) never count.
    /// </summary>
    /// <param name="childrenOf">Child processes by parent: the ended client's own notification-area copy isn't a new window.</param>
    /// <returns>true when this one counts as closed by Roblox (auto-rejoin then leaves it closed).</returns>
    bool CheckClosedByRoblox(Guid account, int pid, DateTime startedAt, DateTime now, Func<int, List<int>> childrenOf)
    {
        // Without Multi-Roblox holding Roblox's single-window lock, Roblox closing the older window is how it always works.
        if (!MultiInstance.Armed || _userClosed.Contains(account) || _closingForRejoin.Contains(pid))
            return false;
        bool newClient = false;
        List<int>? children = null;
        foreach (var (other, at) in _clientStarts)
        {
            if (other == pid || at <= startedAt || now - at > NewClientWindow)
                continue;
            if ((children ??= childrenOf(pid)).Contains(other))
                continue;
            newClient = true;
            break;
        }
        if (!newClient)
            return false;
        if (_logs.ClosedByNewWindowAt(pid) is not null)
        {
            _closedByRoblox.Add(account);
            CountClosedByRoblox(1, now);
            return true;
        }
        _suspectEnds.RemoveAll(e => now - e.At > CloseBurst || now < e.At || e.Account == account);
        _suspectEnds.Add((account, now, false));
        if (_suspectEnds.Count < 2)
            return false;
        int counted = 0;
        for (int i = 0; i < _suspectEnds.Count; i++)
        {
            var end = _suspectEnds[i];
            if (end.Counted)
                continue;
            _suspectEnds[i] = end with { Counted = true };
            counted++;
            // One that ended a moment earlier may have a rejoin queued already: call it off.
            if (end.Account != account)
                _rejoinQueue.RemoveAll(r => r.AccountId == end.Account);
        }
        _closedByRoblox.Add(account);
        CountClosedByRoblox(counted, now);
        return true;
    }

    void CountClosedByRoblox(int windows, DateTime now)
    {
        if (windows <= 0)
            return;
        _closedPending += windows;
        _closedNoticeDue ??= now + ClosedNoticeDelay;
        _togetherSince = null;
        Prefs.RobloxClosedAt = DateTime.UtcNow;
        Prefs.RobloxClosedCount = _closedPending;
        Prefs.Save();
    }

    /// <summary>
    /// From the process tick: looks for a new Roblox build at the install check's own pace (every 3 minutes), also
    /// while the window is in the tray, and sends the "Roblox closed windows" notice — at most one a day. The warning
    /// goes by itself once two clients have been in a game at the same time for a few seconds (what "it works" looks
    /// like) with none closed by Roblox since, or once a newer Roblox build is installed (what it said may not hold for it).
    /// </summary>
    /// <param name="inGame">Roblox clients in a game now, by their logs.</param>
    void TickRobloxUpdates(DateTime now, int inGame)
    {
        if (_installs is null || now - _installsReadAt >= InstallCacheLifetime || now < _installsReadAt)
        {
            string? before = _installs?.Client?.Folder;
            if (Installs().Client?.Folder != before)
                PushStateIfChanged();
        }
        if (inGame < 2 || !MultiInstance.Armed || now < _togetherSince)
            _togetherSince = null;
        else
            _togetherSince ??= now;
        if (_closedNoticeDue is null && Prefs.RobloxClosedAt is { } closedAt
            && (_togetherSince is { } since && now - since >= WorkingAgain && closedAt < since
                || Prefs.RobloxHistory.Any(h => !h.First && h.At > closedAt)))
        {
            _togetherSince = null;
            HideRobloxClosed();
        }
        if (_closedNoticeDue is not { } due || now < due)
            return;
        int windows = _closedPending;
        _closedPending = 0;
        _closedNoticeDue = null;
        if (Prefs.RobloxClosedNotified is { } told && told.ToLocalTime().Date == DateTime.Now.Date)
        {
            PushStateIfChanged();
            return;
        }
        Prefs.RobloxClosedNotified = DateTime.UtcNow;
        Prefs.Save();
        Notify(L.F("Roblox closed {0} window(s) when another one opened — Roblox may have blocked running several windows. Multi-Roblox may need an update.", windows), "warn", "launch");
        PushStateIfChanged();
    }

    /// <summary>
    /// Adds a client build not seen before to the history: the first one as "first seen", later ones timed by when
    /// their folder was made. A build older than the last one (another launcher's copy) isn't an update.
    /// </summary>
    static void NoteRobloxBuild(RobloxInstalls.ClientVersion? client)
    {
        if (client is null || client.Folder.Length == 0 || client.Version.Length == 0)
            return;
        var history = Prefs.RobloxHistory;
        if (history.Any(h => h.Folder.Equals(client.Folder, StringComparison.OrdinalIgnoreCase)))
            return;
        var last = history.LastOrDefault();
        if (last is not null && Version.TryParse(client.Version, out var have) && Version.TryParse(last.Version, out var had) && have < had)
            return;
        var now = DateTime.UtcNow;
        var at = now;
        if (last is not null && client.Exe is { } exe)
        {
            try
            {
                var made = Directory.GetCreationTimeUtc(Path.GetDirectoryName(exe) ?? "");
                if (made > last.At && made <= now)
                    at = made;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException or NotSupportedException)
            {
            }
        }
        history.Add(new Prefs.RobloxBuild(at, client.Version, client.Folder, client.Source ?? "", First: last is null));
        if (history.Count > MaxRobloxHistory)
            history.RemoveRange(0, history.Count - MaxRobloxHistory);
        Prefs.Save();
    }

    object RobloxUpdatesDto()
    {
        var installed = Installs().Client;
        var now = DateTime.UtcNow;
        // Only an update before the windows were closed can explain it; one after it is no reason.
        var lastUpdate = Prefs.RobloxClosedAt is { } closedAt ? Prefs.RobloxHistory.LastOrDefault(h => !h.First && h.At <= closedAt) : null;
        return new
        {
            installed = installed is null ? null : new { version = installed.Version, build = installed.Folder, source = installed.Source },
            history = Prefs.RobloxHistory.AsEnumerable().Reverse()
                .Select(h => new { at = h.At, version = h.Version, build = h.Folder, source = h.Source, first = h.First }).ToArray(),
            closed = Prefs.RobloxClosedAt is { } at && now - at < ClosedWarningShown && at <= now + TimeSpan.FromMinutes(5)
                ? new
                {
                    at,
                    count = Math.Max(1, Prefs.RobloxClosedCount),
                    updatedAt = lastUpdate is not null && now - lastUpdate.At < RecentRobloxUpdate ? lastUpdate.At : (DateTime?)null,
                }
                : null,
        };
    }

    /// <summary>Roblox's notes for a version's release ("0.742.0.…" is release 742), in the default browser.</summary>
    static object? OpenReleaseNotes(string version)
    {
        var parts = version.Split('.');
        return parts.Length >= 2 && int.TryParse(parts[1], NumberStyles.None, CultureInfo.InvariantCulture, out int release) && release is > 0 and < 100000
            ? OpenUrl($"https://create.roblox.com/docs/release-notes/release-notes-{release}")
            : OpenUrl(ReleaseNotesIndex);
    }

    object? HideRobloxClosed()
    {
        Prefs.RobloxClosedAt = null;
        Prefs.Save();
        PushState();
        return null;
    }
}
