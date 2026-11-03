using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>Each account in a server of its own, and leaving crowded servers (AppController.ServerHop.cs).</summary>
static class ServerPickSuite
{
    public static Task Run(Checks c, string dir)
    {
        static ServerInfo S(string job, int playing, int max = 12) => new(job, playing, max, 0, 60);
        var random = new Random(7);

        // Own server each: the list's first with room that none of the accounts is in.
        var list = new List<ServerInfo> { S("a", 2), S("b", 12), S("c", 5), S("d", 11) };
        var taken = new HashSet<string> { "a" };
        c.Check("apart: the first free one with room", AppController.ServerApart(list, taken, "fewest", random)?.JobId == "c");
        taken.Add("c");
        c.Check("apart: a full one is skipped", AppController.ServerApart(list, taken, "most", random)?.JobId == "d");
        taken.Add("d");
        c.Check("apart: none left", AppController.ServerApart(list, taken, "fewest", random) is null);
        var picked = Enumerable.Range(0, 30).Select(_ => AppController.ServerApart(list, new HashSet<string>(), "random", random)!.JobId).ToHashSet();
        c.Check("apart: random picks among those with room", picked.SetEquals(["a", "c", "d"]));

        // Crowded: more than 3 other players, read emptiest first.
        var seen = new List<ServerInfo> { S("q", 1), S("r", 3), S("ours1", 4), S("t", 6) };
        var ours = new Dictionary<string, int> { ["ours1"] = 1 };
        c.Check("in the list: its players less the accounts'", AppController.CrowdedServers(seen, false, ours, 3).Count == 0);
        c.Check("…one more and it's crowded", AppController.CrowdedServers(seen, false, ours, 2).Contains("ours1"));
        var two = new Dictionary<string, int> { ["ours1"] = 2 };
        c.Check("two of the accounts there don't count", AppController.CrowdedServers(seen, false, two, 2).Count == 0);
        var missing = new Dictionary<string, int> { ["gone"] = 1 };
        c.Check("not in the list, which went past: crowded", AppController.CrowdedServers(seen, false, missing, 3).Contains("gone"));
        var short_ = new List<ServerInfo> { S("q", 1), S("r", 2) };
        c.Check("not in the list, which stopped early: unknown, left", AppController.CrowdedServers(short_, false, missing, 3).Count == 0);
        c.Check("not in a list that ended: full", AppController.CrowdedServers(short_, true, missing, 3).Contains("gone"));
        c.Check("…full but small enough: left", AppController.CrowdedServers([S("q", 1, 4)], true, missing, 3).Count == 0);

        // A quiet one: at most that many there, room, not where the accounts are.
        var avoid = new HashSet<string> { "q" };
        c.Check("quiet: at most N there, not avoided", AppController.QuietServer(seen, 3, avoid, random)?.JobId == "r");
        c.Check("quiet: none", AppController.QuietServer(seen, 0, avoid, random) is null);
        c.Check("the setting's choices start at off", AppController.HopAboveChoices[0] == 0 && AppController.HopAboveChoices.Contains(5));
        return Task.CompletedTask;
    }
}
