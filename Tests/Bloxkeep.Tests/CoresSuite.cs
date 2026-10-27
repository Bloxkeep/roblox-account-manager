using System.Diagnostics;
using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The fast cores for the window played (CpuCores, AppController.Cores.cs), on a stand-in process (ping.exe), no
/// Roblox: a window in the background goes into Windows' efficiency mode (power saving, below-normal priority) on every
/// core, never pinned to the efficiency cores (pinned, they filled up while the fast ones idled and windows starved,
/// 2026-10-21); the one in front gets full speed at normal priority, and turning it off (or Bloxkeep closing) gives
/// Windows its own choice back.
/// </summary>
static class CoresSuite
{
    [StructLayout(LayoutKind.Sequential)]
    struct PowerThrottling { public uint Version, ControlMask, StateMask; }
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetProcessInformation(IntPtr process, int infoClass, ref PowerThrottling info, int size);

    static readonly Type Cores = typeof(Prefs).Assembly.GetType("RobloxManager.CpuCores")!;
    static object Role(string name) => Enum.Parse(Cores.GetNestedType("Role")!, name);
    static bool Set(int pid, string role) => (bool)Cores.GetMethod("Set", Harness.F)!.Invoke(null, [pid, Role(role)])!;
    static ulong Efficient => (ulong)Cores.GetProperty("EfficientMask", Harness.F)!.GetValue(null)!;

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetProcessInformation(IntPtr process, int infoClass, ref uint info, int size);

    /// <summary>The process's RAM priority: 5 normal, 2 low.</summary>
    static uint MemoryPriority(Process p)
    {
        uint priority = 0;
        GetProcessInformation(p.Handle, 0, ref priority, sizeof(uint));
        return priority;
    }

    static (ulong Affinity, uint Control, uint State, ProcessPriorityClass Priority) Read(Process p)
    {
        p.Refresh();
        var t = new PowerThrottling { Version = 1 };
        GetProcessInformation(p.Handle, 4, ref t, Marshal.SizeOf<PowerThrottling>());
        return ((ulong)(long)p.ProcessorAffinity, t.ControlMask, t.StateMask, p.PriorityClass);
    }

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        ulong all = (1UL << Environment.ProcessorCount) - 1;
        ulong eff = Efficient;
        bool hybrid = eff != 0;
        Console.WriteLine($"  (this CPU: {(hybrid ? $"two kinds of cores, efficiency mask 0x{eff:X}" : "one kind of core")})");
        if (hybrid)
            c.Check("the efficiency cores are some of the cores, not all", eff != all && (eff & all) == eff, $"0x{eff:X} of 0x{all:X}");

        using var stand = Process.Start(new ProcessStartInfo("ping.exe", "-t 127.0.0.1") { CreateNoWindow = true, UseShellExecute = false })!;
        try
        {
            c.Check("a process in the background can be moved", Set(stand.Id, "Background"));
            var bg = Read(stand);
            c.Check("…not pinned to the efficiency cores: every core, so a fast one that is free still serves it", bg.Affinity == all, $"0x{bg.Affinity:X}");
            c.Check("…in power-saving mode", bg.Control == 1 && bg.State == 1, $"control {bg.Control} state {bg.State}");
            c.Check("…at below-normal priority (it gives way to the one in front)", bg.Priority == ProcessPriorityClass.BelowNormal, bg.Priority);
            // A window pinned by an older Bloxkeep (efficiency cores only) is unpinned.
            if (hybrid)
            {
                stand.ProcessorAffinity = (IntPtr)(long)eff;
                Set(stand.Id, "Background");
                c.Check("…and one an older Bloxkeep pinned to the efficiency cores is unpinned", Read(stand).Affinity == all, $"0x{Read(stand).Affinity:X}");
            }

            Set(stand.Id, "Front");
            var front = Read(stand);
            c.Check("in front: every core again", front.Affinity == all, $"0x{front.Affinity:X}");
            c.Check("…at full speed (power saving explicitly off), normal priority", front.Control == 1 && front.State == 0 && front.Priority == ProcessPriorityClass.Normal,
                $"control {front.Control} state {front.State} {front.Priority}");

            Set(stand.Id, "Default");
            var back = Read(stand);
            c.Check("given back: every core, Windows deciding the speed, normal priority", back.Affinity == all && back.Control == 0 && back.State == 0 && back.Priority == ProcessPriorityClass.Normal,
                $"0x{back.Affinity:X} control {back.Control} {back.Priority}");

            // Through the app: the setting on puts a client that isn't in front in the background; off gives it back.
            var seed = Vault.Create(Harness.Password);
            seed.Data.Settings.SplitCores = true;
            seed.Save();
            seed.Dispose();
            var posted = new List<string>();
            var app = new AppController(new Form(), json => { lock (posted) posted.Add(json); });
            await Harness.Unlock(app);
            void Split() => typeof(AppController).GetMethod("SplitCores", Harness.F)!.Invoke(app, [new List<int> { stand.Id }]);
            Split();
            var on = Read(stand);
            c.Check("the setting on: a client not in front goes into efficiency mode", on.State == 1 && on.Priority == ProcessPriorityClass.BelowNormal && on.Affinity == all,
                $"0x{on.Affinity:X} state {on.State} {on.Priority}");
            await Harness.Call(app, "setSettings", new { splitCores = false });
            var off = Read(stand);
            c.Check("turned off: Windows' own choice at once", off.Affinity == all && off.Control == 0 && off.Priority == ProcessPriorityClass.Normal, $"0x{off.Affinity:X} control {off.Control} {off.Priority}");
            await Harness.Call(app, "setSettings", new { splitCores = true });
            c.Check("on again: back to the background cores", Read(stand).State == 1);

            // Another window brought to the front by the person: this one, in front before, goes to the background at once
            // (grabbing a window in the background to drag it was slow for its first 0.7 s, 2026-10-21). One anti-AFK
            // brings forward for a moment: only a moment later.
            var foreground = (Delegate)typeof(AppController).GetField("_foregroundProc", Harness.F)!.GetValue(app)!;
            void BroughtForward() => foreground.DynamicInvoke(IntPtr.Zero, 3u, IntPtr.Zero, 0, 0, 0u, 0u);
            var coreRoles = Harness.Get<System.Collections.IDictionary>(app, "_coreRoles");
            Set(stand.Id, "Front");
            coreRoles[stand.Id] = Role("Front");
            Harness.Set(app, "_nudging", true);
            BroughtForward();
            c.Check("a window anti-AFK brings forward for a moment: the cores follow only a moment later", Read(stand).State == 0);
            Harness.Set(app, "_nudging", false);
            BroughtForward();
            c.Check("another window brought forward by the person: this one goes to the background at once", Read(stand).State == 1);

            // A client in front that ended since the last scan (seen 2026-10-19: Bloxkeep closed a window to rejoin,
            // and another came to the front): it has nothing to move, and Windows refused nothing. Both ways it can
            // be gone: exited while a handle still holds it (Bloxkeep's close keeps one), and no longer there at all.
            var roles = Harness.Get<System.Collections.IDictionary>(app, "_coreRoles");
            foreach (bool held in new[] { true, false })
            {
                var gone = Process.Start(new ProcessStartInfo("ping.exe", "-n 30 127.0.0.1") { CreateNoWindow = true, UseShellExecute = false })!;
                int pid = gone.Id;
                gone.Kill();
                gone.WaitForExit();
                if (!held)
                    gone.Dispose();
                roles[pid] = Role("Front");
                typeof(AppController).GetField("_coresRefusedShown", Harness.F)!.SetValue(app, false); // said once per run otherwise
                lock (posted)
                    posted.Clear();
                typeof(AppController).GetMethod("SplitCores", Harness.F)!.Invoke(app, [new List<int> { stand.Id, pid }]);
                bool told;
                lock (posted)
                    told = posted.Any(json => json.Contains("refused to move", StringComparison.Ordinal));
                c.Check(held ? "a client that has exited (a handle still on it): no \"Windows refused\" notice" : "…nor one that is gone altogether", !told);
                if (held)
                    gone.Dispose();
            }
            // The same for the background windows' memory (Settings' trim): one that closed since the scan isn't
            // "Windows didn't let Bloxkeep lower a Roblox window's memory priority".
            Harness.Get<Vault>(app, "_vault").Data.Settings.TrimBackground = true;
            var dead = Process.Start(new ProcessStartInfo("ping.exe", "-n 30 127.0.0.1") { CreateNoWindow = true, UseShellExecute = false })!;
            int deadPid = dead.Id;
            dead.Kill();
            dead.WaitForExit();
            dead.Dispose();
            lock (posted)
                posted.Clear();
            Harness.Invoke(app, "GuardResources", new List<RobloxProcesses.PlayerProcess> { new(deadPid, 0) }, new MemoryStatus(16000, 8000, 50), DateTime.UtcNow);
            bool trimTold;
            lock (posted)
                trimTold = posted.Any(json => json.Contains("memory priority", StringComparison.Ordinal));
            c.Check("a window that closed since the scan: no memory-priority notice", !trimTold);

            // The PC short of RAM (97 % used) with the setting on: a window in the background gets a low RAM priority, but
            // its RAM isn't emptied outright (with 5 windows open some stuttered after each emptying, 2026-10-21).
            stand.Refresh();
            long before = stand.WorkingSet64;
            Harness.Invoke(app, "GuardResources", new List<RobloxProcesses.PlayerProcess> { new(stand.Id, 0) }, new MemoryStatus(32000, 900, 97), DateTime.UtcNow);
            stand.Refresh();
            c.Check("short of RAM: a window in the background keeps its RAM (not emptied)", stand.WorkingSet64 > before / 2, $"{before / 1024} KB -> {stand.WorkingSet64 / 1024} KB");
            c.Check("…and has a low RAM priority", MemoryPriority(stand) == 2, MemoryPriority(stand));
            // Brought to the front: its normal RAM priority back at once, not at the next look at the windows.
            Harness.Invoke(app, "FrontMemory", stand.Id);
            c.Check("brought to the front: its normal RAM priority back at once", MemoryPriority(stand) == 5, MemoryPriority(stand));
            app.Dispose();
            var closed = Read(stand);
            c.Check("Bloxkeep closing gives the cores back", closed.Affinity == all && closed.Control == 0, $"0x{closed.Affinity:X} control {closed.Control}");
        }
        finally
        {
            try { stand.Kill(); } catch (InvalidOperationException) { }
        }
    }
}
