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
    }
}
