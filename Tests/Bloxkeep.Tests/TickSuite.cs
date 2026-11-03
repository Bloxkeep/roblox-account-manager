using System.Reflection;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The process check (AppController.TrackProcesses) as a whole: one of its jobs that throws is logged and the rest still
/// run, above all the save and auto-rejoin (audit R1). The check runs as in the tray with nothing of Roblox's found, so it
/// reads no process list and touches no window; the Roblox logs are a folder of the test's own.
/// </summary>
static class TickSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        string logs = Path.Combine(dir, "roblox-logs");
        Directory.CreateDirectory(logs);
        Environment.SetEnvironmentVariable("BLOXKEEP_ROBLOX_LOGS", logs);
        Prefs.CountOnline = false;
        Prefs.CheckUpdates = false;
        Vault.Create(Harness.Password).Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);

        var maximised = Harness.Get<object>(app, "_maximised");
        (Exception? Thrown, List<string> Ran) Tick()
        {
            // In the tray, nothing found at the last read moments ago: no process list is read (ScanDue).
            Harness.Set(app, "_background", true);
            Harness.Set(app, "_lastScanFoundNone", true);
            Harness.Set(app, "_lastScan", DateTime.UtcNow);
            Harness.Set(app, "_saveDue", DateTime.UtcNow.AddSeconds(-1));
            var parts = new List<(string What, TimeSpan Took)>();
            Harness.Set(app, "_tickParts", parts);
            Exception? thrown = null;
            try
            {
                Harness.Invoke(app, "TrackProcesses");
            }
            catch (TargetInvocationException e)
            {
                thrown = e.InnerException;
            }
            Harness.Set(app, "_tickParts", null);
            return (thrown, parts.Select(p => p.What).ToList());
        }

        // Bad data in one job of the first half (here: what Settings' Maximise keeps, gone).
        Harness.Set(app, "_maximised", null);
        var (thrown, ran) = Tick();
        c.Check("a job that throws doesn't end the process check", thrown is null, thrown);
        c.Check("…the save still runs in the same check", ran.Contains("() => PersistIfDue(now)") && Harness.Get<DateTime?>(app, "_saveDue") is null,
            string.Join(" | ", ran));
        c.Check("…and so does auto-rejoin", ran.Contains("() => RunDueRejoin(now)"), string.Join(" | ", ran));
        c.Check("…with the jobs after the one that threw", ran.Contains("() => TickMacros(now)") && ran.Contains("() => TickPanels()"),
            string.Join(" | ", ran));
        string log = File.Exists(Path.Combine(dir, "error.log")) ? File.ReadAllText(Path.Combine(dir, "error.log")) : "";
        c.Check("…and the error is logged", log.Contains(nameof(NullReferenceException)), log);

        Harness.Set(app, "_maximised", maximised);
        (thrown, ran) = Tick();
        c.Check("the next check runs whole", thrown is null && ran.Contains("() => RunDueRejoin(now)"), thrown);
    }
}
