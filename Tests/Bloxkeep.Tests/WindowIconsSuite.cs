using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The account's picture as its window's icon (WindowIcons), on a window of the test's own: put on the window, kept
/// (not drawn again) while the picture is the same, replaced when it changes, put back when the window drops it (at
/// its next check, half a minute apart, one client at a time), and the window's own icon back when it is turned off or
/// the client has closed.
/// </summary>
static class WindowIconsSuite
{
    [DllImport("user32.dll")]
    static extern IntPtr SendMessage(IntPtr hWnd, int msg, IntPtr wParam, IntPtr lParam);
    static IntPtr Icon(IntPtr hWnd) => SendMessage(hWnd, 0x7F, 1, IntPtr.Zero);
    static void SetIcon(IntPtr hWnd, IntPtr icon) => SendMessage(hWnd, 0x80, 1, icon);

    static readonly Type Icons = typeof(Prefs).Assembly.GetType("RobloxManager.WindowIcons")!;
    static void Apply(int pid, IntPtr hWnd, string key, Func<System.Drawing.Bitmap> draw, bool check = false) =>
        Icons.GetMethod("Apply", Harness.F)!.Invoke(null, [pid, new List<IntPtr> { hWnd }, key, draw, check]);
    static int? NextToCheck(params int[] pids) => (int?)Icons.GetMethod("NextToCheck", Harness.F)!.Invoke(null, [pids]);
    static void Remove(int pid, IntPtr hWnd) => Icons.GetMethod("Remove", Harness.F)!.Invoke(null, [pid, new List<IntPtr> { hWnd }]);
    static void Forget(IReadOnlySet<int> alive) => Icons.GetMethod("Forget", Harness.F)!.Invoke(null, [alive]);
    static System.Drawing.Bitmap Initials(string name) => (System.Drawing.Bitmap)Icons.GetMethod("Initials", Harness.F)!.Invoke(null, [name])!;

    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        long now = 1_000_000;
        Icons.GetProperty("Clock", Harness.F)!.SetValue(null, (Func<long>)(() => now));
        using var form = new Form { ShowInTaskbar = false };
        IntPtr hWnd = form.Handle;
        IntPtr own = Icon(hWnd);
        int draws = 0;
        System.Drawing.Bitmap Draw(string name) { draws++; return Initials(name); }

        Apply(4242, hWnd, "initials:Account 1", () => Draw("Account 1"));
        IntPtr first = Icon(hWnd);
        c.Check("the account's picture is put on its window", first != IntPtr.Zero && first != own);
        Apply(4242, hWnd, "initials:Account 1", () => Draw("Account 1"));
        c.Check("…and kept, not drawn again, while it is the same", draws == 1 && Icon(hWnd) == first, draws);

        SetIcon(hWnd, IntPtr.Zero); // the window dropped it
        now += 29_000;
        int? early = NextToCheck(4242);
        Apply(4242, hWnd, "initials:Account 1", () => Draw("Account 1"), check: early == 4242);
        c.Check("a window isn't asked about its icon again within half a minute", early is null && Icon(hWnd) == IntPtr.Zero, early);
        now += 1_000;
        int? due = NextToCheck(4242);
        Apply(4242, hWnd, "initials:Account 1", () => Draw("Account 1"), check: due == 4242);
        c.Check("a window that dropped it gets it again at its next check", due == 4242 && Icon(hWnd) == first, due);

        Apply(4242, hWnd, "https://tr.rbxcdn.com/avatar.png", () => Draw("Avatar"));
        c.Check("a new picture (the avatar has loaded) replaces the initials", Icon(hWnd) != first && Icon(hWnd) != IntPtr.Zero && draws == 2);

        // Two clients due at once: one is asked a process check, the one asked longest ago first, then the other.
        using var other = new Form { ShowInTaskbar = false };
        now += 1_000;
        Apply(4244, other.Handle, "initials:Account 3", () => Initials("Account 3"));
        now += 30_000;
        int?[] turns = [NextToCheck(4242, 4244), NextToCheck(4242, 4244), NextToCheck(4242, 4244)];
        c.Check("one client checked at a time, the one checked longest ago first, then the other", turns is [4242, 4244, null], string.Join(",", turns));

        Remove(4242, hWnd);
        c.Check("turned off: the window's own icon back", Icon(hWnd) == own);

        Apply(4243, hWnd, "initials:Account 2", () => Draw("Account 2"));
        Forget(new HashSet<int>());
        c.Check("a client that closed: its icons go", !((IEnumerable<int>)Icons.GetProperty("Pids", Harness.F)!.GetValue(null)!).Contains(4243));

        // A run that ended suddenly (crash, End task) left its icons, gone with it, on its windows, and the next run took
        // them for the windows' own (review 2026-10-30): the windows given icons are written down, cleared at the next start.
        int me = Environment.ProcessId;
        string file = Path.Combine(Vault.DataDir, "window-icons.txt");
        Apply(me, hWnd, "initials:Account 4", () => Initials("Account 4"));
        c.Check("the windows given icons are written down", File.Exists(file) && File.ReadAllText(file).Contains($"{me} {hWnd.ToInt64()}"));
        ((System.Collections.IDictionary)Icons.GetField("ByPid", Harness.F)!.GetValue(null)!).Clear(); // the crash: the run's memory gone
        int cleared = (int)Icons.GetMethod("ClearLeftovers", Harness.F)!.Invoke(null, [])!;
        c.Check("the next start takes a crashed run's icons off its windows", cleared == 1 && Icon(hWnd) == IntPtr.Zero && !File.Exists(file), cleared);
        Apply(me, hWnd, "initials:Account 4", () => Initials("Account 4"));
        Remove(me, hWnd);
        c.Check("…and none of them is put back later as a window's own", Icon(hWnd) == IntPtr.Zero && !File.Exists(file));

        using var some = Initials("");
        c.Check("a name without letters still gets a picture", some.Width == 96);
        return Task.CompletedTask;
    }
}
