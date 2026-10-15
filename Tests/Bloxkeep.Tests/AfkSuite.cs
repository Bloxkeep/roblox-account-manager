using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Anti-AFK's decisions with made-up process ids, window handles, idle times, lock and full-screen states: which windows
/// are nudged, why the others wait, retries, and windows already open when Bloxkeep starts. The nudge itself never runs
/// (_nudging set, AntiAfkDue called directly), so nothing on the desktop is touched.
/// </summary>
static class AfkSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        seed.Data.Settings.AntiAfk = true;
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        Harness.Set(app, "_nudging", true);
        var t = typeof(AppController);
        var run = t.GetMethod("RunAntiAfk", Harness.F)!;
        var plan = t.GetMethod("AntiAfkDue", Harness.F)!;
        var retryAt = t.GetMethod("NudgeRetryAt", Harness.F)!;
        var lastInput = Harness.Get<Dictionary<int, DateTime>>(app, "_lastInput");
        var retry = Harness.Get<Dictionary<int, DateTime>>(app, "_nudgeRetryAt");
        var held = Harness.Get<System.Collections.IDictionary>(app, "_afkHeld");
        var afk = typeof(Vault).Assembly.GetType("RobloxManager.AntiAfk")!;
        var lastChance = (TimeSpan)afk.GetField("LastChance", Harness.F)!.GetValue(null)!;

        // Windows already open when Bloxkeep starts (an update's restart).
        var now = DateTime.UtcNow;
        run.Invoke(app, [now, new Dictionary<int, List<IntPtr>> { [990001] = [1], [990002] = [2] }]);
        c.Check("open at start: due at once, even for a nudge while the PC is in use", lastInput.TryGetValue(990001, out var a) && a == now - lastChance, a);
        c.Check("both windows open at start", lastInput.TryGetValue(990002, out var b) && b == now - lastChance, b);
        var later = now.AddSeconds(30);
        run.Invoke(app, [later, new Dictionary<int, List<IntPtr>> { [990001] = [1], [990002] = [2], [990003] = [3] }]);
        c.Check("opened later: counts from now", lastInput.TryGetValue(990003, out var cc) && cc == later, cc);
        c.Check("start windows keep their time", lastInput[990001] == now - lastChance);
        run.Invoke(app, [later.AddSeconds(1), new Dictionary<int, List<IntPtr>> { [990003] = [3] }]);
        c.Check("closed windows forgotten", !lastInput.ContainsKey(990001) && lastInput.ContainsKey(990003));

        // The decision. Windows idle 5 min (A), 13 min (B), 19 min (C); D is a Roblox window that may be in front.
        const int A = 1001, B = 1002, C = 1003, D = 1004, Other = 4242;
        var wins = new Dictionary<int, List<IntPtr>> { [A] = [11], [B] = [12], [C] = [13], [D] = [14] };
        var on = new HashSet<int> { A, B, C, D };
        bool fullAsked = false;
        List<int> Decide(TimeSpan idle, bool locked = false, int foreground = Other, bool fullScreen = false, HashSet<int>? only = null)
        {
            var at = DateTime.UtcNow;
            lastInput.Clear(); retry.Clear(); held.Clear();
            lastInput[A] = at.AddMinutes(-5); lastInput[B] = at.AddMinutes(-13); lastInput[C] = at.AddMinutes(-19); lastInput[D] = at.AddMinutes(-1);
            fullAsked = false;
            return ((List<int>)plan.Invoke(app, [at, only ?? on, wins, locked, idle, foreground, (Func<bool>)(() => { fullAsked = true; return fullScreen; })])!).Order().ToList();
        }
        string Held(int pid) => held.Contains(pid) ? held[pid]!.ToString()! : "-";

        var r = Decide(TimeSpan.FromSeconds(90));
        c.Check("away: windows idle 12+ min nudged, the 5-min one not", r.SequenceEqual([B, C]), string.Join(",", r));
        r = Decide(TimeSpan.FromSeconds(20));
        c.Check("in use, 20 s since input: the window about to be kicked goes, the 13-min one waits", r.SequenceEqual([C]), string.Join(",", r));
        c.Check("in use: due windows noted as held (PC in use)", Held(B) == "InUse" && Held(C) == "InUse" && Held(A) == "-", $"{Held(A)} {Held(B)} {Held(C)}");
        r = Decide(TimeSpan.FromSeconds(20), only: [A, B]);
        c.Check("in use, nothing near its kick: nothing nudged", r.Count == 0, string.Join(",", r));
        c.Check("no last-chance window: full-screen state not asked", !fullAsked);
        r = Decide(TimeSpan.FromSeconds(1));
        c.Check("in use, 1 s pause: last-chance window waits", r.Count == 0, string.Join(",", r));
        r = Decide(TimeSpan.FromSeconds(3));
        c.Check("in use, 3 s pause: only the window about to be kicked", r.SequenceEqual([C]), string.Join(",", r));
        c.Check("full-screen state asked only when it matters", fullAsked);
        r = Decide(TimeSpan.FromSeconds(3), fullScreen: true);
        c.Check("full-screen app in front: not interrupted", r.Count == 0 && Held(C) == "FullScreen", $"{string.Join(",", r)} {Held(C)}");
        r = Decide(TimeSpan.FromSeconds(3), foreground: D, fullScreen: true);
        c.Check("a Roblox window in front (even full screen): the other one about to be kicked goes", r.SequenceEqual([C]) && !fullAsked, $"{string.Join(",", r)} asked={fullAsked}");
        c.Check("the Roblox window in front counts as played in", (DateTime.UtcNow - lastInput[D]).TotalSeconds < 5, lastInput[D]);
        r = Decide(TimeSpan.FromMinutes(5), locked: true);
        c.Check("locked: nothing tried, noted as locked", r.Count == 0 && Held(B) == "Locked" && Held(C) == "Locked", $"{string.Join(",", r)} {Held(B)} {Held(C)}");

        // A refused window waits before its next try, but not past its last chance.
        var at0 = DateTime.UtcNow;
        double In(int misses, double idleMin) => ((DateTime)retryAt.Invoke(null, [at0, misses, at0.AddMinutes(-idleMin)])! - at0).TotalMinutes;
        c.Check("miss 1 at 13 min: next try in 1 min", Math.Abs(In(1, 13) - 1) < .01, In(1, 13));
        c.Check("miss 4 at 13 min: not after the last chance (18 min)", Math.Abs(In(4, 13) - 5) < .01, In(4, 13));
        c.Check("past the last chance: tried again within a minute", Math.Abs(In(9, 19) - 1) < .01, In(9, 19));
        c.Check("long run of misses early on: 15 min at most", Math.Abs(In(5, 2) - 15) < .01, In(5, 2));

        // A last-chance window keeps its wait while the PC is in use; the others' waits are cleared.
        lastInput.Clear(); retry.Clear(); held.Clear();
        var t1 = DateTime.UtcNow;
        lastInput[B] = t1.AddMinutes(-13); lastInput[C] = t1.AddMinutes(-19);
        retry[B] = t1.AddMinutes(3); retry[C] = t1.AddSeconds(40);
        var rw = (List<int>)plan.Invoke(app, [t1, new HashSet<int> { B, C }, wins, false, TimeSpan.FromSeconds(3), Other, (Func<bool>)(() => false)])!;
        c.Check("last-chance window still waiting: not tried yet", rw.Count == 0, string.Join(",", rw));
        c.Check("in use clears the other windows' waits only", !retry.ContainsKey(B) && retry.ContainsKey(C), string.Join(",", retry.Keys));
    }
}
