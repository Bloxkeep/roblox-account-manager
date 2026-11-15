using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// A rejoin's window goes back where the account's last window stood (a user's request, 2026-11-11): noted while the old
/// one is open, put back once the new one is in its game and has settled, checked once more after, and left alone when
/// Arrange placed it meanwhile. Real windows of the test's own, invisible (no opacity), out of the taskbar and never
/// activated, so nothing shows or takes the keyboard on the PC running it.
/// </summary>
static class RejoinPlaceSuite
{
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }

    sealed class Quiet : Form
    {
        protected override bool ShowWithoutActivation => true;
        protected override CreateParams CreateParams
        {
            get
            {
                const int WS_EX_NOACTIVATE = 0x08000000, WS_EX_TOOLWINDOW = 0x80;
                var p = base.CreateParams;
                p.ExStyle |= WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW;
                return p;
            }
        }
    }

    static Rectangle Rect(IntPtr hWnd) => GetWindowRect(hWnd, out var r) ? Rectangle.FromLTRB(r.Left, r.Top, r.Right, r.Bottom) : default;

    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var seed = Vault.Create(Harness.Password);
        seed.Data.Accounts.Add(new Account { Username = "Tiled", UserId = 41, Cookie = "cookie-tiled", CookieValid = true });
        seed.Save();
        using var form = new Form();
        using var app = new AppController(form, _ => { });
        await Harness.Unlock(app);
        var account = Harness.Get<Vault>(app, "_vault").Data.Accounts.Single();

        // Two windows on a thread with a message loop, as Roblox's: the old tile and the rejoin's new one.
        var area = Screen.PrimaryScreen!.WorkingArea;
        var tile = new Rectangle(area.X + 40, area.Y + 40, 640, 516);
        var robloxSize = new Rectangle(area.X + 300, area.Y + 200, 816, 638);
        Quiet? oldWindow = null, newWindow = null;
        using var ready = new ManualResetEventSlim();
        var ui = new Thread(() =>
        {
            Quiet Make(Rectangle at) => new() { StartPosition = FormStartPosition.Manual, Bounds = at, Opacity = 0, ShowInTaskbar = false, Text = "Roblox" };
            oldWindow = Make(tile);
            newWindow = Make(robloxSize);
            oldWindow.Show();
            newWindow.Show();
            ready.Set();
            Application.Run();
        }) { IsBackground = true };
        ui.SetApartmentState(ApartmentState.STA);
        ui.Start();
        ready.Wait();
        IntPtr oldHwnd = (IntPtr)oldWindow!.Invoke(() => oldWindow.Handle), newHwnd = (IntPtr)newWindow!.Invoke(() => newWindow.Handle);
        var oldPlace = Rect(oldHwnd);

        var t = typeof(AppController);
        var running = Harness.Get<System.Collections.IDictionary>(app, "_running");
        var launchType = t.GetNestedType("PendingLaunch", Harness.F)!;
        var tupleType = running.GetType().GetGenericArguments()[1];
        var watcher = Harness.Get<RobloxLogWatcher>(app, "_logs");
        var tails = Harness.Get<System.Collections.IDictionary>(watcher, "_tails");
        var tailType = typeof(RobloxLogWatcher).GetNestedType("Tail", Harness.F)!;
        var goingBack = Harness.Get<System.Collections.IDictionary>(app, "_goingBack");
        var lastPlace = Harness.Get<System.Collections.IDictionary>(app, "_lastPlace");
        const int OldPid = 777201, NewPid = 777202;
        void Run(int pid, bool rejoin) => running[account.Id] = Activator.CreateInstance(tupleType, pid,
            Activator.CreateInstance(launchType, account.Id, new HashSet<int>(), DateTime.UtcNow, new GameTarget(4242), rejoin, null)!);
        void Log(int pid, string status)
        {
            var tail = Activator.CreateInstance(tailType, $"stand-in-{pid}.log", pid)!;
            tailType.GetField("State")!.SetValue(tail, new RobloxLogWatcher.ClientState(status, 4242, null, null, DateTime.UtcNow));
            tails[pid] = tail;
        }
        void Tick(DateTime now, int pid, IntPtr hWnd) =>
            Harness.Invoke(app, "KeepRejoinPlaces", now, new Dictionary<int, List<IntPtr>> { [pid] = [hWnd] });

        // The old window, in its tile and in its game: its place is noted.
        var now = DateTime.UtcNow;
        Run(OldPid, false);
        Log(OldPid, "ingame");
        Tick(now, OldPid, oldHwnd);
        c.Check("the account's window place is noted while it is open", lastPlace.Contains(account.Id) && (Rectangle)lastPlace[account.Id]! == oldPlace,
            lastPlace.Contains(account.Id) ? lastPlace[account.Id] : "none");

        // It crashes; the rejoin's window comes up at Roblox's own size, still joining.
        Run(NewPid, true);
        Log(NewPid, "joining");
        Tick(now, NewPid, newHwnd);
        c.Check("the rejoin's window waits to go back", goingBack.Contains(NewPid));
        Tick(now.AddSeconds(20), NewPid, newHwnd);
        await Task.Delay(200);
        c.Check("not moved while it joins", Rect(newHwnd) == robloxSize, Rect(newHwnd));
        c.Check("the account's place isn't overwritten by the new window's Roblox size", (Rectangle)lastPlace[account.Id]! == oldPlace);

        // In its game: a moment to settle, then back in the old tile.
        Log(NewPid, "ingame");
        Tick(now.AddSeconds(21), NewPid, newHwnd);
        await Task.Delay(200);
        c.Check("not moved the moment it gets in", Rect(newHwnd) == robloxSize, Rect(newHwnd));
        Tick(now.AddSeconds(25), NewPid, newHwnd);
        await Task.Delay(300);
        c.Check("put back where the old window stood", Rect(newHwnd) == oldPlace, $"{Rect(newHwnd)} vs {oldPlace}");

        // Roblox sizes it again a moment after: put back once more at the check.
        newWindow.Invoke(() => newWindow.Bounds = robloxSize);
        await Task.Delay(200);
        Tick(now.AddSeconds(32), NewPid, newHwnd);
        await Task.Delay(300);
        c.Check("sized again after joining, it is put back once more", Rect(newHwnd) == oldPlace, Rect(newHwnd));
        c.Check("done going back", !goingBack.Contains(NewPid));
        c.Check("Settings' Maximise leaves it where it went",
            Harness.Get<Dictionary<int, string>>(app, "_maximised").TryGetValue(NewPid, out var step) && step == "ingame");

        // Moved by hand later: that is the place noted for the next rejoin.
        var moved = new Rectangle(tile.X + 100, tile.Y + 60, 700, 540);
        newWindow.Invoke(() => newWindow.Bounds = moved);
        await Task.Delay(200);
        Tick(now.AddSeconds(40), NewPid, newHwnd);
        c.Check("a window moved by hand: its new place is noted", (Rectangle)lastPlace[account.Id]! == Rect(newHwnd), lastPlace[account.Id]);

        // A rejoin that Arrange placed before it went back stays in its new tile.
        const int ThirdPid = 777203;
        oldWindow.Invoke(() => oldWindow.Bounds = robloxSize);
        await Task.Delay(200);
        Run(ThirdPid, true);
        Log(ThirdPid, "joining");
        Tick(now.AddSeconds(50), ThirdPid, oldHwnd);
        c.Check("the next rejoin waits to go back", goingBack.Contains(ThirdPid));
        Harness.Invoke(app, "ForgetGoingBack");
        Log(ThirdPid, "ingame");
        Tick(now.AddSeconds(60), ThirdPid, oldHwnd);
        Tick(now.AddSeconds(65), ThirdPid, oldHwnd);
        await Task.Delay(300);
        c.Check("arranged meanwhile: not taken back to the old place", Rect(oldHwnd) == robloxSize, Rect(oldHwnd));

        running.Clear();
        oldWindow.Invoke(() => { oldWindow.Close(); newWindow.Close(); Application.ExitThread(); });
    }
}
