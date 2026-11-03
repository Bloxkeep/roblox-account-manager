using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// A window opened outside Bloxkeep (Roblox's own app) that joined a game: the join report in its log names the signed-in
/// user, and Bloxkeep takes it as that account's window. Not a stranger's, not a second window for an account that has one,
/// not while a launch could still claim it. No real window is involved.
/// </summary>
static class AdoptSuite
{
    const long Place = 4242;

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        seed.Data.Accounts.Add(new Account { Username = "Main", UserId = 31, Cookie = "cookie-main", CookieValid = true });
        seed.Data.Accounts.Add(new Account { Username = "Alt", UserId = 32, Cookie = "cookie-alt", CookieValid = true });
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var accounts = Harness.Get<Vault>(app, "_vault").Data.Accounts;
        Guid main = accounts.Single(a => a.UserId == 31).Id, alt = accounts.Single(a => a.UserId == 32).Id;

        var t = typeof(AppController);
        var running = Harness.Get<System.Collections.IDictionary>(app, "_running");
        var watcher = Harness.Get<RobloxLogWatcher>(app, "_logs");
        var tails = Harness.Get<System.Collections.IDictionary>(watcher, "_tails");
        var tailType = typeof(RobloxLogWatcher).GetNestedType("Tail", Harness.F)!;
        var apply = typeof(RobloxLogWatcher).GetMethod("Apply", Harness.F)!;
        var adopt = t.GetMethod("AdoptWindowsOpenedElsewhere", Harness.F)!;
        object Window(int pid, string line)
        {
            var tail = Activator.CreateInstance(tailType, "stand-in.log", pid)!;
            tailType.GetField("State")!.SetValue(tail, new RobloxLogWatcher.ClientState("ingame", Place, "job", null, DateTime.UtcNow));
            apply.Invoke(null, [tail, line]);
            tails[pid] = tail;
            return tail;
        }
        static string Report(long user) => $"2026-11-02T22:26:48.946Z,1.5,1a2b,6 [FLog::GameJoinLoadTime] Report game_join_loadtime: placeid:{Place}, join_time:1.5, universeid:9, referral_page:Home, sid:b73ee3cd-d8e0-4fcc-850b-b461be75a8e1, clienttime:1.2, userid:{user},";
        bool Adopt(params int[] pids) => (bool)adopt.Invoke(app, [pids.Select(p => new RobloxProcesses.PlayerProcess(p, 0)).ToList()])!;
        int PidOf(Guid id) => running.Contains(id) ? (int)running[id]!.GetType().GetField("Item1")!.GetValue(running[id])! : 0;

        Window(900001, Report(31));
        c.Check("the join report gives the signed-in user", watcher.UserIdOf(900001) == 31, $"{watcher.UserIdOf(900001)}");
        Window(900002, "2026-11-02T22:26:40.000Z,1.0,1a2b,6 [FLog::Output] ! Joining game 'job' place 4242 at 1.2.3.4");
        c.Check("…and other lines give none", watcher.UserIdOf(900002) is null);

        Window(900003, Report(99));
        c.Check("a window opened elsewhere, signed in as a vault account, becomes its window",
            Adopt(900001, 900002, 900003) && PidOf(main) == 900001, $"{PidOf(main)}");
        c.Check("…a stranger's window and one not yet in a game are left alone", running.Count == 1);

        running.Remove(main);
        Window(900004, Report(31));
        Window(900005, Report(32));
        var pending = Harness.Get<System.Collections.IList>(app, "_pending");
        var launch = Activator.CreateInstance(t.GetNestedType("PendingLaunch", Harness.F)!, alt, new HashSet<int>(), DateTime.UtcNow, new GameTarget(Place), false, null)!;
        pending.Add(launch);
        c.Check("not while a launch could still claim the window", !Adopt(900004, 900005) && running.Count == 0);
        pending.Clear();

        c.Check("one window per account: the first one", Adopt(900004, 900001, 900005) && PidOf(main) == 900004 && PidOf(alt) == 900005,
            $"main {PidOf(main)} alt {PidOf(alt)}");
        c.Check("…and nothing again for windows already known", !Adopt(900004, 900005));

        // Signed out in the window: the user it named is gone with it, so another account signing in there isn't taken
        // for the first (audit B15).
        var signedOut = Window(900006, Report(31));
        apply.Invoke(null, [signedOut, "2026-11-02T22:30:00.000Z,1.0,1a2b,6 [FLog::Output] logout sync cookie to native"]);
        c.Check("a sign-out in the window forgets the signed-in user", watcher.UserIdOf(900006) is null, $"{watcher.UserIdOf(900006)}");

        running.Clear();
        // A Global and a Roblox VNG account can have the same user id (VNG counts its own): the log can't say which (audit B12).
        accounts.Add(new Account { Username = "Twin", UserId = 33, Cookie = "cookie-twin", CookieValid = true });
        accounts.Add(new Account { Username = "TwinVng", UserId = 33, Realm = Realm.Vng, Cookie = "cookie-twin-vng", CookieValid = true });
        Window(900007, Report(33));
        c.Check("a user id two accounts share (Global and VNG) adopts nothing", !Adopt(900007) && running.Count == 0, $"{running.Count}");

        // Being closed: Close all's sweep, or the monitor tab's Close of a window nobody tracked (audit R15).
        Harness.Set(app, "_sweepBackgroundUntil", DateTime.UtcNow.AddSeconds(15));
        c.Check("nothing is adopted during Close all's sweep", !Adopt(900004) && running.Count == 0);
        Harness.Set(app, "_sweepBackgroundUntil", DateTime.MinValue);
        Window(900009, Report(31));
        RobloxProcesses.Close(900009); // an odd number: never a real process id
        c.Check("…nor a window being closed", !Adopt(900009) && running.Count == 0);

        // A separate sign-in window (the tray's New Roblox window) isn't a vault account's client (audit R16).
        Harness.Set(app, "_openingHome", true);
        c.Check("nothing is adopted while a separate sign-in window opens", !Adopt(900004) && running.Count == 0);
        Harness.Set(app, "_openingHome", false);

        // Saved soon, not on the window's thread in the tick, and not maximised as a window Bloxkeep opened (audit R16).
        Harness.Set(app, "_saveDue", null);
        bool adopted = Adopt(900004);
        var maximised = Harness.Get<Dictionary<int, string>>(app, "_maximised");
        c.Check("an adopted window is saved with the next save, not at once", adopted && Harness.Get<DateTime?>(app, "_saveDue") is not null);
        c.Check("…and isn't maximised by Settings' Maximise", maximised.ContainsKey(900004), string.Join(",", maximised.Keys));

        // Its server is unknown: a private or reserved one would be rejoined as a public server of the place (audit R9).
        // Only a window whose own start asked for a public server of the place it is in keeps that game.
        var accountMain = accounts.Single(a => a.Id == main);
        accountMain.AutoRejoin = true;
        var queueRejoin = t.GetMethod("QueueRejoin", Harness.F)!;
        var rejoinQueue = Harness.Get<System.Collections.IList>(app, "_rejoinQueue");
        var dropped = new RobloxLogWatcher.ClientState("disconnected", Place, "job", 277, DateTime.UtcNow);
        object LaunchOf(Guid id) => running[id]!.GetType().GetField("Item2")!.GetValue(running[id])!;
        GameTarget? TargetOf(Guid id) => (GameTarget?)LaunchOf(id).GetType().GetProperty("Target")!.GetValue(LaunchOf(id));
        bool Drop(Guid id) => (bool)queueRejoin.Invoke(app, [id, LaunchOf(id), dropped])!;
        c.Check("a window with no sign of a public server gets no game to rejoin", TargetOf(main) is null, $"{TargetOf(main)}");
        c.Check("…so a drop isn't rejoined into a public server", !Drop(main) && rejoinQueue.Count == 0);
        const string Exe = "\"C:\\Users\\x\\AppData\\Local\\Roblox\\Versions\\version-1\\RobloxPlayerBeta.exe\" ";
        static string Launcher(string query) => "roblox-player:1+launchmode:play+gameinfo:abc+launchtime:1+placelauncherurl:"
            + Uri.EscapeDataString("https://www.roblox.com/Game/PlaceLauncher.ashx?" + query) + "+browsertrackerid:7+robloxLocale:en_us";
        string? commandLine = null;
        if (t.GetField("_commandLineOf", Harness.F) is { } seam)
            seam.SetValue(app, new Func<int, long, string?>((_, _) => commandLine));
        void AdoptWith(string? line)
        {
            running.Remove(main);
            commandLine = line;
            Adopt(900004);
        }
        AdoptWith(Exe + Launcher($"request=RequestGame&browserTrackerId=7&placeId={Place}&isPlayTogetherGame=false"));
        c.Check("a window started for a public server of its place keeps that game", TargetOf(main) is { PlaceId: Place, JobId: null, IsPrivate: false },
            $"{TargetOf(main)}");
        c.Check("…and is rejoined after a drop", Drop(main) && rejoinQueue.Count == 1);
        rejoinQueue.Clear();
        AdoptWith(Exe + Launcher($"request=RequestPrivateGame&browserTrackerId=7&placeId={Place}&accessCode={Guid.NewGuid()}"));
        c.Check("a private server's window gets none", TargetOf(main) is null, $"{TargetOf(main)}");
        AdoptWith(Exe + Launcher("request=RequestGame&browserTrackerId=7&placeId=77&isPlayTogetherGame=false"));
        c.Check("…nor one in another place than it started for", TargetOf(main) is null, $"{TargetOf(main)}");
        AdoptWith(Exe + "--app");
        c.Check("…nor one started on Roblox's home screen", TargetOf(main) is null, $"{TargetOf(main)}");
        accountMain.AutoRejoin = false;

        // A late launch's window is checked against the log's user before it is claimed (audit R11).
        var claimCheck = t.GetMethod("ClaimUserMatches", Harness.F);
        bool Matches(int pid, Guid id) => claimCheck is not null && (bool)claimCheck.Invoke(app, [pid, accounts.Single(a => a.Id == id)])!;
        c.Check("a window signed in as another user isn't the account's", claimCheck is not null && !Matches(900005, main));
        c.Check("…one signed in as the account is", Matches(900004, main));
        c.Check("…and one whose log hasn't named a user yet may be", Matches(900002, main));

        // Another vault's windows, still open after a switch, aren't written into this vault's open clients (audit R18).
        var stranger = Guid.NewGuid();
        running[stranger] = Activator.CreateInstance(running[main]!.GetType(), 900011, LaunchOf(main));
        Harness.Invoke(app, "NoteOpenSessions");
        var noted = Harness.Get<Vault>(app, "_vault").Data.OpenSessions;
        c.Check("another vault's open clients aren't noted in this one", noted.Count > 0 && noted.All(s => s.AccountId != stranger),
            string.Join(",", noted.Select(s => s.AccountId)));
        running.Remove(stranger);
    }
}
