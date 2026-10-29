namespace RobloxManager;

/// <summary>Hotkeys from any app (ShellForm registers them with Windows): Ctrl+Alt+H hides every Roblox window, or shows
/// them again when any is hidden; Ctrl+Alt+A arranges them on the main screen; Ctrl+Alt+M minimizes them all, or opens
/// them again when all are; Ctrl+Alt+N brings the next account's window to the front; Ctrl+Alt+B opens Bloxkeep (the
/// window handles that one). Settings › Bloxkeep › This app turns them off.</summary>
public sealed partial class AppController
{
    public const int HotkeyHide = 1, HotkeyArrange = 2, HotkeyMinimize = 3, HotkeyNext = 4, HotkeyBloxkeep = 5;

    /// <summary>The window registers or drops the hotkeys again (the switch changed).</summary>
    public Action? HotkeysChanged { get; set; }

    /// <summary>The hotkeys another app already holds ("Ctrl+Alt+H"…), as the window found when registering them.</summary>
    public List<string> HotkeysTaken { get; set; } = [];

    public void OnHotkey(int id)
    {
        if (_disposed)
            return;
        if (id == HotkeyHide)
        {
            var pids = RobloxProcesses.GetPlayerPids();
            if (pids.Any(RobloxProcesses.HasHidden))
                RobloxProcesses.ShowWindows();
            else
                RobloxProcesses.HideWindows(pids);
            PushState();
        }
        else if (id == HotkeyArrange && _vault is not null)
        {
            _ = ArrangeWindowsAsync(-1);
        }
        else if (id == HotkeyMinimize)
        {
            MinimizeOrRestoreAll();
        }
        else if (id == HotkeyNext)
        {
            FocusNextWindow();
        }
    }

    /// <summary>The Roblox windows on screen (not the hidden ones), the accounts' in the list's order first, then any other.</summary>
    /// <param name="pids">The clients; every one running when null (the checks pass a window of their own).</param>
    List<(int Pid, IntPtr Window)> ShownRobloxWindows(IReadOnlyCollection<int>? pids = null)
    {
        pids ??= RobloxProcesses.GetPlayerPids();
        int Order(int pid)
        {
            if (_vault is null)
                return int.MaxValue;
            int i = _vault.Data.Accounts.FindIndex(a => _running.TryGetValue(a.Id, out var run) && run.Pid == pid);
            return i < 0 ? int.MaxValue : i;
        }
        var windows = RobloxProcesses.VisibleWindows(pids);
        return pids.OrderBy(Order).ThenBy(pid => pid)
            .Select(pid => (Pid: pid, Window: RobloxProcesses.MainWindowOf(windows[pid])))
            .Where(w => w.Window != IntPtr.Zero && !RobloxProcesses.IsHidden(w.Window))
            .ToList();
    }

    /// <summary>
    /// Ctrl+Alt+M: every Roblox window minimized (resting: the RAM limit then applies), or, when all of them already
    /// are, opened again without taking the keyboard from the app in front.
    /// </summary>
    void MinimizeOrRestoreAll(IReadOnlyCollection<int>? pids = null)
    {
        var shown = ShownRobloxWindows(pids);
        if (shown.Any(w => !RobloxProcesses.IsMinimised(w.Window)))
        {
            foreach (var w in shown)
                RobloxProcesses.Minimise(w.Window);
        }
        else
        {
            foreach (var w in shown)
                RobloxProcesses.RestoreQuietly(w.Window);
        }
    }

    /// <summary>Ctrl+Alt+N: the window after the one in front (the first when none is), opened if minimized, in front.</summary>
    void FocusNextWindow(IReadOnlyCollection<int>? pids = null)
    {
        var shown = ShownRobloxWindows(pids);
        if (shown.Count == 0)
            return;
        int front = shown.FindIndex(w => w.Pid == RobloxProcesses.ForegroundPid());
        RobloxProcesses.Focus(shown[(front + 1) % shown.Count].Pid);
    }
}
