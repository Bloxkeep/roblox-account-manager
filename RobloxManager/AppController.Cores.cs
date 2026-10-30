using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Fast cores for the window you play (<see cref="AppSettings.SplitCores"/>): background Roblox windows run in Windows'
/// efficiency mode, the front one at full speed (<see cref="CpuCores"/>). Followed on a foreground WinEvent and every
/// tick; a client is touched only when its role changes. Off or Bloxkeep closing: Windows' own choice again.
/// </summary>
public sealed partial class AppController
{
    /// <summary>A client not in it has Windows' own role (never touched, or given back).</summary>
    readonly Dictionary<int, CpuCores.Role> _coreRoles = new();
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
    /// A window brought to the front gets its cores at once: dragging a background one was slow for 0.7 s (2026-10-21).
    /// One anti-AFK brings forward (<see cref="_nudging"/>) waits, so a nudge doesn't flip the windows back and forth.
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

    /// <summary>The front window gets its normal RAM priority back at once, not at the next scan seconds later.</summary>
    void FrontMemory(int front)
    {
        if (_lowMemoryPriority.Remove(front))
            RobloxProcesses.SetMemoryPriority(front, background: false);
        // Its RAM limit too (not for anti-AFK's moment in front: limited again right after, for nothing).
        if (!_nudging && !_picturing && _ramLimited.Remove(front))
            RobloxProcesses.LimitRam(front, null);
    }

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
                // A client that ended since the last scan (often what brought another to the front) is no refusal.
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

    static object CoresDto() => new { hybrid = CpuCores.Hybrid, efficient = CpuCores.EfficientCount, logical = CpuCores.LogicalCount };
}
