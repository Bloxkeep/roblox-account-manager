using System.Drawing;
using System.Drawing.Imaging;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>A Roblox window a macro plays into, as the desktop has it now.</summary>
/// <param name="Client">Its client area on the screen, in pixels.</param>
/// <param name="Fps">The frame rate limit Bloxkeep gave it; 0: not known.</param>
/// <param name="InGame">Its log says it is in a game (or there is no log to say otherwise).</param>
/// <param name="Elevated">It runs as administrator, which Windows won't let Bloxkeep's input reach.</param>
sealed record MacroWindow(Guid AccountId, int Pid, IntPtr Handle, Rectangle Client, int Fps, bool InGame, bool Elevated);

/// <summary>
/// The desktop as macros see it: the real one (<see cref="MacroDesk"/>), or a check's stand-in with a clock of its own,
/// so no check ever sends input or brings a window forward.
/// </summary>
interface IMacroDesk
{
    DateTime Now { get; }
    Task Delay(int ms, CancellationToken ct);
    /// <summary>Why nothing may be played anywhere now (the PC locked, a person at it, a full-screen app in front); null: free.</summary>
    string? Hold();
    /// <summary>The account's window now; null when it has none.</summary>
    MacroWindow? Window(Guid accountId);
    /// <summary>The window brought forward for input (shown unseen if minimised or hidden), or null when Windows refused.</summary>
    Task<IMacroTurn?> BeginTurnAsync(MacroWindow window, CancellationToken ct);
    /// <summary>The window's client area as it is now, at full size; null when it can't be pictured.</summary>
    Task<Bitmap?> PictureAsync(MacroWindow window, IMacroTurn? turn);
    /// <summary>A Vietnamese input method (UniKey, EVKey) is running: it may rewrite some keys typed.</summary>
    bool ImeOn();
    /// <summary>Words read in an area of a picture, per language; null when this PC can't read text.</summary>
    Task<List<List<MacroOcr.Line>>?> ReadAsync(Bitmap picture, Rectangle area);
    /// <summary>Only an area of the client (pixels of the client), quickly, during a turn: what real-time steps look at
    /// many times a second. Null when it can't be pictured.</summary>
    Task<Bitmap?> AreaAsync(MacroWindow window, IMacroTurn turn, Rectangle area);
}

/// <summary>A window in front with input going to it. Disposing gives the PC back as it was.</summary>
interface IMacroTurn : IAsyncDisposable
{
    /// <summary>The window's client area on the screen now (it may have moved).</summary>
    Rectangle Client { get; }
    /// <summary>It is still the window in front.</summary>
    bool StillFront();
    /// <summary>A person's input came since the turn began (ours never counts).</summary>
    bool PersonInput();
    void Key(MacroKey key, bool up);
    void Char(char c, bool up);
    void MoveTo(Point screen);
    void MoveBy(int dx, int dy);
    void Button(string button, bool up);
    void Wheel(int notches);
    /// <summary>
    /// Before a press at a place on the screen: Bloxkeep's own windows there (a macro's panel, the mini window) let clicks
    /// through until the turn ends. False when another app's window is there: the press would go to it.
    /// </summary>
    bool Clear(Point screen);
    /// <summary>Where the pointer is now, on the screen.</summary>
    Point CursorAt();
    /// <summary>Bloxkeep closing: every key and button this turn pressed and still holds let go, the pointer and the
    /// windows put back, now, with no wait (nothing after the closing would run). Nothing is sent after it.</summary>
    void Abort();
}

/// <summary>Another app's window is over the place a step would click.</summary>
sealed class MacroCovered : Exception;

/// <summary>One macro on one account's window, from queued to finished.</summary>
sealed class MacroRun
{
    public required Macro Macro { get; init; }
    public required Guid AccountId { get; init; }
    public required int[] Jumps { get; init; }
    /// <summary>A draft run once from the editor ("Try it"): not the saved macro's.</summary>
    public bool Trial { get; init; }
    /// <summary>"waiting", "running", "paused", "done", "failed" or "stopped".</summary>
    public string Status { get; set; } = "waiting";
    public string? Reason { get; set; }
    /// <summary>The step to run next (0-based).</summary>
    public int Pc { get; set; }
    /// <summary>Passes through loops so far.</summary>
    public int Loops { get; set; }
    public DateTime StartedAt { get; init; }
    public DateTime? EndsAt { get; set; }
    /// <summary>Not before this (a long wait, a picture to look at again).</summary>
    public DateTime ResumeAt { get; set; }
    /// <summary>The wait being waited for times out at this.</summary>
    public DateTime? Deadline { get; set; }
    /// <summary>waitStill: the last small picture, and since when it hasn't changed.</summary>
    public MacroVision.Gray? LastFrame { get; set; }
    public DateTime? StillSince { get; set; }
    /// <summary>Each loop's passes (by its first step).</summary>
    public Dictionary<int, int> Passes { get; } = new();
    /// <summary>The loop at this step was reached from its end, not from above.</summary>
    public int Again { get; set; } = -1;
    /// <summary>Keys and buttons this run holds down (pressed again when its window's next turn begins).</summary>
    public HashSet<string> Held { get; } = new();
    /// <summary>The key each Hold step pressed, by the key as the step names it ("W", or "$o1" for a setting): its Release
    /// lets go of that one, even when the setting was changed on the panel meanwhile.</summary>
    public Dictionary<string, string> HeldAs { get; } = new();
    /// <summary>What is left of a wait cut short (the person, the window leaving the front, anti-AFK's turn), in ms.</summary>
    public int? WaitLeft { get; set; }
    /// <summary>The process of the window its last turn played into; 0 before its first.</summary>
    public int Pid { get; set; }
    public int FocusMisses { get; set; }
    /// <summary>Its "recover" step (-1: none): the macro's own steps end there, the If stuck part follows.</summary>
    public int RecoverAt { get; init; } = -1;
    /// <summary>Running its If stuck part; from the top once that's done.</summary>
    public bool Recovering { get; set; }
    /// <summary>Since when what its steps look for hasn't been seen (time paused left out); null: it was just seen.</summary>
    public DateTime? MissSince { get; set; }
    public DateTime? PausedAt { get; set; }
    /// <summary>Restarts in a row with nothing seen in between.</summary>
    public int Restarts { get; set; }
    /// <summary>The feature playing now (its id); null: the main steps.</summary>
    public string? Section { get; set; }
    /// <summary>Each feature's jumps, by its id.</summary>
    public Dictionary<string, int[]> FeatureJumps { get; init; } = new();
    /// <summary>The main steps' "End repeat"s of repeats inside no other block: each pass's end is a safe point.</summary>
    public HashSet<int> SafeEnds { get; init; } = new();
    /// <summary>Features still to play at this safe point, after the one playing.</summary>
    public Queue<string> Due { get; } = new();
    /// <summary>Where the main steps go on once the features have played, and their repeats' passes meanwhile.</summary>
    public int ReturnTo { get; set; }
    public Dictionary<int, int>? MainPasses { get; set; }
    /// <summary>Back from the features at a safe point: that step plays on without looking for them again.</summary>
    public bool Resumed { get; set; }
    /// <summary>Run a macro steps put in place: each step's own number in the macro as saved, by section ("" the main
    /// steps); none when it runs no other macro.</summary>
    public Dictionary<string, int[]> Origins { get; init; } = new();
    /// <summary>The step to show as playing (or where it stopped): the Run a macro step for the other macro's steps.</summary>
    public int ShownPc => Origins.TryGetValue(Section ?? "", out var o) && o.Length > 0 ? o[Math.Clamp(Pc, 0, o.Length - 1)] : Pc;
    /// <summary>The macro the step playing now is from, when a Run a macro step put it here: its settings are read.</summary>
    public Macro? Owner { get; set; }
    /// <summary>The last web step, so a loop doesn't ask an address many times a second.</summary>
    public DateTime LastWebAt { get; set; }
    /// <summary>The steps playing now: the feature's, or the main ones.</summary>
    public List<MacroStep> Steps => Feature?.Steps ?? Macro.Steps;
    public int[] JumpsNow => Section is { } id && FeatureJumps.TryGetValue(id, out var jumps) ? jumps : Jumps;
    public MacroFeature? Feature => Section is { } id ? Macro.Features.FirstOrDefault(f => f.Id == id) : null;
    public CancellationTokenSource Cancel { get; } = new();
    public bool Finished => Status is "done" or "failed" or "stopped";
    /// <summary>For error.log: steps played, the last pause written, notes written once, the last picture match.</summary>
    public int StepsRun { get; set; }
    public string? LoggedPause { get; set; }
    /// <summary>The run before it on this account ended well: its start and end aren't logged (a repeat would fill the log).</summary>
    public bool Quiet { get; init; }
    public HashSet<string> Noted { get; } = new();
    public double? LastScore { get; set; }
    /// <summary>For error.log: the macro's and the account's ids (first 8 characters), never a name.</summary>
    public string Tag => $"{Macro.Id.ToString()[..8]}{(Trial ? " (Try it)" : "")} on account {AccountId.ToString()[..8]}";
}

/// <summary>A feature on one account: when it last played, and why it didn't play through last time (null: it did).</summary>
sealed class FeatureRecord
{
    public DateTime? LastRan { get; set; }
    public string? Failed { get; set; }
    /// <summary>The step (0-based) it stopped at when it failed.</summary>
    public int FailedStep { get; set; }
}

/// <summary>
/// Plays macros into Roblox windows. Runs (macro × account) are queued; one window at a time gets a turn, in rotation:
/// a turn plays steps until a long wait or a wait on the picture, then the window is given back and the next one goes,
/// so long macros on many windows share the PC rather than one holding it. A turn brings its window forward only once
/// it has input to send, takes <see cref="InputTurn"/> (anti-AFK's nudge and a screenshot take theirs between two turns),
/// and stops when a person touches the PC, or the window is no longer in front. Everything held is let go whenever a
/// turn ends. Runs wait while the PC is locked, in use, or a full-screen app is in front.
/// </summary>
sealed class MacroPlayer(IMacroDesk desk)
{
    /// <summary>A wait longer than this gives the window back until it is over.</summary>
    public static readonly TimeSpan LongWait = TimeSpan.FromMilliseconds(1500);
    IMacroDesk Desk => desk;
    /// <summary>One turn plays this long at most while other windows wait.</summary>
    static readonly TimeSpan TurnShare = TimeSpan.FromSeconds(8);
    /// <summary>Between two turns: the anti-AFK nudge or screenshot asked meanwhile goes first.</summary>
    const int TurnGap = 120;
    /// <summary>How often a wait on the picture looks again.</summary>
    const int LookEvery = 500, ReadEvery = 1000, StillEvery = 250;
    /// <summary>A full-screen app in front holds runs back this long at most (the person is away: see <see cref="Hold"/>).</summary>
    static readonly TimeSpan FullScreenWait = TimeSpan.FromMinutes(5);
    /// <summary>Steps one turn runs at most (a loop of variables only would never end it otherwise).</summary>
    const int TurnSteps = 2000;
    /// <summary>A turn this old hands the PC on as soon as anti-AFK asks for it, held keys or not: every other window's
    /// nudge waits behind it, and Roblox kicks a window idle for 20 minutes.</summary>
    internal static readonly TimeSpan AfkBudget = TimeSpan.FromSeconds(30);
    /// <summary>A real-time step (clickColour again and again, or each patch) gives up after this (or its timeout if
    /// longer): a colour that never goes (it also matches something always there) would keep the window for good.</summary>
    internal static readonly TimeSpan RealTimeCap = TimeSpan.FromMinutes(10);
    /// <summary>A wait in front is waited in pieces this long, so the person, the window leaving the front, the run's
    /// longest time and anti-AFK's turn each end it at once.</summary>
    const int WaitSlice = 250;
    const int MaxKept = 200;
    /// <summary>Restarts in a row, nothing seen in between, before a stuck run gives up.</summary>
    internal const int MaxRestarts = 5;

    readonly List<MacroRun> _runs = new();
    int _next;
    Task? _loop;
    /// <summary>The turn going on now (one at a time: InputTurn), for <see cref="ReleaseAll"/>.</summary>
    Turn? _turn;
    DateTime? _fullScreenSince;
    /// <summary>error.log: the last queue line per macro that started nothing, and whether the full-screen wait ran out.</summary>
    readonly Dictionary<Guid, string?> _queueNoted = new();
    bool _fullScreenNoted;

    /// <summary>Each account's variables (setVar, addVar), in memory only.</summary>
    public Dictionary<Guid, Dictionary<string, double>> Vars { get; } = new();
    /// <summary>Each account's text variables (a Set variable step's text, a Read step's words), in memory only.</summary>
    public Dictionary<Guid, Dictionary<string, string>> Texts { get; } = new();
    /// <summary>Each feature's record on each account, kept across runs (a rejoin doesn't make one play again at once).</summary>
    public Dictionary<(Guid Macro, Guid Account, string Feature), FeatureRecord> Features { get; } = new();
    /// <summary>Runs, finished ones too (the last of each macro and account), in the order queued.</summary>
    public IReadOnlyList<MacroRun> Runs => _runs;
    public bool Active => _runs.Any(r => !r.Finished);
    /// <summary>Something to show changed (throttled by the app).</summary>
    public Action? Changed { get; set; }
    /// <summary>A turn sent input to the window of this process (anti-AFK counts it).</summary>
    public Action<Guid, int>? Played { get; set; }
    /// <summary>Asks Bloxkeep to rejoin the account's server; the answer is why it can't, or null.</summary>
    public Func<Guid, string?>? Rejoin { get; set; }
    /// <summary>A Discord alert and an activity entry about the account (macro, text, the window's picture as PNG or null).</summary>
    public Action<Guid, Macro, string, byte[]?>? Alert { get; set; }
    /// <summary>Recording: every run waits.</summary>
    public bool Recording { get; set; }
    /// <summary>For the checks: random timing and offsets.</summary>
    public Random Random { get; set; } = Random.Shared;
    /// <summary>A saved macro by its id (a copy), for Run a macro steps; null when it's gone.</summary>
    public Func<Guid, Macro?>? FindMacro { get; set; }
    /// <summary>The clipboard, files and web addresses (Read and Write steps).</summary>
    public IMacroOutside Outside { get; set; } = new MacroOutside();

    /// <summary>Queues the macro on these accounts; one already running there is left alone, except the editor's Try it
    /// of it, which the saved macro takes over from (Run, its hotkey or trigger did nothing, with no word, before). Returns
    /// how many were queued.</summary>
    public int Queue(Macro macro, IEnumerable<Guid> accounts, bool trial = false)
    {
        var origins = new Dictionary<string, int[]>();
        if (MacroRules.AllSteps(macro).Any(s => s.Kind == "call"))
        {
            macro = MacroRules.Copy(macro);
            (macro.Steps, origins[""]) = Expand(macro.Steps, [macro.Id]);
            foreach (var f in macro.Features)
                (f.Steps, origins[f.Id]) = Expand(f.Steps ?? [], [macro.Id]);
        }
        var jumps = MacroRules.Jumps(macro.Steps);
        int queued = 0;
        var now = desk.Now;
        var takenOver = new List<MacroRun>();
        var featureJumps = macro.Features.ToDictionary(f => f.Id, f => MacroRules.Jumps(f.Steps));
        var safeEnds = SafeEnds(macro.Steps);
        int asked = 0, busy = 0, already = 0, quietQueued = 0;
        foreach (var account in accounts.Distinct())
        {
            asked++;
            // One macro on a window at a time: two would take turns on it, one's clicks in the middle of the other's.
            if (BusyWith(account, macro.Id) is not null)
            {
                busy++;
                continue;
            }
            var going = _runs.FirstOrDefault(r => !r.Finished && r.Macro.Id == macro.Id && r.AccountId == account);
            if (going is { Trial: true } && !trial)
            {
                // One run of a macro per account in the list (the page shows that one): Try it's goes.
                _runs.Remove(going);
                takenOver.Add(going);
            }
            else if (going is not null)
            {
                already++;
                continue;
            }
            bool quiet = _runs.Any(r => r.Finished && r.Macro.Id == macro.Id && r.AccountId == account && r.Status == "done" && r.Reason is null);
            if (quiet)
                quietQueued++;
            _runs.RemoveAll(r => r.Finished && r.Macro.Id == macro.Id && r.AccountId == account);
            _runs.Add(new MacroRun
            {
                Quiet = quiet,
                Macro = macro, AccountId = account, Jumps = jumps, StartedAt = now, ResumeAt = now, Trial = trial,
                FeatureJumps = featureJumps, SafeEnds = safeEnds, Origins = origins,
                RecoverAt = macro.Steps.FindIndex(st => st.Kind == "recover"),
                EndsAt = now + TimeSpan.FromMinutes(macro.MaxMinutes),
            });
            queued++;
        }
        // Stopped once the list is as it will be: what the try holds is let go as its turn ends.
        foreach (var run in takenOver)
            Finish(run, "stopped", "byUser");
        while (_runs.Count > MaxKept && _runs.FindIndex(r => r.Finished) is >= 0 and var old)
            _runs.RemoveAt(old);
        string queueNote = $"queued on {queued} of {asked} accounts{(trial ? " (Try it)" : "")}"
            + (busy > 0 ? $", {busy} busy with another macro" : "") + (already > 0 ? $", {already} already running it" : "");
        // A repeat that finds it still running would write the same line every period.
        if (queued > quietQueued || _queueNoted.GetValueOrDefault(macro.Id) != queueNote)
            Log.Note($"Macro {macro.Id.ToString()[..8]}: {queueNote}.");
        _queueNoted[macro.Id] = queued > 0 ? null : queueNote;
        if (queued > 0)
        {
            Changed?.Invoke();
            if (_loop is null || _loop.IsCompleted)
                _loop = LoopAsync();
        }
        return queued;
    }

    /// <summary>Another macro going on (or waiting) on the account; null when none.</summary>
    public Macro? BusyWith(Guid account, Guid macroId) =>
        _runs.FirstOrDefault(r => !r.Finished && r.AccountId == account && r.Macro.Id != macroId)?.Macro;

    /// <summary>The "End repeat"s of the main steps' repeats inside no other block, before the If stuck part.</summary>
    internal static HashSet<int> SafeEnds(List<MacroStep> steps)
    {
        var ends = new HashSet<int>();
        int depth = 0;
        for (int i = 0; i < steps.Count && steps[i].Kind != "recover"; i++)
        {
            switch (steps[i].Kind)
            {
                case "if" or "loop" or "loopUntil":
                    depth++;
                    break;
                case "endIf":
                    depth--;
                    break;
                case "endLoop":
                    // Inside no other block: nothing else was open.
                    if (depth-- == 1)
                        ends.Add(i);
                    break;
            }
        }
        return ends;
    }

    /// <summary>The loop running the queue, for the checks to wait on.</summary>
    internal Task Idle => _loop ?? Task.CompletedTask;

    /// <summary>Stops one macro's runs (all when null), on one account when given, only the editor's Try it (whatever id it got) when
    /// <paramref name="trialOnly"/>; what they hold is let go as their turn ends.</summary>
    public int Stop(Guid? macroId, string reason, Guid? accountId = null, bool trialOnly = false)
    {
        int stopped = 0;
        foreach (var run in _runs.Where(r => !r.Finished && (macroId is null || r.Macro.Id == macroId || trialOnly) && (accountId is null || r.AccountId == accountId)
            && (!trialOnly || r.Trial)))
        {
            Finish(run, "stopped", reason);
            stopped++;
        }
        if (stopped > 0)
            Changed?.Invoke();
        return stopped;
    }

    /// <summary>
    /// Bloxkeep closing: what the turn going on holds (keys and buttons held, a press or click halfway) let go and the PC
    /// given back before this returns, then every run stopped. The usual endings run in the turn's awaits, which never
    /// come back once the app closes: keys stayed down until the person pressed them.
    /// </summary>
    public void ReleaseAll(string why)
    {
        var turn = _turn;
        Log.Note($"Macro: letting go of everything ({why}){(turn is null ? "" : $", a turn on window {turn.Window.Pid} was going on")}.");
        // Stopped first: the turn, if it goes on a moment, never brings a window forward again.
        Stop(null, why);
        turn?.Abort();
    }

    /// <summary>The vault went: runs stopped, variables forgotten, finished runs too.</summary>
    public void Clear(string reason)
    {
        Stop(null, reason);
        Vars.Clear();
        Texts.Clear();
        Features.Clear();
        _runs.Clear();
        Changed?.Invoke();
    }

    void Finish(MacroRun run, string status, string? reason = null)
    {
        if (run.Finished)
            return;
        run.Status = status;
        run.Reason = reason;
        run.EndsAt = desk.Now;
        run.Cancel.Cancel();
        if (run.Quiet && status == "done" && reason is null)
            return;
        Log.Note($"Macro {run.Tag}: {status}{(reason is null ? "" : $" ({reason})")} after {run.StepsRun} steps, at step {run.ShownPc + 1}"
            + $"{(run.Section is { } feature ? $" of feature {feature}" : "")}, window {run.Pid}.");
    }

    /// <summary>error.log: a note written once per run for a condition that would repeat every look.</summary>
    static void NoteOnce(MacroRun run, string key, string text)
    {
        if (run.Noted.Add(key))
            Log.Note($"Macro {run.Tag}: {text}");
    }

    static string KindAt(MacroRun run) => run.Pc >= 0 && run.Pc < run.Steps.Count ? run.Steps[run.Pc].Kind : "end";

    void Set(MacroRun run, string status, string? reason)
    {
        if (run.Status == status && run.Reason == reason)
            return;
        // Each pause reason once in a row: the person at the PC would write one every turn.
        if (status == "paused" && reason != run.LoggedPause)
        {
            run.LoggedPause = reason;
            Log.Note($"Macro {run.Tag}: paused ({reason}) at step {run.ShownPc + 1}.");
        }
        // Time held back isn't time stuck.
        if (status == "paused")
            run.PausedAt ??= desk.Now;
        else if (run.PausedAt is { } paused)
        {
            if (run.MissSince is { } since)
                run.MissSince = since + (desk.Now - paused);
            run.PausedAt = null;
        }
        run.Status = status;
        run.Reason = reason;
        Changed?.Invoke();
    }

    async Task LoopAsync()
    {
        while (_runs.Any(r => !r.Finished))
        {
            var now = desk.Now;
            foreach (var run in _runs.Where(r => !r.Finished && r.EndsAt <= now).ToList())
            {
                Finish(run, "stopped", "maxTime");
                Changed?.Invoke();
            }
            if (Hold() is { } held)
            {
                foreach (var run in _runs.Where(r => !r.Finished))
                    Set(run, "paused", held);
                await desk.Delay(500, CancellationToken.None);
                continue;
            }
            // Free again: runs held for the PC's sake wait for their turn.
            foreach (var run in _runs.Where(r => r is { Finished: false, Status: "paused", Reason: "personActive" or "locked" or "fullscreen" or "recording" }))
                Set(run, "waiting", null);
            var ready = Next(now);
            if (ready is null)
            {
                var soonest = _runs.Where(r => !r.Finished).Select(r => r.ResumeAt).DefaultIfEmpty(now).Min();
                await desk.Delay((int)Math.Clamp((soonest - now).TotalMilliseconds, 10, 250), CancellationToken.None);
                continue;
            }
            try
            {
                await TurnAsync(ready);
            }
            catch (OperationCanceledException)
            {
                // Stopped while waiting for its turn.
            }
            catch (Exception ex)
            {
                Log.Write(ex);
                Finish(ready, "failed");
            }
            Changed?.Invoke();
            await desk.Delay(TurnGap, CancellationToken.None);
        }
        Changed?.Invoke();
    }

    /// <summary>Why every run waits now: recording, then the desktop's reasons. A full-screen app holds them back five
    /// minutes at most: the person isn't at the PC (that would be "personActive"), so it is likely a video left playing.</summary>
    string? Hold()
    {
        if (Recording)
            return "recording";
        string? held = desk.Hold();
        if (held != "fullscreen")
        {
            _fullScreenSince = null;
            return held;
        }
        _fullScreenSince ??= desk.Now;
        if (desk.Now - _fullScreenSince < FullScreenWait)
        {
            _fullScreenNoted = false;
            return held;
        }
        if (!_fullScreenNoted)
        {
            _fullScreenNoted = true;
            Log.Note("Macro: a full-screen app has been in front 5 minutes; macros play anyway.");
        }
        return null;
    }

    /// <summary>The next run due, in rotation.</summary>
    MacroRun? Next(DateTime now)
    {
        var live = _runs.Where(r => !r.Finished).ToList();
        for (int i = 0; i < live.Count; i++)
        {
            var run = live[(_next + i) % live.Count];
            if (run.ResumeAt <= now)
            {
                _next = (_next + i + 1) % Math.Max(1, live.Count);
                return run;
            }
        }
        return null;
    }

    bool OthersReady(MacroRun run) => _runs.Any(r => r != run && !r.Finished && r.ResumeAt <= desk.Now);

    /// <summary>How a step left the turn: go on, give the window back, or the run is over.</summary>
    enum Then { Next, EndTurn, Over }

    /// <summary>One window's turn.</summary>
    async Task TurnAsync(MacroRun run)
    {
        var window = desk.Window(run.AccountId);
        if (window is null)
        {
            Finish(run, "failed", "windowGone");
            return;
        }
        if (window.Elevated)
        {
            Finish(run, "failed", "elevated");
            return;
        }
        if (!window.InGame)
        {
            Set(run, "paused", "notInGame");
            run.ResumeAt = desk.Now.AddSeconds(2);
            return;
        }
        if (run.Pid != 0 && run.Pid != window.Pid)
        {
            Log.Note($"Macro {run.Tag}: its window changed from {run.Pid} to {window.Pid}; it plays again from the top.");
            StartOver(run);
        }
        else if (run.Pid == 0 && !run.Quiet)
            Log.Note($"Macro {run.Tag}: first turn, window {window.Pid} ({window.Client.Width}x{window.Client.Height}, fps limit {window.Fps}).");
        run.Pid = window.Pid;
        var ct = run.Cancel.Token;
        IDisposable? lease = InputTurn.TryEnter("macro");
        if (lease is null)
        {
            Set(run, "paused", "busy");
            lease = await InputTurn.EnterAsync("macro", ct);
        }
        var turn = new Turn(this, run, window);
        _turn = turn;
        try
        {
            Set(run, "running", run.Recovering ? "recovering" : Ime(run) ? "imeOn" : null);
            // Keys it held when its last turn ended (let go then) are held again before anything counts: a wait with W
            // held is a walk, not standing still.
            if (run.Held.Count > 0 && await turn.FrontAsync(ct) is null)
                return;
            var started = desk.Now;
            for (int steps = 0; ; steps++)
            {
                ct.ThrowIfCancellationRequested();
                if (run.EndsAt <= desk.Now)
                {
                    Finish(run, "stopped", "maxTime"); // its longest time, even in the middle of a turn
                    return;
                }
                if (run.Section is not null && run.Pc >= run.Steps.Count)
                {
                    FeatureEnded(run, null);
                    continue;
                }
                if (run.Pc >= (run.Recovering || run.RecoverAt < 0 ? run.Macro.Steps.Count : run.RecoverAt))
                {
                    if (run.Recovering)
                    {
                        // Unstuck: from the top.
                        run.Recovering = false;
                        run.Pc = 0;
                        Set(run, "running", null);
                        continue;
                    }
                    // The end of the main steps: the features on play first.
                    if (SafePoint(run))
                        continue;
                    Finish(run, "done");
                    return;
                }
                if (Interrupted(turn) is not null)
                    return;
                // A feature switched off on the panel: it stops here, letting go of what it holds.
                if (run.Feature is { } feature && !SwitchOn(run.Macro, feature))
                {
                    turn.LetGo();
                    FeatureEnded(run, null);
                    continue;
                }
                MacroStep step;
                try
                {
                    run.Owner = run.Steps[run.Pc].Owner;
                    step = MacroValues.Resolve(run.Steps[run.Pc], name => Value(run, name));
                }
                catch (MacroValueMissing)
                {
                    Finish(run, "failed", "missingValue");
                    return;
                }
                Then then;
                run.StepsRun++;
                try
                {
                    then = await StepAsync(turn, step, ct);
                }
                catch (MacroValueMissing)
                {
                    Finish(run, "failed", "missingValue");
                    return;
                }
                catch (MacroExprError)
                {
                    Finish(run, "failed", "badFormula");
                    return;
                }
                catch (MacroCovered)
                {
                    // Another app's window over the place: no click into it. Tried again in a moment.
                    Set(run, "paused", "covered");
                    run.ResumeAt = desk.Now.AddSeconds(2);
                    return;
                }
                if (then == Then.Over || run.Finished)
                    return;
                if (then == Then.EndTurn)
                    return;
                if (steps >= TurnSteps || (desk.Now - started >= TurnShare && run.Held.Count == 0 && OthersReady(run)))
                    return; // its share is used up: the others go, it carries on next time round
            }
        }
        catch (OperationCanceledException) when (run.Finished || ct.IsCancellationRequested)
        {
            // Stopped (by the person, a lock, the window going): what it holds is let go below.
        }
        finally
        {
            try
            {
                await turn.EndAsync();
            }
            finally
            {
                // Handed on even when giving the window back failed: every nudge, picture and hotkey waits for it.
                lease.Dispose();
                if (_turn == turn)
                    _turn = null;
            }
        }
    }

    /// <summary>
    /// Why the turn can't go on now: the run's longest time is up (<see cref="Then.Over"/>), the person touched the PC, the
    /// window left the front, or anti-AFK has waited for the PC while this turn ran past <see cref="AfkBudget"/>
    /// (<see cref="Then.EndTurn"/>: what it holds is let go as the turn ends, and held again next turn); null: it goes on.
    /// Asked before every step and inside every step that keeps the window a while.
    /// </summary>
    Then? Interrupted(Turn turn)
    {
        var run = turn.Run;
        if (run.EndsAt <= desk.Now)
        {
            Finish(run, "stopped", "maxTime");
            return Then.Over;
        }
        if (turn.Input is { } input)
        {
            if (input.PersonInput())
            {
                Set(run, "paused", "personActive");
                return Then.EndTurn;
            }
            if (!input.StillFront())
            {
                Set(run, "paused", "focusRefused");
                run.ResumeAt = desk.Now.AddSeconds(2);
                return Then.EndTurn;
            }
        }
        if (desk.Now - turn.Started >= AfkBudget && InputTurn.Waits("afk"))
            return EndNow(run); // the nudge goes, then this run carries on where it was
        return null;
    }

    /// <summary>
    /// Its window was replaced since its last turn (a rejoin, a crash: another process): a new server is a new start, so
    /// it plays from the top, holding nothing (what it held went with the old window, let go as that turn ended).
    /// </summary>
    static void StartOver(MacroRun run)
    {
        LeaveFeatures(run);
        run.Held.Clear();
        run.HeldAs.Clear();
        run.Pc = 0;
        run.Passes.Clear();
        run.Again = -1;
        run.WaitLeft = null;
        run.Deadline = null;
        run.LastFrame = null;
        run.StillSince = null;
        run.MissSince = null;
        run.Recovering = false;
        run.Restarts = 0;
    }

    /// <summary>Back to the main steps, with no feature due (a restart, a new window).</summary>
    static void LeaveFeatures(MacroRun run)
    {
        run.Section = null;
        run.Due.Clear();
        run.Resumed = false;
        run.MainPasses = null;
    }

    internal static bool SwitchOn(Macro m, MacroFeature f) =>
        m.Options.FirstOrDefault(o => o.Name == f.Option) is { } o && MacroOptions.ValueOf(o) is { ValueKind: JsonValueKind.Number } v && v.GetDouble() != 0;

    FeatureRecord RecordOf(MacroRun run, string feature)
    {
        var key = (run.Macro.Id, run.AccountId, feature);
        return Features.TryGetValue(key, out var record) ? record : Features[key] = new();
    }

    /// <summary>A feature due now: switched on, and not played within its <see cref="MacroFeature.EveryMs"/>.</summary>
    bool Due(MacroRun run, MacroFeature f) => SwitchOn(run.Macro, f)
        && (f.EveryMs is not { } every || RecordOf(run, f.Id).LastRan is not { } last || desk.Now - last >= TimeSpan.FromMilliseconds(every));

    /// <summary>
    /// A safe point of the main steps (their end, or the end of a pass of a repeat inside no other block): the features
    /// due play now, whole, in the list's order, then the main steps go on from here. Not while a key is held (they would
    /// play with it down), nor in the If stuck part. True when one starts.
    /// </summary>
    bool SafePoint(MacroRun run)
    {
        if (run.Resumed)
        {
            run.Resumed = false; // back from them: this step plays on
            return false;
        }
        if (run.Recovering || run.Held.Count > 0 || run.Macro.Features.Count == 0)
            return false;
        foreach (var f in run.Macro.Features.Where(f => Due(run, f)))
            run.Due.Enqueue(f.Id);
        if (!run.Due.TryDequeue(out var first))
            return false;
        run.ReturnTo = run.Pc;
        run.MainPasses = new(run.Passes);
        StartFeature(run, first);
        return true;
    }

    void StartFeature(MacroRun run, string id)
    {
        run.Section = id;
        run.Pc = 0;
        run.Passes.Clear();
        run.Again = -1;
        ClearWait(run);
        Changed?.Invoke();
    }

    /// <summary>A feature played through, switched off, or skipped (<paramref name="failed"/>: why, at the step it
    /// stopped): the next one due plays, else the main steps go on where they were.</summary>
    void FeatureEnded(MacroRun run, string? failed)
    {
        var record = RecordOf(run, run.Section!);
        record.LastRan = desk.Now;
        record.Failed = failed;
        record.FailedStep = failed is null ? 0 : run.Pc;
        if (run.Due.TryDequeue(out var next))
        {
            StartFeature(run, next);
            return;
        }
        run.Section = null;
        run.Pc = run.ReturnTo;
        run.Passes.Clear();
        foreach (var (at, passes) in run.MainPasses ?? [])
            run.Passes[at] = passes;
        run.MainPasses = null;
        run.Again = -1;
        ClearWait(run);
        run.Resumed = true;
        Changed?.Invoke();
    }

    static void ClearWait(MacroRun run)
    {
        run.WaitLeft = null;
        run.Deadline = null;
        run.LastFrame = null;
        run.StillSince = null;
    }

    bool Ime(MacroRun run) =>
        MacroRules.AllSteps(run.Macro).Any(s => s.Kind == "type" || s.Kind is "key" or "down" && MacroKeys.TelexRewrites(s.Key ?? "")) && desk.ImeOn();

    /// <summary>A window's turn: brought forward only once there is input for it.</summary>
    sealed class Turn(MacroPlayer player, MacroRun run, MacroWindow window)
    {
        public IMacroTurn? Input { get; private set; }
        public MacroWindow Window => window;
        public MacroRun Run => run;
        public DateTime Started { get; } = player.Desk.Now;
        /// <summary>The picture for this step (taken once per look).</summary>
        Bitmap? _picture;

        /// <summary>The window in front, or null when Windows wouldn't (the run waits and tries again).</summary>
        public async Task<IMacroTurn?> FrontAsync(CancellationToken ct)
        {
            if (Input is not null)
                return Input;
            ct.ThrowIfCancellationRequested(); // stopped (Bloxkeep closing): no window comes forward
            var input = await player.Desk.BeginTurnAsync(window, ct);
            if (input is null)
            {
                run.FocusMisses++;
                player.Set(run, "paused", "focusRefused");
                // 2 s, 4 s… up to 30 s between tries.
                run.ResumeAt = player.Desk.Now.AddSeconds(Math.Min(30, 2 << Math.Min(run.FocusMisses - 1, 4)));
                return null;
            }
            run.FocusMisses = 0;
            Input = input;
            // Keys and buttons held when its last turn ended are held again.
            foreach (string held in run.Held)
            {
                if (MacroKeys.All.TryGetValue(held, out var key))
                    input.Key(key, up: false);
                else
                    input.Button(MacroRules.MouseKeys.GetValueOrDefault(held, held), up: false);
            }
            return input;
        }

        public async Task<Bitmap?> PictureAsync()
        {
            _picture?.Dispose();
            _picture = await player.Desk.PictureAsync(window, Input);
            if (_picture is null)
                NoteOnce(run, "picture", $"couldn't take a picture of window {window.Pid} at step {run.ShownPc + 1}.");
            return _picture;
        }

        /// <summary>Lets go of everything the run holds, now (a restart carries nothing over).</summary>
        public void LetGo()
        {
            if (Input is { } input)
            {
                foreach (string held in run.Held)
                {
                    if (MacroKeys.All.TryGetValue(held, out var key))
                        input.Key(key, up: true);
                    else
                        input.Button(MacroRules.MouseKeys.GetValueOrDefault(held, held), up: true);
                }
            }
            run.Held.Clear();
            run.HeldAs.Clear();
        }

        /// <summary>Everything let go and the PC given back now, with no wait (<see cref="ReleaseAll"/>).</summary>
        public void Abort()
        {
            if (Input is not { } input)
                return;
            try
            {
                LetGo();
            }
            finally
            {
                Input = null;
                input.Abort(); // a press halfway, the pointer, the windows
            }
        }

        public async Task EndAsync()
        {
            _picture?.Dispose();
            _picture = null;
            if (Input is not { } input)
                return;
            Input = null;
            try
            {
                // Let go of everything: a key left down would go on "held" in whatever window comes in front.
                foreach (string held in run.Held)
                {
                    if (MacroKeys.All.TryGetValue(held, out var key))
                        input.Key(key, up: true);
                    else
                        input.Button(MacroRules.MouseKeys.GetValueOrDefault(held, held), up: true);
                }
                if (run.Finished)
                {
                    run.Held.Clear();
                    run.HeldAs.Clear();
                }
            }
            finally
            {
                await input.DisposeAsync();
                player.Played?.Invoke(run.AccountId, window.Pid);
            }
        }
    }

    int Jitter(int ms, int jitter) => jitter <= 0 ? ms : Math.Max(0, ms + Random.Next(-jitter, jitter + 1));

    /// <summary>A frame of the window's frame rate limit (60 fps when not known).</summary>
    static int Frame(MacroWindow w) => w.Fps > 0 ? (int)Math.Ceiling(1000.0 / w.Fps) : 17;

    /// <summary>
    /// How long a key or button is held: at least two frames (a game reads its input once a frame, and a press shorter
    /// than one can fall between two) and never under 60 ms.
    /// </summary>
    internal static int HoldFor(int ms, int fps) => Math.Max(ms, Math.Max(60, fps > 0 ? (int)Math.Ceiling(2000.0 / fps) : 0));

    async Task<Then> StepAsync(Turn turn, MacroStep s, CancellationToken ct)
    {
        var run = turn.Run;
        var m = run.Macro;
        int frame = Frame(turn.Window);
        switch (s.Kind)
        {
            case "key":
            {
                if (await turn.FrontAsync(ct) is not { } input)
                    return Then.EndTurn;
                var key = MacroKeys.All[s.Key!];
                input.Key(key, up: false);
                try
                {
                    await desk.Delay(HoldFor(Jitter(s.Ms ?? 60, m.JitterMs), turn.Window.Fps), ct);
                }
                finally
                {
                    input.Key(key, up: true);
                }
                await desk.Delay(frame, ct);
                break;
            }
            case "down" or "up":
            {
                if (await turn.FrontAsync(ct) is not { } input)
                    return Then.EndTurn;
                bool up = s.Kind == "up";
                // Release lets go of what its Hold pressed: a setting changed on the panel meanwhile names another key.
                string named = MacroRules.KeyNamed(run.Steps[run.Pc]), key = s.Key!;
                if (up && run.HeldAs.Remove(named, out var pressed))
                    key = pressed;
                if (MacroRules.MouseKeys.TryGetValue(key, out string? mouse))
                {
                    // A mouse button held where the pointer is: it must be the game's window there.
                    if (!up && !input.Clear(input.CursorAt()))
                        throw new MacroCovered();
                    if (up ? run.Held.Remove(key) : run.Held.Add(key))
                        input.Button(mouse, up);
                }
                else if (up ? run.Held.Remove(key) : run.Held.Add(key))
                    input.Key(MacroKeys.All[key], up);
                if (!up)
                    run.HeldAs[named] = key;
                await desk.Delay(frame, ct);
                break;
            }
            case "click":
            {
                if (await turn.FrontAsync(ct) is not { } input)
                    return Then.EndTurn;
                await ClickAsync(input, turn, MacroGeometry.ClientPoint(input.Client, s.X!.Value, s.Y!.Value), s.Button ?? "left", s.Double == true, ct, s.HoldMs ?? 0);
                break;
            }
            case "clickImage":
            {
                run.Deadline ??= desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
                var found = await FindImageAsync(turn, s.Image!, s.Region, s.Threshold ?? 0.9);
                if (found is null)
                    return await NotYetAsync(turn, "notFound", LookEvery, ct);
                if (await turn.FrontAsync(ct) is not { } input)
                    return Then.EndTurn;
                var (rect, w, h) = found.Value;
                var client = input.Client;
                // The picture's pixels to the client's (the same unless the window changed size since).
                var centre = new Point(client.X + (int)((rect.X + rect.Width / 2.0) * client.Width / w), client.Y + (int)((rect.Y + rect.Height / 2.0) * client.Height / h));
                run.Deadline = null;
                await ClickAsync(input, turn, centre, s.Button ?? "left", false, ct);
                break;
            }
            case "camera":
            {
                if (await turn.FrontAsync(ct) is not { } input)
                    return Then.EndTurn;
                await CameraAsync(input, turn, s.Dx ?? 0, s.Dy ?? 0, s.Ms ?? 200, s.Button == "right", ct);
                break;
            }
            case "scroll":
            {
                if (await turn.FrontAsync(ct) is not { } input)
                    return Then.EndTurn;
                var centre = MacroGeometry.ClientPoint(input.Client, 0.5, 0.5);
                if (!input.Clear(centre))
                    throw new MacroCovered();
                input.MoveTo(centre);
                await desk.Delay(frame, ct);
                int notches = s.Notches ?? 0;
                for (int i = 0; i < Math.Abs(notches); i++)
                {
                    input.Wheel(Math.Sign(notches));
                    await desk.Delay(Math.Max(40, frame), ct);
                }
                break;
            }
            case "move":
            {
                if (await turn.FrontAsync(ct) is not { } input)
                    return Then.EndTurn;
                input.MoveTo(Offset(MacroGeometry.ClientPoint(input.Client, s.X!.Value, s.Y!.Value), m.JitterPx));
                await desk.Delay(frame, ct);
                break;
            }
            case "wait":
            {
                // What is left of one cut short, else the whole of it.
                int ms = run.WaitLeft ?? Jitter(s.Ms ?? 0, m.JitterMs);
                run.WaitLeft = null;
                // A long wait gives the window back, unless keys are held (walking for ten seconds stays in front).
                if (ms > LongWait.TotalMilliseconds && run.Held.Count == 0)
                {
                    run.Pc++;
                    run.ResumeAt = desk.Now.AddMilliseconds(ms);
                    Set(run, "waiting", null);
                    return Then.EndTurn;
                }
                // In front: in pieces, each asking whether the turn may go on. Cut short, the rest is waited next turn,
                // with the keys held again first.
                var until = desk.Now.AddMilliseconds(ms);
                while (desk.Now < until)
                {
                    if (Interrupted(turn) is { } then)
                    {
                        if (then != Then.Over)
                            run.WaitLeft = (int)Math.Ceiling((until - desk.Now).TotalMilliseconds);
                        return then;
                    }
                    await desk.Delay((int)Math.Min(WaitSlice, Math.Ceiling((until - desk.Now).TotalMilliseconds)), ct);
                }
                run.Pc++;
                return Then.Next;
            }
            case "waitImage":
            {
                run.Deadline ??= desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
                bool seen = await FindImageAsync(turn, s.Image!, s.Region, s.Threshold ?? 0.9) is not null;
                if (seen == (s.Gone == true))
                    return await NotYetAsync(turn, "notFound", LookEvery, ct);
                break;
            }
            case "waitText":
            {
                run.Deadline ??= desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
                bool seen = await FindTextAsync(turn, s.Text, s.Words, s.Extra, s.Region) is not null;
                if (seen == (s.Gone == true))
                    return await NotYetAsync(turn, "timeout", ReadEvery, ct);
                break;
            }
            case "waitColour":
            {
                run.Deadline ??= desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
                bool seen = await turn.PictureAsync() is { } picture && (s.Region is { } area
                    ? MacroFollow.Blobs(MacroFollow.Pixels.Of(picture, MacroVision.Area(area, picture.Width, picture.Height)), s.Colour!, s.Tolerance ?? 24, 2).Count > 0
                    : MacroVision.ColourAt(picture, s.X!.Value, s.Y!.Value, s.Colour!, s.Tolerance ?? 24));
                if (seen == (s.Gone == true))
                    return await NotYetAsync(turn, "timeout", LookEvery, ct);
                break;
            }
            case "waitStill":
            {
                run.Deadline ??= desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
                if (await turn.PictureAsync() is { } picture)
                {
                    var gray = MacroVision.ToGray(picture);
                    var frameNow = MacroVision.Signature(gray, MacroVision.Area(s.Region, gray.W, gray.H));
                    if (run.LastFrame is { } last && MacroVision.Difference(last, frameNow) <= MacroVision.StillDifference)
                        run.StillSince ??= desk.Now;
                    else
                        run.StillSince = null;
                    run.LastFrame = frameNow;
                    if (run.StillSince is { } since && desk.Now - since >= TimeSpan.FromMilliseconds(s.Ms ?? 500))
                    {
                        run.LastFrame = null;
                        run.StillSince = null;
                        break;
                    }
                }
                return await NotYetAsync(turn, "timeout", StillEvery, ct, keepFrame: true);
            }
            case "if":
            {
                bool yes = await TestAsync(turn, s.Test!);
                if (Stuck(run))
                    return await FailedAsync(turn, "stuck");
                // No: to the step after its else (or its end).
                run.Pc = yes ? run.Pc + 1 : run.JumpsNow[run.Pc] + 1;
                return Then.Next;
            }
            case "else":
                // The if's own steps are done: past its end.
                run.Pc = run.JumpsNow[run.Pc] + 1;
                return Then.Next;
            case "loop":
            {
                int start = run.Pc;
                if (run.Again != start)
                    run.Passes[start] = 0;
                run.Again = -1;
                if (s.Times is > 0 and var times && run.Passes[start] >= times)
                {
                    run.Passes.Remove(start);
                    run.Pc = run.JumpsNow[start] + 1;
                    return Then.Next;
                }
                break;
            }
            case "loopUntil":
            {
                int start = run.Pc;
                if (run.Again != start)
                    run.Passes[start] = 0;
                run.Again = -1;
                bool met = s.Max is > 0 and var max && run.Passes[start] >= max || await TestAsync(turn, s.Test!);
                if (Stuck(run))
                    return await FailedAsync(turn, "stuck");
                if (met)
                {
                    run.Passes.Remove(start);
                    run.Pc = run.JumpsNow[start] + 1;
                    return Then.Next;
                }
                break;
            }
            case "endLoop":
            {
                // The end of a pass of a repeat inside no other block: the features on play first.
                if (run.Section is null && run.SafeEnds.Contains(run.Pc) && SafePoint(run))
                    return Then.Next;
                int start = run.JumpsNow[run.Pc];
                run.Passes[start] = run.Passes.GetValueOrDefault(start) + 1;
                run.Loops++;
                run.Again = start;
                run.Pc = start;
                Changed?.Invoke();
                // Round the loop: a turn that has played a while lets the others go first.
                return desk.Now - turn.Started >= LongWait && run.Held.Count == 0 && OthersReady(run) ? EndNow(run) : Then.Next;
            }
            case "break":
            {
                int end = run.JumpsNow[run.Pc];
                run.Passes.Remove(run.JumpsNow[end]);
                run.Pc = end + 1;
                return Then.Next;
            }
            case "endIf":
                break;
            case "type":
            {
                if (await turn.FrontAsync(ct) is not { } input)
                    return Then.EndTurn;
                await TypeAsync(input, turn, AlertText(run, s.Text!), ct);
                break;
            }
            case "setVar" or "addVar":
            {
                if (s.Kind == "setVar" && s.Text is { } text)
                {
                    SetText(run, s.Name!, AlertText(run, text));
                    break;
                }
                double by = s.Expr is { } expr ? MacroExpr.Eval(expr, name => NumberOf(run, name)) : s.Value!.Value;
                SetNumber(run, s.Name!, s.Kind == "setVar" ? by : NumberOf(run, s.Name!) + by);
                break;
            }
            case "fail":
                Finish(run, "failed", s.Text ?? "missingMacro");
                return Then.Over;
            case "write":
            {
                string text = AlertText(run, s.Text!);
                try
                {
                    switch (s.To)
                    {
                        case "clipboard": await Outside.CopyAsync(text); break;
                        case "file": await Outside.AppendFileAsync(s.FileName!, text); break;
                        case "web":
                            if (await WebAsync(run, s.Url!, text, ct) is null)
                                NoteOnce(run, $"write{run.Section}:{run.Pc}", $"write step {run.ShownPc + 1}: the web address didn't take it; the macro goes on.");
                            break;
                    }
                }
                catch (Exception ex) when (ex is not (MacroExprError or OperationCanceledException))
                {
                    // A clipboard another app holds, a file open elsewhere: noted, and the macro goes on.
                    NoteOnce(run, $"write{run.Section}:{run.Pc}:{ex.GetType().Name}", $"write step {run.ShownPc + 1} to {s.To}: {ex.GetType().Name}; the macro goes on.");
                }
                break;
            }
            case "read":
            {
                run.Deadline ??= desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
                bool screen = s.From is null or "screen";
                string? words;
                try
                {
                    words = s.From switch
                    {
                        "clipboard" => await Outside.ClipboardAsync(),
                        "file" => await Outside.ReadFileAsync(s.FileName!),
                        "web" => await WebAsync(run, s.Url!, null, ct),
                        _ => await ReadAreaAsync(turn, s.Region),
                    };
                }
                catch (Exception ex) when (ex is not (MacroExprError or OperationCanceledException))
                {
                    NoteOnce(run, $"read{run.Section}:{run.Pc}:{ex.GetType().Name}", $"read step {run.ShownPc + 1} from {s.From}: {ex.GetType().Name}.");
                    words = null;
                }
                if (words is null && s.From == "file")
                    NoteOnce(run, $"nofile{run.Section}:{run.Pc}", $"read step {run.ShownPc + 1}: its file isn't in Documents\\Bloxkeep.");
                int again = screen ? ReadEvery : s.From == "web" ? 3000 : 1000;
                if (s.Mode == "text")
                {
                    if (string.IsNullOrWhiteSpace(words))
                        return await NotYetAsync(turn, "timeout", again, ct);
                    SetText(run, s.Name!, words.Length > 200 ? words[..200].Trim() : words.Trim());
                }
                else
                {
                    // Only the length: what was read may be a name.
                    if (words is { Length: > 0 } && MacroExpr.NumberIn(words) is null)
                        NoteOnce(run, $"nonumber{run.Section}:{run.Pc}", $"read step {run.ShownPc + 1} from {s.From ?? "screen"} read {words.Length} characters with no number in them.");
                    if (words is null || MacroExpr.NumberIn(words) is not { } n)
                        return await NotYetAsync(turn, "timeout", again, ct);
                    SetNumber(run, s.Name!, n);
                }
                break;
            }
            case "rejoin":
            {
                string? refused = Rejoin?.Invoke(run.AccountId);
                if (refused is not null)
                    Log.Note($"Macro {run.Tag}: its rejoin step {run.ShownPc + 1} was refused.");
                Finish(run, refused is null ? "done" : "failed");
                return Then.Over;
            }
            case "alert":
                Alert?.Invoke(run.AccountId, m, AlertText(run, s.Text!), s.Picture == true ? await PngAsync(turn) : null);
                break;
            case "clickColour":
                return await ClickColourAsync(turn, s, ct);
            case "follow":
                return await FollowAsync(turn, s, ct);
            case "stop":
                Finish(run, "done");
                return Then.Over;
        }
        if (s.Kind is "waitImage" or "waitText" or "waitColour" or "waitStill" or "clickImage" || s is { Kind: "read", From: null or "screen" })
            Seen(run);
        run.Pc++;
        return Then.Next;
    }

    /// <summary>Steps of a run with its Run a macro steps put in place: ten macros of the most steps.</summary>
    const int MaxExpanded = 10 * MacroRules.MaxSteps;

    /// <summary>
    /// Run a macro steps put in place: the other macro's own steps (not its If stuck part), its settings and areas read
    /// now. One that's gone, runs a macro already running here, or goes too deep becomes a step that fails the run.
    /// </summary>
    (List<MacroStep> Steps, int[] Origin) Expand(List<MacroStep> steps, List<Guid> chain)
    {
        var result = new List<MacroStep>();
        var origin = new List<int>();
        for (int i = 0; i < steps.Count; i++)
        {
            var inner = steps[i].Kind == "call" ? Inner(steps[i], chain) : [steps[i]];
            result.AddRange(inner);
            origin.AddRange(Enumerable.Repeat(i, inner.Count));
            if (result.Count > MaxExpanded)
                return ([Fail("missingMacro")], [i]);
        }
        return (result, origin.ToArray());

        static MacroStep Fail(string why) => new() { Kind = "fail", Text = why };

        List<MacroStep> Inner(MacroStep call, List<Guid> chain)
        {
            if (FindMacro?.Invoke(call.MacroId ?? Guid.Empty) is not { } callee || chain.Contains(callee.Id) || chain.Count > MacroRules.MaxCallDepth)
                return [Fail("missingMacro")];
            int end = callee.Steps.FindIndex(s => s.Kind == "recover");
            var own = new List<MacroStep>();
            foreach (var s in end < 0 ? callee.Steps : callee.Steps.Take(end))
            {
                var mine = s.Clone();
                mine.Owner = callee;
                own.Add(mine);
            }
            return Expand(own, [.. chain, callee.Id]).Steps;
        }
    }

    /// <summary>Ends the turn here; the run is due again at once (after the others).</summary>
    Then EndNow(MacroRun run)
    {
        run.ResumeAt = desk.Now;
        return Then.EndTurn;
    }

    /// <summary>
    /// A wait on the picture not met yet: looked at again in a moment (with keys held, without giving the window back),
    /// or, past its time, the macro's onFail.
    /// </summary>
    async Task<Then> NotYetAsync(Turn turn, string reason, int again, CancellationToken ct, bool keepFrame = false)
    {
        var run = turn.Run;
        if (Missed(run))
            reason = "stuck";
        if (reason == "stuck" || desk.Now >= run.Deadline)
        {
            run.Deadline = null;
            run.LastFrame = null;
            run.StillSince = null;
            return await FailedAsync(turn, reason);
        }
        if (!keepFrame)
            run.LastFrame = null;
        if (run.Held.Count > 0)
        {
            await desk.Delay(again, ct);
            return Then.Next; // the same step again
        }
        run.ResumeAt = desk.Now.AddMilliseconds(again);
        return Then.EndTurn;
    }

    /// <summary>
    /// A wait timed out, an image wasn't there, or it is stuck: the macro's onFail. In the If stuck part a step that
    /// finds nothing is passed over (it does what it can), unless that part is stuck too.
    /// </summary>
    async Task<Then> FailedAsync(Turn turn, string reason)
    {
        var run = turn.Run;
        var m = run.Macro;
        // Once per step and reason: a step that skips on fail would write one every pass.
        string kind = KindAt(run);
        NoteOnce(run, $"fail{run.Section}:{run.Pc}:{reason}", $"step {run.ShownPc + 1} ({kind}{(run.Section is { } f ? $", feature {f}" : "")}) failed ({reason})"
            + (kind is "clickImage" or "waitImage" && run.LastScore is { } score ? $", best picture match {score:0.00}" : "")
            + (run.Recovering ? " in its If stuck part" : "") + $"; on fail: {(run.Feature is { OnFail: "skip" } ? "skip the feature" : m.OnFail)}, restarts {run.Restarts}.");
        if (run.Recovering && reason != "stuck")
        {
            run.Pc++;
            return Then.Next;
        }
        // A feature that skips: not this time, back to the main steps (it plays again when it is due).
        if (run.Feature is { OnFail: "skip" } && reason != "stuck")
        {
            turn.LetGo();
            FeatureEnded(run, reason);
            return Then.Next;
        }
        switch (m.OnFail)
        {
            case "skip" when reason != "stuck":
                run.Pc++;
                return Then.Next;
            case "skip" or "restart":
                return Restart(turn);
            case "rejoin":
                if (Rejoin?.Invoke(run.AccountId) is not null)
                    Log.Note($"Macro {run.Tag}: its rejoin on fail was refused.");
                break;
            case "alert":
                Alert?.Invoke(run.AccountId, m, reason switch
                {
                    "stuck" => L.F("Macro {0} stopped: it found nothing for {1} minutes.", m.Name, m.StuckMinutes ?? 0),
                    "notFound" => L.F("Macro {0} stopped: step {1} didn't find its picture.", m.Name, run.Pc + 1),
                    _ => L.F("Macro {0} stopped: step {1} waited too long.", m.Name, run.Pc + 1),
                }, await PngAsync(turn));
                break;
        }
        Finish(run, "failed", reason);
        return Then.Over;
    }

    /// <summary>Back to the top, through the If stuck part first when there is one. After <see cref="MaxRestarts"/> in a
    /// row with nothing seen in between, it gives up.</summary>
    Then Restart(Turn turn)
    {
        var run = turn.Run;
        if (++run.Restarts > MaxRestarts)
        {
            Finish(run, "failed", "stuck");
            return Then.Over;
        }
        turn.LetGo();
        LeaveFeatures(run);
        run.Passes.Clear();
        run.Again = -1;
        run.WaitLeft = null;
        run.Deadline = null;
        run.LastFrame = null;
        run.StillSince = null;
        run.MissSince = null;
        run.Recovering = run.RecoverAt >= 0;
        run.Pc = run.RecoverAt + 1; // 0 without one
        Set(run, "running", run.Recovering ? "recovering" : null);
        return EndNow(run);
    }

    /// <summary>What its steps look for was seen: not stuck.</summary>
    static void Seen(MacroRun run)
    {
        run.MissSince = null;
        if (!run.Recovering)
            run.Restarts = 0;
    }

    /// <summary>What a step looks for wasn't seen; true when that makes it stuck.</summary>
    bool Missed(MacroRun run)
    {
        run.MissSince ??= desk.Now;
        return Stuck(run);
    }

    bool Stuck(MacroRun run) => run.Macro.StuckMinutes is > 0 and var minutes && run.MissSince is { } since
        && desk.Now - since >= TimeSpan.FromMinutes(minutes);

    /// <summary>The window's picture as PNG, for an alert; null when it can't be pictured.</summary>
    async Task<byte[]?> PngAsync(Turn turn)
    {
        if (await turn.PictureAsync() is not { } picture)
            return null;
        using var stream = new MemoryStream();
        picture.Save(stream, ImageFormat.Png);
        return stream.ToArray();
    }

    async Task<(Rectangle Found, int W, int H)?> FindImageAsync(Turn turn, string image, MacroRect? region, double threshold)
    {
        if (await turn.PictureAsync() is not { } picture)
            return null;
        // Cut from a window of another width: scaled to this one's.
        double scale = turn.Run.Macro.RecordedSize is { W: > 0 } size ? (double)picture.Width / size.W : 1;
        var gray = MacroVision.ToGray(picture);
        var result = await Task.Run(() => MacroVision.Template(image, scale) is { } template
            ? MacroVision.Find(gray, template, MacroVision.Area(region, gray.W, gray.H)) : null);
        turn.Run.LastScore = result?.Score;
        return result is { } r && r.Score >= threshold ? (r.Found, picture.Width, picture.Height) : null;
    }

    /// <summary>The text read in the area, or any of the words (a list the player ticks: the whole list, from its
    /// setting, keeps a longer name read from counting as a shorter one).</summary>
    async Task<RectangleF?> FindTextAsync(Turn turn, string? text, List<string>? words, Dictionary<string, JsonElement>? extra, MacroRect? region)
    {
        if (await turn.PictureAsync() is not { } picture)
            return null;
        var read = await desk.ReadAsync(picture, MacroVision.Area(region, picture.Width, picture.Height));
        if (read is null)
        {
            NoteOnce(turn.Run, "ocr", $"step {turn.Run.ShownPc + 1} looks for text, but Windows can't read text on this PC.");
            return null;
        }
        var all = MacroValues.RefOf(extra, "words") is { } name && turn.Run.Macro.Options.FirstOrDefault(o => o.Name == name)?.Choices is { } list
            ? list : new List<string>();
        foreach (var lines in read)
        {
            if ((words is not null ? MacroOcr.LocateAny(lines, words, all)?.Box : MacroOcr.Locate(lines, text!)) is { } at)
                return at;
        }
        return null;
    }

    static bool SetsVar(Macro m, string name) => MacroRules.AllSteps(m).Any(s => s.Kind is "setVar" or "addVar" or "read" && s.Name == name);

    /// <summary>A web step: at most one a second for each run.</summary>
    async Task<string?> WebAsync(MacroRun run, string url, string? post, CancellationToken ct)
    {
        var wait = run.LastWebAt + TimeSpan.FromSeconds(1) - desk.Now;
        if (wait > TimeSpan.Zero)
            await desk.Delay((int)wait.TotalMilliseconds, ct);
        run.LastWebAt = desk.Now;
        return await Outside.WebAsync(url, post, ct);
    }

    /// <summary>The words read in the area, its lines joined by spaces; null when Windows can't read text here.</summary>
    async Task<string?> ReadAreaAsync(Turn turn, MacroRect? region)
    {
        if (await turn.PictureAsync() is not { } picture)
            return null;
        var read = await desk.ReadAsync(picture, MacroVision.Area(region, picture.Width, picture.Height));
        if (read is null)
            NoteOnce(turn.Run, "ocr", $"step {turn.Run.ShownPc + 1} reads text, but Windows can't read text on this PC.");
        return read?.FirstOrDefault(lines => lines.Count > 0) is { } found ? string.Join(" ", found.Select(l => l.Text)).Trim() : null;
    }

    /// <summary>A name's number for a formula: a variable the macro sets itself is 0 before it is first set; a text that
    /// holds a number counts as that number.</summary>
    double NumberOf(MacroRun run, string name)
    {
        if (Value(run, name) is not { } v)
            return SetsVar(run.Macro, name) ? 0 : throw new MacroValueMissing(name);
        return v.ValueKind switch
        {
            JsonValueKind.Number => v.GetDouble(),
            JsonValueKind.True => 1,
            JsonValueKind.False => 0,
            JsonValueKind.String when MacroExpr.NumberIn(v.GetString()!) is { } n => n,
            _ => throw new MacroExprError(L.F("“{0}” isn't a number.", name)),
        };
    }

    void SetNumber(MacroRun run, string name, double value)
    {
        (Vars.TryGetValue(run.AccountId, out var v) ? v : Vars[run.AccountId] = new())[name] = Math.Round(value, 9);
        Texts.GetValueOrDefault(run.AccountId)?.Remove(name);
        Changed?.Invoke();
    }

    void SetText(MacroRun run, string name, string value)
    {
        (Texts.TryGetValue(run.AccountId, out var t) ? t : Texts[run.AccountId] = new())[name] = value;
        Vars.GetValueOrDefault(run.AccountId)?.Remove(name);
        Changed?.Invoke();
    }

    /// <summary>An if's or loopUntil's question, on the window's picture now or the account's variables.</summary>
    async Task<bool> TestAsync(Turn turn, MacroTest t)
    {
        bool yes = t.Kind switch
        {
            "image" => await FindImageAsync(turn, t.Image!, t.Region, t.Threshold ?? 0.9) is not null,
            "text" => await FindTextAsync(turn, t.Text, t.Words, t.Extra, t.Region) is not null,
            "colour" => await turn.PictureAsync() is { } picture && MacroVision.ColourAt(picture, t.X!.Value, t.Y!.Value, t.Colour!, t.Tolerance ?? 24),
            "var" => MacroValues.VarTest(t, name => Value(turn.Run, name), SetsVar(turn.Run.Macro, t.Name!)),
            _ => false,
        };
        // A question isn't an expectation: a "no" isn't counted towards stuck, a "seen" is progress.
        if (yes && t.Kind != "var")
            Seen(turn.Run);
        return t.Kind == "var" ? yes : yes != (t.Not == true);
    }

    /// <summary>A value a run reads by name: the account's variable first, then the macro's option ("o1"); an area
    /// by "@a1".</summary>
    internal JsonElement? Value(MacroRun run, string name) =>
        Texts.GetValueOrDefault(run.AccountId)?.TryGetValue(name, out var text) == true ? JsonSerializer.SerializeToElement(text)
            : Value(run.Owner ?? run.Macro, Vars.GetValueOrDefault(run.AccountId), name);

    internal static JsonElement? Value(Macro macro, Dictionary<string, double>? vars, string name)
    {
        if (name is ['@', .. var area])
            return macro.Areas?.FirstOrDefault(a => a.Name == area) is { } found ? JsonSerializer.SerializeToElement(found.Rect, MacroRules.PageJson) : null;
        if (vars is not null && vars.TryGetValue(name, out double v))
            return JsonSerializer.SerializeToElement(v);
        return macro.Options?.FirstOrDefault(o => o.Name == name) is { } option ? MacroOptions.ValueOf(option) : null;
    }

    /// <summary>An alert's text with "{name}" put in: a variable or a setting (a counter's number, a choice's word).</summary>
    internal string AlertText(MacroRun run, string text) => Regex.Replace(text, @"\{([A-Za-z_][A-Za-z0-9_]{0,31})\}", match =>
        Value(run, match.Groups[1].Value) is not { } v ? match.Value
            : v.ValueKind == JsonValueKind.Number ? v.GetDouble().ToString("0.##", System.Globalization.CultureInfo.InvariantCulture)
            : v.ValueKind == JsonValueKind.String ? v.GetString()! : match.Value);

    internal static bool Compare(double a, string op, double b) => op switch
    {
        "<" => a < b,
        "<=" => a <= b,
        "=" => Math.Abs(a - b) < 1e-9,
        ">=" => a >= b,
        ">" => a > b,
        "!=" => Math.Abs(a - b) >= 1e-9,
        _ => false,
    };

    /// <summary>A real-time step keeps the window in front as long as it runs: the others wait meanwhile.</summary>
    void OthersWait(MacroRun run)
    {
        foreach (var other in _runs.Where(r => r != run && !r.Finished && r.Status != "paused"))
            Set(other, "waiting", "busy");
        Changed?.Invoke();
    }

    /// <summary>
    /// clickColour: a patch of the colour anywhere in the area is clicked in its middle; with repeatUntilGoneMs, again
    /// each time one shows up, until none has shown for that long (buttons popping up at random places). With
    /// eachAfterMs, every patch is clicked once, when it has shown that long (rings to hit as they come).
    /// </summary>
    async Task<Then> ClickColourAsync(Turn turn, MacroStep s, CancellationToken ct)
    {
        if (s.EachAfterMs is { } after)
            return await ClickEachAsync(turn, s, after, ct);
        var run = turn.Run;
        run.Deadline ??= desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
        int every = Math.Max(Frame(turn.Window) * 2, 50);
        bool repeat = s.RepeatUntilGoneMs is > 0;
        bool clicked = false;
        DateTime lastSeen = desk.Now;
        var gaveUpAt = GiveUpAt(s);
        if (repeat)
            OthersWait(run);
        while (true)
        {
            ct.ThrowIfCancellationRequested();
            if (Interrupted(turn) is { } stop)
                return stop;
            if (desk.Now >= gaveUpAt)
            {
                run.Deadline = null;
                return await FailedAsync(turn, "timeout");
            }
            Rectangle? hit = null;
            int w = 0, h = 0;
            if (await turn.PictureAsync() is { } picture)
            {
                (w, h) = (picture.Width, picture.Height);
                var area = MacroVision.Area(s.Region, w, h);
                if (MacroFollow.Blobs(MacroFollow.Pixels.Of(picture, area), s.Colour!, s.Tolerance ?? 24, s.MinSize ?? 4) is [var first, ..])
                    hit = first with { X = first.X + area.X, Y = first.Y + area.Y };
            }
            if (hit is { } box)
            {
                if (await turn.FrontAsync(ct) is not { } input)
                    return Then.EndTurn;
                var client = input.Client;
                var centre = new Point(client.X + (int)((box.X + box.Width / 2.0) * client.Width / w), client.Y + (int)((box.Y + box.Height / 2.0) * client.Height / h));
                await ClickAsync(input, turn, centre, s.Button ?? "left", false, ct);
                clicked = true;
                lastSeen = desk.Now;
                run.Deadline = desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
                if (!repeat)
                    break;
            }
            else if (clicked && desk.Now - lastSeen >= TimeSpan.FromMilliseconds(s.RepeatUntilGoneMs ?? 0))
                break; // it stopped coming back
            else if (!clicked)
            {
                if (!repeat)
                    return await NotYetAsync(turn, "notFound", LookEvery, ct);
                if (Missed(run) || desk.Now >= run.Deadline)
                {
                    run.Deadline = null;
                    return await FailedAsync(turn, Stuck(run) ? "stuck" : "notFound");
                }
            }
            await desk.Delay(every, ct);
        }
        run.Deadline = null;
        Seen(run);
        run.Pc++;
        return Then.Next;
    }

    /// <summary>When a real-time step that clicks as things come gives up: <see cref="RealTimeCap"/> after it began, or its
    /// timeout when that is longer.</summary>
    DateTime GiveUpAt(MacroStep s) => desk.Now + TimeSpan.FromMilliseconds(Math.Max(s.TimeoutMs ?? 10_000, RealTimeCap.TotalMilliseconds));

    /// <summary>A patch being watched: where its middle is (picture pixels), since when, and whether it was clicked.</summary>
    sealed record Spot(PointF Centre, float Size, DateTime Since, bool Clicked);

    /// <summary>
    /// clickColour with eachAfterMs: the patches are followed from picture to picture (one counts as the same while its
    /// middle stays within its size); each is clicked once, when it has shown that long, and forgotten when gone. Ends
    /// when nothing new has come for repeatUntilGoneMs after a click, or at the timeout with none clicked.
    /// </summary>
    async Task<Then> ClickEachAsync(Turn turn, MacroStep s, int after, CancellationToken ct)
    {
        var run = turn.Run;
        run.Deadline ??= desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
        int every = Math.Max(Frame(turn.Window) * 2, 30);
        var spots = new List<Spot>();
        bool clicked = false;
        DateTime lastNew = desk.Now;
        var gaveUpAt = GiveUpAt(s);
        OthersWait(run);
        while (true)
        {
            ct.ThrowIfCancellationRequested();
            if (Interrupted(turn) is { } stop)
                return stop;
            if (desk.Now >= gaveUpAt)
            {
                run.Deadline = null;
                return await FailedAsync(turn, "timeout");
            }
            if (await turn.PictureAsync() is { } picture)
            {
                int w = picture.Width, h = picture.Height;
                var area = MacroVision.Area(s.Region, w, h);
                var now = new List<Spot>();
                var taken = new HashSet<Spot>(ReferenceEqualityComparer.Instance);
                foreach (var b in MacroFollow.Blobs(MacroFollow.Pixels.Of(picture, area), s.Colour!, s.Tolerance ?? 24, s.MinSize ?? 4))
                {
                    var centre = new PointF(area.X + b.X + b.Width / 2f, area.Y + b.Y + b.Height / 2f);
                    float size = Math.Max(Math.Max(b.Width, b.Height), 8);
                    var same = spots.FirstOrDefault(p => !taken.Contains(p) && Math.Abs(p.Centre.X - centre.X) <= Math.Max(size, p.Size) && Math.Abs(p.Centre.Y - centre.Y) <= Math.Max(size, p.Size));
                    if (same is null)
                        lastNew = desk.Now;
                    else
                        taken.Add(same);
                    now.Add(same is null ? new Spot(centre, size, desk.Now, false) : same with { Centre = centre, Size = size });
                }
                spots = now; // the ones gone are forgotten
                for (int k = 0; k < spots.Count; k++)
                {
                    if (spots[k].Clicked || desk.Now - spots[k].Since < TimeSpan.FromMilliseconds(after))
                        continue;
                    if (await turn.FrontAsync(ct) is not { } input)
                        return Then.EndTurn;
                    var client = input.Client;
                    var at = new Point(client.X + (int)(spots[k].Centre.X * client.Width / w), client.Y + (int)(spots[k].Centre.Y * client.Height / h));
                    await ClickAsync(input, turn, at, s.Button ?? "left", false, ct);
                    spots[k] = spots[k] with { Clicked = true };
                    clicked = true;
                    lastNew = desk.Now;
                    run.Deadline = desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
                }
            }
            if (clicked && desk.Now - lastNew >= TimeSpan.FromMilliseconds(s.RepeatUntilGoneMs ?? 1000) && spots.All(p => p.Clicked))
                break; // none has come for a while
            if (!clicked && (Missed(run) || desk.Now >= run.Deadline))
            {
                run.Deadline = null;
                return await FailedAsync(turn, Stuck(run) ? "stuck" : "notFound");
            }
            await desk.Delay(every, ct);
        }
        run.Deadline = null;
        Seen(run);
        run.Pc++;
        return Then.Next;
    }

    /// <summary>
    /// follow: keeps a marker on a moving target in a bar (a reel, a charge), looking at the area many times a second
    /// with the window in front the whole time. On/off holds the button while the marker is behind the target; smooth
    /// presses in short pulses sized by distance and speed. Ends when the target has been gone for untilGoneMs, or at
    /// its timeout (then the macro's onFail). Whatever it holds is let go at the end.
    /// </summary>
    async Task<Then> FollowAsync(Turn turn, MacroStep s, CancellationToken ct)
    {
        var run = turn.Run;
        if (await turn.FrontAsync(ct) is not { } input)
            return Then.EndTurn;
        var marker = MacroFollow.Marker(s.Marker, name => Value(run, name));
        OthersWait(run);
        string button = s.Button ?? "left";
        MacroKey? key = button == "key" ? MacroKeys.All[s.Key!] : null;
        if (key is null)
        {
            // The press goes where the pointer is: the middle of the bar's area, inside the game.
            var client = input.Client;
            var area = MacroVision.Area(s.Region, client.Width, client.Height);
            var at = new Point(client.X + area.X + area.Width / 2, client.Y + area.Y + area.Height / 2);
            if (!input.Clear(at))
                throw new MacroCovered();
            input.MoveTo(at);
        }
        void Press(bool down)
        {
            if (key is { } k)
                input.Key(k, up: !down);
            else
                input.Button(button, up: !down);
        }
        int frameMs = Math.Max(4, 1000 / Math.Clamp(s.Fps ?? 60, 15, 240));
        var deadline = desk.Now.AddMilliseconds(s.TimeoutMs ?? 10_000);
        DateTime? goneSince = null;
        bool seenOnce = false, held = false;
        var pd = new MacroFollow.Pd(s.Kp ?? 1.0, s.Kd ?? 0.4, 1);
        var predictor = new MacroFollow.Predictor();
        try
        {
            while (true)
            {
                ct.ThrowIfCancellationRequested();
                int pulsed = 0;
                // The person, the window leaving the front, anti-AFK's turn: the step starts over next turn. Its longest time: over.
                if (Interrupted(turn) is { } stop)
                    return stop;
                var client = input.Client;
                var area = MacroVision.Area(s.Region, client.Width, client.Height);
                using var shot = await desk.AreaAsync(turn.Window, input, area);
                var sight = shot is null ? default : MacroFollow.Look(MacroFollow.Pixels.Of(shot), s.Target!, marker, s.Tolerance ?? 24);
                if (sight.Target is not { } target)
                {
                    goneSince ??= desk.Now;
                    if (held)
                    {
                        Press(false);
                        held = false;
                    }
                    // Gone after it was there: the minigame is over. Never there: still waiting for it, up to the timeout.
                    if (seenOnce && desk.Now - goneSince >= TimeSpan.FromMilliseconds(s.UntilGoneMs ?? 1000))
                        break;
                }
                else
                {
                    seenOnce = true;
                    goneSince = null;
                    if (sight.Marker is { } at)
                    {
                        double ahead = s.Predict == true ? predictor.Ahead(at) : at;
                        if (s.Mode == "smooth")
                        {
                            pd = pd.Width == area.Width ? pd : new MacroFollow.Pd(s.Kp ?? 1.0, s.Kd ?? 0.4, area.Width);
                            double pressMs = pd.PressMs(target, ahead, frameMs);
                            if (pressMs >= 1)
                            {
                                Press(true);
                                held = true;
                                await desk.Delay((int)Math.Round(pressMs), ct);
                                Press(false);
                                held = false;
                                pulsed = (int)Math.Round(pressMs);
                            }
                        }
                        else if (MacroFollow.OnOff(ahead, target) is { } down && down != held)
                        {
                            Press(down);
                            held = down;
                        }
                    }
                    else if (held)
                    {
                        Press(false); // the marker isn't seen: nothing to steer by
                        held = false;
                    }
                }
                if (desk.Now >= deadline)
                {
                    if (held)
                        Press(false);
                    held = false;
                    return await FailedAsync(turn, seenOnce ? "timeout" : "notFound");
                }
                await desk.Delay(Math.Max(1, frameMs - pulsed), ct);
            }
        }
        finally
        {
            if (held)
                Press(false);
        }
        Seen(run);
        run.Pc++;
        return Then.Next;
    }

    Point Offset(Point p, int px) => px <= 0 ? p : new Point(p.X + Random.Next(-px, px + 1), p.Y + Random.Next(-px, px + 1));

    /// <param name="holdMs">Held this long (a charge); 0: a plain click.</param>
    async Task ClickAsync(IMacroTurn input, Turn turn, Point at, string button, bool twice, CancellationToken ct, int holdMs = 0)
    {
        var m = turn.Run.Macro;
        int frame = Frame(turn.Window);
        at = Offset(at, m.JitterPx);
        if (!input.Clear(at))
            throw new MacroCovered();
        input.MoveTo(at);
        await desk.Delay(frame, ct); // the game sees the pointer arrive before the press
        for (int i = 0; i < (twice ? 2 : 1); i++)
        {
            if (i > 0)
                await desk.Delay(Math.Max(60, frame), ct);
            input.Button(button, up: false);
            try
            {
                await desk.Delay(HoldFor(Jitter(holdMs > 0 ? holdMs : 60, m.JitterMs), turn.Window.Fps), ct);
            }
            finally
            {
                input.Button(button, up: true);
            }
        }
        await desk.Delay(frame, ct);
    }

    /// <summary>The camera turned by a relative mouse move spread over <paramref name="ms"/>, about one piece a frame,
    /// with the right button held (Roblox's camera drag) unless the game locks the pointer itself.</summary>
    async Task CameraAsync(IMacroTurn input, Turn turn, int dx, int dy, int ms, bool drag, CancellationToken ct)
    {
        int frame = Frame(turn.Window);
        var centre = MacroGeometry.ClientPoint(input.Client, 0.5, 0.5);
        if (!input.Clear(centre))
            throw new MacroCovered();
        input.MoveTo(centre);
        await desk.Delay(frame, ct);
        if (drag)
        {
            input.Button("right", up: false);
            turn.Run.Held.Add("right");
            await desk.Delay(Math.Max(frame, 30), ct);
        }
        try
        {
            int pieces = Math.Max(1, ms / Math.Max(frame, 8));
            foreach (var (x, y) in MacroGeometry.Pieces(dx, dy, pieces))
            {
                input.MoveBy(x, y);
                await desk.Delay(Math.Max(1, ms / pieces), ct);
            }
        }
        finally
        {
            if (drag)
            {
                input.Button("right", up: true);
                turn.Run.Held.Remove("right");
            }
        }
        await desk.Delay(frame, ct);
    }

    /// <summary>Chat: "/" opens it, the text, Enter sends it. Letters and signs as keys (Shift where needed); any other
    /// character (Vietnamese letters) typed as itself.</summary>
    async Task TypeAsync(IMacroTurn input, Turn turn, string text, CancellationToken ct)
    {
        int hold = HoldFor(30, turn.Window.Fps), frame = Frame(turn.Window);
        async Task Tap(MacroKey key, bool shift)
        {
            var shiftKey = MacroKeys.All["Shift"];
            if (shift)
                input.Key(shiftKey, up: false);
            input.Key(key, up: false);
            try
            {
                await desk.Delay(Math.Min(hold, 40), ct);
            }
            finally
            {
                input.Key(key, up: true);
                if (shift)
                    input.Key(shiftKey, up: true);
            }
            await desk.Delay(Math.Max(15, frame / 2), ct);
        }
        await Tap(MacroKeys.All["Slash"], false);
        await desk.Delay(Math.Max(150, frame * 3), ct); // the chat box opening
        foreach (char c in text)
        {
            if (MacroKeys.ForChar(c) is { } k)
                await Tap(k.Key, k.Shift);
            else
            {
                input.Char(c, up: false);
                input.Char(c, up: true);
                await desk.Delay(Math.Max(15, frame / 2), ct);
            }
        }
        await desk.Delay(Math.Max(60, frame * 2), ct);
        await Tap(MacroKeys.All["Enter"], false);
    }

    /// <summary>
    /// Roughly how long one round over <paramref name="windows"/> windows takes for this macro: bringing each forward,
    /// and its steps up to its first long wait.
    /// </summary>
    public static double SweepSeconds(Macro m, int windows, int fps = 0)
    {
        double turn = 0.5; // forward and back
        int frame = fps > 0 ? (int)Math.Ceiling(1000.0 / fps) : 17;
        foreach (var s in m.Steps)
        {
            double ms = s.Kind switch
            {
                "key" => HoldFor(s.Ms ?? 60, fps) + frame,
                "down" or "up" or "move" or "setVar" or "addVar" => frame,
                "read" => 300,
                "click" => frame * 2 + HoldFor(60, fps) * (s.Double == true ? 2 : 1),
                "clickImage" => 300 + frame * 2 + HoldFor(60, fps),
                "camera" => (s.Ms ?? 200) + frame * 3,
                "scroll" => Math.Abs(s.Notches ?? 0) * Math.Max(40, frame) + frame,
                "type" => 300 + (s.Text?.Length ?? 0) * 60,
                "wait" => s.Ms ?? 0,
                _ => 0,
            };
            if (s.Kind is "waitImage" or "waitText" or "waitColour" or "waitStill" || (s.Kind == "wait" && ms > LongWait.TotalMilliseconds))
                break;
            turn += ms / 1000;
            if (turn > TurnShare.TotalSeconds)
            {
                turn = TurnShare.TotalSeconds;
                break;
            }
        }
        return Math.Round(windows * turn, 1);
    }
}

/// <summary>Where on the screen a place in a window is, and how SendInput names it.</summary>
static class MacroGeometry
{
    /// <summary>A place (0..1 of the client area) in screen pixels, kept inside the client.</summary>
    public static Point ClientPoint(Rectangle client, double x, double y) =>
        new(client.X + Math.Clamp((int)Math.Round(x * client.Width), 0, Math.Max(0, client.Width - 1)),
            client.Y + Math.Clamp((int)Math.Round(y * client.Height), 0, Math.Max(0, client.Height - 1)));

    /// <summary>
    /// A screen pixel as SendInput's absolute coordinates over the whole desktop (MOUSEEVENTF_VIRTUALDESK): 0..65535 from
    /// the left of the leftmost screen to the right of the rightmost, which may start left of or above the main screen.
    /// </summary>
    public static (int X, int Y) Absolute(Point screen, Rectangle desktop) =>
        ((int)Math.Round((screen.X - desktop.X) * 65535.0 / Math.Max(1, desktop.Width - 1)),
         (int)Math.Round((screen.Y - desktop.Y) * 65535.0 / Math.Max(1, desktop.Height - 1)));

    /// <summary>A move split into <paramref name="count"/> pieces that add up to it exactly.</summary>
    public static IEnumerable<(int X, int Y)> Pieces(int dx, int dy, int count)
    {
        count = Math.Max(1, count);
        int doneX = 0, doneY = 0;
        for (int i = 1; i <= count; i++)
        {
            int x = (int)Math.Round((double)dx * i / count), y = (int)Math.Round((double)dy * i / count);
            if (x - doneX != 0 || y - doneY != 0)
                yield return (x - doneX, y - doneY);
            (doneX, doneY) = (x, y);
        }
    }
}
