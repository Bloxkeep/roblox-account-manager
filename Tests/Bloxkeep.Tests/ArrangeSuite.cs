using System.Reflection;
using System.Runtime.InteropServices;
using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// Arrange windows with windows that have a least size bigger than their tiles, as Roblox's (800 × 600 in its own units):
/// 5 of them on a 150 % laptop screen ran over each other and past the screen's edge (2026-10-21). They are given their
/// tiles all the same now; windows that grow back whatever they are given are spread evenly over the area instead, each
/// over the one before it. And the layouts picked as pictures (2026-10-23): a grid's cells whatever the number of
/// windows (two can be tiny), more windows than cells spread over the area, and a size of one's own. Real windows of the
/// test's own, on a thread with a message loop as a Roblox window has.
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

    /// <summary>
    /// A window that, once armed, takes two thirds of its size on its next move, at once or a moment after: as Roblox's
    /// window answers a move from a 150 % screen onto a 100 % one (the size Windows gives it for the new scale).
    /// </summary>
    sealed class ShrinksOnMove(bool later) : Form
    {
        public bool Armed;

        protected override void OnMove(EventArgs e)
        {
            base.OnMove(e);
            if (!Armed || !IsHandleCreated)
                return;
            Armed = false;
            void Shrink() => Size = new Size(Width * 2 / 3, Height * 2 / 3);
            if (later)
                BeginInvoke(Shrink);
            else
                Shrink();
        }
    }

    public static async Task Run(Checks c, string dir)
    {
        // Picked in the page: a layout the app knows is kept, anything else left as it was; the size held to what it takes.
        Harness.UseDataFolder(dir);
        Vault.Create(Harness.Password).Dispose();
        using (var form = new Form())
        using (var app = new AppController(form, _ => { }))
        {
            await Harness.Unlock(app);
            var s = Harness.Get<Vault>(app, "_vault").Data.Settings;
            await Harness.Call(app, "setSettings", new { windowLayout = "4x4" });
            await Harness.Call(app, "setSettings", new { windowLayout = "9x9" });
            c.Check("a grid picked is kept; one the app doesn't know is left out", s.WindowLayout == "4x4", s.WindowLayout);
            await Harness.Call(app, "setSettings", new { windowLayout = "custom", windowWidth = 50, windowHeight = 99999 });
            c.Check("a size of one's own, held to 160 × 120 at least and 7680 × 4320 at most",
                s.WindowLayout == "custom" && s.WindowWidth == 160 && s.WindowHeight == 4320, $"{s.WindowLayout} {s.WindowWidth} × {s.WindowHeight}");
        }

        // Where 5 Roblox windows (1224 × 957 pixels each) go on that screen's 2560 × 1528 work area.
        var spots = RobloxProcesses.OverlapSpots(5, new Rectangle(0, 0, 2560, 1528), new Size(1224, 957));
        c.Check("5 windows too big for their tiles: 3 and 2, spread evenly from corner to corner",
            spots.SequenceEqual([new Point(0, 0), new Point(668, 0), new Point(1336, 0), new Point(0, 571), new Point(668, 571)]),
            string.Join(" ", spots));
        c.Check("…every one of them on the screen", spots.All(p => p.X + 1224 <= 2560 && p.Y + 957 <= 1528));
        var two = RobloxProcesses.OverlapSpots(2, new Rectangle(100, 0, 2560, 1528), new Size(1224, 957));
        c.Check("2 that fit side by side: side by side, not spread apart", two.SequenceEqual([new Point(100, 0), new Point(1324, 0)]), string.Join(" ", two));

        // The layouts: a grid's cells for 2 windows as for 16; a size of one's own, as many across as fit, held to the screen.
        var screen = new Size(2560, 1528);
        c.Check("a 4 × 4 grid: 2 windows get a sixteenth of the screen each, not half",
            RobloxProcesses.GridCells(2, screen, "4x4") == (4, 640, 382), RobloxProcesses.GridCells(2, screen, "4x4"));
        c.Check("a size of one's own: 640 × 480, 4 across", RobloxProcesses.GridCells(2, screen, "custom", new Size(640, 480)) == (4, 640, 480),
            RobloxProcesses.GridCells(2, screen, "custom", new Size(640, 480)));
        c.Check("…no smaller than 160 × 120 and no larger than the screen",
            RobloxProcesses.GridCells(1, screen, "custom", new Size(20, 10)) == (16, 160, 120)
            && RobloxProcesses.GridCells(1, screen, "custom", new Size(9000, 9000)) == (1, 2560, 1528));
        // An edge dragged by the person, Roblox keeping 1224 × 957 (its least size on a 150 % screen): the size the mouse asked.
        var from = new Rectangle(100, 100, 1224, 957);
        var right = RobloxProcesses.AfterDrag(from, from, new Point(1322, 500), new Point(822, 500), 12);
        c.Check("the right edge dragged 500 in, Roblox keeping its least size: 724 wide, the left edge where it was",
            right == new Rectangle(100, 100, 724, 957), right);
        var corner = RobloxProcesses.AfterDrag(from, from, new Point(101, 101), new Point(801, 601), 12);
        c.Check("…the top-left corner dragged in: the bottom-right corner stays", corner == Rectangle.FromLTRB(800, 600, 1324, 1057), corner);
        var tiny = RobloxProcesses.AfterDrag(from, from, new Point(1322, 1055), new Point(0, 0), 12);
        c.Check("…no smaller than 160 × 120", tiny == new Rectangle(100, 100, 160, 120), tiny);
        c.Check("…made bigger, as Windows did it: left alone",
            RobloxProcesses.AfterDrag(from, new Rectangle(100, 100, 1424, 957), new Point(1322, 500), new Point(1522, 500), 12) is null);
        c.Check("…moved by its title bar: left to Windows",
            RobloxProcesses.AfterDrag(from, new Rectangle(300, 300, 1300, 1000), new Point(600, 120), new Point(800, 320), 12) is null);

        c.Check("layouts the app takes: fill, the grids up to 8 × 8, custom and the old fixed tiles; nothing else",
            new[] { "fill", "2x1", "4x4", "8x8", "custom", "small", "medium", "large" }.All(RobloxProcesses.IsLayout)
            && !new[] { "", "9x9", "0x2", "2x0", "4x", "x4", "44", "4x4x", "huge" }.Any(RobloxProcesses.IsLayout));

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
                    async Task<List<Rectangle>> Arrange(List<Form> these, Form? app = null, string layout = "fill", Size custom = default)
                    {
                        await Task.Delay(200);
                        int count = await (Task<int>)arrange.Invoke(null, [these.Select(w => w.Handle).ToList(), area, layout, app?.Handle ?? IntPtr.Zero, custom])!;
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

                    // Windows moved onto a screen at another scale take Windows' size for it (review 2026-10-30): placed
                    // first, then sized, they keep their tiles; one whose answer comes after its size is given it again.
                    foreach (bool later in new[] { false, true })
                    {
                        var scaled = Open(() => new ShrinksOnMove(later));
                        foreach (var w in scaled.Cast<ShrinksOnMove>())
                            w.Armed = true;
                        got = await Arrange(scaled);
                        var fill = new[] { (0, 0), (300, 0), (600, 0), (0, 350), (300, 350) }.Select(p => new Rectangle(area.X + p.Item1, area.Y + p.Item2, 300, 350)).ToList();
                        c.Check(later ? "…its answer coming after the size: given its tile again" : "windows that take a new scale's size as they move: their tiles all the same",
                            got.SequenceEqual(fill), string.Join(" ", got));
                        foreach (var w in scaled)
                            w.Close();
                    }

                    // A 3 × 2 grid of 900 × 700: 2 windows in its first two cells, 300 × 350 each.
                    var grid = Open(() => new Form());
                    got = await Arrange(grid.Take(2).ToList(), layout: "3x2");
                    c.Check("a 3 × 2 grid with 2 windows: its first two cells, not half the area each",
                        got.SequenceEqual([new Rectangle(area.X, area.Y, 300, 350), new Rectangle(area.X + 300, area.Y, 300, 350)]), string.Join(" ", got));
                    // A 2 × 2 grid with 5: more than its 4 cells, so cell-sized windows spread over the area, none past its edge.
                    got = await Arrange(grid, layout: "2x2");
                    var overflow = new[] { (0, 0), (225, 0), (450, 0), (0, 350), (225, 350) }.Select(p => new Rectangle(area.X + p.Item1, area.Y + p.Item2, 450, 350)).ToList();
                    c.Check("…more windows than cells: their cell's size, spread evenly over the area", got.SequenceEqual(overflow), string.Join(" ", got));
                    // A size of one's own: 200 × 150, side by side from the top-left.
                    got = await Arrange(grid.Take(3).ToList(), layout: "custom", custom: new Size(200, 150));
                    c.Check("a size of one's own (200 × 150): side by side from the top-left",
                        got.SequenceEqual([new Rectangle(area.X, area.Y, 200, 150), new Rectangle(area.X + 200, area.Y, 200, 150), new Rectangle(area.X + 400, area.Y, 200, 150)]),
                        string.Join(" ", got));

                    // A window that keeps a least size (as Roblox's), its right edge dragged 150 in: given that size.
                    var held = grid[0];
                    held.MinimumSize = new Size(400, 300);
                    held.Bounds = new Rectangle(area.X, area.Y, 400, 300);
                    await Task.Delay(200);
                    var start = held.Bounds;
                    var keep = typeof(AppController).GetMethod("KeepDraggedSizeAsync", BindingFlags.NonPublic | BindingFlags.Static)!;
                    await (Task)keep.Invoke(null, [held.Handle, start, new Point(start.Right - 2, start.Y + 150), new Point(start.Right - 152, start.Y + 150)])!;
                    await Task.Delay(200);
                    c.Check("a window with a least size, its edge dragged in past it: the size the mouse asked, its place kept",
                        held.Bounds == new Rectangle(start.X, start.Y, 250, 300), held.Bounds);
                    foreach (var w in grid)
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
        await done.Task;
    }
}
