using System.Reflection;
using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Arrange windows with windows that won't get as small as their tiles, as Roblox's (at least 800 × 600 in its own units):
/// 5 of them on a 150 % laptop screen ran over each other and past the screen's edge (2026-10-21). They are spread
/// evenly over the area instead, each over the one before it. Real windows of the test's own, on a thread with a message
/// loop as a Roblox window has, given a least size the tiles are under.
/// </summary>
static class ArrangeSuite
{
    delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")]
    static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    public static Task Run(Checks c, string dir)
    {
        // Where 5 Roblox windows (1224 × 957 pixels each) go on that screen's 2560 × 1528 work area.
        var spots = RobloxProcesses.OverlapSpots(5, new Rectangle(0, 0, 2560, 1528), new Size(1224, 957));
        c.Check("5 windows too big for their tiles: 3 and 2, spread evenly from corner to corner",
            spots.SequenceEqual([new Point(0, 0), new Point(668, 0), new Point(1336, 0), new Point(0, 571), new Point(668, 571)]),
            string.Join(" ", spots));
        c.Check("…every one of them on the screen", spots.All(p => p.X + 1224 <= 2560 && p.Y + 957 <= 1528));
        var two = RobloxProcesses.OverlapSpots(2, new Rectangle(100, 0, 2560, 1528), new Size(1224, 957));
        c.Check("2 that fit side by side: side by side, not spread apart", two.SequenceEqual([new Point(100, 0), new Point(1324, 0)]), string.Join(" ", two));

        var done = new TaskCompletionSource();
        var thread = new Thread(() =>
        {
            using var host = new Form { ShowInTaskbar = false, StartPosition = FormStartPosition.Manual, Location = new Point(-2000, -2000), Size = new Size(300, 200) };
            host.Load += async (_, _) =>
            {
                var windows = new List<Form>();
                try
                {
                    var work = Screen.PrimaryScreen!.WorkingArea;
                    var area = new Rectangle(work.X + 20, work.Y + 20, 900, 700);
                    for (int i = 0; i < 5; i++)
                    {
                        var w = new Form
                        {
                            Text = $"Arrange check {i + 1}", ShowInTaskbar = false, StartPosition = FormStartPosition.Manual,
                            Location = new Point(area.X + 40 * i, area.Y + 30 * i), Size = new Size(450, 330), MinimumSize = new Size(400, 300),
                        };
                        windows.Add(w);
                        w.Show();
                    }
                    await Task.Delay(200);
                    // Tiles of 300 × 350 for 3 and 2 in 900 × 700: 100 too narrow for these windows, so 400 × 350 each,
                    // spread 250 apart across and 350 down.
                    var arrange = typeof(RobloxProcesses).GetMethod("ArrangeAsync", BindingFlags.NonPublic | BindingFlags.Static)!;
                    int count = await (Task<int>)arrange.Invoke(null, [windows.Select(w => w.Handle).ToList(), area, "fill"])!;
                    await Task.Delay(300);
                    var want = new[] { (0, 0), (250, 0), (500, 0), (0, 350), (250, 350) }.Select(p => new Rectangle(area.X + p.Item1, area.Y + p.Item2, 400, 350)).ToList();
                    var got = windows.Select(w => w.Bounds).ToList();
                    c.Check("windows that won't get as small as their tiles: spread evenly over the area, none past its edge",
                        count == 5 && got.SequenceEqual(want), string.Join(" ", got));
                    var order = new List<IntPtr>();
                    EnumWindows((h, _) => { order.Add(h); return true; }, IntPtr.Zero);
                    var rank = windows.Select(w => order.IndexOf(w.Handle)).ToList();
                    c.Check("…each over the one before it, so every title bar shows", rank.Zip(rank.Skip(1)).All(p => p.First > p.Second), string.Join(",", rank));
                    // Windows that do get as small as their tiles: tiled as before, side by side.
                    foreach (var w in windows)
                        w.MinimumSize = new Size(100, 100);
                    await (Task<int>)arrange.Invoke(null, [windows.Select(w => w.Handle).ToList(), area, "fill"])!;
                    await Task.Delay(300);
                    var tiles = new[] { (0, 0), (300, 0), (600, 0), (0, 350), (300, 350) }.Select(p => new Rectangle(area.X + p.Item1, area.Y + p.Item2, 300, 350)).ToList();
                    c.Check("windows that do get that small: tiled side by side, as before",
                        windows.Select(w => w.Bounds).SequenceEqual(tiles), string.Join(" ", windows.Select(w => w.Bounds)));
                }
                catch (Exception ex)
                {
                    c.Check("the arrange checks ran to their end", false, ex);
                }
                finally
                {
                    foreach (var w in windows)
                        w.Close();
                    host.Close();
                }
            };
            WindowsFormsSynchronizationContext.AutoInstall = true;
            Application.Run(host);
            done.SetResult();
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        return done.Task;
    }
}
