using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Hiding a window while its program runs (RobloxProcesses.HideWindows), on a window of the test's own (no Roblox is
/// touched): it goes from the screen but still counts as the client's window; Show brings it back; the windows hidden
/// are written down, and a run that ended without showing them (a crash) has them shown again at the next start.
/// </summary>
static class HideWindowsSuite
{
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool ShowWindowAsync(IntPtr hWnd, int cmd);

    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var done = new TaskCompletionSource();
        var thread = new Thread(() =>
        {
            try
            {
                Check(c);
            }
            catch (Exception ex)
            {
                c.Check("the hide checks ran", false, ex);
            }
            done.SetResult();
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        return done.Task;
    }

    static void Check(Checks c)
    {
        int me = Environment.ProcessId;
        using var form = new Form { Text = "Bloxkeep hide check", FormBorderStyle = FormBorderStyle.FixedToolWindow, ShowInTaskbar = true, StartPosition = FormStartPosition.Manual, Left = -3000, Top = -3000, Width = 200, Height = 120 };
        form.Show();
        Application.DoEvents();
        IntPtr hWnd = form.Handle;
        bool Visible() { Application.DoEvents(); Thread.Sleep(150); Application.DoEvents(); return IsWindowVisible(hWnd); }
        string file = Path.Combine(Vault.DataDir, "hidden-windows.txt");

        c.Check("a shown window to start with", Visible() && RobloxProcesses.VisibleWindows([me])[me].Contains(hWnd),
            $"visible {IsWindowVisible(hWnd)}, listed {string.Join(",", RobloxProcesses.VisibleWindows([me])[me])}, form {hWnd}");
        int hid = RobloxProcesses.HideWindows([me]);
        c.Check("hidden: gone from the screen", hid == 1 && !Visible(), hid);
        c.Check("…still counted as the program's window (never taken for a windowless copy, closed gracefully)",
            RobloxProcesses.VisibleWindows([me])[me].Contains(hWnd) && RobloxProcesses.HasHidden(me));
        c.Check("…written down, so a crash doesn't leave it hidden", File.Exists(file) && File.ReadAllText(file).Contains($"{me} {hWnd.ToInt64()}"));
        c.Check("hiding it again does nothing", RobloxProcesses.HideWindows([me]) == 0);

        // Anti-AFK on a hidden window: shown unseen for the moment of its key (a stand-in here, nothing typed), then
        // hidden again.
        bool wasShownForKey = false;
        var afk = typeof(RobloxProcesses).Assembly.GetType("RobloxManager.AntiAfk")!;
        var nudge = (Task<List<IntPtr>>)afk.GetMethod("NudgeAsync", Harness.F)!.Invoke(null,
            [(IReadOnlyList<IntPtr>)[hWnd], (Func<bool>)(() => true), (Func<IntPtr, Task>)(_ => { wasShownForKey = IsWindowVisible(hWnd); return Task.CompletedTask; })])!;
        while (!nudge.IsCompleted)
        {
            Application.DoEvents();
            Thread.Sleep(20);
        }
        c.Check("anti-AFK on a hidden window: shown for its key, hidden again after", (!nudge.Result.Contains(hWnd) || wasShownForKey) && !Visible()
            && RobloxProcesses.HasHidden(me), $"reached {nudge.Result.Count}, shown {wasShownForKey}");

        // Hidden while minimized: the nudge restores it, and it went back to the screen minimized (2026-10-28).
        RobloxProcesses.ShowWindows([me]);
        Visible();
        form.WindowState = FormWindowState.Minimized;
        Visible();
        int hidMin = RobloxProcesses.HideWindows([me]);
        string before = $"hid {hidMin}, visible after hide {Visible()}, iconic {IsIconic(hWnd)}";
        var nudgeMin = (Task<List<IntPtr>>)afk.GetMethod("NudgeAsync", Harness.F)!.Invoke(null,
            [(IReadOnlyList<IntPtr>)[hWnd], (Func<bool>)(() => true), (Func<IntPtr, Task>)(_ => Task.CompletedTask)])!;
        while (!nudgeMin.IsCompleted)
        {
            Application.DoEvents();
            Thread.Sleep(20);
        }
        c.Check("anti-AFK on a hidden, minimized window: hidden again after, not back on the taskbar", !Visible() && RobloxProcesses.HasHidden(me),
            $"{before}; visible {IsWindowVisible(hWnd)}, iconic {IsIconic(hWnd)}, reached {nudgeMin.Result.Count}");
        RobloxProcesses.ShowWindows([me]);
        Visible();
        form.WindowState = FormWindowState.Normal;
        RobloxProcesses.HideWindows([me]);

        int shown = RobloxProcesses.ShowWindows([me]);
        c.Check("shown again", shown == 1 && Visible() && !RobloxProcesses.HasHidden(me), shown);
        c.Check("…and no longer written down", !File.Exists(file));

        // A run that ended without showing it: Bloxkeep's memory of it is gone, the file is left.
        RobloxProcesses.HideWindows([me]);
        var hidden = (Dictionary<IntPtr, int>)typeof(RobloxProcesses).GetField("_hidden", Harness.F)!.GetValue(null)!;
        lock (hidden)
            hidden.Clear();
        c.Check("crashed with it hidden: hidden, and no longer known", !Visible() && !RobloxProcesses.HasHidden(me));
        int back = RobloxProcesses.ShowLeftHidden();
        c.Check("the next start shows it again from the file", back == 1 && Visible() && !File.Exists(file), back);

        // A client that ended hidden: known to have been (its rejoin's window is hidden too), then forgotten.
        RobloxProcesses.HideWindows([me]);
        c.Check("a client that ended with its window hidden is said so", RobloxProcesses.ForgetHidden(me) && !RobloxProcesses.HasHidden(me) && !File.Exists(file));
        c.Check("…once", !RobloxProcesses.ForgetHidden(me));
        ShowWindowAsync(hWnd, 8);
        Visible();

        // A light-AFK preset's minimise and Settings' Maximise leave a hidden window hidden (they showed it, still known
        // as hidden); Show (Mini window, tray, Monitor) shows it again, known as shown (review 2026-10-30).
        RobloxProcesses.HideWindows([me]);
        RobloxProcesses.Minimise(hWnd);
        RobloxProcesses.Maximise(hWnd);
        c.Check("minimised or maximised by Bloxkeep, a hidden window stays hidden", !Visible() && RobloxProcesses.HasHidden(me),
            $"visible {IsWindowVisible(hWnd)}, iconic {IsIconic(hWnd)}");
        RobloxProcesses.Focus(me);
        c.Check("Show on a hidden window shows it, no longer hidden", Visible() && !RobloxProcesses.HasHidden(me) && !File.Exists(file),
            $"visible {IsWindowVisible(hWnd)}, hidden {RobloxProcesses.HasHidden(me)}");

        InTurns(c, form, Visible);
    }

    /// <summary>
    /// The page's Show and the Monitor tab's Minimise and Restore while a nudge, a picture or a macro has the window
    /// (InputTurn): they wait for the turn to end. Done during it, the turn put the window back as it found it at its
    /// start, so a window shown from the page went invisible again while Bloxkeep counted it as shown (B10).
    /// </summary>
    static void InTurns(Checks c, Form form, Func<bool> visible)
    {
        const int SW_HIDE = 0, SW_SHOWNOACTIVATE = 4, SW_RESTORE = 9;
        int me = Environment.ProcessId;
        IntPtr hWnd = form.Handle;
        void Settle(Func<bool> until)
        {
            for (var w = System.Diagnostics.Stopwatch.StartNew(); w.ElapsedMilliseconds < 2000 && !until();)
            {
                Application.DoEvents();
                Thread.Sleep(20);
            }
            Application.DoEvents();
        }

        Vault.Create(Harness.Password).Dispose();
        using var owner = new Form();
        using var app = new AppController(owner, _ => { });
        var unlock = Harness.Unlock(app);
        for (var w = System.Diagnostics.Stopwatch.StartNew(); !unlock.IsCompleted && w.ElapsedMilliseconds < 30000;)
        {
            Application.DoEvents();
            Thread.Sleep(20);
        }
        c.Check("the vault unlocked for the turn checks", unlock.IsCompletedSuccessfully, unlock.Status);
        if (!unlock.IsCompletedSuccessfully)
            return;
        InputTurn.Reset();

        // Hidden; a turn shows it unseen for its key and hides it again at its end, as it found it (AntiAfk.NudgeAsync).
        RobloxProcesses.HideWindows([me]);
        visible();
        var turn = InputTurn.TryEnter("macro")!;
        bool unshown = !IsWindowVisible(hWnd);
        ShowWindowAsync(hWnd, SW_SHOWNOACTIVATE);
        visible();
        var show = Harness.Call(app, "showWindows", new { ids = Array.Empty<string>() });
        visible();
        if (unshown)
            ShowWindowAsync(hWnd, SW_HIDE);
        Settle(() => !IsWindowVisible(hWnd));
        turn.Dispose();
        Settle(() => IsWindowVisible(hWnd));
        c.Check("Show from the page during a turn: shown once the turn ends, not left unseen while counted as shown",
            show.IsCompletedSuccessfully && IsWindowVisible(hWnd) && !RobloxProcesses.IsHidden(hWnd),
            $"answered {show.Status}, visible {IsWindowVisible(hWnd)}, hidden {RobloxProcesses.IsHidden(hWnd)}");
        RobloxProcesses.ShowWindows([me]);
        visible();

        // Shown; a turn brings it forward, and puts it back as it found it (not minimised) at its end.
        var client = new AppController.MonitorClient(hWnd, me, null, "Bloxkeep hide check", "", 0);
        object? Act(string what) => Harness.Invoke(app, "MonitorAct", client, what);
        turn = InputTurn.TryEnter("macro")!;
        bool wasMinimised = IsIconic(hWnd);
        Act("minimize");
        visible();
        if (!wasMinimised)
            ShowWindowAsync(hWnd, SW_RESTORE);
        Settle(() => !IsIconic(hWnd));
        turn.Dispose();
        Settle(() => IsIconic(hWnd));
        c.Check("Minimise on the Monitor tab during a turn: minimised once the turn ends, not undone by it", IsIconic(hWnd),
            $"iconic {IsIconic(hWnd)}");

        // Minimised; a turn restores it for its key and minimises it again at its end.
        ShowWindowAsync(hWnd, 7); // SW_SHOWMINNOACTIVE
        Settle(() => IsIconic(hWnd));
        turn = InputTurn.TryEnter("macro")!;
        wasMinimised = IsIconic(hWnd);
        ShowWindowAsync(hWnd, SW_RESTORE);
        visible();
        Act("restore");
        visible();
        if (wasMinimised)
            ShowWindowAsync(hWnd, 7); // SW_SHOWMINNOACTIVE
        Settle(() => IsIconic(hWnd)); // as a turn waits for its window to be minimised again
        turn.Dispose();
        Settle(() => !IsIconic(hWnd));
        c.Check("Restore on the Monitor tab during a turn: open once the turn ends, not minimised again by it", wasMinimised && !IsIconic(hWnd),
            $"minimised before {wasMinimised}, iconic {IsIconic(hWnd)}");
        InputTurn.Reset();
    }
}
