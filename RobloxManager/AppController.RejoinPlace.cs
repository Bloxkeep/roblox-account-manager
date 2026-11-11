namespace RobloxManager;

/// <summary>
/// A rejoin's window goes back where the account's last one was: a window arranged into a tile that crashed, was kicked or
/// lost its connection came back at Roblox's own size (or maximised) over the others, and had to be arranged again (a
/// user's request, 2026-11-11). Each account's window place is noted while it is an ordinary window; a maximised one is
/// left to Roblox and Settings' Maximise.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Where each account's window last stood as an ordinary window (not maximised, minimised or hidden).</summary>
    readonly Dictionary<Guid, Rectangle> _lastPlace = new();
    /// <summary>Rejoins' windows (by process id) going back to their account's place: the place, when the window was first
    /// seen, and when it was put there (null until then).</summary>
    readonly Dictionary<int, (Rectangle Place, DateTime Seen, DateTime? PutAt)> _goingBack = new();
    /// <summary>Windows whose place is noted (theirs, or one already gone back).</summary>
    readonly HashSet<int> _placeNoted = new();

    /// <summary>Roblox sizes its window again as it joins its game: put back once in it (or up this long), a moment after.</summary>
    static readonly TimeSpan GoBackWithoutGame = TimeSpan.FromSeconds(60), GoBackSettle = TimeSpan.FromSeconds(3),
        GoBackCheck = TimeSpan.FromSeconds(6);

    void KeepRejoinPlaces(DateTime now, Dictionary<int, List<IntPtr>> windows)
    {
        foreach (int gone in _goingBack.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _goingBack.Remove(gone);
        _placeNoted.RemoveWhere(pid => !windows.ContainsKey(pid));
        foreach (int gone in _inGameSince.Keys.Where(pid => !_goingBack.ContainsKey(pid)).ToList())
            _inGameSince.Remove(gone);
        if (_vault is null || WindowsInTurn)
            return;
        foreach (var (id, run) in _running)
        {
            if (!windows.TryGetValue(run.Pid, out var list) || list.Count == 0)
                continue;
            var main = RobloxProcesses.MainWindowOf(list);
            if (main == IntPtr.Zero)
                continue;
            // A rejoin's new window: first seen, it takes the place the account's last window had.
            if (run.Launch.Rejoin && !_placeNoted.Contains(run.Pid) && !_goingBack.ContainsKey(run.Pid))
            {
                if (_lastPlace.TryGetValue(id, out var place) && OnAScreen(place))
                    _goingBack[run.Pid] = (place, now, null);
                else
                    _placeNoted.Add(run.Pid); // nothing to go back to (it was maximised, or off every screen now)
            }
            if (_goingBack.TryGetValue(run.Pid, out var back))
            {
                GoBack(run.Pid, main, back, now);
                continue;
            }
            _placeNoted.Add(run.Pid);
            if (RobloxProcesses.IsHidden(main) || RobloxProcesses.IsMinimised(main))
                continue; // put away: the place it had stays
            if (RobloxProcesses.IsPlainWindow(main) && RobloxProcesses.WindowRect(main) is { } rect)
                _lastPlace[id] = rect;
            else
                _lastPlace.Remove(id); // maximised or full screen: the next window is left to Roblox and Maximise
        }
    }

    void GoBack(int pid, IntPtr main, (Rectangle Place, DateTime Seen, DateTime? PutAt) back, DateTime now)
    {
        if (back.PutAt is { } putAt)
        {
            // Checked once more: Roblox can size it again a moment after joining.
            if (now - putAt < GoBackCheck)
                return;
            if (RobloxProcesses.IsPlainWindow(main) && RobloxProcesses.WindowRect(main) is { } after && after != back.Place)
            {
                Log.Note($"Rejoin place: window {pid} moved again to {after.Width}×{after.Height} after going back; put back once more.");
                RobloxProcesses.PlaceWindow(main, back.Place);
            }
            _goingBack.Remove(pid);
            _placeNoted.Add(pid);
            _maximised[pid] = "ingame"; // where it was put stays: Settings' Maximise doesn't take it after
            return;
        }
        bool inGame = _logs.StateOf(pid)?.Status == "ingame";
        if (!inGame && now - back.Seen < GoBackWithoutGame)
            return;
        _inGameSince.TryAdd(pid, now);
        if (now - _inGameSince[pid] < GoBackSettle && inGame)
            return;
        _inGameSince.Remove(pid);
        // Put away by then (minimised, hidden by a hidden rejoin): left as it is, its place kept for the next one.
        if (RobloxProcesses.IsHidden(main) || RobloxProcesses.IsMinimised(main))
        {
            Log.Note($"Rejoin place: window {pid} left as it is, {(RobloxProcesses.IsHidden(main) ? "hidden" : "minimised")}.");
            _goingBack.Remove(pid);
            _placeNoted.Add(pid);
            return;
        }
        // Maximised by Roblox (its last close): back to an ordinary window first.
        RobloxProcesses.UnmaximiseQuietly(main);
        RobloxProcesses.PlaceWindow(main, back.Place);
        Log.Note($"Rejoin place: window {pid} put back at {back.Place.Width}×{back.Place.Height}, where the account's last window was"
            + $"{(inGame ? "" : $" (not in its game after {(int)GoBackWithoutGame.TotalSeconds} s)")}.");
        _goingBack[pid] = back with { PutAt = now };
    }

    /// <summary>When each window going back was first seen in its game (it settles a moment first).</summary>
    readonly Dictionary<int, DateTime> _inGameSince = new();

    /// <summary>A rejoin's window going back to its account's place: Settings' Maximise leaves it alone.</summary>
    bool GoingBack(int pid) => _goingBack.ContainsKey(pid);

    /// <summary>Arrange placed every window since: one still waiting to go back stays in its new tile.</summary>
    void ForgetGoingBack()
    {
        foreach (int pid in _goingBack.Keys)
            _placeNoted.Add(pid);
        _goingBack.Clear();
    }

    static bool OnAScreen(Rectangle place) => Screen.AllScreens.Any(s => s.WorkingArea.IntersectsWith(place));
}
