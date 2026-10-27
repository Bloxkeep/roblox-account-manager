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
        // The tray icon's quick panel and menu (TrayWindow, its page ui/src/Tray.tsx; what the page shows is in the
        // interface checks, tray-*): what it hands the page, and what each command the page sends does.
        using var window = new TrayWindow();
        var farm = Guid.NewGuid();
        var alt = Guid.NewGuid();
        var main = Guid.NewGuid();
        var info = new AppController.TrayInfo(true, "ready", 2, [(farm, "Farm")], [(alt, "Alt", false), (main, "Main", true)],
            Clients: [new(main, "Main", "Blox Fruits", 4321, 1500)], RamMb: 2800);
        window.State = () => info;
        window.MiniOpen = () => true;
        window.PictureOf = id => id == main ? "https://tr.rbxcdn.com/main.png" : null;
        var sent = System.Text.Json.JsonSerializer.SerializeToElement(window.Message());
        var data = sent.GetProperty("data");
        var st = data.GetProperty("state");
        c.Check("the tray's page gets the state: Multi-Roblox, the windows and their RAM, the mini window, presets, accounts and running ones",
            sent.GetProperty("name").GetString() == "tray" && st.GetProperty("multi").GetString() == "ready" && st.GetProperty("robloxCount").GetInt32() == 2
            && st.GetProperty("ramMb").GetInt64() == 2800 && st.GetProperty("miniOpen").GetBoolean() && st.GetProperty("presets")[0].GetProperty("name").GetString() == "Farm"
            && st.GetProperty("accounts").GetArrayLength() == 2 && st.GetProperty("clients")[0].GetProperty("pid").GetInt32() == 4321, sent);
        c.Check("…each with its picture's address once known, none otherwise (its initials then)",
            st.GetProperty("clients")[0].GetProperty("avatar").GetString() == "https://tr.rbxcdn.com/main.png"
            && st.GetProperty("accounts")[0].GetProperty("avatar").ValueKind == System.Text.Json.JsonValueKind.Null);
        c.Check("…with the theme and the language", data.GetProperty("theme").GetString() == Themes.Current.Id && data.GetProperty("language").GetString() == L.Language);
        Guid? ran = null, opened = null, closedOne = null;
        int? shown = null;
        bool closedAll = false, fresh = false, quit = false, mini = false, open = false;
        bool? switched = null;
        window.AccountClicked += id => opened = id;
        window.PresetClicked += id => ran = id;
        window.NewRobloxClicked += () => fresh = true;
        window.CloseAllClicked += () => closedAll = true;
        window.CloseClientClicked += id => closedOne = id;
        window.ShowClientClicked += pid => shown = pid;
        window.QuitClicked += () => quit = true;
        window.MiniClicked += () => mini = true;
        window.OpenClicked += () => open = true;
        window.MultiClicked += on => switched = on;
        void Act(object args) => window.Act(System.Text.Json.JsonSerializer.SerializeToElement(args));
        Act(new { what = "account", id = alt.ToString() });
        Act(new { what = "preset", id = farm.ToString() });
        Act(new { what = "newRoblox" });
        Act(new { what = "closeAll" });
        Act(new { what = "closeClient", id = main.ToString() });
        Act(new { what = "show", pid = 4321 });
        Act(new { what = "multi", on = true });
        Act(new { what = "mini" });
        Act(new { what = "open" });
        Act(new { what = "quit" });
        c.Check("its commands: an account, a preset, a new Roblox, close all, close or show one window, Multi-Roblox, the mini window, Bloxkeep, quit",
            opened == alt && ran == farm && fresh && closedAll && closedOne == main && shown == 4321 && switched == true && mini && open && quit);
        opened = null;
        Act(new { what = "account", id = "not an id" });
        Act(new { what = "nothing" });
        window.OnMessage("not json");
        c.Check("…anything else it sends does nothing", opened is null);
        return Task.CompletedTask;
    }
}
