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
    }
}
