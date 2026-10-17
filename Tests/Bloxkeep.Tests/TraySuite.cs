using System.Diagnostics;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Roblox's notification-area copy ("RobloxPlayerBeta.exe --launch-to-tray") is told by its command line. Checked on
/// stand-ins: a process started with that word in its command line counts as one, the same program without it doesn't,
/// and an answer is kept per process (a pid Windows gives to a later process is asked again).
/// </summary>
static class TraySuite
{
    public static Task Run(Checks c, string dir)
    {
        // ping waits as long as asked and ignores nothing it doesn't know after the target: a quiet stand-in.
        static Process Start(string args) => Process.Start(new ProcessStartInfo("ping.exe", args) { CreateNoWindow = true, UseShellExecute = false })!;
        using var tray = Start("-n 20 127.0.0.1 --launch-to-tray");
        using var plain = Start("-n 20 127.0.0.1");
        try
        {
            long created = tray.StartTime.ToFileTimeUtc();
            c.Check("a process started with --launch-to-tray is Roblox's notification-area copy", RobloxProcesses.IsTrayCopy(tray.Id, created));
            c.Check("the same program without it isn't", !RobloxProcesses.IsTrayCopy(plain.Id, plain.StartTime.ToFileTimeUtc()));
            c.Check("asked again, the same answer", RobloxProcesses.IsTrayCopy(tray.Id, created));
            c.Check("a process that isn't there isn't one", !RobloxProcesses.IsTrayCopy(int.MaxValue - 7, 1));
        }
        finally
        {
            foreach (var p in new[] { tray, plain })
            {
                try { p.Kill(); } catch (InvalidOperationException) { }
            }
        }
        // The tray icon's menu: what it offers with a vault open, two Roblox windows and a preset; and locked.
        var menuType = typeof(Prefs).Assembly.GetType("RobloxManager.TrayMenu")!;
        using var menu = (ContextMenuStrip)Activator.CreateInstance(menuType, true)!;
        var farm = Guid.NewGuid();
        var info = new AppController.TrayInfo(true, "ready", 2, [(farm, "Farm")]);
        menuType.GetProperty("State")!.SetValue(menu, (Func<AppController.TrayInfo>)(() => info));
        Guid? ran = null;
        bool closed = false;
        menuType.GetEvent("PresetClicked")!.AddEventHandler(menu, (Action<Guid>)(id => ran = id));
        menuType.GetEvent("CloseAllClicked")!.AddEventHandler(menu, (Action)(() => closed = true));
        void Open() => menuType.GetMethod("OnOpening", Harness.F)!.Invoke(menu, [new System.ComponentModel.CancelEventArgs()]);
        Open();
        var items = menu.Items.OfType<ToolStripMenuItem>().ToList();
        string Shown() => string.Join(" | ", menu.Items.OfType<ToolStripMenuItem>().Where(i => i.Available).Select(i => i.Text));
        c.Check("tray menu: Multi-Roblox's state, presets, close all, then the window items", Shown() == "Multi-Roblox: on | Run preset | Close all Roblox | Mini window | Open Bloxkeep | Quit Bloxkeep", Shown());
        items[1].DropDownItems[0].PerformClick();
        items[2].DropDownItems[0].PerformClick();
        c.Check("…a preset runs from its submenu, and closing asks with a second item that says how many",
            ran == farm && closed && items[2].DropDownItems[0].Text == "Close 2 Roblox windows", items[2].DropDownItems[0].Text);
        info = new AppController.TrayInfo(false, "off", 0, []);
        Open();
        c.Check("tray menu while locked: Multi-Roblox's state and the window items only", Shown() == "Multi-Roblox: off | Mini window | Open Bloxkeep | Quit Bloxkeep", Shown());
        return Task.CompletedTask;
    }
}
