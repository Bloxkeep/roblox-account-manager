using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The process check timed (AppController.NoteTickTime): one that held the window's thread 100 ms or more goes to
/// error.log with its slowest parts, named by the code; another in the same hour is only counted, unless it took twice
/// as long as the slowest written (then it is written too, with the count). A quick one writes nothing.
/// </summary>
static class TickTimeSuite
{
    static string LogText(string dir) => File.Exists(Path.Combine(dir, "error.log")) ? File.ReadAllText(Path.Combine(dir, "error.log")) : "";

    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        void Note(int ms, params (string What, int Ms)[] parts) => Harness.Invoke(app, "NoteTickTime", TimeSpan.FromMilliseconds(ms),
            parts.Select(p => (p.What, TimeSpan.FromMilliseconds(p.Ms))).ToList());

        Note(60, ("process list", 4), ("() => ApplyWindowIcons(pids, windows)", 50));
        c.Check("a quick check writes nothing", LogText(dir) == "", LogText(dir));

        Note(250, ("process list", 4), ("() => ApplyWindowIcons(pids, windows)", 200), ("Roblox logs", 30), ("window list", 2),
            ("() => TickDiscord(now, players)", 1));
        string first = LogText(dir);
        c.Check("a slow check is written with its time and its slowest parts, named by the code",
            first.Contains("Slow process check: 250 ms") && first.Contains("ApplyWindowIcons(pids, windows) 200 ms, Roblox logs 30 ms")
            && !first.Contains("() =>") && !first.Contains("TickDiscord") && first.Contains("the rest 13 ms"), first);

        Note(400, ("Roblox logs", 390));
        c.Check("another in the same hour, under twice as long, is only counted", LogText(dir) == first, LogText(dir));

        Note(500, ("() => PersistIfDue(now)", 480));
        string second = LogText(dir)[first.Length..];
        c.Check("one twice as long is written too, with how many came unwritten",
            second.Contains("Slow process check: 500 ms") && second.Contains("PersistIfDue(now) 480 ms") && second.Contains("1 slow check(s) came"), second);

        // What is installed (Roblox, its launchers): read again on a worker a little before the cached read runs out,
        // taken in at a later tick, never read on the window's thread by the tick while a read is cached (2026-10-29:
        // 0.9 s of one check on the owner's PC).
        Harness.Invoke(app, "Installs");
        var readAt = DateTime.UtcNow.AddMinutes(-3).AddSeconds(5); // 5 s before the cached read runs out
        Harness.Set(app, "_installsReadAt", readAt);
        Harness.Invoke(app, "TickRobloxUpdates", DateTime.UtcNow, 0);
        c.Check("the tick starts reading what is installed on a worker, the cached read still in use",
            Harness.Get<object>(app, "_installsReading") is Task && Harness.Get<DateTime>(app, "_installsReadAt") == readAt);
        var reading = (Task)Harness.Get<object>(app, "_installsReading");
        reading.Wait(TimeSpan.FromSeconds(30));
        Harness.Invoke(app, "TickRobloxUpdates", DateTime.UtcNow, 0);
        c.Check("…its read taken in at a later tick", Harness.Get<object?>(app, "_installsReading") is null
            && Harness.Get<DateTime>(app, "_installsReadAt") > readAt);
        // A read started before the cache was dropped (another vault, another launch option) is left unused.
        Harness.Set(app, "_installsReadAt", readAt);
        Harness.Invoke(app, "TickRobloxUpdates", DateTime.UtcNow, 0);
        var stale = (Task)Harness.Get<object>(app, "_installsReading");
        stale.Wait(TimeSpan.FromSeconds(30));
        var fresh = DateTime.UtcNow;
        Harness.Set(app, "_installsReadAt", fresh); // read again meanwhile
        Harness.Invoke(app, "TickRobloxUpdates", DateTime.UtcNow, 0);
        c.Check("…one started before another read is left unused", Harness.Get<DateTime>(app, "_installsReadAt") == fresh);
        return Task.CompletedTask;
    }
}
