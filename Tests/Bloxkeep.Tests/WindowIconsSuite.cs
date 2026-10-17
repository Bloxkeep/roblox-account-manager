using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The account's picture as its window's icon (WindowIcons), on a window of the test's own: put on the window, kept
/// (not drawn again) while the picture is the same, replaced when it changes, put back when the window drops it, and
/// the window's own icon back when it is turned off or the client has closed.
/// </summary>
static class WindowIconsSuite
{
    [DllImport("user32.dll")]
    static extern IntPtr SendMessage(IntPtr hWnd, int msg, IntPtr wParam, IntPtr lParam);
    static IntPtr Icon(IntPtr hWnd) => SendMessage(hWnd, 0x7F, 1, IntPtr.Zero);
    static void SetIcon(IntPtr hWnd, IntPtr icon) => SendMessage(hWnd, 0x80, 1, icon);

    static readonly Type Icons = typeof(Prefs).Assembly.GetType("RobloxManager.WindowIcons")!;
    static void Apply(int pid, IntPtr hWnd, string key, Func<System.Drawing.Bitmap> draw) =>
        Icons.GetMethod("Apply", Harness.F)!.Invoke(null, [pid, new List<IntPtr> { hWnd }, key, draw]);
    static void Remove(int pid, IntPtr hWnd) => Icons.GetMethod("Remove", Harness.F)!.Invoke(null, [pid, new List<IntPtr> { hWnd }]);
    static void Forget(IReadOnlySet<int> alive) => Icons.GetMethod("Forget", Harness.F)!.Invoke(null, [alive]);
    static System.Drawing.Bitmap Initials(string name) => (System.Drawing.Bitmap)Icons.GetMethod("Initials", Harness.F)!.Invoke(null, [name])!;

    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
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
        Apply(4242, hWnd, "initials:Account 1", () => Draw("Account 1"));
        c.Check("a window that dropped it gets it again", Icon(hWnd) == first);

        Apply(4242, hWnd, "https://tr.rbxcdn.com/avatar.png", () => Draw("Avatar"));
        c.Check("a new picture (the avatar has loaded) replaces the initials", Icon(hWnd) != first && Icon(hWnd) != IntPtr.Zero && draws == 2);

        Remove(4242, hWnd);
        c.Check("turned off: the window's own icon back", Icon(hWnd) == own);

        Apply(4243, hWnd, "initials:Account 2", () => Draw("Account 2"));
        Forget(new HashSet<int>());
        c.Check("a client that closed: its icons go", !((IEnumerable<int>)Icons.GetProperty("Pids", Harness.F)!.GetValue(null)!).Contains(4243));

        using var some = Initials("");
        c.Check("a name without letters still gets a picture", some.Width == 96);
        return Task.CompletedTask;
    }
}
