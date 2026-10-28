namespace RobloxManager;

/// <summary>
/// Hiding Roblox windows while their games run (the account menu, the Windows button, a hotkey): the window and its
/// taskbar button go, the game, auto-rejoin and anti-AFK go on (see RobloxProcesses.HideWindows). No accounts: every
/// Roblox window of this PC.
/// </summary>
public sealed partial class AppController
{
    int[] WindowPids(Guid[] ids) => ids.Length == 0
        ? RobloxProcesses.GetPlayerPids()
        : ids.Where(_running.ContainsKey).Select(id => _running[id].Pid).ToArray();

    object HideWindows(Guid[] ids)
    {
        int hidden = RobloxProcesses.HideWindows(WindowPids(ids));
        PushState();
        return hidden;
    }

    object ShowWindows(Guid[] ids)
    {
        int shown = RobloxProcesses.ShowWindows(ids.Length == 0 ? null : WindowPids(ids));
        PushState();
        return shown;
    }
}
