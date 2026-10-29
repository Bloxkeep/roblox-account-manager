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
    /// <summary>The clients the last process check found, and when.</summary>
    (List<RobloxProcesses.PlayerProcess> Players, DateTime At)? _lastPlayers;

    /// <summary>
    /// Whether this tick reads the process list: always, unless the window is in the tray, the last read found no
    /// Roblox client, nothing is opening, closing or waiting to rejoin, and the last read is under 15 s old. A Roblox
    /// started by hand meanwhile is noticed at the next read; one Bloxkeep starts makes every tick read it again.
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
        // A timer callback must never take the app down; log (once for a failure that repeats) and try again on the next tick.
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

    /// <summary>
    /// Matches launches to the processes they start, keeps window titles in sync, publishes RAM use, runs
    /// auto-rejoin and the resource guard, re-arms multi-instance, and schedules presence refreshes.
    /// </summary>
    void TrackProcesses()
    {
        var now = DateTime.UtcNow;
        // The clock was set back: everything timed by it would wait that much longer, so periodic work is due again
        // now, and deadlines and start times are moved back by the same amount.
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
        // Windows' process list is the costly part of a tick (a few ms, whatever runs): in the tray with no Roblox
        // running and nothing on its way, it is read every 15 s rather than every 5.
        bool scan = ScanDue(now);
        long part = Stopwatch.GetTimestamp();
        var players = scan ? RobloxProcesses.GetPlayers() : [];
        Took("process list", part);
        IReadOnlyList<RobloxProcesses.PlayerProcess> scanned = players.ToArray();
        if (scan)
            _lastScan = now;
        var pids = players.Select(p => p.Pid).ToHashSet();
        // One pass over the desktop's windows serves every window check of this tick.
        part = Stopwatch.GetTimestamp();
        var windows = RobloxProcesses.VisibleWindows(pids);
        Took("window list", part);
        bool HasWindow(int pid) => windows.TryGetValue(pid, out var list) && list.Count > 0;
        // The same snapshot answers every parent → children question (it has each client's parent).
        Dictionary<int, List<int>>? tree = null;
        List<int> ChildrenOf(int parent) => (tree ??= RobloxProcesses.PlayerChildren(scanned)).GetValueOrDefault(parent) ?? [];
        int ParentOf(int child) => (tree ??= RobloxProcesses.PlayerChildren(scanned)).FirstOrDefault(kv => kv.Value.Contains(child)).Key;
        // Roblox's copy in the notification area (RobloxProcesses.IsTrayCopy): not a client. It has no window to wait
        // for or to claim, isn't a start that closed another window, and doesn't count; once a window of ours has
        // closed, or after Close all, it is ended, since it would take over the next launch and open nothing.
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
        // The browser tracker id each client's command line names: which launch it is (see ClaimWindows).
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
            // Read now, with the log's last lines (a leave by choice logged right before the client quit), before the
            // log watcher drops the pid below.
            var log = EndedState(info.Pid);
            // Its log said Windows refused it memory: its drop is said as that (QueueRejoin).
            if (_logs.OutOfMemoryAt(info.Pid) is not null)
                _ranOutOfMemory.Add(id);
            // Roblox closed it because another client opened: not a hand-over, and no auto-rejoin. Only a window that closed
            // in order can be that: its log says it shut down, or it left its notification-area copy behind.
            bool inOrder = _logs.ClosedInOrderAt(info.Pid) is not null || ChildrenOf(info.Pid).Any(trays.Contains);
            bool closedByRoblox = CheckClosedByRoblox(id, info.Pid, info.Launch.StartedAt, now, inOrder, ChildrenOf);
            // Its log not followed (not found in time), but it left its notification-area copy behind: Roblox starts
            // that when its window is closed (the person's X, or Bloxkeep's own close), never on a drop or a crash.
            // A window the person closed is a leave by choice, not a drop to rejoin.
            if (log is null && !closedByRoblox && !_closingForRejoin.Contains(info.Pid) && ChildrenOf(info.Pid).Any(trays.Contains))
                log = new RobloxLogWatcher.ClientState("left", info.Launch.Target?.PlaceId, info.Launch.Target?.JobId, null, now);
            _endedHere[id] = (log?.JobId ?? info.Launch.Target?.JobId, log?.PlaceId ?? info.Launch.Target?.PlaceId, now);
            // Its window was hidden: the one auto-rejoin opens for it is hidden too, once in its game (RejoinFromLogs).
            if (RobloxProcesses.ForgetHidden(info.Pid))
                _hideWhenIn.Add(id);
            EndPlay(id, log, now);
            RefreshPresenceSoon(TimeSpan.FromSeconds(8));
            // The first process may only bootstrap and hand over to a child: wait for that only when a new,
            // unclaimed client is actually running. A window the person closed ends the session right away.
            // A client that got as far as joining a game (its log says so) was the real one, not a bootstrap: a
            // windowless child of it is the copy Roblox starts in the notification area, not a hand-over.
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
            // No window yet (Roblox updating itself on a slow link, its launcher stuck): one that comes later is still
            // this launch's client, rather than nobody's, or the next account's (see _late).
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

        // The copy Roblox starts in the notification area when a client closes (by its X button, a crash): ended
        // while it has no window — never one a launch could be waiting for, or one that isn't in this tick's list.
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
        // Every Roblox window's log: the ones Bloxkeep opened for their status, all of them for anti-AFK.
        part = Stopwatch.GetTimestamp();
        changed |= _logs.Poll(pids.ToList()) && _running.Count > 0;
        Took("Roblox logs", part);
        // A client moved to a place not looked up yet: name it now rather than at the next status check.
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

        // Poll fast only while something is happening; an idle app should cost next to nothing.
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
        // The periodic jobs from here on don't depend on each other: one that fails (logged) doesn't stop the rest,
        // least of all the vault's save and auto-rejoin.
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
        // Also while locked: windows go on running, and one that ends soon after the commit limit ran out ran out of memory.
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
        }, "EmitMetrics, GuardResources");
        Step(() => RunDueRejoin(now));
        Step(() => RestartDueClients(now));
        Step(() => RunSignInCheckIfDue(now));
        Step(SaveRenewedSignIns); // a cookie Roblox renewed in an answer goes to its account
        Step(() => RefreshRobuxIfDue(now));
        Step(() => TickDiscord(now, players));
        if (!_presenceBusy && now >= _nextPresence)
            _ = RefreshPresenceAsync();
    }

    /// <summary>
    /// One job of the process check, on its own: a failure is logged (see <see cref="LogTickError"/>) and the next job
    /// runs. Timed as a part of the check (<see cref="NoteTickTime"/>), named by its code.
    /// </summary>
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

    /// <summary>The parts of the process check under way and how long each took; null between checks.</summary>
    List<(string What, TimeSpan Took)>? _tickParts;

    /// <summary>A part of the process check under way took from <paramref name="since"/> (a Stopwatch timestamp) until now.</summary>
    void Took(string what, long since) => _tickParts?.Add((what, Stopwatch.GetElapsedTime(since)));

    /// <summary>
    /// A process check that took this long goes to the log, with its slowest parts. It runs on the window's thread, so
    /// Bloxkeep's window answers nothing meanwhile (a click, the page's commands). When all is well a check takes a few
    /// ms: its costliest part, the process list, takes a few.
    /// </summary>
    static readonly TimeSpan SlowTick = TimeSpan.FromMilliseconds(100);
    /// <summary>When the hour of the slow checks written began, the slowest written in it, and how many came unwritten since the last.</summary>
    (DateTime Since, TimeSpan Slowest, int More)? _slowTick;

    /// <summary>
    /// A slow process check is written once an hour, as <see cref="LogTickError"/> does, and sooner when it took twice
    /// as long as the slowest written in that hour, so the worst one is in the log with its parts. The parts are named
    /// by the code (no account data).
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

    /// <summary>The last failure of the process check and how often it came again since it was written.</summary>
    (string Key, DateTime Logged, int Repeats)? _tickError;
    /// <summary>A failure that keeps coming back is written again at most this often (with how many times it came).</summary>
    static readonly TimeSpan TickErrorRepeat = TimeSpan.FromHours(1);

    /// <summary>
    /// A failure in the process check goes to the log once, not on every tick (every 1.5 s, which would also delete
    /// and rewrite error.log every few minutes): the same failure again is counted, and written once an hour.
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
        var workingSets = players.ToDictionary(p => p.Pid, p => p.WorkingSet);
        string json = JsonSerializer.Serialize(new
        {
            accounts = _running.Select(r => new { id = r.Key, ramMb = workingSets.GetValueOrDefault(r.Value.Pid) / MB }),
            robloxCount = players.Count,
            // The monitor tab's tiles: clients with a visible window (see MonitorClients), from this tick's window pass.
            monitorCount = players.Count(p => windows.TryGetValue(p.Pid, out var list) && list.Count > 0),
            totalMb = players.Sum(p => p.WorkingSet) / MB,
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
    /// Opening <paramref name="toOpen"/> more Roblox windows, each holding about <paramref name="perClientCommitMb"/> of the
    /// commit limit, would leave less than the margin of what is left of it and the room the page files may still grow:
    /// Play's "virtual memory is almost full" warning.
    /// </summary>
    internal static bool VirtualMemoryShort(MemoryStatus memory, int toOpen, long perClientCommitMb) =>
        toOpen > 0 && memory.CommitLimitMb > 0 && memory.CommitRoomMb - toOpen * perClientCommitMb < CommitMarginMb;

    /// <summary>What a Roblox client holds of Windows' commit limit: the average of those running, else <see cref="DefaultClientCommitMb"/>.</summary>
    static long ClientCommitMb(IReadOnlyCollection<RobloxProcesses.PlayerProcess> players) =>
        players.Count(p => p.Private > 0) is > 0 and var known
            ? Math.Max(DefaultClientCommitMb / 2, players.Where(p => p.Private > 0).Sum(p => p.Private) / known / MB) : DefaultClientCommitMb;

    /// <summary>
    /// Windows is short of memory: another Roblox wouldn't fit in what is left of the commit limit (RAM and paging file
    /// together, with the room the paging files may still grow), or RAM is nearly all used. The main window then closes its browser sooner in the background.
    /// </summary>
    public static bool MemoryTight(MemoryStatus memory) =>
        (memory.CommitLimitMb > 0 && memory.CommitRoomMb < DefaultClientCommitMb + CommitMarginMb)
        || memory.LoadPercent >= MemoryWarningLoad;

    /// <summary>
    /// Warns when the PC runs low on RAM. With "save RAM on background windows", Roblox windows you aren't playing get a
    /// low memory priority (Windows takes RAM from them first, only when it runs short — nothing changes while there is
    /// plenty), and when the PC is really short of RAM they are asked to give some back, at most every two minutes.
    /// A window gets its normal priority back as soon as it is in front, or when the option is turned off.
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
        // RAM may be fine while Windows' commit limit (RAM and paging file together) is nearly used up, with no room left
        // for the paging file to grow: a Roblox window that needs more then crashes.
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

        // No more emptying their RAM outright every 2 minutes once the PC used 85 % of it: a game in the background needed
        // it all back at once, and with 5 windows open some of them stuttered, a window grabbed to drag most of all
        // (the owner's video, 2026-10-21). The low priority lets Windows take RAM from them first, when it runs short.
        if (refused && !_trimRefusedShown)
        {
            _trimRefusedShown = true;
            // Nothing the person can do about it: noted, not shown.
            Log.Note("Save RAM: Windows refused to lower a Roblox window's memory priority; it runs as usual.");
        }
    }
}
