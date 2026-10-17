using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Settings › Staying in game › Fast cores for the window you play (<see cref="AppSettings.SplitCores"/>): every Roblox
/// window in the background runs on the efficiency cores in power-saving mode, the one in front on every core
/// (<see cref="CpuCores"/>). Followed at once when another window comes to the front (a WinEvent hook), and at every
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

    /// <summary>A window coming to the front gets its cores a moment later (not anti-AFK's own brief nudges).</summary>
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
            if (!_splitCoresOn || _corePids.Count == 0 || _disposed)
                return;
            _coreCheck.Stop();
            _coreCheck.Start();
        };
        _foregroundHook = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, IntPtr.Zero, _foregroundProc, 0, 0, WINEVENT_OUTOFCONTEXT);
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
                refused = true;
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
            Toast(L.T("Windows refused to move a Roblox window to other cores, so it was left as it was."), "info");
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
