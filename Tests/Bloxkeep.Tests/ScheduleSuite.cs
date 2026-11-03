using System.Text.Json;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>The schedule engine on a walked clock (<see cref="Clock"/>): a minute runs once, a clock set back or forward
/// doesn't run twice, a gap and a locked minute are each said once, a one-off that went by is turned off, and the DST
/// fall-back hour doesn't run a preset twice while the spring-forward hour reports the minute it skipped.
/// Each preset has no accounts, so a run that fires ends at once in "didn't run: no accounts left": nothing opens Roblox.</summary>
static class ScheduleSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var now = new DateTime(2026, 11, 2, 7, 59, 30, DateTimeKind.Local); // a Monday
        long tick = 1_000_000;
        Clock.Now = () => now;
        Clock.UtcNow = () => now.ToUniversalTime();
        Clock.Tick = () => tick;

        var every = new List<int> { 0, 1, 2, 3, 4, 5, 6 };
        LaunchPreset Daily(string name, string time) =>
            new() { Name = name, Schedule = new PresetSchedule { Enabled = false, Time = time, Days = [.. every] } };
        var seed = Vault.Create(Harness.Password);
        seed.Data.Settings.Presets.AddRange([
            Daily("Daily", "08:00"),
            new LaunchPreset { Name = "Once", Schedule = new PresetSchedule { Enabled = true, Time = "09:00", Date = "2026-11-01" } },
            Daily("Gap", "10:00"),
            Daily("Locked", "12:00"),
            Daily("Fall", "01:30"),
            Daily("Spring", "02:30"),
        ]);
        seed.Save();

        using var form = new Form();
        var toasts = new List<string>();
        using var app = new AppController(form, json =>
        {
            using var doc = JsonDocument.Parse(json);
            if (doc.RootElement.TryGetProperty("name", out var name) && name.GetString() == "toast")
                lock (toasts)
                    toasts.Add(doc.RootElement.GetProperty("data").GetProperty("text").GetString() ?? "");
        });
        await Harness.Unlock(app);
        var vault = Harness.Get<Vault>(app, "_vault");
        LaunchPreset P(string name) => vault.Data.Settings.Presets.First(p => p.Name == name);
        int Count(string text) { lock (toasts) return toasts.Count(t => t.Contains(text)); }
        int Runs(string name) => Count($"Scheduled preset {name} didn't run");
        async Task At(DateTime local, long ms)
        {
            now = local;
            tick += ms;
            Harness.Invoke(app, "CheckSchedules");
            await Task.Delay(150); // the run itself is async: it ends at once in "no accounts left"
        }
        DateTime T(int day, int h, int m, int s = 0) => new(2026, 11, day, h, m, s, DateTimeKind.Local);

        // A one-off whose date went by while Bloxkeep was closed: turned off, never run.
        P("Daily").Schedule!.Enabled = true;
        await At(T(2, 7, 59, 30), 0);
        c.Check("a one-off that went by while closed is turned off, not run", P("Once").Schedule!.Enabled == false && Runs("Once") == 0);

        // The minute comes: one run, however many ticks land in it.
        await At(T(2, 8, 0, 5), 35_000);
        await At(T(2, 8, 0, 50), 45_000);
        c.Check("its minute runs the preset once, however many ticks land in it", Runs("Daily") == 1 && P("Daily").Schedule!.LastRun == "2026-11-02 08:00",
            $"runs {Runs("Daily")}, last {P("Daily").Schedule!.LastRun}");

        // The clock set back into the minute that ran: not again.
        await At(T(2, 8, 1, 10), 20_000);
        await At(T(2, 8, 0, 20), 10_000);
        c.Check("the clock set back into the minute that ran: not run again", Runs("Daily") == 1, $"runs {Runs("Daily")}");

        // Next day's minute runs; then the clock set back a day (uptime says a minute went by): not again.
        await At(T(3, 8, 0, 10), 86_400_000);
        c.Check("the next day's minute runs", Runs("Daily") == 2, $"runs {Runs("Daily")}");
        await At(T(2, 8, 0, 30), 60_000);
        c.Check("the clock set back a day right after a run: not run again", Runs("Daily") == 2, $"runs {Runs("Daily")}");

        // The clock jumps forward past a run (PC asleep, or set forward): said once as missed, not run late.
        await At(T(3, 9, 59), 60_000);
        P("Gap").Schedule!.Enabled = true;
        await At(T(3, 10, 30), 60_000);
        await At(T(3, 10, 31), 60_000);
        c.Check("a minute skipped by a jump forward is said once as missed, not run", Count("Gap didn't run at 10:00: the PC was asleep or busy.") == 1 && Runs("Gap") == 0,
            $"missed {Count("Gap didn't run at 10:00")}, runs {Runs("Gap")}");
        P("Gap").Schedule!.Enabled = false;

        // Locked through its minute: said once that minute is over, not while it is still on.
        P("Locked").Schedule!.Enabled = true;
        await At(T(3, 11, 59, 30), 60_000);
        Harness.Invoke(app, "RememberSchedulesForLock");
        Harness.Set(app, "_vault", null);
        await At(T(3, 12, 0, 10), 40_000);
        int during = Count("Locked didn't run at 12:00: Bloxkeep was locked.");
        await At(T(3, 12, 1, 5), 55_000);
        await At(T(3, 12, 2, 5), 60_000);
        c.Check("locked through its minute: said once, after the minute (an unlock within it could still run it)",
            during == 0 && Count("Locked didn't run at 12:00: Bloxkeep was locked.") == 1 && Runs("Locked") == 0,
            $"during {during}, after {Count("Locked didn't run at 12:00")}");
        Harness.Set(app, "_vault", vault);
        await At(T(3, 12, 3, 5), 60_000);
        P("Locked").Schedule!.Enabled = false;

        // DST fall-back: 01:00–02:00 comes twice. The preset runs in the first 01:30 only.
        await At(T(4, 1, 29, 50), 13 * 3_600_000);
        P("Fall").Schedule!.Enabled = true;
        await At(T(4, 1, 30, 10), 20_000);
        await At(T(4, 1, 59, 30), 29 * 60_000);
        await At(T(4, 1, 0, 30), 60_000); // clocks go back an hour
        for (int m = 1; m <= 31; m++)
            await At(T(4, 1, m, 30), 60_000);
        c.Check("DST fall-back: the repeated 01:30 doesn't run the preset twice", Runs("Fall") == 1 && Count("Fall didn't run at") == 0,
            $"runs {Runs("Fall")}, missed {Count("Fall didn't run at")}");
        P("Fall").Schedule!.Enabled = false;

        // DST spring-forward: 02:00–03:00 never comes. A preset set inside it is said as missed, once.
        await At(T(5, 1, 59), 24 * 3_600_000);
        P("Spring").Schedule!.Enabled = true;
        await At(T(5, 3, 0, 5), 65_000);
        await At(T(5, 3, 1, 5), 60_000);
        c.Check("DST spring-forward: the skipped 02:30 is said once as missed", Count("Spring didn't run at 02:30: the PC was asleep or busy.") == 1 && Runs("Spring") == 0,
            $"missed {Count("Spring didn't run at 02:30")}, runs {Runs("Spring")}");
    }
}
