using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>The macOS themes' window buttons: the page's "window" command reaches the window with the right action, nothing else gets through.</summary>
static class WindowSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var got = new List<string>();
        using var form = new Form();
        using var app = new AppController(form, _ => { }) { WindowControl = a => got.Add(a) };
        foreach (var action in new[] { "close", "minimize", "zoom" })
            await Harness.Call(app, "window", new { action });
        c.Check("close, minimize and zoom reach the window", string.Join(",", got) == "close,minimize,zoom", string.Join(",", got));
        c.Check("an unknown action is refused", await Harness.Try(app, "window", new { action = "quit" }) is UserError && got.Count == 3, string.Join(",", got));

        // A saved place brought back inside the screen, not only its size held to it (review 2026-10-30): a 1920 × 1032
        // work area, 8 px of Windows' resize border allowed.
        var inside = typeof(Prefs).Assembly.GetType("RobloxManager.Ui")!.GetMethod("Inside", Harness.F)!;
        Point Inside(Rectangle r) => (Point)inside.Invoke(null, [r, new Rectangle(0, 0, 1920, 1032), 8])!;
        c.Check("saved at x = 1000, held to 1920 wide: moved back onto the screen", Inside(new Rectangle(1000, 100, 1920, 900)) == new Point(8, 100),
            Inside(new Rectangle(1000, 100, 1920, 900)));
        c.Check("…saved low on a taller screen: moved up to show its bottom", Inside(new Rectangle(1500, 600, 380, 700)) == new Point(1500, 340),
            Inside(new Rectangle(1500, 600, 380, 700)));
        c.Check("…snapped to an edge (its invisible border past it) or well inside: left where it was",
            Inside(new Rectangle(-7, 0, 974, 1039)) == new Point(-7, 0) && Inside(new Rectangle(200, 150, 1320, 840)) == new Point(200, 150));
    }
}
