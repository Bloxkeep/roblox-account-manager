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
        var alt = Guid.NewGuid();
        var info = new AppController.TrayInfo(true, "ready", 2, [(farm, "Farm")], [(alt, "Alt", false), (Guid.NewGuid(), "Main", true)]);
        menuType.GetProperty("State")!.SetValue(menu, (Func<AppController.TrayInfo>)(() => info));
        Guid? ran = null, opened = null;
        bool closed = false, fresh = false;
        menuType.GetEvent("AccountClicked")!.AddEventHandler(menu, (Action<Guid>)(id => opened = id));
        menuType.GetEvent("NewRobloxClicked")!.AddEventHandler(menu, (Action)(() => fresh = true));
        menuType.GetEvent("PresetClicked")!.AddEventHandler(menu, (Action<Guid>)(id => ran = id));
        menuType.GetEvent("CloseAllClicked")!.AddEventHandler(menu, (Action)(() => closed = true));
        void Open() => menuType.GetMethod("OnOpening", Harness.F)!.Invoke(menu, [new System.ComponentModel.CancelEventArgs()]);
        Open();
        var items = menu.Items.OfType<ToolStripMenuItem>().ToList();
        string Shown() => string.Join(" | ", menu.Items.OfType<ToolStripMenuItem>().Where(i => i.Available).Select(i => i.Text));
        ToolStripMenuItem Item(string text) => items.Single(i => i.Text == text);
        c.Check("tray menu: what is going on, Bloxkeep's window items, accounts, presets, a new Roblox, Multi-Roblox, close all, quit",
            Shown() == "Multi-Roblox on · 2 Roblox windows | Open Bloxkeep | Mini window | Play as | Run preset | New Roblox window to sign in | Multi-Roblox | Close all Roblox | Quit Bloxkeep", Shown());
        c.Check("…the first row is a header, not a command; Multi-Roblox and close all say their value",
            !items[0].CanSelect && Item("Multi-Roblox").ShortcutKeyDisplayString == "On" && Item("Close all Roblox").ShortcutKeyDisplayString == "2");
        Item("New Roblox window to sign in").PerformClick();
        Item("Play as").DropDownItems[0].PerformClick();
        Item("Run preset").DropDownItems[0].PerformClick();
        Item("Close all Roblox").DropDownItems[0].PerformClick();
        var main = (ToolStripMenuItem)Item("Play as").DropDownItems[1];
        c.Check("…a new Roblox, an account and a preset open from it, and closing asks with a second item that says how many",
            fresh && opened == alt && ran == farm && closed && Item("Close all Roblox").DropDownItems[0].Text == "Close 2 Roblox windows", Item("Close all Roblox").DropDownItems[0].Text);
        c.Check("…an account already open is ticked and can't be opened twice", main.Checked && !main.Enabled);
        bool CanSwitch(bool on) => (bool)menuType.GetMethod("CanSwitchMulti")!.Invoke(menu, [on])!;
        c.Check("…with a vault, Multi-Roblox turns off from here (in its submenu), and on only in Bloxkeep (its warning)",
            CanSwitch(true) && !CanSwitch(false) && Item("Multi-Roblox").DropDownItems.OfType<ToolStripMenuItem>().Any(i => i.Available && i.Text == "Turn off Multi-Roblox"));
        info = new AppController.TrayInfo(false, "off", 0, []);
        Open();
        c.Check("tray menu while locked: the header says so, Bloxkeep's window items, a new Roblox and Multi-Roblox",
            Shown() == "Locked · Multi-Roblox off | Open Bloxkeep | Mini window | New Roblox window to sign in | Multi-Roblox | Quit Bloxkeep", Shown());
        c.Check("…locked, Multi-Roblox isn't switched from here", !CanSwitch(true) && !CanSwitch(false));
        info = new AppController.TrayInfo(false, "ready", 3, [], [], MultiOnly: true);
        Open();
        c.Check("Multi-Roblox only (no vault): it turns on and off from here, and every Roblox closes",
            CanSwitch(true) && CanSwitch(false)
            && Shown() == "Multi-Roblox on · 3 Roblox windows | Open Bloxkeep | Mini window | New Roblox window to sign in | Multi-Roblox | Close all Roblox | Quit Bloxkeep", Shown());

        // The quick panel (a left click): what its parts do, clicked as the pointer would.
        var panelType = typeof(Prefs).Assembly.GetType("RobloxManager.TrayPanel")!;
        using var panel = (Form)Activator.CreateInstance(panelType, true)!;
        var accountsList = new List<(Guid, string, bool)> { (alt, "Alt", false), (Guid.NewGuid(), "Main", true) };
        var clients = new List<AppController.TrayClient> { new(Guid.NewGuid(), "Main", "Blox Fruits", 4321, 1500) };
        info = new AppController.TrayInfo(true, "ready", 1, [(farm, "Farm")], accountsList, false, clients, 1500);
        panelType.GetProperty("State")!.SetValue(panel, (Func<AppController.TrayInfo>)(() => info));
        int multiSwitched = 0, opens = 0, closeAll = 0, played = 0;
        Guid? playedId = null;
        panelType.GetEvent("MultiClicked")!.AddEventHandler(panel, (Action<bool>)(_ => multiSwitched++));
        panelType.GetEvent("OpenClicked")!.AddEventHandler(panel, (Action)(() => opens++));
        panelType.GetEvent("CloseAllClicked")!.AddEventHandler(panel, (Action)(() => closeAll++));
        panelType.GetEvent("AccountClicked")!.AddEventHandler(panel, (Action<Guid>)(id => { played++; playedId = id; }));
        panelType.GetMethod("Place", Harness.F)!.Invoke(panel, [new Point(800, 600)]);
        var hits = (System.Collections.IList)panelType.GetField("_hits", Harness.F)!.GetValue(panel)!;
        List<string> Keys() => hits.Cast<object>().Select(h => (string)h.GetType().GetField("Item2")!.GetValue(h)!).ToList();
        void Click(string key)
        {
            var hit = hits.Cast<object>().First(h => (string)h.GetType().GetField("Item2")!.GetValue(h)! == key);
            ((Action)hit.GetType().GetField("Item3")!.GetValue(hit)!)();
        }
        c.Check("quick panel: Bloxkeep's buttons, Multi-Roblox, close all, the running account, then Play, Presets and a new Roblox",
            string.Join(",", Keys().Select(k => k.Split(':')[0])) == "mini,open,multi,closeall,client,play,presets,new", string.Join(",", Keys()));
        Click("multi");
        c.Check("…with a vault and Multi-Roblox on, its card turns it off here", multiSwitched == 1 && opens == 0);
        Click("closeall");
        c.Check("…close all asks with a second click first", closeAll == 0);
        Click("closeall");
        c.Check("…and closes on that second click", closeAll == 1);
        Click("play");
        Click(Keys().First(k => k.StartsWith("account:")));
        c.Check("…Play opens its page of accounts; the one already playing isn't offered", played == 1 && playedId == alt && Keys().Count(k => k.StartsWith("account:")) == 1);
        info = new AppController.TrayInfo(true, "off", 0, [], accountsList);
        panelType.GetField("_page", Harness.F)!.SetValue(panel, "home");
        panelType.GetMethod("Reload", Harness.F)!.Invoke(panel, [true]);
        Click("multi");
        c.Check("…Multi-Roblox off: the card opens Bloxkeep, whose switch asks first", multiSwitched == 1 && opens == 1);
        return Task.CompletedTask;
    }
}
