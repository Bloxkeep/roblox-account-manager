using System.Drawing;

namespace RobloxManager;

/// <summary>
/// A Roblox window's place and size from before it went full screen, put back when it leaves it: a user's report says
/// Roblox comes back at its default size (2026-11-01). Where it doesn't, the place already matches and nothing moves.
/// </summary>
public sealed class WindowSizeKeeper
{
    /// <summary>Roblox sizes its window again as it settles, so the size is held for a moment.</summary>
    static readonly TimeSpan Hold = TimeSpan.FromSeconds(5);

    sealed class Seen
    {
        public Rectangle Place;
        public bool Full;
        public DateTime HoldUntil;
    }

    readonly Dictionary<int, Seen> _seen = new();

    /// <summary>The place to put the window back to, if any.</summary>
    /// <param name="full">Covering its screen, without a title bar.</param>
    /// <param name="plain">An ordinary window: not full screen, minimized, maximized or hidden.</param>
    public Rectangle? Look(int pid, Rectangle now, bool full, bool plain, DateTime at)
    {
        if (!_seen.TryGetValue(pid, out var seen))
        {
            if (plain)
                _seen[pid] = new Seen { Place = now };
            return null;
        }
        if (full)
        {
            seen.Full = true;
            return null;
        }
        if (!plain)
            return null;
        if (seen.Full)
        {
            seen.Full = false;
            seen.HoldUntil = at + Hold;
        }
        if (at < seen.HoldUntil)
            return now == seen.Place ? null : seen.Place;
        seen.Place = now;
        return null;
    }

    public void Forget(IEnumerable<int> alive)
    {
        foreach (int gone in _seen.Keys.Except(alive).ToList())
            _seen.Remove(gone);
    }

    /// <summary>Bloxkeep is about to place the windows itself: what it gives them is the size to keep.</summary>
    public void Clear() => _seen.Clear();
}
