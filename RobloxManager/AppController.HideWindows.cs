namespace RobloxManager;

/// <summary>
/// Hiding Roblox windows while their games run: the window and taskbar button go, the game, auto-rejoin and anti-AFK
/// go on (see RobloxProcesses.HideWindows). No accounts: every Roblox window of this PC.
/// </summary>
public sealed partial class AppController
{
    int[] WindowPids(Guid[] ids) => ids.Length == 0
        ? RobloxProcesses.GetPlayerPids()
        : ids.Where(_running.ContainsKey).Select(id => _running[id].Pid).ToArray();

    // Both wait for a nudge, a picture or a macro's turn under way, as the hotkey does: a turn puts its windows back as it
    // found them, so a window shown meanwhile went unseen again while counted as shown (InputTurn.cs).

    /// <summary>How many were hidden; null when it waits for a turn under way (done once it ends).</summary>
    object? HideWindows(Guid[] ids)
    {
        int? hidden = null;
        InputTurn.RunWhenFree("hide", () =>
        {
            hidden = RobloxProcesses.HideWindows(WindowPids(ids));
            PushState();
        });
        return hidden;
    }

    /// <summary>How many were shown; null when it waits for a turn under way (done once it ends).</summary>
    object? ShowWindows(Guid[] ids)
    {
        int? shown = null;
        InputTurn.RunWhenFree("hide", () =>
        {
            shown = RobloxProcesses.ShowWindows(ids.Length == 0 ? null : WindowPids(ids));
            PushState();
        });
        return shown;
    }
}
