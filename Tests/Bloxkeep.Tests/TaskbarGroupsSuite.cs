using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Settings › Taskbar buttons (TaskbarGroups), on a window of the test's own: a group of Bloxkeep's put on the window
/// (read back as Windows keeps it), kept while the same, moved when it changes, and the window's own id back after.
/// </summary>
static class TaskbarGroupsSuite
{
    static readonly Type Groups = typeof(Prefs).Assembly.GetType("RobloxManager.TaskbarGroups")!;
    static void Apply(int pid, IntPtr hWnd, string group) => Groups.GetMethod("Apply", Harness.F)!.Invoke(null, [pid, new List<IntPtr> { hWnd }, group]);
    static void Remove(int pid, IntPtr hWnd) => Groups.GetMethod("Remove", Harness.F)!.Invoke(null, [pid, new List<IntPtr> { hWnd }]);
    static string? Read(IntPtr hWnd) => (string?)Groups.GetMethod("Read", Harness.F)!.Invoke(null, [hWnd]);

    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        using var form = new Form { ShowInTaskbar = false };
        IntPtr hWnd = form.Handle;
        string? own = Read(hWnd);
        c.Check("a window of its own program has no group id of its own", own is null, own);
        Apply(4242, hWnd, "Bloxkeep.Roblox.account1");
        c.Check("an account's group is put on its window", Read(hWnd) == "Bloxkeep.Roblox.account1", Read(hWnd));
        Apply(4242, hWnd, "Bloxkeep.Preset.farm");
        c.Check("…moved when the grouping changes", Read(hWnd) == "Bloxkeep.Preset.farm", Read(hWnd));
        Remove(4242, hWnd);
        c.Check("turned off: the window's own grouping back", Read(hWnd) is null, Read(hWnd));
        return Task.CompletedTask;
    }
}
