using RobloxManager;

namespace Bloxkeep.Tests;

/// <summary>
/// The one turn at the PC (InputTurn): anti-AFK's nudge, a macro's turn, a picture, a hotkey. Many askers at once, some
/// giving up: one inside at a time, in the order asked, none that gave up ever let in, and nothing left held at the end.
/// A turn's lease belongs to whoever got it: one from before a reset can't hand on someone else's.
/// </summary>
static class InputTurnSuite
{
    public static async Task Run(Checks c, string dir)
    {
        Harness.UseDataFolder(dir);
        await Contention(c);
        Leases(c);
        InputTurn.Reset();
    }

    static async Task Contention(Checks c)
    {
        InputTurn.Reset();
        const int Workers = 64;
        var rnd = new Random(5);
        var first = InputTurn.TryEnter("first")!;
        int inside = 0, most = 0;
        var order = new List<int>();
        var cancels = new List<CancellationTokenSource>();
        var tasks = new List<Task<bool>>();
        for (int i = 0; i < Workers; i++)
        {
            var cts = new CancellationTokenSource();
            cancels.Add(cts);
            if (rnd.Next(4) == 0)
                cts.CancelAfter(rnd.Next(0, 40));
            var asked = InputTurn.EnterAsync("w" + i, cts.Token);
            int me = i;
            tasks.Add(Task.Run(async () =>
            {
                IDisposable lease;
                try
                {
                    lease = await asked;
                }
                catch (OperationCanceledException)
                {
                    return false;
                }
                int now = Interlocked.Increment(ref inside);
                lock (order)
                {
                    order.Add(me);
                    most = Math.Max(most, now);
                }
                await Task.Delay(rnd.Next(0, 3));
                Interlocked.Decrement(ref inside);
                lease.Dispose();
                return true;
            }));
        }
        await Task.Delay(20);
        first.Dispose();
        var all = Task.WhenAll(tasks);
        await Task.WhenAny(all, Task.Delay(20_000));
        bool finished = all.IsCompleted;
        var leased = finished ? tasks.Select((t, i) => (t.Result, i)).Where(x => x.Result).Select(x => x.i).ToList() : [];
        c.Check("64 askers, some giving up: every one answered", finished);
        c.Check("never two inside at once", most == 1, most);
        c.Check("the ones let in went in the order they asked", order.SequenceEqual(order.Order()), string.Join(",", order));
        c.Check("each one either went in or gave up, and those that gave up were passed over", order.Count == leased.Count
            && tasks.Count(t => finished && !t.Result) == Workers - leased.Count && leased.Count > 0, $"{order.Count} {leased.Count}");
        c.Check("nothing held at the end", !InputTurn.Busy, InputTurn.Holder);
        foreach (var cts in cancels)
            cts.Dispose();
    }

    static void Leases(Checks c)
    {
        InputTurn.Reset();
        var stale = InputTurn.TryEnter("macro")!;
        InputTurn.Reset(); // the checks' reset, or a turn given up on
        var fresh = InputTurn.TryEnter("macro")!;
        stale.Dispose();
        c.Check("a lease from before can't hand on the turn someone has now", InputTurn.Busy && InputTurn.Holder == "macro");
        fresh.Dispose();
        c.Check("its own lease hands it on", !InputTurn.Busy);
        var held = InputTurn.TryEnter("macro")!;
        c.Check("anti-AFK waiting is seen by the one holding the turn", !InputTurn.Waits("afk"));
        using var cts = new CancellationTokenSource();
        var afk = InputTurn.EnterAsync("afk", cts.Token);
        bool waits = InputTurn.Waits("afk");
        cts.Cancel();
        bool gaveUp = !InputTurn.Waits("afk");
        held.Dispose();
        c.Check("…while it waits, not once it gave up", waits && gaveUp && afk.IsCanceled && !InputTurn.Busy, $"{waits} {gaveUp} {afk.Status}");
    }
}
