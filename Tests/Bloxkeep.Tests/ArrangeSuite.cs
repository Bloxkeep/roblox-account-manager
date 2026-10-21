using System.Reflection;
using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Arrange windows with windows that have a least size bigger than their tiles, as Roblox's (800 × 600 in its own units):
/// 5 of them on a 150 % laptop screen ran over each other and past the screen's edge (2026-10-21). They are given their
/// tiles all the same now; windows that grow back whatever they are given are spread evenly over the area instead, each
/// over the one before it. Real windows of the test's own, on a thread with a message loop as a Roblox window has.
/// </summary>
static class ArrangeSuite
{
    delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")]
    static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    /// <summary>A window that sizes itself back up to its least size whenever it is made smaller, however that was done.</summary>
    sealed class GrowsBack : Form
    {
        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            if (IsHandleCreated && (Width < MinimumSize.Width || Height < MinimumSize.Height))
                BeginInvoke(() => Size = new Size(Math.Max(Width, MinimumSize.Width), Math.Max(Height, MinimumSize.Height)));
        }
    }

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
                    List<Form> Open(Func<Form> make)
                    {
                        var opened = new List<Form>();
                        for (int i = 0; i < 5; i++)
                        {
                            var w = make();
                            w.Text = $"Arrange check {i + 1}";
                            w.ShowInTaskbar = false;
                            w.StartPosition = FormStartPosition.Manual;
                            w.Location = new Point(area.X + 40 * i, area.Y + 30 * i);
                            w.Size = new Size(450, 330);
                            opened.Add(w);
                            w.Show();
                        }
                        windows.AddRange(opened);
                        return opened;
                    }
                    var arrange = typeof(RobloxProcesses).GetMethod("ArrangeAsync", BindingFlags.NonPublic | BindingFlags.Static)!;
                    async Task<List<Rectangle>> Arrange(List<Form> these, Form? app = null)
                    {
                        await Task.Delay(200);
                        int count = await (Task<int>)arrange.Invoke(null, [these.Select(w => w.Handle).ToList(), area, "fill", app?.Handle ?? IntPtr.Zero])!;
                        await Task.Delay(300);
                        return count == these.Count ? these.Select(w => w.Bounds).ToList() : [];
                    }
                    int Rank(Form f)
                    {
                        var order = new List<IntPtr>();
                        EnumWindows((h, _) => { order.Add(h); return true; }, IntPtr.Zero);
                        return order.IndexOf(f.Handle);
                    }
                    // Tiles of 300 × 350 for 3 and 2 in 900 × 700, 100 narrower than these windows' least size, as Roblox's
                    // window has one: given all the same, side by side.
                    var least = Open(() => new Form { MinimumSize = new Size(400, 300) });
                    var tiles = new[] { (0, 0), (300, 0), (600, 0), (0, 350), (300, 350) }.Select(p => new Rectangle(area.X + p.Item1, area.Y + p.Item2, 300, 350)).ToList();
                    var got = await Arrange(least);
                    c.Check("windows with a least size bigger than their tiles, as Roblox's: given their tiles all the same, side by side",
                        got.SequenceEqual(tiles), string.Join(" ", got));
                    foreach (var w in least)
                        w.Close();
                    // Windows that grow back to their least size whatever they are given: 400 × 350 each, spread 250 apart
                    // across and 350 down.
                    var stubborn = Open(() => new GrowsBack { MinimumSize = new Size(400, 300) });
                    var spread = new[] { (0, 0), (250, 0), (500, 0), (0, 350), (250, 350) }.Select(p => new Rectangle(area.X + p.Item1, area.Y + p.Item2, 400, 350)).ToList();
                    got = await Arrange(stubborn);
                    c.Check("windows that grow back to their least size: spread evenly over the area, none past its edge", got.SequenceEqual(spread), string.Join(" ", got));
                    var order = new List<IntPtr>();
                    EnumWindows((h, _) => { order.Add(h); return true; }, IntPtr.Zero);
                    var rank = stubborn.Select(w => order.IndexOf(w.Handle)).ToList();
                    c.Check("…each over the one before it, so every title bar shows", rank.Zip(rank.Skip(1)).All(p => p.First > p.Second), string.Join(",", rank));
                    foreach (var w in stubborn)
                        w.Close();

                    // Another app's window over them, Bloxkeep's over that: tiled after a launch, the order stays; clicked in
                    // Bloxkeep, they come over every app, each over the one before it, and Bloxkeep's window goes under them.
                    var raised = Open(() => new Form());
                    Form Other(string name) => new() { Text = name, ShowInTaskbar = false, StartPosition = FormStartPosition.Manual, Location = new Point(area.X + 300, area.Y + 200), Size = new Size(400, 300) };
                    var other = Other("Arrange check other app");
                    var app = Other("Arrange check Bloxkeep");
                    windows.Add(other);
                    windows.Add(app);
                    other.Show();
                    app.Show();
                    await Arrange(raised);
                    c.Check("tiled after a launch: another app's window stays over them", raised.All(w => Rank(w) > Rank(other)),
                        $"{string.Join(",", raised.Select(Rank))} other {Rank(other)}");
                    await Arrange(raised, app);
                    var ranks = raised.Select(Rank).ToList();
                    c.Check("clicked in Bloxkeep: over every other app, each over the one before it, Bloxkeep's window under them",
                        ranks.All(r => r < Rank(other) && r < Rank(app)) && ranks.Zip(ranks.Skip(1)).All(p => p.First > p.Second),
                        $"{string.Join(",", ranks)} other {Rank(other)} Bloxkeep {Rank(app)}");
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
