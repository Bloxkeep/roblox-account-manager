namespace RobloxManager;

/// <summary>Hotkeys from any app (ShellForm registers them with Windows): Ctrl+Alt+H hides every Roblox window, or shows
/// them again when any is hidden; Ctrl+Alt+A arranges them on the main screen. Settings › Bloxkeep › This app turns them off.</summary>
public sealed partial class AppController
{
    public const int HotkeyHide = 1, HotkeyArrange = 2;

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
    }
}
