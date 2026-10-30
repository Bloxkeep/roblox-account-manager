using System.ComponentModel;
using System.Diagnostics;
using System.Text.Json;

namespace RobloxManager;

/// <summary>The process check every second: Roblox's windows found, matched to the launches that opened them, timed, and the PC's memory watched.</summary>
public sealed partial class AppController
{
    /// <summary>A time moved back by a clock jump; one never set (the default) stays as it is.</summary>
    static DateTime MovedBack(DateTime at, TimeSpan back) => at > DateTime.MinValue + back ? at - back : at;

    /// <summary>How often Windows' process list is read while nothing of Roblox's runs and the window is in the tray.</summary>
    static readonly TimeSpan IdleScanInterval = TimeSpan.FromSeconds(15);
    DateTime _lastScan;
    bool _lastScanFoundNone;
    (List<RobloxProcesses.PlayerProcess> Players, DateTime At)? _lastPlayers;

    /// <summary>
    /// Always, unless in the tray with no client found, nothing opening, closing or waiting to rejoin, and the last
    /// read under 15 s old. A Roblox started by hand meanwhile is noticed at the next read.
    /// </summary>
    bool ScanDue(DateTime now) =>
        !(_background && _lastScanFoundNone && _running.Count == 0 && _pending.Count == 0 && _late.Count == 0 && _rejoinQueue.Count == 0
          && !_rejoining && _launchCts is null && now >= _sweepBackgroundUntil && _traySweep.Count == 0 && _closingForRejoin.Count == 0
          && now >= _lastScan && now - _lastScan < IdleScanInterval);

    void OnProcTimer()
    {
        if (_disposed)
            return;
        // Set up on the first tick, once the message loop runs: its window gets Windows' input messages.
        if (!_robloxInputTried)
        {
            _robloxInputTried = true;
            try
            {
                _robloxInput = new RobloxInput();
                if (_robloxInput.Working)
                    _robloxInputAt = _robloxInput.ReachedAt;
            }
            catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or InvalidOperationException)
            {
                Log.Write(ex); // anti-AFK counts input as before
            }
        }
        long start = Stopwatch.GetTimestamp();
        var parts = new List<(string What, TimeSpan Took)>();
        _tickParts = parts;
        try
        {
            TrackProcesses();
        }
        // A timer callback must never take the app down.
        catch (Exception ex)
        {
            LogTickError(ex);
        }
        finally
        {
            _tickParts = null;
            NoteTickTime(Stopwatch.GetElapsedTime(start), parts);
        }
    }

    void TrackProcesses()
    {
        var now = DateTime.UtcNow;
        // The clock was set back: periodic work is due now, and deadlines and start times move back by as much.
        if (now < _lastTick)
        {
            var back = _lastTick - now;
            if (_saveDue is not null)
                _saveDue = now;
            _nextPresence = _nextRobloxVersionCheck = _nextUpdateCheck = now;
            _sweepBackgroundUntil = MovedBack(_sweepBackgroundUntil, back);
            _resumedAt = MovedBack(_resumedAt, back);
            _started = MovedBack(_started, back);
            _lastSessionStart = MovedBack(_lastSessionStart, back);
            _robloxSettingsWrittenAt = MovedBack(_robloxSettingsWrittenAt, back);
            foreach (var launch in _pending.Concat(_late).Concat(_running.Values.Select(r => r.Launch)))
                launch.StartedAt = MovedBack(launch.StartedAt, back);
            foreach (var parent in _traySweep.Keys.ToList())
                _traySweep[parent] = MovedBack(_traySweep[parent], back);
            foreach (var launch in _pendingSeenAlive.Keys.ToList())
                _pendingSeenAlive[launch] = MovedBack(_pendingSeenAlive[launch], back);
            for (int i = 0; i < _rejoinQueue.Count; i++)
                _rejoinQueue[i] = _rejoinQueue[i] with { Due = MovedBack(_rejoinQueue[i].Due, back) };
            _signInCheckRetryAt = MovedBack(_signInCheckRetryAt, back);
            MoveDiscordBack(back);
            if (Prefs.LastSignInCheck is { } checkedAt)
                Prefs.LastSignInCheck = MovedBack(checkedAt, back); // written with the next prefs change; a restart clamps it anyway
        }
        NoteCheckGap(now, _lastTick);
        _lastTick = now;
        // The process list is the costly part of a tick (a few ms, whatever runs).
        bool scan = ScanDue(now);
        long part = Stopwatch.GetTimestamp();
        var players = scan ? RobloxProcesses.GetPlayers() : [];
        Took("process list", part);
        IReadOnlyList<RobloxProcesses.PlayerProcess> scanned = players.ToArray();
        if (scan)
            _lastScan = now;
        var pids = players.Select(p => p.Pid).ToHashSet();
        part = Stopwatch.GetTimestamp();
        var windows = RobloxProcesses.VisibleWindows(pids);
        Took("window list", part);
        bool HasWindow(int pid) => windows.TryGetValue(pid, out var list) && list.Count > 0;
        Dictionary<int, List<int>>? tree = null;
        List<int> ChildrenOf(int parent) => (tree ??= RobloxProcesses.PlayerChildren(scanned)).GetValueOrDefault(parent) ?? [];
        int ParentOf(int child) => (tree ??= RobloxProcesses.PlayerChildren(scanned)).FirstOrDefault(kv => kv.Value.Contains(child)).Key;
        // Roblox's copy in the notification area (RobloxProcesses.IsTrayCopy) is not a client. Once a window of ours
        // closed, or after Close all, it is ended: it would take over the next launch and open nothing.
        part = Stopwatch.GetTimestamp();
        var trays = players.Where(p => !HasWindow(p.Pid) && RobloxProcesses.IsTrayCopy(p.Pid, p.Created)).Select(p => p.Pid).ToHashSet();
        if (now < _sweepBackgroundUntil || _traySweep.Values.Any(until => now <= until))
        {
            foreach (int pid in trays)
            {
                RobloxProcesses.Kill(pid);
                ForgetClientStart(pid);
            }
        }
        pids.ExceptWith(trays);
        players.RemoveAll(p => trays.Contains(p.Pid));
        ReleaseHomeSessionIfIdle(scan, scanned, trays, now);
        NoteClientStarts(pids, now, ParentOf);
        // The browser tracker id on a client's command line says which launch it is (see ClaimWindows).
        var createdOf = new Dictionary<int, long>();
        foreach (var p in scanned)
            createdOf[p.Pid] = p.Created;
        string? TrackerOf(int pid) => RobloxProcesses.LaunchTrackerOf(pid, createdOf.GetValueOrDefault(pid));
        bool changed = false;

        foreach (var (id, info) in _running.ToList())
        {
            if (pids.Contains(info.Pid))
                continue;
            _running.Remove(id);
            PersistSoon(); // the open clients on disk (see Persist)
            // Read before the log watcher drops the pid below, with the log's last lines (a leave logged right before quitting).
            var log = EndedState(info.Pid);
            if (_logs.OutOfMemoryAt(info.Pid) is not null)
                _ranOutOfMemory.Add(id);
            // Closed by Roblox because another client opened: no hand-over, no auto-rejoin. Only a window that closed in
            // order can be that: its log says it shut down, or it left its notification-area copy behind.
            bool inOrder = _logs.ClosedInOrderAt(info.Pid) is not null || ChildrenOf(info.Pid).Any(trays.Contains);
            bool closedByRoblox = CheckClosedByRoblox(id, info.Pid, info.Launch.StartedAt, now, inOrder, ChildrenOf);
            // No log, but a notification-area copy left behind: Roblox starts that only when its window is closed (X or
            // Bloxkeep's close), never on a drop or crash. So a leave by choice, not a drop to rejoin.
            if (log is null && !closedByRoblox && !_closingForRejoin.Contains(info.Pid) && ChildrenOf(info.Pid).Any(trays.Contains))
                log = new RobloxLogWatcher.ClientState("left", info.Launch.Target?.PlaceId, info.Launch.Target?.JobId, null, now);
            _endedHere[id] = (log?.JobId ?? info.Launch.Target?.JobId, log?.PlaceId ?? info.Launch.Target?.PlaceId, now);
            // Hidden: the window auto-rejoin opens for it is hidden too, once in its game (RejoinFromLogs).
            if (RobloxProcesses.ForgetHidden(info.Pid))
                _hideWhenIn.Add(id);
            EndPlay(id, log, now);
            RefreshPresenceSoon(TimeSpan.FromSeconds(8));
            // The first process may only bootstrap and hand over to a child: wait for that only when a new, unclaimed
            // client runs. One whose log shows a join was the real one: its windowless child is the tray copy.
            var claimedNow = _running.Values.Select(r => r.Pid).ToHashSet();
            List<int> trayCopies = log is null ? [] : ChildrenOf(info.Pid).Where(p => !HasWindow(p)).ToList();
            if (log is not null)
                _traySweep[info.Pid] = now + TraySweep;
            // Roblox's updater started since the launch is one too: an outdated client hands the launch to it.
            bool handOver = now - info.Launch.StartedAt < HandOverWindow && !_userClosed.Contains(id) && !closedByRoblox
                && (pids.Any(p => !info.Launch.Before.Contains(p) && !claimedNow.Contains(p) && !trayCopies.Contains(p) && MayBeLaunchOf(TrackerOf(p), info.Launch))
                    || RobloxProcesses.UpdaterRunningSince(info.Launch.StartedAt));
            if (handOver)
            {
                _pending.Add(info.Launch);
                _pendingSeenAlive[info.Launch] = now;
                _handedOver[info.Launch] = (info.Pid, log);
            }
            else
                OnSessionEnded(id, info.Launch, log);
            changed = true;
        }

        foreach (var expired in _pending.Where(p => now - p.StartedAt > PendingTimeout).ToList())
        {
            // No window yet (Roblox updating on a slow link, its launcher stuck): one that comes later is still this
            // launch's, not the next account's (see _late).
            bool waitLonger = !_closeWhenClaimed.Contains(expired) && !_handedOver.ContainsKey(expired);
            DropPending(expired, late: waitLonger);
            if (waitLonger)
            {
                _late.RemoveWhere(l => l.AccountId == expired.AccountId);
                _late.Add(expired);
            }
            changed = true;
        }
        changed |= ClaimWindows(pids, HasWindow, ChildrenOf, TrackerOf, launch => RobloxProcesses.UpdaterRunningSince(launch.StartedAt), now);
        foreach (var gone in _pendingSeenAlive.Keys.Where(l => !_pending.Contains(l)).ToList())
            _pendingSeenAlive.Remove(gone);

        // The tray copy left by a closed client: ended while windowless, never one a launch could be waiting for.
        foreach (var (parent, until) in _traySweep.ToList())
        {
            if (now > until)
            {
                _traySweep.Remove(parent);
                continue;
            }
            var tracked = _running.Values.Select(r => r.Pid).ToHashSet();
            foreach (int child in ChildrenOf(parent))
            {
                if (!pids.Contains(child) || HasWindow(child) || tracked.Contains(child) || _pending.Any(l => !l.Before.Contains(child)))
                    continue;
                RobloxProcesses.Kill(child);
                pids.Remove(child);
                ForgetClientStart(child);
            }
        }
        Took("clients started, ended and matched to launches", part);
        players.RemoveAll(p => !pids.Contains(p.Pid));
        if (scan)
            Step(() => SplitCores(pids));
        if (scan)
        {
            _lastScanFoundNone = pids.Count == 0;
            _lastPlayers = (players.ToList(), now); // the monitor tab's tiles use it (MonitorClients)
        }

        _closingForRejoin.RemoveWhere(pid => !pids.Contains(pid));
        // Once a client has started (its window is up), the file goes back to the person's own settings.
        if (_robloxSettingsPending && _launchCts is null && _pending.Count == 0 && !_rejoining
            && now - _robloxSettingsWrittenAt > TimeSpan.FromSeconds(10))
        {
            bool final = pids.Count == 0;
            RobloxGameSettings.Restore(final);
            if (final)
                _robloxSettingsPending = false;
        }
        // Light AFK's flags: read by the windows they were for, so the file is the person's own again.
        if (_robloxFlagsPending && _launchCts is null && _pending.Count == 0 && !_rejoining
            && now - _robloxSettingsWrittenAt > TimeSpan.FromSeconds(10))
        {
            RobloxFlags.Restore();
            _robloxFlagsPending = false;
        }
        // Every client's log: the ones Bloxkeep opened for their status, all of them for anti-AFK.
        part = Stopwatch.GetTimestamp();
        changed |= _logs.Poll(pids.ToList()) && _running.Count > 0;
        Took("Roblox logs", part);
        var unnamed = _running.Values.Select(r => _logs.StateOf(r.Pid)?.PlaceId ?? 0)
            .Where(id => id > 0 && !HasGame(id, false) && !(_gameFetchedAt.TryGetValue(id, out var asked) && now - asked < GameRetry))
            .Distinct().ToList();
        if (unnamed.Count > 0)
            _ = NameLoggedPlacesAsync(unnamed);
        part = Stopwatch.GetTimestamp();
        RejoinFromLogs();
        ForgetLiftedGameBans();
        ForgetSettledRejoins(now);
        TrackPlay(now);
        RunAntiAfk(now, windows);
        MinimiseAfkWindows(windows);
        MaximiseNewWindows(now, windows);
        Took("rejoins, play time, anti-AFK and window states", part);

        if (_vault is { Data.Settings.RenameWindows: true })
        {
            part = Stopwatch.GetTimestamp();
            foreach (var (id, info) in _running)
            {
                // With its process id, so a window can be matched to Task Manager and to its Roblox log.
                if (FindAccount(id) is { } account && windows.TryGetValue(info.Pid, out var list))
                    RobloxProcesses.SetWindowTitle(list, $"Roblox - {NameText(account)} (PID {info.Pid})");
            }
            Took("window titles", part);
        }
        if (scan)
        {
            Step(() => ApplyWindowIcons(pids, windows));
            Step(() => ApplyTaskbarGroups(pids, windows));
        }

        if (_vault is { Data.Settings.MultiInstance: true } && scan && pids.Count == 0)
        {
            ApplyWatchNames();
            if (!MultiInstance.Armed || MultiInstance.Stale)
                MultiInstance.Enable();
        }
        if (MultiInstance.Armed != _lastArmed)
        {
            _lastArmed = MultiInstance.Armed;
            changed = true;
        }
        if (changed)
            PushStateIfChanged();

        bool active = pids.Count > 0 || _pending.Count > 0 || _rejoinQueue.Count > 0 || now < _sweepBackgroundUntil;
        int interval = _background ? 5000 : active ? 1500 : 4000;
        if (_procTimer.Interval != interval)
            _procTimer.Interval = interval;

        if (now >= _nextRobloxVersionCheck && !_background)
        {
            _nextRobloxVersionCheck = now + RobloxVersionInterval;
            _ = RefreshLatestRobloxAsync();
            _ = RefreshRobloxWatchAsync();
        }
        // Independent jobs: one that fails doesn't stop the rest, least of all the save and auto-rejoin.
        Step(() => TickRobloxUpdates(now, pids.Count(pid => _logs.StateOf(pid)?.Status == "ingame")));

        if (Prefs.CheckUpdates && _updater.Supported && now >= _nextUpdateCheck)
        {
            _nextUpdateCheck = now + UpdateCheckInterval;
            _ = _updater.CheckAsync();
        }
        Step(() => TickLatest(now)); // a new version, from Bloxkeep's bot: the check above brought forward
        Step(() => RelaunchIfQuiet(now)); // a downloaded update, installed once nobody is using the app
        Step(SayUpdateFailed); // one that didn't install, said once something can show it

        Step(CheckSchedules); // also while locked: a run due then is reported as missed
        Step(() => TickDiscordPresence(now)); // Settings › Discord › Show on my Discord profile, locked or not
        Step(() => TickOnline(now)); // Settings › Bloxkeep › Add me to the online count, locked or not
        Step(TickRemote); // Settings › Discord › Control from Discord, locked or not (locked, it says only that)
        // Also while locked: a window that ends soon after the commit limit ran out ran out of memory.
        MemoryStatus memory = default;
        Step(() =>
        {
            memory = SystemMemory.Read();
            if (memory.CommitLimitMb > 0 && memory.CommitRoomMb < CommitMarginMb)
                _memoryShortAt = now;
        }, "SystemMemory.Read()");
        if (_vault is null)
            return;
        Step(() => CheckAutoLock(AntiAfk.IdleTime(), DateTime.UtcNow));
        Step(() => PersistIfDue(now));
        Step(() =>
        {
            NoteClientMemory(players);
            EmitMetrics(players, windows, memory);
            GuardResources(players, memory, now);
            LimitRestingRam(players, windows);
        }, "EmitMetrics, GuardResources");
        Step(() => RunDueRejoin(now));
        Step(() => RestartDueClients(now));
        Step(() => RunSignInCheckIfDue(now));
        Step(() => NoteGameHoursIfDue(now));
        Step(SaveRenewedSignIns); // a cookie Roblox renewed in an answer goes to its account
        Step(() => RefreshRobuxIfDue(now));
        Step(() => TickDiscord(now, players));
        if (!_presenceBusy && now >= _nextPresence)
            _ = RefreshPresenceAsync();
    }

    /// <summary>A failure is logged (<see cref="LogTickError"/>) and the next job runs; timed and named by its code.</summary>
    void Step(Action job, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(job))] string what = "")
    {
        long start = Stopwatch.GetTimestamp();
        try
        {
            job();
        }
        catch (Exception ex)
        {
            LogTickError(ex);
        }
        finally
        {
            Took(what, start);
        }
    }

    /// <summary>Null between checks.</summary>
    List<(string What, TimeSpan Took)>? _tickParts;

    /// <param name="since">A Stopwatch timestamp.</param>
    void Took(string what, long since) => _tickParts?.Add((what, Stopwatch.GetElapsedTime(since)));

    /// <summary>
    /// The check runs on the window's thread, so Bloxkeep answers nothing meanwhile. When all is well it takes a few ms.
    /// </summary>
    static readonly TimeSpan SlowTick = TimeSpan.FromMilliseconds(100);
    (DateTime Since, TimeSpan Slowest, int More)? _slowTick;

    /// <summary>
    /// Written once an hour, and sooner when twice as slow as the slowest written that hour, so the worst one is in the
    /// log with its parts (named by the code: no account data).
    /// </summary>
    void NoteTickTime(TimeSpan took, List<(string What, TimeSpan Took)> parts)
    {
        if (took < SlowTick)
            return;
        var now = DateTime.UtcNow;
        var since = now;
        if (_slowTick is { } last && now >= last.Since && now - last.Since < TickErrorRepeat)
        {
            if (took < last.Slowest * 2)
            {
                _slowTick = last with { More = last.More + 1 };
                return;
            }
            since = last.Since; // the same hour: the next one written must take twice as long again
        }
        static string Name(string what) => what.StartsWith("() => ", StringComparison.Ordinal) ? what[6..] : what;
        var slowest = parts.OrderByDescending(p => p.Took).Take(4).Select(p => $"{Name(p.What)} {p.Took.TotalMilliseconds:0} ms");
        var rest = took - parts.Aggregate(TimeSpan.Zero, (sum, p) => sum + p.Took);
        string unwritten = _slowTick is { More: > 0 } previous ? $" {previous.More} slow check(s) came since the last one written." : "";
        Log.Note($"Slow process check: {took.TotalMilliseconds:0} ms, {_lastPlayers?.Players.Count ?? 0} Roblox client(s). Slowest parts: "
            + $"{string.Join(", ", slowest)}; the rest {rest.TotalMilliseconds:0} ms.{unwritten}");
        _slowTick = (since, took, 0);
    }

    (string Key, DateTime Logged, int Repeats)? _tickError;
    static readonly TimeSpan TickErrorRepeat = TimeSpan.FromHours(1);

    /// <summary>
    /// Once, not every tick (every 1.5 s would rewrite error.log every few minutes): a repeat is counted and written
    /// once an hour.
    /// </summary>
    void LogTickError(Exception ex)
    {
        string where = ex.StackTrace is { } trace ? trace.TrimStart().Split('\n', 2)[0] : "";
        string key = $"{ex.GetType().FullName}|{ex.Message}|{where}";
        var now = DateTime.UtcNow;
        if (_tickError is { } last && last.Key == key && now >= last.Logged && now - last.Logged < TickErrorRepeat)
        {
            _tickError = last with { Repeats = last.Repeats + 1 };
            return;
        }
        if (_tickError is { Repeats: > 0 } previous)
            Log.Note($"The process check's last logged error came {previous.Repeats} more time(s) after it was written.");
        _tickError = (key, now, 0);
        Log.Write(ex);
    }

    /// <summary>Live RAM per client and for the PC, sent apart from the state so the page can patch numbers in place.</summary>
    void EmitMetrics(List<RobloxProcesses.PlayerProcess> players, Dictionary<int, List<IntPtr>> windows, MemoryStatus memory)
    {
        if (!PageAttached || _screenLocked)
            return;
        var workingSets = players.ToDictionary(p => p.Pid, p => p.Ram);
        string json = JsonSerializer.Serialize(new
        {
            accounts = _running.Select(r => new { id = r.Key, ramMb = workingSets.GetValueOrDefault(r.Value.Pid) / MB }),
            robloxCount = players.Count,
            // The monitor tab's tiles: clients with a visible window (see MonitorClients), from this tick's window pass.
            monitorCount = players.Count(p => windows.TryGetValue(p.Pid, out var list) && list.Count > 0),
            totalMb = players.Sum(p => p.Ram) / MB,
            systemMb = memory.TotalMb,
            availableMb = memory.AvailableMb / 128 * 128, // coarse, so idle jitter doesn't repaint the page
            usedPct = memory.LoadPercent,
        }, Json);
        // A sleeping page gets the figures of the moment it wakes, not a queue of old ones.
        if (json == _lastMetrics || _pageAsleep)
            return;
        _lastMetrics = json;
        Post($"{{\"type\":\"event\",\"name\":\"metrics\",\"data\":{json}}}");
    }

    /// <summary>
    /// Opening <paramref name="toOpen"/> more windows would leave less than the margin of the commit room: Play's
    /// "virtual memory is almost full" warning.
    /// </summary>
    internal static bool VirtualMemoryShort(MemoryStatus memory, int toOpen, long perClientCommitMb) =>
        toOpen > 0 && memory.CommitLimitMb > 0 && memory.CommitRoomMb - toOpen * perClientCommitMb < CommitMarginMb;

    /// <summary>The average of those running, else <see cref="DefaultClientCommitMb"/>.</summary>
    static long ClientCommitMb(IReadOnlyCollection<RobloxProcesses.PlayerProcess> players) =>
        players.Count(p => p.Private > 0) is > 0 and var known
            ? Math.Max(DefaultClientCommitMb / 2, players.Where(p => p.Private > 0).Sum(p => p.Private) / known / MB) : DefaultClientCommitMb;

    /// <summary>
    /// Another Roblox wouldn't fit in the commit room, or RAM is nearly all used. The main window then closes its
    /// browser sooner in the background.
    /// </summary>
    public static bool MemoryTight(MemoryStatus memory) =>
        (memory.CommitLimitMb > 0 && memory.CommitRoomMb < DefaultClientCommitMb + CommitMarginMb)
        || memory.LoadPercent >= MemoryWarningLoad;

    /// <summary>
    /// Warns when the PC runs low on RAM. With "save RAM on background windows", windows you aren't playing get a low
    /// memory priority: Windows takes RAM from them first, only when it runs short.
    /// </summary>
    void GuardResources(List<RobloxProcesses.PlayerProcess> players, MemoryStatus memory, DateTime now)
    {
        var s = _vault!.Data.Settings;
        if (s.MemoryWarnings && players.Count > 0 && memory.LoadPercent >= MemoryWarningLoad
            && now - _lastMemoryWarning > MemoryWarningInterval)
        {
            _lastMemoryWarning = now;
            Notify(L.F("Your PC is using {0}% of its RAM. Close some Roblox windows if things get slow.", memory.LoadPercent), "warn", "ram");
        }
        // RAM may be fine while the commit limit is nearly used up: a Roblox window that needs more then crashes.
        else if (s.MemoryWarnings && players.Count > 0 && memory.CommitLimitMb > 0 && memory.CommitRoomMb < CommitMarginMb
            && now - _lastMemoryWarning > MemoryWarningInterval)
        {
            _lastMemoryWarning = now;
            Notify(L.T("RAM is free, but Windows' virtual memory is almost full, so a Roblox window may crash. Close other apps or increase virtual memory."), "warn", "ram");
        }

        foreach (int gone in _lowMemoryPriority.Where(pid => players.All(p => p.Pid != pid)).ToList())
            _lowMemoryPriority.Remove(gone);
        if (!s.TrimBackground)
        {
            foreach (int pid in _lowMemoryPriority)
                RobloxProcesses.SetMemoryPriority(pid, background: false);
            _lowMemoryPriority.Clear();
            return;
        }
        if (players.Count == 0)
            return;

        // Only touch a window when its role changes (it went to the background, or came to the front).
        int foreground = RobloxProcesses.ForegroundPid();
        bool refused = false;
        foreach (var p in players)
        {
            bool background = p.Pid != foreground;
            if (background == _lowMemoryPriority.Contains(p.Pid))
                continue;
            if (!RobloxProcesses.SetMemoryPriority(p.Pid, background))
            {
                refused |= !RobloxProcesses.Ended(p.Pid); // one that closed since the scan had nothing to change
                continue;
            }
            if (background)
                _lowMemoryPriority.Add(p.Pid);
            else
                _lowMemoryPriority.Remove(p.Pid);
        }

        // No emptying their RAM outright: a background game needed it all back at once, and with 5 windows some
        // stuttered, a window grabbed to drag most of all (the owner's video, 2026-10-21).
        if (refused && !_trimRefusedShown)
        {
            _trimRefusedShown = true;
            // Nothing the person can do about it: noted, not shown.
            Log.Note("Save RAM: Windows refused to lower a Roblox window's memory priority; it runs as usual.");
        }
    }

    /// <summary>The choices of Settings › Staying in game › RAM limit for resting windows, in MB (0 = off).</summary>
    internal static readonly int[] RamLimitChoices = [0, 1024, 1536, 2048];

    /// <summary>Clients given a RAM limit, and the limit (-1: one found already limited, by a Bloxkeep that closed before lifting it).</summary>
    readonly Dictionary<int, long> _ramLimited = new();
    bool _ramLimitsAdopted;

    /// <summary>
    /// RAM limit for resting windows: a Roblox window that isn't drawing (minimized, or hidden by Bloxkeep) keeps at most
    /// the chosen RAM, the rest waiting in virtual memory, so more accounts fit on one PC (a resting game uses little of
    /// what it holds: about 1 of its 3 GB, measured 2026-10-29). Lifted once the window shows, comes to the front, or the
    /// option is off. Not for one in view: it still draws, and limited it took twice the CPU.
    /// </summary>
    void LimitRestingRam(List<RobloxProcesses.PlayerProcess> players, Dictionary<int, List<IntPtr>> windows)
    {
        foreach (int gone in _ramLimited.Keys.Where(pid => players.All(p => p.Pid != pid)).ToList())
            _ramLimited.Remove(gone);
        if (!_ramLimitsAdopted)
        {
            _ramLimitsAdopted = true;
            foreach (var p in players.Where(p => RobloxProcesses.HasRamLimit(p.Pid)))
                _ramLimited[p.Pid] = -1;
        }
        // A window shown unseen for a moment (anti-AFK's nudge, /pc screenshot) is left as it is meanwhile.
        if (_nudging || _picturing)
            return;
        long limit = _vault!.Data.Settings.RamLimitMb;
        int front = RobloxProcesses.ForegroundPid();
        foreach (var p in players)
        {
            var window = windows.TryGetValue(p.Pid, out var list) ? RobloxProcesses.MainWindowOf(list) : IntPtr.Zero;
            bool resting = limit > 0 && p.Pid != front && window != IntPtr.Zero
                           && (RobloxProcesses.IsMinimised(window) || RobloxProcesses.IsHidden(window));
            bool limited = _ramLimited.TryGetValue(p.Pid, out long now);
            if (resting && (!limited || now != limit))
            {
                if (RobloxProcesses.LimitRam(p.Pid, limit))
                    _ramLimited[p.Pid] = limit;
            }
            else if (!resting && limited)
            {
                RobloxProcesses.LimitRam(p.Pid, null);
                _ramLimited.Remove(p.Pid);
            }
        }
    }

    /// <summary>Every client's RAM limit lifted (Bloxkeep closing: none is left limited without it).</summary>
    void LiftRamLimits()
    {
        foreach (int pid in _ramLimited.Keys)
            RobloxProcesses.LimitRam(pid, null);
        _ramLimited.Clear();
    }
}
