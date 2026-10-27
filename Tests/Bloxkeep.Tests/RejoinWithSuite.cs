using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// A preset's "Rejoin server" (LaunchPreset.RejoinWith): saved only with a game, as "any" or one of its ticked accounts;
/// a dropped window rejoins the server its chosen account (or any other of the preset's) plays in, as that window's log
/// says, passing over a server Roblox calls full; nobody in a game, or no server that will take it: any server, as
/// without the option. Windows and logs are stand-ins; nothing is launched.
/// </summary>
static class RejoinWithSuite
{
    const long Place = 4242;

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        var a1 = new Account { Username = "farm1", Cookie = "x1", UserId = 1001, CookieValid = true };
        var a2 = new Account { Username = "farm2", Cookie = "x2", UserId = 1002, CookieValid = true };
        var a3 = new Account { Username = "farm3", Cookie = "x3", UserId = 1003, CookieValid = true };
        var other = new Account { Username = "main", Cookie = "x4", UserId = 1004, CookieValid = true };
        seed.Data.Accounts.AddRange([a1, a2, a3, other]);
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var vault = Harness.Get<Vault>(app, "_vault");
        // The game's name isn't asked from Roblox here.
        Harness.Get<System.Collections.IDictionary>(app, "_games")[Place] = null;

        async Task<LaunchPreset> Save(object rejoinWith, string place = "4242", string server = "any")
        {
            var id = vault.Data.Settings.Presets.FirstOrDefault()?.Id;
            await Harness.Call(app, "savePreset", new { id = id?.ToString() ?? "", name = "Farm", ids = new[] { a1.Id, a2.Id, a3.Id }, placeInput = place, server, rejoinWith });
            return vault.Data.Settings.Presets.Single();
        }
        var preset = await Save(a2.Id.ToString());
        c.Check("a ticked account is saved as the one to rejoin with", preset.RejoinWith == a2.Id.ToString(), preset.RejoinWith);
        c.Check("…and comes back with the preset", Harness.Invoke(app, "PresetDto", preset)!.GetType().GetProperty("RejoinWith")!.GetValue(Harness.Invoke(app, "PresetDto", preset)) as string == a2.Id.ToString());
        preset = await Save(other.Id.ToString());
        c.Check("an account the preset doesn't open: any server", preset.RejoinWith == "", preset.RejoinWith);
        preset = await Save("any", place: "");
        c.Check("no game: any server", preset.RejoinWith == "", preset.RejoinWith);
        preset = await Save("nonsense");
        c.Check("anything else: any server", preset.RejoinWith == "", preset.RejoinWith);
        preset = await Save(a2.Id.ToString());

        // Windows of the preset's accounts, each with its log's state.
        var t = typeof(AppController);
        var running = Harness.Get<System.Collections.IDictionary>(app, "_running");
        var launchType = t.GetNestedType("PendingLaunch", Harness.F)!;
        var tupleType = running.GetType().GetGenericArguments()[1];
        var watcher = Harness.Get<RobloxLogWatcher>(app, "_logs");
        var tails = Harness.Get<System.Collections.IDictionary>(watcher, "_tails");
        var tailType = typeof(RobloxLogWatcher).GetNestedType("Tail", Harness.F)!;
        void Window(Account account, int pid, string status, string? job, int minutesAgo)
        {
            var launch = Activator.CreateInstance(launchType, account.Id, new HashSet<int>(), DateTime.UtcNow.AddMinutes(-minutesAgo), new GameTarget(Place), false, preset.Id)!;
            running[account.Id] = Activator.CreateInstance(tupleType, pid, launch);
            var tail = Activator.CreateInstance(tailType, $"stand-in-{pid}.log", pid)!;
            tailType.GetField("State")!.SetValue(tail, new RobloxLogWatcher.ClientState(status, Place, job, null, DateTime.UtcNow));
            tails[pid] = tail;
        }
        Window(a2, 902, "ingame", "server-of-farm2", 5);
        Window(a3, 903, "ingame", "server-of-farm3", 9);

        var full = new HashSet<string>();
        var asked = new List<string?>();
        Harness.Set(app, "_askJoin", (Func<Account, GameTarget, CancellationToken, Task<RobloxApi.JoinAnswer>>)((_, target, _) =>
        {
            asked.Add(target.JobId);
            return Task.FromResult(new RobloxApi.JoinAnswer(200, full.Contains(target.JobId!) ? 6 : 2, null));
        }));
        var requestType = t.GetNestedType("RejoinRequest", Harness.F)!;
        async Task<GameTarget?> Rejoin(Account account)
        {
            asked.Clear();
            var request = Activator.CreateInstance(requestType, account.Id, new GameTarget(Place), DateTime.UtcNow, 0, preset.Id, false)!;
            return await (Task<GameTarget?>)Harness.Invoke(app, "TeammateTargetAsync", account, request, CancellationToken.None)!;
        }

        var target = await Rejoin(a1);
        c.Check("dropped: rejoins the server its chosen account plays in", target is { PlaceId: Place, JobId: "server-of-farm2" }, target);
        full.Add("server-of-farm2");
        target = await Rejoin(a1);
        c.Check("…that server full: any server (not another account's when one is chosen)", target is null && asked.SequenceEqual(["server-of-farm2"]), string.Join(",", asked));
        target = await Rejoin(a2);
        c.Check("the chosen account itself dropping: any server", target is null, target);

        preset = await Save("any");
        full.Clear();
        full.Add("server-of-farm3");
        target = await Rejoin(a1);
        c.Check("with another account: the earliest opened first, a full server passed over for the next one", target is { JobId: "server-of-farm2" }
            && asked.SequenceEqual(["server-of-farm3", "server-of-farm2"]), string.Join(",", asked));
        full.Clear();
        full.Add("server-of-farm2");
        Window(a3, 903, "joining", null, 9);
        target = await Rejoin(a1);
        c.Check("…nobody else in a game (one full, one still joining): any server", target is null, target);
        full.Clear();
        target = await Rejoin(a1);
        c.Check("…the full one has room again: rejoins it", target is { JobId: "server-of-farm2" }, target);

        // A window of the same account opened by hand (no preset) doesn't count as the preset's.
        running.Remove(a2.Id);
        Window(other, 904, "ingame", "server-of-main", 3);
        var launch = Activator.CreateInstance(launchType, other.Id, new HashSet<int>(), DateTime.UtcNow, new GameTarget(Place), false, null)!;
        running[other.Id] = Activator.CreateInstance(tupleType, 904, launch);
        target = await Rejoin(a1);
        c.Check("another account's window not opened by the preset: not followed", target is null, target);
    }
}
