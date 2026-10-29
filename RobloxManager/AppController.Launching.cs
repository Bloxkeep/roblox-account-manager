using System.ComponentModel;
using System.Text.Json;

namespace RobloxManager;

/// <summary>Opening Roblox for the chosen accounts, one after another, into the game and server asked for.</summary>
public sealed partial class AppController
{
    static string MultiNotReadyText()
    {
        string text = L.T("Multi-Roblox isn't working yet: Roblox was already open before it was turned on.");
        if (MultiInstance.FindTrayAutostart() is { Enabled: true })
            text += " " + L.T("Roblox also starts by itself in the background when Windows starts.");
        return text + " " + L.T("A new Roblox window may close the old one.");
    }

    static string FormatMb(long mb) => mb >= 1024 ? $"{mb / 1024.0:0.#} GB" : $"{mb} MB";

    async Task<object?> LaunchAsync(JsonElement a)
    {
        var vault = RequireVault();
        var accounts = InLaunchOrder(vault, Ids(a)); // several selected: the launch order from the ⋯ menu
        CheckCanLaunch(accounts);
        bool appOnly = Bool(a, "appOnly");

        var s = vault.Data.Settings;
        s.LastPlaceInput = Str(a, "placeInput").Trim();
        s.LastJobId = Str(a, "jobId").Trim();
        GameTarget? target = null;
        if (!appOnly && !GameTarget.TryParse(s.LastPlaceInput, s.LastJobId, out target, out string error))
            throw new UserError(error);
        return await LaunchAccountsAsync(vault, accounts, target, Bool(a, "force"), s.LaunchDelaySeconds);
    }

    void CheckCanLaunch(List<Account> accounts)
    {
        if (_launchCts is not null || _openingHome)
            throw new UserError(L.T("Already launching — wait a moment or press Stop."));
        if (accounts.Count == 0)
            throw new UserError(L.T("Select at least one account."));
    }

    string BannedNotOpened(Account account) =>
        L.F("{0} is banned on Roblox, so it wasn't opened. If the ban is over, Check sign-in clears this.", Name(account));

    /// <summary>
    /// Opens the accounts one after another (the dock's Play and presets); callers run <see cref="CheckCanLaunch"/>
    /// first. Asks for a confirmation (without <paramref name="force"/>) when Multi-Roblox or free RAM look wrong.
    /// </summary>
    /// <param name="pickServer">Chooses the server once the launch has started, so Stop works meanwhile; gets how many accounts will open.</param>
    /// <param name="presetId">The preset being run: the clients it opens follow its options.</param>
    async Task<LaunchResult> LaunchAccountsAsync(Vault vault, List<Account> accounts, GameTarget? target, bool force, int delaySeconds,
        Func<GameTarget, int, CancellationToken, Task<GameTarget>>? pickServer = null, Guid? presetId = null)
    {
        // Accounts Roblox reports banned (the account checks read it) aren't opened: their window would only show the
        // ban. Said with the result, like an account that failed to open; with nothing else to open, said at once.
        // Roblox VNG accounts open like the others, with a ticket from Roblox VNG's sign-in (tried from 2026-10-19).
        var leftOut = accounts.Where(acc => acc.Banned == true).ToList();
        var leftOutText = leftOut.Select(BannedNotOpened).ToList();
        if (leftOut.Count > 0)
        {
            if (leftOut.Count == accounts.Count)
                throw new UserError(string.Join("\n", leftOutText));
            accounts = accounts.Except(leftOut).ToList();
        }
        // The slot is taken before the first await: a second Play (or preset) meanwhile would pass CheckCanLaunch,
        // and its own "switching" list could close the client this one had just opened.
        var cts = _launchCts = new CancellationTokenSource();
        var ct = cts.Token;
        void Release()
        {
            if (_launchCts == cts)
                _launchCts = null;
            cts.Dispose();
        }

        var s = vault.Data.Settings;
        // [kind, text]: the kinds in SkippableLaunchWarnings can be answered "Don't ask again" one by one.
        var warnings = new List<(string Kind, string Text)>();
        var switching = new List<Account>();
        var errors = leftOutText;
        // Accounts left as they were (already open where asked): not failures.
        var skipped = new List<string>();
        // Clients open here that a new client closes when Multi-Roblox isn't ready: not drops to rejoin from.
        var replaced = new List<Guid>();
        // Accounts Roblox wouldn't sign in (a warning to accept, a lock): the page offers their roblox.com, which says why.
        var refusedSignIn = new List<Guid>();
        int launched = 0;
        // Past the checks and warnings: from then on an error is reported with the result rather than thrown.
        bool started = false;
        // One try from the moment the slot is taken: whatever happens, the slot is freed and the progress text cleared.
        try
        {
            // A share link only names the private server once Roblox has resolved it; clients already in that
            // server are compared by its link code, so it is resolved before deciding who switches.
            if (target?.ShareCode is { } shareCode)
            {
                Progress(L.T("Reading the private server link…"));
                var (placeId, linkCode) = await ResolveShareAsync(accounts, shareCode, ct);
                target = target with { PlaceId = placeId, LinkCode = linkCode, ShareCode = null };
                Progress(null);
            }
            // Bans noted before: Roblox is asked first whether they still stand.
            if (target is not null)
                await RecheckGameBansAsync(accounts, target, ct);
            // A preset (from the tray or its schedule too, where nobody answers a question) leaves out the accounts this
            // game banned, as Bloxkeep saw it, like the ones Roblox banned: a question would stop the whole preset.
            if (presetId is not null && target is not null && accounts.Where(acc => GameBanOf(acc, target.PlaceId) is not null).ToList() is { Count: > 0 } gameBanned)
            {
                errors.AddRange(gameBanned.Select(acc => L.F("{0} was banned from this game, so the preset didn't open it. Play it on its own to try anyway.", Name(acc))));
                if (gameBanned.Count == accounts.Count)
                    throw new UserError(string.Join("\n", errors));
                accounts = accounts.Except(gameBanned).ToList();
            }
            // Things worth a second look before opening more windows.
            var players = RobloxProcesses.GetPlayers();
            ApplyWatchNames();
            if (s.MultiInstance && (!MultiInstance.Armed || MultiInstance.Stale) && players.Count == 0)
                MultiInstance.Enable();
            // Accounts already playing here: left alone in the same game, switched over when another game (or server)
            // is chosen. Roblox kicks a second client of the same account, so the old one is closed first.
            if (target is null)
            {
                // "Open Roblox" (home screen): clients in a game go back to the home screen; ones already there stay.
                switching = accounts.Where(acc => _running.TryGetValue(acc.Id, out var run) && InGameHere(run)).ToList();
                if (switching.Where(acc => PlayingHere(_running[acc.Id])).ToList() is { Count: > 0 } leavingGame)
                    warnings.Add(("switch", L.F("{0} will leave the game and go to the Roblox home screen.", Names(leavingGame))));
            }
            else if (accounts.Any(acc => _running.ContainsKey(acc.Id)))
            {
                await LoadGamesAsync(accounts.Where(acc => _running.ContainsKey(acc.Id)).Select(acc => (CurrentPlace(acc.Id), 0L))
                    .Append((target.PlaceId, 0L)).Where(p => p.Item1 > 0).ToList());
                switching = accounts.Where(acc => _running.TryGetValue(acc.Id, out var run) && NeedsSwitch(run, target)).ToList();
                // Asked only for the ones in a game: one on Roblox's home screen (or an error screen) loses nothing by
                // giving way to the game asked for, so it simply does.
                var leaving = switching.Where(acc => LeavesAGame(_running[acc.Id], target)).ToList();
                if (leaving.Count > 0)
                    warnings.Add(("switch", L.F("{0} will leave what they're playing now and open this game instead.",
                        Names(leaving))));
            }
            // Accounts Roblox says can't play this game (an age check, its region, a restricted account, a game to buy…),
            // as its own client asks before joining: asked first, with why, instead of a window that never gets in. Only on
            // Play: a preset may run with nobody there to answer, and an answered question (force) isn't asked again.
            if (target is not null && presetId is null && !force)
            {
                Progress(L.T("Asking Roblox whether the accounts can play this game…"));
                var unplayable = await UnplayableAsync(accounts, target.PlaceId, ct);
                Progress(null);
                foreach (var why in unplayable.GroupBy(u => u.Why))
                    warnings.Add(("playability", L.F("{0} can't play this game: {1}.", Names(why.Select(u => u.Account)), why.Key)));
            }
            // Accounts this game banned, as Bloxkeep saw it: Roblox would most likely show Join Error 600. Asked when
            // opened by hand; a preset left them out above.
            if (target is not null && accounts.Where(acc => GameBanOf(acc, target.PlaceId) is not null).ToList() is { Count: > 0 } bannedHere)
                warnings.Add(("gameBan", L.P(bannedHere.Count,
                    "{0} was banned from this game, so Roblox will most likely not let them in (Join Error 600).",
                    "{0} were banned from this game, so Roblox will most likely not let them in (Join Error 600).", Names(bannedHere))));
            bool robloxOpen = RobloxProcesses.VisibleWindows(players.Select(p => p.Pid)).Values.Any(list => list.Count > 0);
            if ((accounts.Count > 1 || robloxOpen) && !MultiInstance.Armed)
                warnings.Add(s.MultiInstance ? ("multiNotReady", MultiNotReadyText())
                    : ("multiOff", L.T("Multi-Roblox is off, so each new Roblox window closes the previous one.")));
            else if ((accounts.Count > 1 || robloxOpen) && WatchWarning() is { } watchWarning)
                warnings.Add(("watch", watchWarning));
            int toOpen = accounts.Count(acc => !_running.ContainsKey(acc.Id));
            var memory = SystemMemory.Read();
            long perClientMb = players.Count > 0 ? Math.Max(400, (long)players.Average(p => p.WorkingSet) / MB) : DefaultClientMb;
            long neededMb = toOpen * perClientMb;
            // Windows' commit limit (RAM and paging file together, "virtual memory") can run out with RAM to spare when the
            // paging file is small: a client that can't get its memory then crashes as it opens.
            long perClientCommitMb = ClientCommitMb(players);
            if (s.MemoryWarnings && memory.TotalMb > 0 && memory.AvailableMb - neededMb < memory.TotalMb * 8 / 100)
                warnings.Add(("memory", L.P(toOpen, "Only {0} of RAM is free, and {1} more Roblox window needs about {2}. The PC may slow down or Roblox may crash.",
                    "Only {0} of RAM is free, and {1} more Roblox windows need about {2}. The PC may slow down or Roblox may crash.",
                    FormatMb(memory.AvailableMb), toOpen, FormatMb(neededMb))));
            else if (s.MemoryWarnings && VirtualMemoryShort(memory, toOpen, perClientCommitMb))
                warnings.Add(("memory", L.P(toOpen, "RAM is free, but Windows' virtual memory is almost full, so {0} more Roblox window may crash as it opens. Close other apps or increase virtual memory.",
                    "RAM is free, but Windows' virtual memory is almost full, so {0} more Roblox windows may crash as they open. Close other apps or increase virtual memory.", toOpen)));
            warnings.RemoveAll(w => SkippableLaunchWarnings.Contains(w.Kind) && Prefs.SkipConfirm.Contains(LaunchWarningKey(w.Kind)));
            if (warnings.Count > 0 && !force)
                return new LaunchResult(NeedsConfirm: true, Message: string.Join("\n\n", warnings.Select(w => w.Text)),
                    Asks: warnings.Select(w => w.Kind).ToList());

            started = true;
            PushState();
            await WaitForSweepAsync(ct);
            int party = accounts.Count(acc => (!_running.ContainsKey(acc.Id) || switching.Contains(acc)) && !StillOpening(acc.Id));
            if (pickServer is not null && target is not null)
                target = await pickServer(target, party, ct);
            // Several accounts sent to one server: the first to open shows whether the server takes anyone before the
            // rest follow it there; one that doesn't (it has ended, or is full) is swapped for another with room for them.
            bool checkServer = party > 1;
            var closedServers = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

            for (int i = 0; i < accounts.Count && _vault == vault; i++)
            {
                ct.ThrowIfCancellationRequested();
                var account = accounts[i];
                await WaitForPendingAsync(ct);
                if (FindAccount(account.Id) is null)
                    continue; // deleted while the earlier ones opened
                if (switching.Contains(account) && _running.TryGetValue(account.Id, out var old))
                {
                    Progress(L.F("Closing {0}'s Roblox to switch games…", Name(account)));
                    _userClosed.Add(account.Id); // not a disconnect: no auto-rejoin into the old game
                    RobloxProcesses.Close(old.Pid, InGameNow(old.Pid));
                    try
                    {
                        // The process check ends the session once the client is gone; only then can the account open again.
                        for (int waited = 0; _running.ContainsKey(account.Id) && waited < 15_000; waited += 300)
                            await Task.Delay(300, ct);
                    }
                    finally
                    {
                        // Still open (it wouldn't close, or Stop was pressed): it stays this account's client, and a
                        // real drop later must still rejoin.
                        if (_running.ContainsKey(account.Id))
                            _userClosed.Remove(account.Id);
                    }
                }
                if (_running.ContainsKey(account.Id) || StillOpening(account.Id))
                {
                    bool stayed = _running.ContainsKey(account.Id) && !switching.Contains(account);
                    skipped.Add(stayed && target is null ? L.F("{0}: already on the home screen — skipped.", Name(account))
                        : stayed ? L.F("{0}: already in this game — skipped.", Name(account))
                        : L.F("{0}: already running — skipped.", Name(account)));
                    continue;
                }

                Progress(L.F("Launching {0} ({1}/{2})…", Name(account), i + 1, accounts.Count));
                var startedAt = DateTime.UtcNow;
                if (!MultiInstance.Armed)
                {
                    foreach (var id in _running.Keys)
                    {
                        if (id != account.Id && _userClosed.Add(id))
                            replaced.Add(id);
                    }
                }
                bool opened = false;
                try
                {
                    await StartSessionAsync(account, target, s.LaunchVia, ct, presetId: presetId);
                    launched++;
                    opened = true;
                }
                catch (Exception ex) when (!ct.IsCancellationRequested
                                           && (RobloxApi.IsExpectedError(ex) || ex is Win32Exception or FileNotFoundException or UserError))
                {
                    // Marked (and said) signed out only when the sign-in check agrees: an outage answers 401 for everyone.
                    bool refused = ex is RobloxApiException { CookieInvalid: true } && !await ConfirmSignedOutAsync(account, ct);
                    errors.Add($"{NameText(account)}: {(refused ? RefusedText : ex is Win32Exception or FileNotFoundException ? ex.Message : ErrorText(ex))}");
                    if (ex is RobloxApiException { SignInRefused: true } && Realm.Normalize(account.Realm) != Realm.Vng)
                        refusedSignIn.Add(account.Id);
                }
                PushState();

                if (i < accounts.Count - 1)
                {
                    await WaitBeforeNextLaunchAsync(account, startedAt, delaySeconds, accounts.Skip(i + 1).Select(acc => acc.Id), ct);
                    if (!opened || !checkServer || target is not { JobId: { } job, IsPrivate: false })
                        continue;
                    if (await WaitForServerAnswerAsync(account, target, ct) != "closed")
                    {
                        checkServer = false; // it took this account (or its log can't tell): the rest follow
                        continue;
                    }
                    closedServers.Add(job);
                    var other = closedServers.Count > MaxServerSwaps ? target with { JobId = null } : await OtherServerAsync(target, party, closedServers, ct);
                    // Got in after all while the other servers were looked up (a slow join): it stays, the rest follow.
                    if (ServerAnswer(account.Id) == "in")
                    {
                        closedServers.Remove(job);
                        checkServer = false;
                        continue;
                    }
                    target = other;
                    checkServer = target.JobId is not null;
                    Toast(target.JobId is not null ? L.T("That server is full or has closed, so the accounts go to another one with room for them all.")
                        : L.T("That server is full or has closed, and no other server was found with room for all the accounts: Roblox picks one for each."), "warn");
                    // This account goes again, to the new server; not a drop for auto-rejoin to bring back to the old one.
                    if (_running.TryGetValue(account.Id, out var stuck))
                    {
                        _userClosed.Add(account.Id);
                        RobloxProcesses.Close(stuck.Pid, force: true);
                        for (int waited = 0; _running.ContainsKey(account.Id) && waited < 15_000; waited += 300)
                            await Task.Delay(300, ct);
                        if (_running.ContainsKey(account.Id))
                        {
                            _userClosed.Remove(account.Id); // it wouldn't close: it stays, and auto-rejoin still looks after it
                            continue;
                        }
                    }
                    launched--;
                    i--;
                }
            }
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested)
        {
            // Stop: a cancelled result, also while the link was still being read.
        }
        catch (Exception ex) when (started && RobloxApi.IsExpectedError(ex))
        {
            errors.Add(ErrorText(ex));
        }
        finally
        {
            Release();
            if (started)
            {
                _ = ForgetReplacedAsync(replaced);
                if (launched > 0 && target is not null && _vault == vault)
                {
                    RememberGame(target.PlaceId);
                    _ = LoadRecentGamesAsync();
                }
                if (launched > 0)
                    RefreshPresenceSoon(TimeSpan.FromSeconds(20));
                // A preset's own choice, else Settings' (Multi-Roblox › arrange the windows once opened).
                if (launched > 0 && (PresetFor(presetId)?.Arrange ?? (s.MultiInstance && s.MultiInstanceTile)))
                    _ = TileWhenOpenAsync(presetId);
                Persist();
            }
            Progress(null);
            PushState();
        }
        // The page shows the details; while the window is in the background a short notification says so.
        if (errors.Count > 0 && !ct.IsCancellationRequested)
            Notify(L.P(errors.Count, "{0} account didn't open.", "{0} accounts didn't open.", errors.Count), "warn", "launch", toast: false);
        return new LaunchResult(Launched: launched, Errors: errors, Cancelled: ct.IsCancellationRequested, Skipped: skipped,
            Refused: refusedSignIn.Count > 0 ? refusedSignIn : null);
    }

    /// <summary>Servers found closed in one launch before Roblox is left to pick one for each account.</summary>
    const int MaxServerSwaps = 3;
    /// <summary>
    /// How long a game start may go without a server before Roblox is asked whether the server takes anyone: a normal join
    /// gets one about a second after the start (1.0 and 1.2 s in Roblox logs from 2026-10-12, at most 1.8 s in the 31 of
    /// 2026-10-21), but one waiting for room logs nothing (17-38 s for four accounts sent to one server, 2026-10-16).
    /// </summary>
    static readonly TimeSpan ServerCheck = TimeSpan.FromSeconds(12);
    /// <summary>How long a launch waits for that answer before going on without it (the log not found, the window gone).</summary>
    static readonly TimeSpan ServerCheckLimit = TimeSpan.FromSeconds(25);

    /// <summary>
    /// What this account's client's log says of the server it was sent to: "in" once it names a server, "silent" when its
    /// game start has had none for <see cref="ServerCheck"/>, null while it says neither.
    /// </summary>
    string? ServerAnswer(Guid accountId) =>
        !_running.TryGetValue(accountId, out var run) || _logs.StateOf(run.Pid) is not { } log ? null
        : log.JobId is not null ? "in"
        : log.Status == "joining" && DateTime.UtcNow - log.At >= ServerCheck ? "silent"
        : null;

    /// <summary>
    /// Whether the server the first account was sent to takes it, at most <see cref="ServerCheckLimit"/>: "in" once its log
    /// names the server; "closed" when, its join silent for <see cref="ServerCheck"/>, Roblox's join answer says the
    /// server has ended or is full; null otherwise (Roblox lets it in and the join is slow, refuses the account, or gives
    /// no answer): the rest follow. Before, the silence alone counted as closed: a join waiting for room (17-38 s seen)
    /// had its window closed and the party sent elsewhere.
    /// </summary>
    async Task<string?> WaitForServerAnswerAsync(Account account, GameTarget target, CancellationToken ct)
    {
        var until = DateTime.UtcNow + ServerCheckLimit;
        Task<RobloxApi.JoinAnswer?>? asked = null;
        while (DateTime.UtcNow < until)
        {
            string? log = ServerAnswer(account.Id);
            if (log == "in")
                return log;
            if (log == "silent" && asked is null && _running.TryGetValue(account.Id, out var run))
            {
                // The answer stays with the window: its own join check reads it rather than asking again.
                asked = CheckJoinAsync(account, target, "A party's server, its first join silent");
                _joinChecks[run.Pid] = (asked, DateTime.UtcNow);
            }
            if (asked is { IsCompletedSuccessfully: true, Result: { } said } && (said.Ended || said.Full))
                return "closed";
            Progress(L.F("Checking that the server lets {0} in…", Name(account)));
            await Task.Delay(500, ct);
        }
        return null;
    }

    /// <summary>
    /// Another public server for accounts whose server didn't take them (<see cref="RoomyServer"/>), from the emptiest
    /// ones. With none, or no answer from Roblox, Roblox picks one for each account.
    /// </summary>
    async Task<GameTarget> OtherServerAsync(GameTarget target, int party, HashSet<string> closed, CancellationToken ct)
    {
        Progress(L.T("Finding a server…"));
        try
        {
            var (servers, _) = await RobloxApi.GetServersAsync(target.PlaceId, true, null, ct);
            if (RoomyServer(servers, party, closed) is { } other)
                return target with { JobId = other.JobId };
        }
        catch (Exception ex) when (!ct.IsCancellationRequested && RobloxApi.IsExpectedError(ex))
        {
            // Rate-limited or offline: Roblox picks.
        }
        return target with { JobId = null };
    }

    /// <summary>
    /// One with room for all the accounts, the busiest of those (a server whose last player leaves closes), never one
    /// already found closed.
    /// </summary>
    static ServerInfo? RoomyServer(IEnumerable<ServerInfo> servers, int party, IReadOnlySet<string> closed) =>
        servers.Where(sv => sv.MaxPlayers - sv.Playing >= party && !closed.Contains(sv.JobId)).MaxBy(sv => sv.Playing);

    /// <summary>A launch of this account that is still waiting for its window (not one the person closed meanwhile).</summary>
    bool StillOpening(Guid accountId) => _pending.Any(p => p.AccountId == accountId && !_closeWhenClaimed.Contains(p));

    /// <summary>How long after its window a new client may still be closing the one it replaces.</summary>
    static readonly TimeSpan ReplacedGrace = TimeSpan.FromSeconds(15);

    /// <summary>
    /// Without Multi-Roblox the new client closes the ones open here — but only once it is up itself, after the launch
    /// has returned. Those stay "closed on purpose" (no auto-rejoin) until the new client has had its window a while;
    /// one still open by then wasn't replaced after all, so a real drop later must rejoin it again.
    /// </summary>
    async Task ForgetReplacedAsync(List<Guid> replaced)
    {
        if (replaced.Count == 0)
            return;
        var deadline = DateTime.UtcNow + PendingTimeout + ReplacedGrace;
        DateTime? settled = null;
        while (!_disposed && replaced.Any(_running.ContainsKey) && DateTime.UtcNow < deadline)
        {
            settled = _pending.Count == 0 ? settled ?? DateTime.UtcNow : null;
            if (settled is { } since && DateTime.UtcNow - since >= ReplacedGrace)
                break;
            await Task.Delay(1000);
        }
        foreach (var id in replaced.Where(_running.ContainsKey))
            _userClosed.Remove(id);
    }

    /// <summary>
    /// Resolves a share link with the first of these accounts whose sign-in Roblox accepts; one it rejects is marked
    /// as signed out (once a second call agrees) and the next one tried, so a dead first account doesn't stop the
    /// whole launch.
    /// </summary>
    async Task<(long PlaceId, string LinkCode)> ResolveShareAsync(List<Account> accounts, string shareCode, CancellationToken ct)
    {
        var candidates = accounts.Where(acc => acc.CookieValid != false).Concat(accounts).Distinct().ToList();
        for (int i = 0; ; i++)
        {
            try
            {
                return await RobloxApi.ResolvePrivateServerShareAsync(candidates[i].Cookie, shareCode, ct);
            }
            catch (RobloxApiException ex) when (ex.CookieInvalid && i < candidates.Count - 1)
            {
                await ConfirmSignedOutAsync(candidates[i], ct);
            }
        }
    }

    /// <summary>
    /// Waits until the client just started has its window, then for the rest of the launch delay. When Roblox has
    /// an update, its launcher downloads it before the window appears; starting more launchers meanwhile would make
    /// them all download at once and fight over the install.
    /// </summary>
    /// <param name="next">The accounts the launch has still to open: their own rejoins don't go during the wait.</param>
    async Task WaitBeforeNextLaunchAsync(Account account, DateTime startedAt, int delaySeconds, IEnumerable<Guid> next, CancellationToken ct)
    {
        while (_pending.Any(p => p.AccountId == account.Id) && DateTime.UtcNow - startedAt < PendingTimeout)
        {
            if (DateTime.UtcNow - startedAt > TimeSpan.FromSeconds(Math.Max(10, delaySeconds)))
            {
                var version = RobloxVersionDto();
                Progress(version.UpToDate == false
                    ? L.F("Waiting for Roblox to open {0}… (Roblox is updating to {1})", Name(account), version.Latest)
                    : L.F("Waiting for Roblox to open {0}… (Roblox may be updating)", Name(account)));
            }
            await Task.Delay(500, ct);
        }
        // The wait itself lets rejoins go (one dropped early in a long launch, two minutes between accounts, would
        // otherwise wait for the whole of it), but not of the accounts still to open: the launch opens them. A rejoin
        // started meanwhile is waited for, and the next account then keeps the same gap after it, so windows still
        // start one every delaySeconds.
        if (delaySeconds <= 0)
            return;
        var ownStart = _lastSessionStart;
        string? said = null;
        _launchWaiting = next.ToHashSet();
        try
        {
            while (true)
            {
                var from = _lastSessionStart != ownStart ? _lastSessionStart : startedAt;
                var rest = TimeSpan.FromSeconds(delaySeconds) - (DateTime.UtcNow - from);
                bool opening = _rejoining || _pending.Count > 0;
                if (rest <= TimeSpan.Zero && !opening)
                    return;
                string text = rest > TimeSpan.Zero ? L.F("Waiting {0}s before the next account…", (int)Math.Ceiling(rest.TotalSeconds))
                    : L.T("Waiting for the previous Roblox window to open…");
                if (text != said)
                    Progress(said = text);
                // The countdown goes down a second at a time; an opening window is looked at twice a second.
                var step = rest <= TimeSpan.Zero ? TimeSpan.FromMilliseconds(500) : rest < TimeSpan.FromSeconds(1) ? rest : TimeSpan.FromSeconds(1);
                await Task.Delay(step, ct);
            }
        }
        finally
        {
            _launchWaiting = null;
        }
    }

    /// <summary>The place a client here is in (its log), else the one it was opened with; 0 when unknown.</summary>
    long CurrentPlace(Guid accountId) =>
        _running.TryGetValue(accountId, out var run) ? _logs.StateOf(run.Pid)?.PlaceId ?? run.Launch.Target?.PlaceId ?? 0 : 0;

    /// <summary>
    /// A client in a game answers the window's close button with "leave the game?" instead of closing (seen
    /// 2026-10-13), so closing it from Bloxkeep ends it straight away; one on the home screen is closed normally.
    /// </summary>
    bool InGameNow(int pid) => _logs.StateOf(pid)?.Status is "ingame" or "joining";

    /// <summary>
    /// Opening <paramref name="target"/> takes this client out of a game it is playing: worth asking first. One on
    /// Roblox's home screen gives way without a word (on 2026-10-21 "will leave what they're playing now" was asked of
    /// an account only on the home screen), as does one stuck on a join (a game's ban, Join Error 600).
    /// </summary>
    bool LeavesAGame((int Pid, PendingLaunch Launch) run, GameTarget target) => NeedsSwitch(run, target) && PlayingHere(run);

    /// <summary>In a game (or joining one), as opposed to the home screen; without a log, by what it was opened with.</summary>
    bool InGameHere((int Pid, PendingLaunch Launch) run) =>
        _logs.StateOf(run.Pid) is { } log ? log.Status is "ingame" or "joining" : run.Launch.Target is not null;

    /// <summary>
    /// Playing here: in a server, or on the way into one it was given; not a join still without a server (one stuck on
    /// a game's ban among them). Without a log, by what it was opened with.
    /// </summary>
    bool PlayingHere((int Pid, PendingLaunch Launch) run) =>
        _logs.StateOf(run.Pid) is { } log ? log is { Status: "ingame" } or { Status: "joining", JobId: not null } : run.Launch.Target is not null;

    /// <summary>Two places of the same game (a game moves players between its places) count as the same.</summary>
    bool SameGame(long a, long b) =>
        a == b || (_games.GetValueOrDefault(a)?.UniverseId is long ua && ua > 0 && ua == _games.GetValueOrDefault(b)?.UniverseId);

    /// <summary>
    /// Whether opening <paramref name="target"/> means leaving what this client is doing: another game, a chosen
    /// server it isn't in, a private server it wasn't opened into, or it is only in the menu / on an error screen.
    /// </summary>
    bool NeedsSwitch((int Pid, PendingLaunch Launch) run, GameTarget target)
    {
        var log = _logs.StateOf(run.Pid);
        long current = log?.PlaceId ?? run.Launch.Target?.PlaceId ?? 0;
        if (current == 0 || log is { Status: "left" or "disconnected" or "signedout" })
            return true;
        if (target.JobId is { } job)
            return !string.Equals(job, log?.JobId, StringComparison.OrdinalIgnoreCase);
        if (target.IsPrivate || target.ShareCode is not null)
            return !target.IsPrivate || run.Launch.Target?.LinkCode != target.LinkCode || run.Launch.Target?.AccessCode != target.AccessCode;
        return !SameGame(current, target.PlaceId);
    }

    /// <summary>
    /// Whether a client can be this launch's, by the browser tracker id its command line names
    /// (<see cref="RobloxProcesses.LaunchTrackerOf"/>): one naming another is someone else's (another launch's, or the
    /// person's own from the website); one naming none, not read, or a launch without one, may be.
    /// </summary>
    static bool MayBeLaunchOf(string? tracker, PendingLaunch launch) => tracker is not { Length: > 0 } || launch.Tracker is null || tracker == launch.Tracker;

    /// <summary>The client names this launch's browser tracker id: it is that launch's, whichever started first.</summary>
    static bool IsLaunchOf(string? tracker, PendingLaunch launch) => tracker is { Length: > 0 } && tracker == launch.Tracker;

    /// <summary>
    /// Gives launches still waiting their client's window (see TrackProcesses). A window whose command line names a
    /// launch's browser tracker id is that launch's, and never another's; only one naming none (or not read) goes by the
    /// order of starts. Before, the order alone decided: a launch past its wait (Roblox updating) and the next account's
    /// could get each other's window, and a window the person opened from the website could be taken for a launch's.
    /// </summary>
    /// <param name="trackerOf">The browser tracker id a client's command line names ("" none, null not read).</param>
    /// <param name="updating">Roblox's updater started since the launch is running (<see cref="RobloxProcesses.UpdaterRunningSince"/>).</param>
    /// <returns>Whether a launch got its window or stopped waiting.</returns>
    bool ClaimWindows(HashSet<int> pids, Func<int, bool> hasWindow, Func<int, List<int>> childrenOf, Func<int, string?> trackerOf,
        Func<PendingLaunch, bool> updating, DateTime now)
    {
        bool changed = false;
        var claimed = _running.Values.Select(r => r.Pid).ToHashSet();
        if (_pending.Count > 0)
        {
            // Newest launch first: a new client can only belong to a launch made before it appeared (its Before
            // list lacks it), and a session waiting for a hand-over must not take the next account's window.
            foreach (var launch in _pending.OrderByDescending(p => p.StartedAt).ToList())
            {
                // A window named for this launch first; a session waiting for a hand-over takes a child of its ended
                // process before any other client.
                List<int> children = _handedOver.TryGetValue(launch, out var handed) ? childrenOf(handed.Pid) : [];
                int pid = pids.Where(p => !launch.Before.Contains(p) && !claimed.Contains(p))
                    .Where(hasWindow)
                    .Where(p => MayBeLaunchOf(trackerOf(p), launch))
                    .OrderByDescending(p => IsLaunchOf(trackerOf(p), launch))
                    .ThenByDescending(children.Contains)
                    .ThenBy(p => p)
                    .FirstOrDefault();
                if (pid == 0)
                    continue;
                claimed.Add(pid);
                _pending.Remove(launch);
                _handedOver.Remove(launch);
                // The person closed this account while it was still opening.
                if (_closeWhenClaimed.Remove(launch))
                    RobloxProcesses.Close(pid);
                else
                {
                    _running[launch.AccountId] = (pid, launch);
                    PersistSoon(); // the open clients on disk (see Persist)
                }
                changed = true;
            }
            // A client appeared for this launch and then every new client quit without a game window: stop waiting.
            // Unless Roblox's updater started since: an outdated client hands the launch to it and quits, and the updated
            // client comes from it a while later (until PendingTimeout, then as a late launch). Before, the launch was
            // dropped 5 s on ("no Roblox window appeared" for a rejoin) and the window that came was nobody's.
            foreach (var launch in _pending.ToList())
            {
                if (pids.Any(p => !launch.Before.Contains(p) && !claimed.Contains(p)))
                    _pendingSeenAlive[launch] = now;
                else if (_pendingSeenAlive.TryGetValue(launch, out var seen) && now - seen > PendingGoneGrace)
                {
                    if (updating(launch))
                    {
                        _pendingSeenAlive[launch] = now;
                        continue;
                    }
                    DropPending(launch);
                    changed = true;
                }
            }
        }
        // Launches past their wait, oldest first, take a window named for them, else only a client no launch still
        // waiting could be taking (one that appeared before all of them started): a late window never takes the window
        // of the launch under way.
        foreach (var launch in _late.OrderBy(l => l.StartedAt).ToList())
        {
            if (now - launch.StartedAt > PendingTimeout + LateTimeout || _running.ContainsKey(launch.AccountId))
            {
                _late.Remove(launch);
                _closeWhenClaimed.Remove(launch);
                if (!_running.ContainsKey(launch.AccountId) && !_pending.Any(p => p.AccountId == launch.AccountId))
                    ForgetScheduledOpen(launch.AccountId); // no client came of it: a scheduled stop has nothing of this account's to close
                continue;
            }
            int pid = pids.Where(p => !launch.Before.Contains(p) && !claimed.Contains(p) && hasWindow(p) && MayBeLaunchOf(trackerOf(p), launch)
                    && (IsLaunchOf(trackerOf(p), launch) || _pending.All(l => l.Before.Contains(p))))
                .OrderByDescending(p => IsLaunchOf(trackerOf(p), launch))
                .ThenBy(p => p)
                .FirstOrDefault();
            if (pid == 0)
                continue;
            claimed.Add(pid);
            _late.Remove(launch);
            if (_closeWhenClaimed.Remove(launch))
                RobloxProcesses.Close(pid);
            else
            {
                _running[launch.AccountId] = (pid, launch);
                PersistSoon();
            }
            changed = true;
        }
        return changed;
    }

    /// <summary>
    /// A window whose command line names no launch's browser tracker id is matched to launches newest first (see
    /// ClaimWindows), so a client still opening (the previous account, a rejoin, a hand-over — also one the person
    /// closed meanwhile, whose window is still on its way) must have its window before the next one starts, or the two
    /// could get each other's window. A rejoin that is still getting its
    /// ticket is waited for too, so one account isn't opened twice.
    /// </summary>
    async Task WaitForPendingAsync(CancellationToken ct)
    {
        bool Opening() => _rejoining || _pending.Count > 0;
        if (!Opening())
            return;
        Progress(L.T("Waiting for the previous Roblox window to open…"));
        // Bounded: a launch stops pending after PendingTimeout at the latest.
        while (Opening())
            await Task.Delay(500, ct);
    }

    /// <summary>Gets a sign-in ticket, starts the client and waits for its window to show up (see TrackProcesses).</summary>
    /// <param name="presetId">The preset this client is opened for (also on its rejoins and restarts): its options apply.</param>
    /// <param name="restart">Opened again by its preset's planned restart (a rejoin too).</param>
    async Task StartSessionAsync(Account account, GameTarget? target, string launchVia, CancellationToken ct = default, bool rejoin = false,
        Guid? presetId = null, bool restart = false)
    {
        string ticket = await RobloxApi.GetAuthTicketAsync(account.Cookie, ct, account.Realm);
        // Stop (or Close all) and a deletion while the ticket was fetched: no client for it any more.
        ct.ThrowIfCancellationRequested();
        if (FindAccount(account.Id) is null)
            throw new UserError(L.T("Account not found."));
        // Roblox's notification-area copy (left when a window was closed, ours or not) would take this launch over and
        // open no window ("didn't rejoin: no Roblox window appeared"; Bloxstrap: "Roblox failed to launch"): ended first.
        if (RobloxProcesses.CloseTrayCopies() > 0)
            await Task.Delay(TimeSpan.FromMilliseconds(500), ct);
        var before = RobloxProcesses.GetPlayerPids().ToHashSet();
        var settings = RequireVault().Data.Settings;
        var config = settings.RobloxConfig;
        bool windowed = settings.MultiInstance && settings.MultiInstanceWindowed;
        // A light-AFK preset gives this client its own values, whatever the shared setup says; the next client without
        // it gets the person's own values back (PrepareFor writes them from the copy it keeps).
        bool afk = PresetFor(presetId)?.Profile == AfkProfile;
        if (afk || config.Mode != "off" || windowed || RobloxGameSettings.HasBaseline)
        {
            bool use = afk || config.Mode == "all" || (config.Mode == "marked" && account.UseRobloxConfig);
            if (RobloxGameSettings.PrepareFor(afk ? RobloxGameSettings.LightAfk : config, use, windowed))
            {
                _robloxSettingsPending = true;
                _robloxSettingsWrittenAt = DateTime.UtcNow;
            }
        }
        string tracker = RobloxLauncher.Launch(ticket, target, launchVia, _vault?.Data.Settings.CustomLauncher ?? "");
        _lastSessionStart = DateTime.UtcNow;
        _userClosed.Remove(account.Id);
        _late.RemoveWhere(l => l.AccountId == account.Id); // opened again: a window of the earlier try is not this one's
        _pending.Add(new PendingLaunch(account.Id, before, DateTime.UtcNow, target, rejoin, presetId) { Tracker = tracker });
        NoteActivity(restart ? "reopened" : rejoin ? "rejoined" : "opened", account.Id, target?.PlaceId); // Statistics › Activity
        account.LastUsed = DateTime.Now;
        SignInAccepted(account);
    }

    object? CancelLaunch()
    {
        _launchCts?.Cancel();
        return null;
    }

    /// <param name="force">End the process at once (a frozen client) instead of asking its windows to close first.</param>
    object CloseAccounts(Guid[] ids, bool force)
    {
        int closed = 0;
        foreach (var id in ids)
        {
            _rejoinQueue.RemoveAll(r => r.AccountId == id);
            _rejoinsInARow.Remove(id); // closed by the person: a client opened later counts its drops from one
            // Still opening (no window yet): its client is closed as soon as it shows up, instead of staying open
            // untracked.
            foreach (var launch in _pending.Where(p => p.AccountId == id))
            {
                if (_closeWhenClaimed.Add(launch))
                {
                    closed++;
                    PushState();
                }
            }
            if (!_running.TryGetValue(id, out var info))
                continue;
            _userClosed.Add(id);
            RobloxProcesses.Close(info.Pid, force || InGameNow(info.Pid));
            closed++;
        }
        return closed;
    }

    object CloseAll(bool force)
    {
        // Also what is still opening: the launch in progress and a rejoin getting its ticket.
        _launchCts?.Cancel();
        _rejoinCts?.Cancel();
        foreach (var id in _running.Keys)
            _userClosed.Add(id);
        _rejoinQueue.Clear();
        _rejoinsInARow.Clear();
        _handedOver.Clear();
        // Clients still opening (a launcher downloading an update) would show up later, untracked: they are closed
        // as soon as they appear instead, like a single account closed while opening.
        if (_pending.Count > 0)
        {
            _closeWhenClaimed.UnionWith(_pending);
            PushState();
        }
        _closeWhenClaimed.UnionWith(_late);
        // Roblox starts a windowless background client when its last window closes; sweep those for a moment
        // so "close all" really leaves nothing running and multi-instance can re-arm.
        _sweepBackgroundUntil = DateTime.UtcNow + BackgroundSweep;
        return RobloxProcesses.CloseAll(force, InGameNow);
    }

    /// <summary>Launching while the sweep runs would get the new client killed, so wait it out.</summary>
    async Task WaitForSweepAsync(CancellationToken ct = default)
    {
        var remaining = _sweepBackgroundUntil - DateTime.UtcNow;
        if (remaining <= TimeSpan.Zero)
            return;
        Progress(L.T("Waiting for Roblox to finish closing…"));
        await Task.Delay(remaining, ct);
    }

    /// <summary>Tiles every Roblox window on one monitor: tracked accounts first (by name), then any other clients.</summary>
    /// <param name="auto">Tiling after a launch: minimised windows (a light-AFK preset's) stay minimised and get no tile.</param>
    async Task<object?> ArrangeWindowsAsync(int monitor, bool auto = false)
    {
        var screens = Screen.AllScreens;
        var screen = monitor >= 0 && monitor < screens.Length ? screens[monitor] : Screen.PrimaryScreen ?? screens[0];
        var tracked = _running
            .OrderBy(r => FindAccount(r.Key)?.Label ?? "", StringComparer.CurrentCultureIgnoreCase)
            .Select(r => r.Value.Pid)
            .ToList();
        var others = RobloxProcesses.GetPlayerPids().Where(p => !tracked.Contains(p)).OrderBy(p => p);
        // After a launch, a light-AFK preset's windows get no tile: they are minimised once in their game.
        var afk = auto ? AfkClients() : [];
        // Clicked in Bloxkeep: the windows come over every other app too (not after a launch: the person may be busy
        // elsewhere by then).
        var app = !auto && _owner.IsHandleCreated && _owner.Visible && _owner.WindowState != FormWindowState.Minimized ? _owner.Handle : IntPtr.Zero;
        var s = _vault?.Data.Settings;
        return await RobloxProcesses.ArrangeGridAsync(tracked.Concat(others).Where(p => !afk.Contains(p)).ToList(), screen.WorkingArea,
            s?.WindowLayout ?? "fill", skipMinimized: auto, app: app, custom: s is null ? default : new Size(s.WindowWidth, s.WindowHeight));
    }

    /// <summary>The clients a light-AFK preset opened (by process id).</summary>
    HashSet<int> AfkClients() =>
        _running.Values.Where(r => PresetFor(r.Launch.PresetId)?.Profile == AfkProfile).Select(r => r.Pid).ToHashSet();

    /// <summary>
    /// Multi-Roblox's "tile" option, or the preset's own Arrange windows: once the launch's last window is up, two or
    /// more windows are tiled on the main screen.
    /// </summary>
    /// <param name="presetId">The preset that launched: its choice decides (asked again at the end, in case it changed).</param>
    async Task TileWhenOpenAsync(Guid? presetId = null)
    {
        var deadline = DateTime.UtcNow + PendingTimeout;
        while (!_disposed && (_pending.Count > 0 || _launchCts is not null) && DateTime.UtcNow < deadline)
            await Task.Delay(500);
        // Roblox sizes its window again as it finishes starting up.
        await Task.Delay(3000);
        if (_disposed || _vault is not { } vault || _launchCts is not null
            || !(PresetFor(presetId)?.Arrange ?? vault.Data.Settings is { MultiInstance: true, MultiInstanceTile: true }))
            return; // locked, switched off (in Settings or the preset), or another launch started (it tiles when it is done)
        // Two or more windows that get a tile: minimised ones and a light-AFK preset's don't (see ArrangeWindowsAsync),
        // so one window isn't stretched over the whole screen as a grid of one.
        var afk = AfkClients();
        if (RobloxProcesses.VisibleWindows(RobloxProcesses.GetPlayerPids())
                .Count(w => !afk.Contains(w.Key) && w.Value.Count > 0 && !RobloxProcesses.IsMinimised(RobloxProcesses.MainWindowOf(w.Value))) >= 2)
            await ArrangeWindowsAsync(-1, auto: true);
    }
}
