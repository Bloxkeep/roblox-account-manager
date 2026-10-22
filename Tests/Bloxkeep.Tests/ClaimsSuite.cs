using System.Diagnostics;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Which Roblox window is which launch's. Each launch link has a browser tracker id of its own, which the client's
/// command line carries: a window naming a launch's is that launch's whatever started first, one naming another isn't,
/// and one naming none goes by the order of starts, as before. The command line is read off stand-in processes (cmd
/// waiting on ping, the words after "rem" unused); the windows handed to launches are made-up process ids, nothing of
/// Roblox runs.
/// </summary>
static class ClaimsSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        const string First = "111111111111", Second = "222222222222";
        string link = RobloxLauncher.BuildLaunchUri("TICKET", new GameTarget(1818), First);
        c.Check("a launch link names the browser tracker id it was given", link.Contains("+browsertrackerid:" + First, StringComparison.Ordinal), link.Replace("TICKET", "…"));
        c.Check("…and a new one when none is given",
            System.Text.RegularExpressions.Regex.IsMatch(RobloxLauncher.BuildLaunchUri("TICKET", null), @"\+browsertrackerid:\d{12}\+"));

        static Process Start(string words) => Process.Start(new ProcessStartInfo("cmd.exe", $"/d /c ping -n 30 127.0.0.1 >nul & rem {words}")
            { CreateNoWindow = true, UseShellExecute = false })!;
        using var linked = Start(link);
        using var old = Start("--app -t TICKET -j https://www.roblox.com/Game/PlaceLauncher.ashx -b 223456789012 --launchtime=1");
        using var none = Start("--app -t TICKET");
        try
        {
            string? Tracker(Process p) => RobloxProcesses.LaunchTrackerOf(p.Id, p.StartTime.ToFileTimeUtc());
            c.Check("a client started with a launch link: its browser tracker id read from its command line", Tracker(linked) == First, Tracker(linked));
            c.Check("…one started by Roblox's launcher of old (-b N) too", Tracker(old) == "223456789012", Tracker(old));
            c.Check("…one naming none: none", Tracker(none) == "", Tracker(none));
            c.Check("…a process that isn't there: not read", RobloxProcesses.LaunchTrackerOf(int.MaxValue - 7, 1) is null);
        }
        finally
        {
            foreach (var p in new[] { linked, old, none })
            {
                try { p.Kill(true); } catch (InvalidOperationException) { }
            }
        }

        var seed = Vault.Create(Harness.Password);
        var a = new Account { Username = "First", UserId = 31 };
        var b = new Account { Username = "Second", UserId = 32 };
        seed.Data.Accounts.Add(a);
        seed.Data.Accounts.Add(b);
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var t = typeof(AppController);
        var launchType = t.GetNestedType("PendingLaunch", Harness.F)!;
        var pending = Harness.Get<System.Collections.IList>(app, "_pending");
        var late = Harness.Get<object>(app, "_late");
        var running = Harness.Get<System.Collections.IDictionary>(app, "_running");
        object Launch(Account who, int secondsAgo, string tracker)
        {
            var launch = Activator.CreateInstance(launchType, who.Id, new HashSet<int>(), DateTime.UtcNow.AddSeconds(-secondsAgo), new GameTarget(1818), false, null)!;
            launchType.GetProperty("Tracker")!.SetValue(launch, tracker);
            return launch;
        }
        int Window(Account who) => running.Contains(who.Id) ? (int)running[who.Id]!.GetType().GetField("Item1")!.GetValue(running[who.Id])! : 0;
        void Claim(Dictionary<int, string?> windows) => Harness.Invoke(app, "ClaimWindows", windows.Keys.ToHashSet(), (Func<int, bool>)(_ => true),
            (Func<int, List<int>>)(_ => []), (Func<int, string?>)(pid => windows[pid]), DateTime.UtcNow);
        void Reset()
        {
            pending.Clear();
            late.GetType().GetMethod("Clear")!.Invoke(late, null);
            running.Clear();
        }

        // Two accounts opening, each window naming its launch: each gets its own, whichever started first. Before, the
        // newest launch took the lowest process id.
        pending.Add(Launch(a, 30, First));
        pending.Add(Launch(b, 10, Second));
        Claim(new() { [1001] = First, [1002] = Second });
        c.Check("two launches, each window naming its own: each account gets its window", Window(a) == 1001 && Window(b) == 1002, $"{Window(a)} {Window(b)}");
        // A launch past its wait (Roblox updating) and the next account's still opening: the window that comes, naming the
        // first, is the first's. Before, the launch under way took it, and the late one took the next account's window.
        Reset();
        late.GetType().GetMethod("Add")!.Invoke(late, [Launch(a, 150, First)]);
        pending.Add(Launch(b, 10, Second));
        Claim(new() { [1003] = First });
        c.Check("a late launch's window, the next account still opening: the late one's, not the next account's",
            Window(a) == 1003 && Window(b) == 0 && pending.Count == 1, $"{Window(a)} {Window(b)}");
        // A window the person opened from the website (its tracker id is the browser's): no launch's.
        Reset();
        pending.Add(Launch(b, 10, Second));
        Claim(new() { [1004] = "999999999999" });
        c.Check("a window naming another tracker id (the person's own, from the website): not taken for a launch's", Window(b) == 0 && pending.Count == 1, Window(b));
        late.GetType().GetMethod("Add")!.Invoke(late, [Launch(a, 150, First)]);
        Claim(new() { [1004] = "999999999999" });
        c.Check("…nor for a late one's", Window(a) == 0, Window(a));
        // A command line naming none, or not read: the order of starts, as before.
        Reset();
        pending.Add(Launch(b, 10, Second));
        Claim(new() { [1005] = "" });
        c.Check("a window naming no tracker id: taken by the order of starts, as before", Window(b) == 1005, Window(b));
        Reset();
        pending.Add(Launch(b, 10, Second));
        Claim(new() { [1006] = null });
        c.Check("…and one whose command line can't be read", Window(b) == 1006, Window(b));
        Reset();
    }
}
