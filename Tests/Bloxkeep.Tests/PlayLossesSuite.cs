using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>Statistics' "Why they dropped": the time each kind of disconnect cost.</summary>
static class PlayLossesSuite
{
    public static Task Run(Checks c, string dir)
    {
        var since = new DateTime(2026, 11, 1, 0, 0, 0, DateTimeKind.Utc);
        var now = since.AddDays(2);
        Guid one = Guid.NewGuid(), two = Guid.NewGuid();
        PlaySession Stay(Guid who, double from, double to, string? reason = null, int? code = null) => new()
        {
            AccountId = who, StartedAt = since.AddHours(from), EndedAt = since.AddHours(to), EndReason = reason, ReasonCode = code,
        };

        c.Check("no drops: nothing", PlayLosses.Of([Stay(one, 0, 2, "left")], since, now).Count == 0);

        var back = PlayLosses.Of([Stay(one, 0, 2, "disconnected", 278), Stay(one, 5, 6, "left")], since, now);
        c.Check("from the drop to the next stay", back.Single() is { Kind: "idle", Times: 1, Seconds: 3 * 3600 });

        var neverBack = PlayLosses.Of([Stay(one, 46, 47, "disconnected", 278)], since, now);
        c.Check("never back: up to now", neverBack.Single().Seconds == 3600);
        var longGone = PlayLosses.Of([Stay(one, 0, 1, "disconnected", null)], since, now);
        c.Check("a long gap counts 12 hours at most", longGone.Single() is { Kind: "other", Seconds: 12 * 3600 });

        var otherAccount = PlayLosses.Of([Stay(one, 0, 2, "disconnected", 273), Stay(two, 3, 4, "left")], since, since.AddHours(5));
        c.Check("another account's stay isn't this one coming back", otherAccount.Single().Seconds == 3 * 3600);

        var before = PlayLosses.Of([Stay(one, -5, -4, "disconnected", 278)], since, now);
        c.Check("a drop before the period isn't in it", before.Count == 0);

        var sorted = PlayLosses.Of(
            [Stay(one, 0, 1, "disconnected", 267), Stay(one, 2, 3, "disconnected", 278), Stay(one, 9, 10, "left"), Stay(two, 0, 1, "disconnected", 278), Stay(two, 2, 3, "left")],
            since, now);
        c.Check("the kind that lost the most first", sorted[0].Kind == "idle" && sorted[1].Kind == "kicked");
        c.Check("…with its times and accounts", sorted[0] is { Times: 2, Seconds: 7 * 3600 } && sorted[0].Accounts.Count == 2);
        return Task.CompletedTask;
    }
}
