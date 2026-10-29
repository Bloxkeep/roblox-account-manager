using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Settings › Staying in game › Fast cores for the window you play (<see cref="AppSettings.SplitCores"/>): every Roblox
/// window in the background runs in Windows' efficiency mode, the one in front at full speed (<see cref="CpuCores"/>). Followed at once when another window comes to the front (a WinEvent hook), and at every
/// tick; a client is touched only when its role changes. Turned off or Bloxkeep closing: Windows' own choice again.
/// </summary>
public sealed partial class AppController
{
    /// <summary>The role each client was last given; a client not in it has Windows' own (never touched, or given back).</summary>
    readonly Dictionary<int, CpuCores.Role> _coreRoles = new();
    /// <summary>The clients of the last scan, for a change of window between ticks.</summary>
    List<int> _corePids = [];
    /// <summary>The setting as the vault last had it, followed while the vault is locked too (the windows go on).</summary>
    bool _splitCoresOn;
    bool _coresRefusedShown;
    IntPtr _foregroundHook;
    WinEventProc? _foregroundProc;
    System.Windows.Forms.Timer? _coreCheck;

    delegate void WinEventProc(IntPtr hook, uint eventType, IntPtr hwnd, int idObject, int idChild, uint thread, uint time);
    [DllImport("user32.dll")]
    static extern IntPtr SetWinEventHook(uint eventMin, uint eventMax, IntPtr module, WinEventProc proc, uint process, uint thread, uint flags);
    [DllImport("user32.dll")]
    static extern bool UnhookWinEvent(IntPtr hook);
    const uint EVENT_SYSTEM_FOREGROUND = 3, WINEVENT_OUTOFCONTEXT = 0;

    /// <summary>
    /// A window the person brings to the front gets its cores at once: grabbing one in the background to drag it was
    /// slow for its first 0.7 s (2026-10-21). One anti-AFK brings forward for a moment (<see cref="_nudging"/>) only a
    /// moment later, so a nudge doesn't move the windows back and forth.
    /// </summary>
    void WatchForeground()
    {
        _coreCheck = new System.Windows.Forms.Timer { Interval = 700 };
        _coreCheck.Tick += (_, _) =>
        {
            _coreCheck.Stop();
            Step(() => SplitCores(_corePids));
        };
        _foregroundProc = (_, _, _, _, _, _, _) =>
        {
            if (_disposed)
                return;
            Step(() => FrontMemory(RobloxProcesses.ForegroundPid()));
            if (!_splitCoresOn || _corePids.Count == 0)
                return;
            _coreCheck.Stop();
            if (_nudging)
                _coreCheck.Start();
            else
                Step(() => SplitCores(_corePids));
        };
        _foregroundHook = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, IntPtr.Zero, _foregroundProc, 0, 0, WINEVENT_OUTOFCONTEXT);
    }

    /// <summary>
    /// A window brought to the front gets its normal RAM priority back at once (Settings' RAM saving gives the ones in
    /// the background a low one), not at the next look at the windows, seconds later.
    /// </summary>
    void FrontMemory(int front)
    {
        if (_lowMemoryPriority.Remove(front))
            RobloxProcesses.SetMemoryPriority(front, background: false);
    }

    /// <summary>Gives each client the role it should have now: in front, in the background, or Windows' own.</summary>
    void SplitCores(IReadOnlyCollection<int> pids)
    {
        if (_vault is not null)
            _splitCoresOn = _vault.Data.Settings.SplitCores;
        _corePids = pids.ToList();
        foreach (int gone in _coreRoles.Keys.Where(pid => !pids.Contains(pid)).ToList())
            _coreRoles.Remove(gone);
        int front = _splitCoresOn ? RobloxProcesses.ForegroundPid() : 0;
        bool refused = false;
        foreach (int pid in pids)
        {
            var want = !_splitCoresOn ? CpuCores.Role.Default : pid == front ? CpuCores.Role.Front : CpuCores.Role.Background;
            bool same = _coreRoles.TryGetValue(pid, out var had) ? had == want : want == CpuCores.Role.Default;
            if (same)
                continue;
            if (!CpuCores.Set(pid, want))
            {
                // A client that ended since the last scan (its window closing is often what brought another to the
                // front) had nothing to move: Windows refused nothing.
                refused |= !RobloxProcesses.Ended(pid);
                continue;
            }
            if (want == CpuCores.Role.Default)
                _coreRoles.Remove(pid);
            else
                _coreRoles[pid] = want;
        }
        if (refused && _splitCoresOn && !_coresRefusedShown)
        {
            _coresRefusedShown = true;
            // Nothing the person can do about it: noted, not shown.
            Log.Note("Fast cores: Windows refused to change a Roblox window's priority; it was left as it was.");
        }
    }

    /// <summary>Bloxkeep closing: every client it moved gets Windows' own cores and speed back.</summary>
    void GiveCoresBack()
    {
        if (_foregroundHook != IntPtr.Zero)
            UnhookWinEvent(_foregroundHook);
        _foregroundHook = IntPtr.Zero;
        _coreCheck?.Dispose();
        foreach (int pid in _coreRoles.Keys.ToList())
            CpuCores.Set(pid, CpuCores.Role.Default);
        _coreRoles.Clear();
    }

    /// <summary>For Settings: whether the CPU has two kinds of cores, and how many of its threads are efficiency cores.</summary>
    static object CoresDto() => new { hybrid = CpuCores.Hybrid, efficient = CpuCores.EfficientCount, logical = CpuCores.LogicalCount };
}
