using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The automatic Roblox update with windows open (AppController.RobloxUpdates.cs). On 2026-11-11 it closed the owner's
/// window a second after unlocking and neither updated nor opened it again: the wait for the windows to close gave up
/// in the very tick it began (its start was a moment ahead of the tick's clock), and the window, whose log hadn't
/// been read yet, wasn't one it could open again.
/// </summary>
static class RobloxUpdateSuite
{
    public static Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        var limit = TimeSpan.FromSeconds(30);
        var tick = new DateTime(2026, 11, 11, 1, 25, 20, DateTimeKind.Utc);

        // The wait starts 200 ms after the tick's clock was read (the automatic update runs inside the tick).
        var since = tick.AddMilliseconds(200);
        c.Check("A wait begun inside a tick isn't over in that tick", !AppController.CloseTimeUp(tick, ref since, limit));
        c.Check("…and counts from the tick's clock", since == tick);
        c.Check("…nor a second later", !AppController.CloseTimeUp(tick.AddSeconds(1.5), ref since, limit));
        c.Check("…it is after 30 s", AppController.CloseTimeUp(tick.AddSeconds(30), ref since, limit));

        // The PC's clock set back an hour while windows close: the 30 s start again, they aren't waited for an hour.
        var before = tick;
        c.Check("A clock set back starts the wait again", !AppController.CloseTimeUp(tick.AddHours(-1), ref before, limit) && before == tick.AddHours(-1));
        c.Check("…and it ends 30 s on", AppController.CloseTimeUp(tick.AddHours(-1).AddSeconds(31), ref before, limit));

        c.Check("Every window open can be opened again: none would stay closed", AppController.WouldStayClosed([11, 22], [22, 11]) == 0);
        c.Check("A window that can't (not in a game yet) would", AppController.WouldStayClosed([11, 22], [22]) == 1);
        c.Check("…and so would one Bloxkeep didn't open", AppController.WouldStayClosed([11, 22, 33], [11, 22]) == 1);
        c.Check("No window open: nothing to keep", AppController.WouldStayClosed([], [11]) == 0);
        return Task.CompletedTask;
    }
}
