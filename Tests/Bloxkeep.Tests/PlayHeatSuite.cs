using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>Statistics' hour map: a stay's hours go to the local day and hour they were played in; an account's run of
/// whole days (23 hours or more) up to yesterday.</summary>
static class PlayHeatSuite
{
    public static Task Run(Checks c, string dir)
    {
        var firstDay = new DateTime(2026, 11, 1);
        DateTime Local(int day, double hour) => firstDay.AddDays(day).AddHours(hour).ToUniversalTime();
        double[][] Heat() => Enumerable.Range(0, 3).Select(_ => new double[24]).ToArray();

        var whole = Heat();
        AppController.AddHeat(whole, firstDay, Local(0, 14), Local(0, 15));
        c.Check("an hour played fills its cell", whole[0][14] == 1 && whole[0].Sum() == 1);

        var half = Heat();
        AppController.AddHeat(half, firstDay, Local(1, 9.5), Local(1, 11));
        c.Check("a stay split by the hours it covers", half[1][9] == .5 && half[1][10] == 1);

        var night = Heat();
        AppController.AddHeat(night, firstDay, Local(0, 23.5), Local(1, 1));
        c.Check("past midnight goes on the next day", night[0][23] == .5 && night[1][0] == 1);

        var outside = Heat();
        AppController.AddHeat(outside, firstDay, Local(-1, 10), Local(-1, 12));
        AppController.AddHeat(outside, firstDay, Local(3, 10), Local(3, 12));
        c.Check("outside the period: nothing", outside.All(day => day.Sum() == 0));

        // Full days in a row, counted back from yesterday (today isn't over): now is 4 November, 15:00.
        var now = Local(3, 15);
        Guid steady = Guid.NewGuid(), broken = Guid.NewGuid(), short_ = Guid.NewGuid();
        PlaySession Stay(Guid id, DateTime from, DateTime? to) => new() { AccountId = id, StartedAt = from, EndedAt = to };
        var streaks = AppController.FullDayStreaks(
        [
            Stay(steady, Local(0, 0), null),                                      // since 1 November, still on
            Stay(broken, Local(1, 0), Local(2, 0)), Stay(broken, Local(2, 8), Local(2, 18)), // yesterday 10 hours only
            Stay(short_, Local(0, 1), Local(1, 0)), Stay(short_, Local(1, 1), Local(2, 0)), Stay(short_, Local(2, 1), Local(3, 0)), // 23 hours a day
        ], now);
        c.Check("three whole days up to yesterday: 3", streaks[steady] == 3);
        c.Check("yesterday not whole: no streak", streaks[broken] == 0);
        c.Check("23 hours a day counts as whole", streaks[short_] == 3);
        return Task.CompletedTask;
    }
}
