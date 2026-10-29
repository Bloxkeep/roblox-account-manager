namespace RobloxManager;

/// <summary>Anti-AFK: nudging the Roblox windows before Roblox kicks them for being idle, and the idle kicks told.</summary>
public sealed partial class AppController
{
    /// <summary>
    /// From <see cref="AntiAfk.Interval"/> per Roblox window (counted from joining a server or its last input), at the
    /// first minute nobody uses the PC, nudges every Roblox window on the PC that is in a server — also ones opened
    /// outside Bloxkeep; from <see cref="AntiAfk.LastChance"/>, in a short pause even while the PC is in use. A window
    /// the person is playing in gets their input anyway. Settings' switch covers them all; a preset's own anti-AFK
    /// option decides for the windows it opened (<see cref="AntiAfkOn"/>).
    /// </summary>
    void RunAntiAfk(DateTime now, Dictionary<int, List<IntPtr>> windows)
    {
        foreach (int gone in _lastInput.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _lastInput.Remove(gone);
        foreach (int gone in _nudgeRetryAt.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _nudgeRetryAt.Remove(gone);
        foreach (int gone in _nudgeMisses.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _nudgeMisses.Remove(gone);
        foreach (int gone in _afkHeld.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _afkHeld.Remove(gone);
        _idleKickReported.RemoveWhere(pid => !windows.ContainsKey(pid));
        // A nudge cut short (Bloxkeep closed in the middle of one) left its window unseen and click-through.
        if (!_nudging)
            AntiAfk.Recover(windows.Values.SelectMany(w => w));
        if (_vault is null || windows.Count == 0)
            return;
        var on = windows.Keys.Where(AntiAfkOn).ToHashSet();
        if (on.Count == 0)
            return;
        ReportIdleKicks(on);
        // A window opened while Bloxkeep runs has been idle at least since it joined its server (or since now). One
        // already open when Bloxkeep started (an update, a restart) may have had no input for nearly 20 minutes —
        // Bloxkeep can't know — so it is due at once, even for a nudge while the PC is in use: counting from the
        // start had Roblox kick it first.
        foreach (int pid in on)
            _lastInput.TryAdd(pid, _logs.StateOf(pid) is { Status: "ingame" } joined ? joined.At : _afkAdopted ? now : now - AntiAfk.LastChance);
        _afkAdopted = true;
        if (_nudging)
            return;
        // A window held on Roblox's bot check: the person may be solving it, and a nudge brings another window to the
        // front for a moment. The check was cancelled twice while another window was due a nudge (2026-10-27), so
        // none goes while it is in front or for BotCheckHold after it came. Not for good: a check nobody solves left
        // every other window without anti-AFK, kicked after 20 minutes.
        int front = RobloxProcesses.ForegroundPid();
        if (windows.Keys.Any(pid => _logs.JoinChallengedAt(pid) is { } at && _logs.StateOf(pid) is { Status: "joining" }
                                    && (pid == front || now - at < BotCheckHold)))
            return;
        var due = AntiAfkDue(now, on, windows, AntiAfk.Locked(), AntiAfk.IdleTime(), RobloxProcesses.ForegroundPid(), AntiAfk.FullScreenInFront);
        if (due.Count > 0)
            _ = NudgeAsync(due.Select(pid => (pid, windows[pid][0])).ToList());
    }

    /// <summary>
    /// One tick's decision: the windows to nudge now (of <paramref name="on"/>); why the others due wait is noted in
    /// <see cref="_afkHeld"/>.
    /// </summary>
    /// <param name="idle">How long nobody has used the PC (anti-AFK's own nudges don't count).</param>
    /// <param name="foreground">The process in front.</param>
    /// <param name="fullScreen">Whether a full-screen app is in front, asked only when it matters.</param>
    List<int> AntiAfkDue(DateTime now, HashSet<int> on, Dictionary<int, List<IntPtr>> windows, bool locked, TimeSpan idle, int foreground, Func<bool> fullScreen)
    {
        bool Due(int pid, TimeSpan after) =>
            windows[pid].Count > 0 && _logs.StateOf(pid)?.Status is null or "ingame" && now - LastInput(pid) >= after;
        bool Waiting(int pid) => _nudgeRetryAt.TryGetValue(pid, out var retry) && now < retry;
        // Locked: no window can come to the front, so none is tried (a minimised one would be restored and minimised
        // again for nothing).
        if (locked)
        {
            foreach (int pid in on.Where(pid => Due(pid, AntiAfk.Interval)))
                _afkHeld[pid] = AfkHeld.Locked;
            return [];
        }
        // The person's own input that reached a Roblox window: keys with it in front, the mouse over it in front (Windows'
        // raw input, RobloxInput). Without that (not set up), the PC used with a Roblox window in front counts for it,
        // as before: a mouse on another monitor too, which Roblox never got.
        if (_robloxInputAt is { } reachedAt)
        {
            foreach (int pid in windows.Keys)
            {
                if (reachedAt(pid) is { } at && at > _lastInput.GetValueOrDefault(pid, DateTime.MinValue) && at <= now)
                {
                    _lastInput[pid] = at;
                    _afkHeld.Remove(pid);
                }
            }
        }
        else if (idle < AntiAfk.UserIdleNeeded && windows.ContainsKey(foreground))
        {
            _lastInput[foreground] = now;
            _afkHeld.Remove(foreground);
        }
        if (idle >= AntiAfk.UserIdleNeeded)
            return on.Where(pid => Due(pid, AntiAfk.Interval) && !Waiting(pid)).ToList();
        // Someone is using the PC: the windows due wait for a minute without input, but one about to be kicked goes
        // in a pause of two seconds (typed into then, a key would reach Roblox) — unless a full-screen app is in
        // front, which isn't interrupted.
        var last = on.Where(pid => Due(pid, AntiAfk.LastChance)).ToHashSet();
        foreach (int pid in on.Where(pid => Due(pid, AntiAfk.Interval)))
            _afkHeld[pid] = AfkHeld.InUse;
        // The next idle spell tries every other window again from the start.
        foreach (int pid in _nudgeRetryAt.Keys.Where(pid => !last.Contains(pid)).ToList())
        {
            _nudgeRetryAt.Remove(pid);
            _nudgeMisses.Remove(pid);
        }
        if (last.Count == 0 || idle < AntiAfk.PauseNeeded)
            return [];
        if (!windows.ContainsKey(foreground) && fullScreen())
        {
            foreach (int pid in last)
                _afkHeld[pid] = AfkHeld.FullScreen;
            return [];
        }
        return last.Where(pid => !Waiting(pid)).ToList();
    }

    /// <summary>When to try a window again after <paramref name="misses"/> missed nudges in a row (1 min, doubling to 15).</summary>
    static DateTime NudgeRetryAt(DateTime now, int misses, DateTime lastInput)
    {
        var later = now + TimeSpan.FromTicks(Math.Min(NudgeRetry.Ticks << Math.Min(misses - 1, 4), MaxNudgeRetry.Ticks));
        var lastChance = lastInput + AntiAfk.LastChance;
        return later < lastChance ? later : lastChance > now + NudgeRetry ? lastChance : now + NudgeRetry;
    }

    /// <summary>Which window the person's own input reached (Windows' raw input), set up on the first process tick.</summary>
    RobloxInput? _robloxInput;
    bool _robloxInputTried;
    /// <summary>When the person's own input last reached a process's window; null when not known (counted as before). A field, so the checks can answer.</summary>
    Func<int, DateTime?>? _robloxInputAt;

    DateTime LastInput(int pid)
    {
        var last = _lastInput.GetValueOrDefault(pid, DateTime.UtcNow);
        // Roblox counts idle time from joining a server (a server change starts it over).
        return _logs.StateOf(pid) is { Status: "ingame" } log && log.At > last ? log.At : last;
    }

    async Task NudgeAsync(List<(int Pid, IntPtr Window)> clients)
    {
        _nudging = true;
        try
        {
            // Turning anti-AFK off (or locking) stops it between two windows.
            var reached = await AntiAfk.NudgeAsync(clients.Select(c => c.Window).ToList(), () => clients.Any(c => AntiAfkOn(c.Pid)));
            foreach (var (pid, window) in clients)
            {
                if (reached.Contains(window))
                {
                    _lastInput[pid] = DateTime.UtcNow;
                    _nudgeRetryAt.Remove(pid);
                    _nudgeMisses.Remove(pid);
                    _afkHeld.Remove(pid);
                }
                else
                {
                    // Not reached: the person came back meanwhile, or Windows kept it in the background.
                    _afkHeld[pid] = AntiAfk.IdleTime() < AntiAfk.UserIdleNeeded ? AfkHeld.InUse : AntiAfk.Locked() ? AfkHeld.Locked : AfkHeld.Refused;
                    // Focus refused: not every tick, and less often while it lasts (a minimised window would otherwise
                    // be restored and minimised again every minute), 15 min at most — but never past the window's last
                    // chance, which is tried again within a minute.
                    int misses = _nudgeMisses[pid] = _nudgeMisses.GetValueOrDefault(pid) + 1;
                    _nudgeRetryAt[pid] = NudgeRetryAt(DateTime.UtcNow, misses, LastInput(pid));
                }
            }
        }
        finally
        {
            _nudging = false;
        }
    }

    /// <summary>
    /// Says so, and why anti-AFK didn't reach it, when an account is kicked for idling although anti-AFK is on for its
    /// window (<paramref name="on"/>). error.log gets the same (no account data), to look into later.
    /// </summary>
    void ReportIdleKicks(HashSet<int> on)
    {
        foreach (var (id, info) in _running)
        {
            if (!on.Contains(info.Pid) || _logs.StateOf(info.Pid) is not { Status: "disconnected", Reason: IdleKick } kick || !_idleKickReported.Add(info.Pid)
                || FindAccount(id) is not { } account)
                continue;
            // Kicked before this run (Bloxkeep restarted while the window stayed on Roblox's message): said back then.
            if (kick.At < _started)
                continue;
            AfkHeld? held = _afkHeld.TryGetValue(info.Pid, out var why) ? why : null;
            Log.Note($"Idle kick with anti-AFK on: Roblox pid {info.Pid}, last input {(_lastInput.TryGetValue(info.Pid, out var last) ? last.ToLocalTime().ToString("HH:mm:ss") : "unknown")}, "
                     + $"held back by: {held?.ToString() ?? "nothing noted"}.");
            Notify(held switch
            {
                AfkHeld.InUse => L.F("{0} was kicked for being AFK: the PC was in use without a pause of two seconds, so anti-AFK couldn't step in.", Name(account)),
                AfkHeld.FullScreen => L.F("{0} was kicked for being AFK: a full-screen app was in front, and anti-AFK doesn't interrupt one.", Name(account)),
                AfkHeld.Locked => L.F("{0} was kicked for being AFK: the PC was locked, and anti-AFK can't reach Roblox then.", Name(account)),
                AfkHeld.Refused => L.F("{0} was kicked for being AFK: Windows didn't let Bloxkeep bring its window to the front.", Name(account)),
                _ => L.F("{0} was kicked for being AFK.", Name(account)),
            }, "warn", "afk", account.Id);
        }
    }
}
