namespace RobloxManager;

/// <summary>
/// One thing at a time brings a Roblox window forward or shows a hidden one for a moment: anti-AFK's nudge, /pc
/// screenshot's picture, a macro's turn, Arrange windows. Each takes its turn in the order asked (a macro's next turn
/// waits behind a nudge asked meanwhile), so none undoes another's window halfway. Work that only moves or sizes windows
/// on a timer (Maximise, window sizes) skips a tick while <see cref="Busy"/>; a click or hotkey waits for the turn.
/// Everything runs on the window's thread, so the queue needs no lock beyond its own.
/// </summary>
static class InputTurn
{
    static readonly object Gate = new();
    static readonly Queue<(string Who, TaskCompletionSource<IDisposable> Ready)> Waiting = new();
    static string? _holder;

    /// <summary>A window is someone's right now (or someone waits for one).</summary>
    public static bool Busy
    {
        get
        {
            lock (Gate)
                return _holder is not null;
        }
    }

    /// <summary>Who has the turn ("afk", "picture", "macro", "arrange"…); null when free.</summary>
    public static string? Holder
    {
        get
        {
            lock (Gate)
                return _holder;
        }
    }

    /// <summary>Someone of this name waits for the turn (anti-AFK's "afk": a long turn hands the PC on).</summary>
    public static bool Waits(string who)
    {
        lock (Gate)
            return Waiting.Any(w => w.Who == who && !w.Ready.Task.IsCompleted);
    }

    /// <summary>The turn at once, or null when someone has it.</summary>
    public static IDisposable? TryEnter(string who)
    {
        lock (Gate)
        {
            if (_holder is not null)
                return null;
            _holder = who;
            return new Lease(who);
        }
    }

    /// <summary>The turn once those asking before have had theirs. Disposing the answer hands it on.</summary>
    public static Task<IDisposable> EnterAsync(string who, CancellationToken ct = default)
    {
        TaskCompletionSource<IDisposable> ready;
        lock (Gate)
        {
            if (_holder is null)
            {
                _holder = who;
                return Task.FromResult<IDisposable>(new Lease(who));
            }
            // Continued on the asker's own context, never inside the Dispose that hands the turn on.
            ready = new TaskCompletionSource<IDisposable>(TaskCreationOptions.RunContinuationsAsynchronously);
            Waiting.Enqueue((who, ready));
        }
        if (ct.CanBeCanceled)
            ct.Register(() => ready.TrySetCanceled(ct));
        return ready.Task;
    }

    /// <summary>Runs <paramref name="action"/> now when the turn is free, else once it is (a click, a hotkey).</summary>
    public static async void RunWhenFree(string who, Action action)
    {
        try
        {
            using (await EnterAsync(who))
                action();
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    static void Leave(string who)
    {
        lock (Gate)
        {
            if (_holder != who)
                return;
            _holder = null;
            while (Waiting.TryDequeue(out var next))
            {
                // One that gave up (cancelled) is passed over.
                if (next.Ready.Task.IsCompleted)
                    continue;
                _holder = next.Who;
                if (next.Ready.TrySetResult(new Lease(next.Who)))
                    return;
                _holder = null;
            }
        }
    }

    /// <summary>For the checks: nobody holds or waits for a turn.</summary>
    internal static void Reset()
    {
        lock (Gate)
        {
            _holder = null;
            Waiting.Clear();
        }
    }

    sealed class Lease(string who) : IDisposable
    {
        int _done;

        public void Dispose()
        {
            if (Interlocked.Exchange(ref _done, 1) == 0)
                Leave(who);
        }
    }
}
