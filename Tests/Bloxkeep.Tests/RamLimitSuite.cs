using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Settings › Staying in game › RAM limit for resting windows (AppController.LimitRestingRam, RobloxProcesses.LimitRam),
/// on the test's own process and a window of its own (no Roblox is touched): off by default; on, a minimized or hidden
/// window gets the hard limit and one in view doesn't; it's lifted when the window shows, when the option is off, and
/// when Bloxkeep closes; left alone while anti-AFK shows the window for a moment; only the choices Settings offers.
/// Also Ctrl+Alt+M (minimize all, or open them again) and Ctrl+Alt+N (the next window), which leave hidden ones hidden.
/// </summary>
static class RamLimitSuite
{
    [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr hWnd, int cmd);

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        int me = Environment.ProcessId;
        c.Check("a hard RAM limit is set and lifted", RobloxProcesses.LimitRam(me, 512) && RobloxProcesses.HasRamLimit(me)
            && RobloxProcesses.LimitRam(me, null) && !RobloxProcesses.HasRamLimit(me));

        using (var seed = Vault.Create(Harness.Password))
            seed.Save();
        using var app = new AppController(new Form(), _ => { });
        // Making a form put WinForms' context on this thread, which has no message loop: awaits would wait on it forever.
        SynchronizationContext.SetSynchronizationContext(null);
        await Harness.Unlock(app);
        // The window lives on a thread of its own, with its own message loop.
        Form? form = null;
        // Asynchronously: the checks must not go on inside the window's own thread (they'd wait on it).
        var ready = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var ui = new Thread(() =>
        {
            form = new Form { Text = "Bloxkeep RAM limit check", ShowInTaskbar = true, StartPosition = FormStartPosition.Manual, Left = -3000, Top = -3000, Width = 300, Height = 200 };
            form.Shown += (_, _) => ready.TrySetResult();
            Application.Run(form);
        });
        ui.SetApartmentState(ApartmentState.STA);
        ui.Start();
        await ready.Task;
        IntPtr hWnd = form!.Invoke(() => form.Handle);
        void State(int cmd) { form.Invoke(() => ShowWindow(hWnd, cmd)); Thread.Sleep(200); }

        var players = new List<RobloxProcesses.PlayerProcess> { new(me, 0) };
        void Tick() => Harness.Invoke(app, "LimitRestingRam", players, RobloxProcesses.VisibleWindows([me]));
        bool Limited() => RobloxProcesses.HasRamLimit(me);

        State(6); // minimized
        Tick();
        c.Check("off by default: a minimized window isn't limited", !Limited());

        c.Check("only the choices Settings offers are taken", await Harness.Try(app, "setSettings", new { ramLimitMb = 777 }) is null
            && Harness.Get<Vault>(app, "_vault").Data.Settings.RamLimitMb == 0);
        await Harness.Call(app, "setSettings", new { ramLimitMb = 1024 });
        Tick();
        c.Check("1 GB chosen: the minimized window is limited", Limited());

        Harness.Set(app, "_nudging", true);
        State(4); // shown for a moment (anti-AFK's nudge)
        Tick();
        c.Check("…left limited while anti-AFK shows it for a moment", Limited());
        Harness.Set(app, "_nudging", false);
        Tick();
        c.Check("in view: lifted", !Limited());

        RobloxProcesses.HideWindows([me]);
        Thread.Sleep(200);
        Tick();
        c.Check("hidden by Bloxkeep: limited", Limited());
        RobloxProcesses.ShowWindows([me]);
        Thread.Sleep(200);
        Tick();
        c.Check("…shown again: lifted", !Limited());

        // Ctrl+Alt+M and Ctrl+Alt+N (AppController.Hotkeys.cs), on the same window.
        [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
        IReadOnlyCollection<int> mine = [me];
        Harness.Invoke(app, "MinimizeOrRestoreAll", mine);
        Thread.Sleep(300);
        c.Check("Ctrl+Alt+M with a window open: minimized", IsIconic(hWnd));
        Tick();
        c.Check("…and so the RAM limit applies to it", Limited());
        Harness.Invoke(app, "MinimizeOrRestoreAll", mine);
        Thread.Sleep(300);
        c.Check("Ctrl+Alt+M with all minimized: opened again", !IsIconic(hWnd));
        Harness.Invoke(app, "MinimizeOrRestoreAll", mine);
        Thread.Sleep(300);
        Harness.Invoke(app, "FocusNextWindow", mine);
        Thread.Sleep(300);
        c.Check("Ctrl+Alt+N: the next window opened from minimized", !IsIconic(hWnd));
        RobloxProcesses.HideWindows([me]);
        Thread.Sleep(200);
        Harness.Invoke(app, "MinimizeOrRestoreAll", mine);
        Thread.Sleep(300);
        c.Check("…a window hidden with Ctrl+Alt+H is left hidden by Ctrl+Alt+M", !IsIconic(hWnd) && RobloxProcesses.HasHidden(me));
        RobloxProcesses.ShowWindows([me]);
        Thread.Sleep(200);

        State(6);
        Tick();
        await Harness.Call(app, "setSettings", new { ramLimitMb = 0 });
        Tick();
        c.Check("turned off: lifted, though still minimized", !Limited());

        await Harness.Call(app, "setSettings", new { ramLimitMb = 2048 });
        Tick();
        bool before = Limited();
        app.Dispose();
        c.Check("Bloxkeep closing lifts it (no window is left limited without it)", before && !Limited(), before);

        form.Invoke(form.Close);
        ui.Join();
    }
}
