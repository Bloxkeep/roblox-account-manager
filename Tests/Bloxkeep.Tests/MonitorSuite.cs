using System.Drawing;
using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The game monitor's picture of a window (WindowThumbnails), on a window of the test's own (no Roblox is touched): blue
/// with an orange edge at its right and bottom. Grown, its picture follows within a moment, whole: the orange edge
/// still shows. Before, the picture kept the old size's part of the window, its top-left corner zoomed in, until the
/// tab's next refresh 1.5 s later (owner, 2026-11-02).
/// </summary>
static class MonitorSuite
{
    [DllImport("user32.dll")] static extern bool SetProcessDPIAware();

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
                c.Check("the monitor checks ran", false, ex);
            }
            done.SetResult();
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        return done.Task;
    }

    static void Check(Checks c)
    {
        // The screen is read in its own pixels, where the forms are placed.
        SetProcessDPIAware();
        void Pump(int ms) { for (var w = System.Diagnostics.Stopwatch.StartNew(); w.ElapsedMilliseconds < ms;) { Application.DoEvents(); Thread.Sleep(10); } }
        const int edge = 40;
        using var source = new Form { Text = "Bloxkeep monitor check", FormBorderStyle = FormBorderStyle.None, StartPosition = FormStartPosition.Manual, Left = 0, Top = 380, Width = 320, Height = 180, ShowInTaskbar = false };
        source.Paint += (_, e) =>
        {
            var size = source.ClientSize;
            e.Graphics.Clear(Color.SteelBlue);
            e.Graphics.FillRectangle(Brushes.Orange, size.Width - edge, 0, edge, size.Height);
            e.Graphics.FillRectangle(Brushes.Orange, 0, size.Height - edge, size.Width, edge);
        };
        source.Resize += (_, _) => source.Invalidate();
        using var host = new Form { Text = "Bloxkeep monitor host", FormBorderStyle = FormBorderStyle.None, StartPosition = FormStartPosition.Manual, Left = 0, Top = 0, Width = 480, Height = 360, ShowInTaskbar = false, TopMost = true };
        var type = typeof(RobloxProcesses).Assembly.GetType("RobloxManager.WindowThumbnails")!;
        using var pictures = (IDisposable)Activator.CreateInstance(type, Harness.F, null, [host], null)!;
        source.Show();
        host.Show();
        // Where the page would say the tile's picture goes: all of the host but a 10 px edge.
        var box = new Rectangle(10, 10, host.ClientSize.Width - 20, host.ClientSize.Height - 20);
        type.GetMethod("Place", Harness.F)!.Invoke(pictures, [new Dictionary<IntPtr, Rectangle> { [source.Handle] = box }, new Rectangle(Point.Empty, host.ClientSize)]);
        Pump(500);

        int Orange()
        {
            using var shot = new Bitmap(host.Width, host.Height);
            using (var g = Graphics.FromImage(shot))
                g.CopyFromScreen(host.Location, Point.Empty, host.Size);
            int n = 0;
            for (int y = 0; y < shot.Height; y += 2)
                for (int x = 0; x < shot.Width; x += 2)
                    if (shot.GetPixel(x, y) is { R: > 220, G: > 130 and < 190, B: < 60 })
                        n++;
            return n;
        }
        int before = Orange();
        c.Check("the picture shows the whole window, its orange edge too", before > 50, before);

        // Twice the size: a stale picture would show the blue top-left quarter only.
        source.Size = new Size(640, 360);
        Pump(250);
        int grown = Orange();
        c.Check("grown, the picture follows within a moment and still shows the orange edge", grown > 50, grown);
    }
}
