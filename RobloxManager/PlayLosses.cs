namespace RobloxManager;

/// <summary>
/// Play time lost to disconnects, by why they happened: from a drop to the account's next stay. Statistics' "Why they dropped".
/// </summary>
public static class PlayLosses
{
    /// <summary>One drop counts this much at most: a longer gap is more likely a PC turned off than time lost.</summary>
    static readonly TimeSpan MostPerDrop = TimeSpan.FromHours(12);

    public sealed record Loss(string Kind, int Times, long Seconds, IReadOnlyList<Guid> Accounts);

    /// <summary>Roblox's disconnect code as one of the kinds the page words: 278 idle, 273 the account joined elsewhere, 267 kicked.</summary>
    public static string KindOf(int? code) => code switch
    {
        278 => "idle",
        273 => "elsewhere",
        267 => "kicked",
        _ => "other",
    };

    /// <summary>The drops inside the period, the kind that lost the most time first.</summary>
    public static List<Loss> Of(IReadOnlyCollection<PlaySession> stays, DateTime since, DateTime now)
    {
        var drops = stays.Where(s => s.EndReason == "disconnected" && s.EndedAt >= since && s.EndedAt <= now);
        return drops.GroupBy(s => KindOf(s.ReasonCode)).Select(kind => new Loss(
                kind.Key,
                kind.Count(),
                (long)kind.Sum(drop => Lost(drop, stays, now).TotalSeconds),
                kind.Select(s => s.AccountId).Distinct().ToList()))
            .OrderByDescending(l => l.Seconds).ThenByDescending(l => l.Times).ToList();
    }

    static TimeSpan Lost(PlaySession drop, IReadOnlyCollection<PlaySession> stays, DateTime now)
    {
        var from = drop.EndedAt!.Value;
        var back = stays.Where(s => s.AccountId == drop.AccountId && s.StartedAt >= from).Select(s => (DateTime?)s.StartedAt).Min();
        var until = back is { } next && next < now ? next : now;
        var lost = until - from;
        return lost > MostPerDrop ? MostPerDrop : lost;
    }
}
