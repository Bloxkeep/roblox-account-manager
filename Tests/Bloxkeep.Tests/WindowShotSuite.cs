using System.Drawing;
using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// /pc screenshot's picture of a window (WindowShot), on a red window of the test's own (no Roblox is touched): in view,
/// minimized and hidden by Bloxkeep, each comes out whole and red, without the title bar; a minimized one is minimized
/// again after and a hidden one hidden again, both with their own look back (not left unseen or click-through).
/// </summary>
static class WindowShotSuite
{
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll", EntryPoint = "GetWindowLongW")] static extern int GetWindowLong(IntPtr hWnd, int index);
    [DllImport("dwmapi.dll")] static extern int DwmFlush();

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
                c.Check("the screenshot checks ran", false, ex);
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
        using var form = new Form { Text = "Bloxkeep screenshot check", BackColor = Color.Red, StartPosition = FormStartPosition.Manual, Left = -3000, Top = -3000, Width = 400, Height = 300 };
        form.Show();
        Application.DoEvents();
        IntPtr hWnd = form.Handle;
        int style = GetWindowLong(hWnd, -20);
        var client = form.ClientSize; // minimized, a form says another size
        void Pump(int ms = 150) { Application.DoEvents(); Thread.Sleep(ms); Application.DoEvents(); }

        var capture = typeof(RobloxProcesses).Assembly.GetType("RobloxManager.WindowShot")!.GetMethod("CaptureAsync", Harness.F)!;
        (byte[] Jpeg, bool Black)? Shot()
        {
            var task = (Task)capture.Invoke(null, [hWnd, 1920])!;
            while (!task.IsCompleted)
                Pump(20);
            return ((Task<(byte[] Jpeg, bool Black)?>)task).Result;
        }
        string Look((byte[] Jpeg, bool Black)? shot)
        {
            if (shot is not { } s)
                return "none";
            using var picture = new Bitmap(new MemoryStream(s.Jpeg));
            var mid = picture.GetPixel(picture.Width / 2, picture.Height / 2);
            var corner = picture.GetPixel(4, 4);
            return $"{picture.Width}x{picture.Height} mid {mid.R},{mid.G},{mid.B} corner {corner.R},{corner.G},{corner.B} black {s.Black}";
        }
        static bool Red(Color p) => p.R > 200 && p.G < 60 && p.B < 60;
        bool AllRed((byte[] Jpeg, bool Black)? shot)
        {
            if (shot is not { } s || s.Black)
                return false;
            using var picture = new Bitmap(new MemoryStream(s.Jpeg));
            // Red to the top edge: the title bar is left out.
            return Red(picture.GetPixel(picture.Width / 2, picture.Height / 2)) && Red(picture.GetPixel(4, 4)) && Red(picture.GetPixel(picture.Width - 5, picture.Height - 5))
                && picture.Width == client.Width && picture.Height == client.Height;
        }

        // Its first frame drawn and composed before the picture: with the full run keeping the PC busy, 150 ms wasn't
        // always enough and the picture came out white (a Roblox window has long been drawing by then).
        bool painted = false;
        form.Paint += (_, _) => painted = true;
        form.Invalidate();
        for (var until = DateTime.UtcNow.AddSeconds(5); !painted && DateTime.UtcNow < until;)
            Pump(20);
        DwmFlush();
        Pump();
        var inView = Shot();
        c.Check("a window in view: pictured whole, its game only (no title bar)", AllRed(inView) && IsWindowVisible(hWnd) && !IsIconic(hWnd), Look(inView));

        form.WindowState = FormWindowState.Minimized;
        Pump();
        var minimized = Shot();
        Pump();
        c.Check("a minimized window: pictured whole all the same", AllRed(minimized), Look(minimized));
        c.Check("…minimized again after, with its own look back (not unseen, not click-through)", IsIconic(hWnd) && GetWindowLong(hWnd, -20) == style,
            $"iconic {IsIconic(hWnd)}, style {GetWindowLong(hWnd, -20):x} was {style:x}");
        form.WindowState = FormWindowState.Normal;
        Pump();

        RobloxProcesses.HideWindows([me]);
        Pump();
        var hidden = Shot();
        Pump();
        c.Check("a window Bloxkeep hid: pictured whole", AllRed(hidden), Look(hidden));
        c.Check("…hidden again after, with its own look back", !IsWindowVisible(hWnd) && RobloxProcesses.HasHidden(me) && GetWindowLong(hWnd, -20) == style,
            $"visible {IsWindowVisible(hWnd)}, style {GetWindowLong(hWnd, -20):x} was {style:x}");
        RobloxProcesses.ShowWindows([me]);
        Pump();

        var gone = (Task)capture.Invoke(null, [IntPtr.Zero, 1920])!;
        c.Check("no window: no picture", ((Task<(byte[] Jpeg, bool Black)?>)gone).Result is null);
    }
}
