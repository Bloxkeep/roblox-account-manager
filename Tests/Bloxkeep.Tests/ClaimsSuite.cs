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
        bool updating = false;
        // Taken for a Func of the private launch type (a Func's argument is contravariant).
        Func<object, bool> updatingNow = _ => updating;
        void Claim(Dictionary<int, string?> windows) => Harness.Invoke(app, "ClaimWindows", windows.Keys.ToHashSet(), (Func<int, bool>)(_ => true),
            (Func<int, List<int>>)(_ => []), (Func<int, string?>)(pid => windows[pid]), updatingNow, DateTime.UtcNow);
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

        // An outdated client hands its launch to Roblox's updater (RobloxPlayerInstaller.exe) and quits; the updated client
        // comes a while later. While the updater runs, the launch is still waited for; without it, the launch whose every
        // client quit stops waiting 5 s on, as before. Before, it was dropped then all the same.
        var alive = Harness.Get<System.Collections.IDictionary>(app, "_pendingSeenAlive");
        var handed = Launch(b, 20, Second);
        pending.Add(handed);
        alive[handed] = DateTime.UtcNow.AddSeconds(-10);
        updating = true;
        Claim(new());
        c.Check("every client of a launch quit, Roblox's updater started since: still waited for", pending.Contains(handed), pending.Count);
        updating = false;
        alive[handed] = DateTime.UtcNow.AddSeconds(-10);
        Claim(new());
        c.Check("…without it: no longer waited for, as before", !pending.Contains(handed), pending.Count);
        Reset();

        // Why Roblox wouldn't start, as the person reads it (a notification, Discord, /pc): .NET's own message names the
        // program it was asked to start, and with Windows opening the link that is the link, sign-in ticket and all.
        var startError = t.GetMethod("StartErrorText", Harness.F)!;
        string Said(Exception ex) => (string)startError.Invoke(null, [ex])!;
        string ticketed = Path.Combine(dir, "gameinfo:SECRET-TICKET.exe".Replace(':', '_'));
        try
        {
            Process.Start(new ProcessStartInfo(ticketed) { UseShellExecute = false })?.Dispose();
            c.Check("a program that isn't there doesn't start", false);
        }
        catch (System.ComponentModel.Win32Exception ex)
        {
            c.Check(".NET's message for a start that failed names the program (the link, when Windows opens one)", ex.Message.Contains("SECRET-TICKET"), ex.Message);
            c.Check("…never shown: a sentence of Bloxkeep's with Windows' error number", !Said(ex).Contains("SECRET") && Said(ex).Contains("(error 2)"), Said(ex));
        }
        string linkNamed = RobloxLauncher.BuildLaunchUri("SECRET-TICKET", new GameTarget(1818));
        var shellFailed = new System.ComponentModel.Win32Exception(1155, $"An error occurred trying to start process '{linkNamed}' with working directory 'C:\\'. No application is associated.");
        c.Check("…nor the link Windows was asked to open", !Said(shellFailed).Contains("SECRET"), Said(shellFailed));
        var notInstalled = new FileNotFoundException("Bloxstrap is not installed.");
        c.Check("Bloxkeep's own sentence (a launcher not installed) is shown as it is", Said(notInstalled) == "Bloxstrap is not installed.", Said(notInstalled));
        c.Check("…a ticket in one hidden all the same", !Said(new FileNotFoundException("gone: " + linkNamed)).Contains("SECRET"));

        // A launch handed to Roblox's updater (its client quit as it started the update) and past its 2 minutes, the updater
        // still downloading on a slow link: still waited for, LateTimeout more at most. Before, it was dropped, a rejoin
        // opened a second outdated client for the account, and the updated window was nobody's.
        var handedOver = Harness.Get<System.Collections.IDictionary>(app, "_handedOver");
        var handedValue = handedOver.GetType().GetGenericArguments()[1];
        void Expire() => Harness.Invoke(app, "ExpirePending", DateTime.UtcNow, updatingNow);
        var slow = Launch(b, 150, Second);
        pending.Add(slow);
        handedOver[slow] = Activator.CreateInstance(handedValue, 4242, null);
        updating = true;
        Expire();
        c.Check("handed to Roblox's updater, past 2 minutes, the updater still at it: still waited for", pending.Contains(slow) && handedOver.Contains(slow));
        updating = false;
        Expire();
        c.Check("…once the updater is done without a window: no longer waited for, as before", !pending.Contains(slow) && !handedOver.Contains(slow));
        var stuck = Launch(b, 60 * 8, Second);
        pending.Add(stuck);
        handedOver[stuck] = Activator.CreateInstance(handedValue, 4242, null);
        updating = true;
        Expire();
        c.Check("…nor for ever: past 7 minutes, given up whatever the updater does", !pending.Contains(stuck));
        updating = false;
        Reset();

        // Which client handed its launch to the updater, by its log (the owner's logs of 2026-10-30, 0.744 -> 0.745): after
        // "Update mode is chosen as BACKGROUND" the client goes on into its game; without it, it quits and the updater opens
        // the updated one. Before, any updater running counted, so one updating in the background that ended was waited
        // for instead of rejoined.
        var watcher = Harness.Get<RobloxLogWatcher>(app, "_logs");
        var tails = Harness.Get<System.Collections.IDictionary>(watcher, "_tails");
        var tailType = typeof(RobloxLogWatcher).GetNestedType("Tail", Harness.F)!;
        var applyText = typeof(RobloxLogWatcher).GetMethod("ApplyText", Harness.F)!;
        const string UpdateStart = @"2026-10-30T04:53:44.774Z,0.774840,9b3c,6,Info [FLog::UpdateController] Update thread: start """"C:\Users\A\AppData\Local\Roblox\Versions\version-1\RobloxPlayerInstaller.exe"" roblox-player:1+launchmode:play+gameinfo:TICKET+launchtime:1+browsertrackerid:111111111111""";
        const string Background = "2026-10-30T04:53:44.770Z,0.770000,9b3c,6,Info [FLog::UpdateController] Update mode is chosen as BACKGROUND. Telemetry sent";
        const string Required = "2026-10-30T04:53:44.691Z,0.691337,54a0,6,Info [FLog::UpdateController] Update check thread: updateRequired TRUE";
        const string GameStart = "2026-10-30T04:53:46.342Z,2.342439,956c,6 [FLog::SingleSurfaceApp] launchUGCGameInternal";
        void Follow(int pid, params string[] lines)
        {
            var tail = Activator.CreateInstance(tailType, "stand-in.log", pid)!;
            applyText.Invoke(null, [tail, string.Join("\n", lines) + "\n"]);
            tails[pid] = tail;
        }
        bool MayHaveHanded(int pid) => (bool)Harness.Invoke(app, "MayHaveHandedToUpdater", pid)!;
        Follow(5001, Required, UpdateStart);
        Follow(5002, Required, Background, UpdateStart, GameStart);
        Follow(5003, GameStart);
        c.Check("a client that started Roblox's updater in the foreground handed its launch to it", watcher.UpdateHandOffAt(5001) is not null && MayHaveHanded(5001));
        c.Check("…one updating in the background didn't: it went on into its game", watcher.UpdateHandOffAt(5002) is null && !MayHaveHanded(5002)
            && watcher.StateOf(5002) is { Status: "joining" });
        c.Check("…nor one that started no update", watcher.UpdateHandOffAt(5003) is null && !MayHaveHanded(5003));
        c.Check("…one whose log isn't found yet (it quit within a second or two) may have", MayHaveHanded(5004));
        tails.Clear();

        // Roblox's notification-area copy held the name Multi-Roblox takes: once it is ended with no Roblox left, the name is
        // taken before the launch's client can; with a Roblox still running it can't be.
        var vault = Harness.Get<Vault>(app, "_vault");
        vault.Data.Settings.MultiInstance = true;
        bool wasArmed = MultiInstance.Armed;
        MultiInstance.Disable();
        try
        {
            Harness.Invoke(app, "ArmMultiIfFree", new[] { 4242 });
            c.Check("a Roblox still running: Multi-Roblox not taken", !MultiInstance.Armed);
            Harness.Invoke(app, "ArmMultiIfFree", Array.Empty<int>());
            // A real Roblox on this PC holds the name: nothing to check then.
            if (RobloxProcesses.GetPlayerPids().Length == 0)
                c.Check("…none left: taken at once", MultiInstance.Armed);
        }
        finally
        {
            MultiInstance.Disable();
            vault.Data.Settings.MultiInstance = false;
            if (wasArmed)
                MultiInstance.Enable();
        }

        // Opened again after Close all: the earlier launch still awaited late leaves the launches to close as well (it
        // stayed there for the rest of the run); one of the account's still opening, to be closed, stays.
        var closeWhen = Harness.Get<object>(app, "_closeWhenClaimed");
        bool In(object set, object item) => (bool)set.GetType().GetMethod("Contains")!.Invoke(set, [item])!;
        void Add(object set, object item) => set.GetType().GetMethod("Add")!.Invoke(set, [item]);
        var lateOne = Launch(a, 150, First);
        var closing = Launch(a, 5, First);
        Add(late, lateOne);
        Add(closeWhen, lateOne);
        pending.Add(closing);
        Add(closeWhen, closing);
        Harness.Invoke(app, "ForgetLate", a.Id);
        c.Check("opened again: the earlier late launch is forgotten, from the launches to close too", !In(late, lateOne) && !In(closeWhen, lateOne));
        c.Check("…one still opening, to be closed, isn't", In(closeWhen, closing));
        closeWhen.GetType().GetMethod("Clear")!.Invoke(closeWhen, null);
        Reset();
        // Roblox's updater read from Windows' process list (a stand-in under its name: ping, copied).
        string updaterDir = Path.Combine(dir, "updater");
        Directory.CreateDirectory(updaterDir);
        string updaterExe = Path.Combine(updaterDir, "RobloxPlayerInstaller.exe");
        File.Copy(Path.Combine(Environment.SystemDirectory, "PING.EXE"), updaterExe, true);
        var before = DateTime.UtcNow.AddSeconds(-2);
        using var updater = Process.Start(new ProcessStartInfo(updaterExe, "-n 30 127.0.0.1") { CreateNoWindow = true, UseShellExecute = false })!;
        try
        {
            c.Check("Roblox's updater running, started after the launch: seen", RobloxProcesses.UpdaterRunningSince(before));
            c.Check("…one started before the launch isn't this launch's", !RobloxProcesses.UpdaterRunningSince(DateTime.UtcNow.AddMinutes(1)));
        }
        finally
        {
            try { updater.Kill(); } catch (InvalidOperationException) { }
            updater.WaitForExit(5000);
        }
        c.Check("…and once it has quit, none", !RobloxProcesses.UpdaterRunningSince(before));
    }
}
