using System.Globalization;

namespace RobloxManager;

/// <summary>
/// Settings › Roblox updates: the Roblox builds seen on this PC (kept in prefs.json, not the vault, so it also works
/// while locked), and noticing when Roblox starts closing its other windows as soon as one more opens — what an
/// update that stops Multi-Roblox working looks like from here. Next to that, what Bloxkeep's Roblox watch found in
/// each build before it got here (<see cref="RobloxWatch"/>), and the builds two windows played together on.
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
    /// <summary>When Windows' commit limit was last all but used up (read every tick, also while locked): clients ending soon after it ran out of memory.</summary>
    DateTime? _memoryShortAt;
    /// <summary>How long after the commit limit was all but used up clients that end count as having run out of memory.</summary>
    static readonly TimeSpan MemoryCrashWindow = TimeSpan.FromSeconds(30);
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
    /// tracked clients that closed in order (<paramref name="inOrder"/>) ending within 5 s of each other right after a new
    /// client started. A crash or a killed process never counts: on 2026-10-22 three windows that ran out of memory at once
    /// were taken for Roblox closing them. Closes Bloxkeep made itself (Close, a rejoin restart) never count.
    /// </summary>
    /// <param name="inOrder">It closed in order: its log says it shut down, or it left its notification-area copy.</param>
    /// <param name="childrenOf">Child processes by parent: the ended client's own notification-area copy isn't a new window.</param>
    /// <returns>true when this one counts as closed by Roblox (auto-rejoin then leaves it closed).</returns>
    bool CheckClosedByRoblox(Guid account, int pid, DateTime startedAt, DateTime now, bool inOrder, Func<int, List<int>> childrenOf)
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
        // Out of memory (its own log says Windows refused it memory, or the commit limit was all but used up just now):
        // clients ending together then crashed for want of it, as on 2026-10-22 (6 windows, Windows' low-virtual-memory
        // event at 99.7 %), not Roblox closing them for a new one, and Multi-Roblox is fine.
        if (!inOrder || _logs.OutOfMemoryAt(pid) is not null || _memoryShortAt is { } shortAt && now >= shortAt && now - shortAt <= MemoryCrashWindow)
            return false;
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
    /// Settings › Roblox version › Update now: the launcher that keeps this copy of Roblox installs the newest one
    /// without opening it (<see cref="RobloxInstalls.UpdaterFor"/>). Bloxstrap does it in the background, leaving open
    /// Roblox windows alone, when it can (its own setting, at most one version behind, 5 GB free); otherwise it closes
    /// every Roblox window first, so the page asks before when some are open. The result shows as the usual
    /// "Roblox updated to …"; the launcher ending without a new build is said too.
    /// </summary>
    object? UpdateRoblox()
    {
        if (_robloxUpdating)
            return null;
        _installsReadAt = DateTime.MinValue; // the launcher and the client as they are now
        string? before = Installs().Client?.Folder;
        if (_updaterExe is not { } exe || !File.Exists(exe))
            throw new UserError(L.T("No launcher here can update Roblox by itself. Roblox updates the next time it opens."));
        var startedAt = DateTime.Now;
        var psi = new System.Diagnostics.ProcessStartInfo(exe) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(exe)! };
        psi.ArgumentList.Add("-player");
        psi.ArgumentList.Add("-nolaunch");
        System.Diagnostics.Process.Start(psi)?.Dispose();
        _robloxUpdating = true;
        PushState();
        _ = WatchRobloxUpdateAsync(Path.GetFileNameWithoutExtension(exe), before, startedAt);
        return null;
    }

    /// <summary>
    /// Until the client's build changes, or the launcher's processes started for it (its window, its background
    /// updater) have all ended, at most 15 minutes.
    /// </summary>
    async Task WatchRobloxUpdateAsync(string launcher, string? before, DateTime startedAt)
    {
        var until = DateTime.UtcNow + TimeSpan.FromMinutes(15);
        try
        {
            await Task.Delay(TimeSpan.FromSeconds(5));
            while (!_disposed && DateTime.UtcNow < until)
            {
                _installsReadAt = DateTime.MinValue; // read again, compared with the last read (its "Roblox updated to …")
                if (Installs().Client?.Folder != before)
                    return; // Installs said "Roblox updated to …"
                if (!LauncherRunningSince(launcher, startedAt))
                {
                    Toast(L.F("{0} found no newer Roblox to install.", launcher), "info");
                    return;
                }
                await Task.Delay(TimeSpan.FromSeconds(3));
            }
        }
        finally
        {
            _robloxUpdating = false;
            if (!_disposed)
                PushState();
        }
    }

    /// <summary>A process of this launcher started at or after this time (not its watchers of windows already open).</summary>
    static bool LauncherRunningSince(string launcher, DateTime since)
    {
        foreach (var p in System.Diagnostics.Process.GetProcessesByName(launcher))
        {
            using (p)
            {
                try
                {
                    if (p.StartTime >= since.AddSeconds(-1))
                        return true;
                }
                catch (Exception ex) when (ex is InvalidOperationException or System.ComponentModel.Win32Exception)
                {
                    // Ended meanwhile, or not ours to read.
                }
            }
        }
        return false;
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
        // Two windows in a game together for a while: Multi-Roblox works on this build, whatever the Roblox watch said.
        if (_togetherSince is { } together && now - together >= WorkingAgain && Installs().Client?.Folder is { Length: > 0 } folder
            && !Prefs.MultiWorked.Contains(folder))
        {
            Prefs.MultiWorked.Add(folder);
            if (Prefs.MultiWorked.Count > MaxRobloxHistory)
                Prefs.MultiWorked.RemoveRange(0, Prefs.MultiWorked.Count - MaxRobloxHistory);
            Prefs.Save();
            PushStateIfChanged();
        }
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
                .Select(h => new
                {
                    at = h.At, version = h.Version, build = h.Folder, source = h.Source, first = h.First,
                    check = RobloxWatch.For(h.Folder)?.Verdict, worked = Prefs.MultiWorked.Contains(h.Folder),
                }).ToArray(),
            watch = WatchDto(),
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

    /// <summary>Reads what Bloxkeep's Roblox watch found (<see cref="RobloxWatch"/>), at the Roblox version check's pace.</summary>
    async Task RefreshRobloxWatchAsync()
    {
        try
        {
            if (await RobloxWatch.RefreshAsync())
                ApplyWatchNames();
            PushStateIfChanged();
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException)
        {
            // Shown as "couldn't check"; tried again next time.
        }
    }

    /// <summary>The lock names the watch says to hold besides Multi-Roblox's own, for the next time it takes them.</summary>
    void ApplyWatchNames() =>
        MultiInstance.SetExtraNames(RobloxWatch.ExtraNames(Installs().Client?.Folder, MultiInstance.EventName));

    /// <summary>
    /// The build Multi-Roblox meets next and what the Roblox watch says about it: the installed one, or the live one
    /// when a launcher updates to it before starting Roblox (an older build here, not started directly).
    /// </summary>
    /// <returns>Worked: two windows played together on that build here, which outweighs anything the watch says.</returns>
    (string? Version, string? Hash, RobloxWatch.Build? Check, bool Worked) WatchedBuild()
    {
        var installed = Installs().Client;
        var rv = RobloxVersionDto();
        var live = RobloxWatch.Current?.Live;
        bool updatesFirst = rv.UpToDate == false && !rv.Direct && live is not null;
        string? hash = updatesFirst ? live!.Hash : installed?.Folder;
        string? version = updatesFirst ? live!.Version : installed?.Version;
        return (version, hash, RobloxWatch.For(hash), hash is not null && Prefs.MultiWorked.Contains(hash));
    }

    /// <summary>
    /// Said before opening several windows when the Roblox watch found the single-window lock changed in the build they
    /// will run, and two windows haven't played together on that build here since.
    /// </summary>
    string? WatchWarning()
    {
        var (version, _, check, worked) = WatchedBuild();
        if (check is null || worked)
            return null;
        return check.Verdict switch
        {
            "changed" => L.F("Bloxkeep's Roblox watch found that Roblox {0} changed its single-window code, so Multi-Roblox may not work. If Roblox closes a window when the next one opens, Bloxkeep will say so.", version),
            "renamed" => L.F("Roblox {0} renamed its single-window lock. Bloxkeep holds the new name as well, but that isn't proven yet. If Roblox closes a window when the next one opens, Bloxkeep will say so.", version),
            "gone" => L.F("Roblox {0} no longer has the single-window lock Multi-Roblox holds, so Roblox will probably close each window when the next one opens.", version),
            _ => null,
        };
    }

    object WatchDto()
    {
        var (version, hash, check, worked) = WatchedBuild();
        var next = RobloxWatch.Current?.Next;
        return new
        {
            readAt = RobloxWatch.ReadAt,
            version,
            updatesFirst = hash is not null && hash != Installs().Client?.Folder,
            check = check?.Verdict,
            worked,
            next = next is null ? null : new { version = next.Version, check = next.Verdict },
            held = MultiInstance.ExtraHeld,
        };
    }
}
