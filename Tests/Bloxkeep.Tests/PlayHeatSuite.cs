using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>Statistics' hour map: a stay's hours go to the local day and hour they were played in.</summary>
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
        return Task.CompletedTask;
    }
}
