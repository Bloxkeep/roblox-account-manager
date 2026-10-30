using System.ComponentModel;

namespace RobloxManager;

/// <summary>Auto-rejoin: a window that ended, why, whether it goes back, and going back (join checks, game bans, waits).</summary>
public sealed partial class AppController
{
    void OnSessionEnded(Guid accountId, PendingLaunch launch, RobloxLogWatcher.ClientState? lastLog = null)
    {
        NoteSessionEnd(accountId, launch, lastLog); // before auto-rejoin reads (and forgets) whether Roblox closed it
        _offlineStrikes.Remove(accountId);
        // A game ban in its last lines, unseen before the window closed: noted all the same, or the next preset would
        // open the account into Join Error 600.
        if (lastLog is { BannedByGame: true } && FindAccount(accountId) is { } banned)
            NoteGameBan(banned, lastLog.ChosenPlace ?? launch.Target?.PlaceId ?? lastLog.PlaceId, lastLog.KickMessage, launch.StartedAt);
        // Closed for good: a scheduled stop no longer owns the account, so a client opened by hand later stays open.
        if (!QueueRejoin(accountId, launch, lastLog))
            ForgetScheduledOpen(accountId);
    }

    /// <returns>true when auto-rejoin (or the preset's planned restart) will bring the client back.</returns>
    bool QueueRejoin(Guid accountId, PendingLaunch launch, RobloxLogWatcher.ClientState? lastLog)
    {
        bool outOfMemory = _ranOutOfMemory.Remove(accountId);
        bool restart = _restarts.Remove(accountId, out var restarting);
        if (!ShouldRejoin(accountId, launch, lastLog, restart, out var account))
        {
            _rejoinsInARow.Remove(accountId); // closed for good: a client opened later counts from one again
            return false;
        }
        _rejoinQueue.RemoveAll(r => r.AccountId == accountId);
        if (restart)
        {
            // Not a drop: not counted, and goes ahead whatever auto-rejoin says.
            _rejoinQueue.Add(new RejoinRequest(accountId, restarting.Target, DateTime.UtcNow + RejoinDelay, PresetId: launch.PresetId, Restart: true));
            return true;
        }
        int inARow = _rejoinsInARow.GetValueOrDefault(accountId);
        _rejoinsInARow[accountId] = inARow + 1;
        _followWaitTold.Remove(accountId); // a new drop: waiting for its followed player is said again
        var wait = RejoinDelay;
        // The same game or private server, not the exact server, which may be gone. A game the person picked in the
        // window since wins over the one opened. A followed player is looked up again (FollowTargetAsync).
        var target = lastLog?.ChosenPlace is { } chosen && chosen != launch.Target!.PlaceId
            ? new GameTarget(chosen) { Follow = launch.Target!.Follow }
            : launch.Target! with { JobId = null };
        _rejoinQueue.Add(new RejoinRequest(accountId, target, DateTime.UtcNow + wait, PresetId: launch.PresetId));
        if (inARow == KeepsDropping)
            // Said once; it goes on trying all the same.
            Notify(L.F("{0} keeps getting disconnected — Bloxkeep keeps rejoining every {1}s.", Name(account), (int)wait.TotalSeconds), "warn", "rejoinSlow", account.Id);
        // Its rejoin waits for room (RunDueRejoin).
        else if (outOfMemory)
            Notify(L.F("{0} ran out of memory and Roblox crashed — rejoining in {1}s…", Name(account), (int)wait.TotalSeconds), "warn", "drop", account.Id);
        else
            Notify(L.F("{0} left the game — rejoining in {1}s…", Name(account), (int)wait.TotalSeconds), "info", "drop", account.Id);
        return true;
    }

    bool ShouldRejoin(Guid accountId, PendingLaunch launch, RobloxLogWatcher.ClientState? lastLog, bool restart,
        [System.Diagnostics.CodeAnalysis.NotNullWhen(true)] out Account? account)
    {
        account = null;
        if (_userClosed.Remove(accountId))
            return false;
        // Roblox closed it when another window opened: a rejoin would only close the next one (AppController.RobloxUpdates.cs).
        if (_closedByRoblox.Remove(accountId))
            return false;
        // Roblox refused the account its game (RejoinFromLogs): a new window would only be refused again.
        if (_joinRefused.Remove(launch))
            return false;
        if (FindAccount(accountId) is not { } found || launch.Target is null)
            return false;
        account = found;
        // Banned: a new window would only show the ban again, every few minutes for good.
        if (found.Banned == true)
            return false;
        // Banned by the game: only Join Error 600 again.
        if (lastLog is { BannedByGame: true } || GameBanOf(found, RejoinPlace(launch, lastLog)) is not null)
            return false;
        if (restart)
            return true;
        if (!RejoinOn(found, launch.PresetId))
            return false;
        // A leave by choice, a sign-in from another device, or a code Roblox won't reconnect after: not a drop.
        return lastLog is not { Status: "left" or "signedout" }
            && !(lastLog is { Status: "disconnected", Reason: var why } && (SignedInElsewhere(why) || RobloxWontReconnect(why)));
    }

    /// <summary>
    /// Rejoins in a row after which "keeps getting disconnected" is said, once. Every rejoin waits
    /// <see cref="RejoinDelay"/> however many came before (the owner's choice, 2026-10-21): a game's ban stops them instead.
    /// </summary>
    const int KeepsDropping = 2;
    /// <summary>A client in its game this long is fine again: its next drop counts from one.</summary>
    static readonly TimeSpan StayResetsRejoins = TimeSpan.FromMinutes(10);

    void ForgetSettledRejoins(DateTime now)
    {
        if (_rejoinsInARow.Count == 0)
            return;
        foreach (var (id, info) in _running)
        {
            if (!_rejoinsInARow.ContainsKey(id))
                continue;
            // From the window's start, not its last server: a game that teleports every few minutes would never
            // settle, and three drops in a day would read as "keeps getting disconnected".
            bool settled = _logs.StateOf(info.Pid) is { } log
                ? log.Status == "ingame" && now - info.Launch.StartedAt >= StayResetsRejoins
                : now - info.Launch.StartedAt >= StayResetsRejoins && FindAccount(id) is { } account
                  && _presence.TryGetValue(account.UserId, out var presence) && presence.Type == 2;
            if (settled)
                _rejoinsInARow.Remove(id);
        }
    }

    object? CancelRejoin(Guid id)
    {
        _rejoinsInARow.Remove(id);
        if (_rejoinQueue.RemoveAll(r => r.AccountId == id) > 0)
            ForgetScheduledOpen(id); // closed for good now
        PushState();
        return null;
    }

    /// <summary>
    /// Acts on a kick or lost connection as soon as the client logs it, rather than after two presence checks, and on
    /// a game start that never got a server (<see cref="GetInTimeout"/>).
    /// </summary>
    void RejoinFromLogs()
    {
        // First, also with nothing open: a process id Windows gives a later client mustn't come with an old one's answer.
        foreach (int gone in _joinChecks.Keys.Where(pid => _running.Values.All(r => r.Pid != pid)).ToList())
            _joinChecks.Remove(gone);
        _gameBanNoted.RemoveWhere(pid => _running.Values.All(r => r.Pid != pid));
        _noRejoinTold.RemoveWhere(pid => _running.Values.All(r => r.Pid != pid));
        foreach (int gone in _inLine.Keys.Where(pid => _running.Values.All(r => r.Pid != pid)).ToList())
            _inLine.Remove(gone);
        _joinRefused.RemoveWhere(launch => _running.Values.All(r => !ReferenceEquals(r.Launch, launch)));
        if (_vault is null || _running.Count == 0 || Settling)
            return;
        var now = DateTime.UtcNow;
        foreach (var (id, info) in _running.ToList())
        {
            if (_logs.StateOf(info.Pid) is not { } state || _closingForRejoin.Contains(info.Pid))
                continue;
            // A rejoin of a hidden window: hidden again once in its game.
            if (state.Status == "ingame" && _hideWhenIn.Remove(id) && info.Launch.Rejoin)
            {
                RobloxProcesses.HideWindows([info.Pid]);
                PushState();
            }
            // Got a server since, or the person started a game: no longer held as banned.
            if (state is { Status: "ingame" } or { Status: "joining", JobId: not null } or { Status: "joining", PersonStarted: true })
            {
                _gameBanNoted.Remove(info.Pid);
                _joinChecks.Remove(info.Pid);
            }
            // Banned by the game (code 267 as for any kick): a new window would only meet Join Error 600. The window
            // stays, with Roblox's message. The ban is noted for the game a rejoin would open, not the place it was in.
            if (state is { Status: "disconnected", BannedByGame: true })
            {
                if (_gameBanNoted.Add(info.Pid) && FindAccount(id) is { } banned)
                    NoteGameBan(banned, state.ChosenPlace ?? info.Launch.Target?.PlaceId ?? state.PlaceId, state.KickMessage, info.Launch.StartedAt);
                continue;
            }
            // Held for Roblox's bot check (RobloxLogWatcher.ChallengedMark): closing and reopening it looped into the same
            // check for good. The window stays, why is said once, and it isn't rejoined once closed.
            if (state is { Status: "joining", JobId: null, PersonStarted: false } && _logs.JoinChallengedAt(info.Pid) is not null)
            {
                if (_joinRefused.Add(info.Launch) && FindAccount(id) is { } challenged && RejoinOn(challenged, info.Launch.PresetId))
                    Notify(L.F("{0} can't get into the game: Roblox wants a check that it isn't a bot first, so Bloxkeep leaves the window open and doesn't try again.",
                        Name(challenged)), "warn", "drop", challenged.Id);
                continue;
            }
            // Not in after JoinCheckAfter: Roblox says why only in the window (a game's ban is Join Error 600) and logs
            // nothing, so its join API is asked. A ban or refusal stops auto-rejoin; a place in a full game's line keeps
            // the window; any other answer leaves it to GetInTimeout.
            if (state is { Status: "joining", JobId: null, PersonStarted: false } && info.Launch.Target is { } wanted
                && FindAccount(id) is { Cookie.Length: > 0 } asking && now - state.At >= JoinCheckAfter(asking, wanted) && !_gameBanNoted.Contains(info.Pid))
            {
                // Asked again after JoinCheckRetry without an answer (often no network after a wake), and every
                // QueueRecheck while in line.
                bool inLine = _joinChecks.TryGetValue(info.Pid, out var asked) && asked.Answer is { IsCompletedSuccessfully: true, Result.QueuePosition: > 0 };
                if (asked.Answer is null
                    || asked.Answer is { IsCompleted: true } and not { IsCompletedSuccessfully: true, Result: not null } && now - asked.At >= JoinCheckRetry
                    || inLine && now - asked.At >= QueueRecheck)
                {
                    if (inLine)
                        _inLine[info.Pid] = asked.Answer!.Result!;
                    else
                        _inLine.Remove(info.Pid);
                    _joinChecks[info.Pid] = (CheckJoinAsync(asking, wanted, inLine ? "A join in a full game's line, asked again" : "A join that never got in, asked again"), now);
                    continue;
                }
                var check = asked.Answer;
                if (check is { IsCompletedSuccessfully: true, Result.Banned: true })
                {
                    _gameBanNoted.Add(info.Pid);
                    NoteGameBan(asking, wanted.PlaceId, check.Result.Reason, info.Launch.StartedAt);
                    continue;
                }
                if (check.IsCompletedSuccessfully)
                    ForgetBanIfLetIn(asking, wanted.PlaceId, check.Result);
                // A private server's join is asked as its public game's, so only a ban counts from that answer.
                if (check is { IsCompletedSuccessfully: true, Result: { } said } && wanted is { LinkCode: null, ShareCode: null, AccessCode: null })
                {
                    // A new window would only be refused again, every minute for good. The window stays; why is said once.
                    if (said.Refused)
                    {
                        if (_joinRefused.Add(info.Launch) && RejoinOn(asking, info.Launch.PresetId))
                            Notify(L.F("{0} can't get into the game, so Bloxkeep doesn't try again: {1}", Name(asking),
                                said.Message ?? L.F("Roblox answered status {0}", said.Status)), "warn", "drop", asking.Id);
                        continue;
                    }
                    // In a full game's line: a new window would start at its end.
                    if (said.QueuePosition is > 0)
                        continue;
                }
            }
            // Stuck on a game's ban (Join Error 600): left as it is, not closed by the timeout below.
            if (_gameBanNoted.Contains(info.Pid))
                continue;
            // Roblox's error box has no Reconnect for this code: the window stays, and why is said once (not for a drop
            // from before this run).
            if (state is { Status: "disconnected", Reason: int refused } && RobloxWontReconnect(refused))
            {
                if (_noRejoinTold.Add(info.Pid) && state.At >= _started && FindAccount(id) is { } kept && RejoinOn(kept, info.Launch.PresetId))
                    Notify(L.F("{0} isn't rejoined ({1}): Roblox doesn't allow reconnecting after this.", Name(kept), KickReason(refused)), "warn", "drop", kept.Id);
                continue;
            }
            bool dropped = state is { Status: "disconnected" } && !SignedInElsewhere(state.Reason) && now - state.At >= TimeSpan.FromSeconds(3);
            // One the person started in the window is theirs: closing it would reopen the game Bloxkeep opened instead.
            bool neverGotIn = state is { Status: "joining", JobId: null, PersonStarted: false } && now - state.At >= GetInTimeout;
            if ((!dropped && !neverGotIn)
                || FindAccount(id) is not { } account || !RejoinOn(account, info.Launch.PresetId) || info.Launch.Target is not { } target
                // No rejoin follows (ShouldRejoin): the window stays rather than closing for nothing.
                || account.Banned == true || GameBanOf(account, RejoinPlace(info.Launch, state)) is not null)
                continue;
            // Not trying again before Roblox's answer is in (asked at JoinCheckAfter, above).
            if (neverGotIn && _joinChecks.TryGetValue(info.Pid, out var pending) && !pending.Answer.IsCompleted)
                continue;
            _closingForRejoin.Add(info.Pid);
            // Said as what it was: on 2026-10-21 a game's "DataStore lock lost, please rejoin the game." went out as
            // "Lost connection".
            Notify(!dropped ? (target.JobId is not null ? L.F("{0} couldn't get into that server (full or closed) — joining another one…", Name(account))
                    : L.F("{0} couldn't get into the game — trying again…", Name(account)))
                : state.KickMessage is { Length: > 0 } words ? L.F("{0} was kicked by the game, rejoining: {1}", Name(account), words)
                : state.Reason == 267 ? L.F("{0} was kicked by the game — rejoining…", Name(account))
                : state.Reason == ServerShutdown ? L.F("{0}'s server shut down — joining another one…", Name(account))
                : state.Reason is int lost && LostConnection(lost) ? L.F("{0} lost connection — restarting Roblox…", Name(account))
                : state.Reason is int other ? L.F("{0} was disconnected ({1}) — restarting Roblox…", Name(account), KickReason(other))
                : L.F("{0} dropped out of the game, reason unknown — restarting Roblox…", Name(account)), "info", "drop", account.Id);
            RobloxProcesses.Close(info.Pid);
        }
    }

    /// <summary>A field, so the checks can answer for Roblox.</summary>
    Func<Account, long, CancellationToken, Task<Playability?>> _askPlayability =
        (account, universeId, ct) => RobloxApi.GetPlayabilityAsync(account.Cookie, account.Realm, universeId, ct);
    /// <summary>No answer in time, no question.</summary>
    static readonly TimeSpan PlayabilityLimit = TimeSpan.FromSeconds(6);

    /// <summary>
    /// The accounts Roblox says can't play the game, with why. An unknown game, no sign-in or no answer in time: none.
    /// </summary>
    async Task<List<(Account Account, string Why)>> UnplayableAsync(List<Account> accounts, long placeId, CancellationToken ct)
    {
        var asking = accounts.Where(a => a.Cookie.Length > 0 && a.CookieValid != false).ToList();
        if (asking.Count == 0)
            return [];
        using var limit = CancellationTokenSource.CreateLinkedTokenSource(ct);
        limit.CancelAfter(PlayabilityLimit);
        try
        {
            if (!HasGame(placeId, false))
                await LoadGamesAsync([(placeId, 0L)]).WaitAsync(limit.Token);
            if (_games.GetValueOrDefault(placeId)?.UniverseId is long universe && universe > 0)
            {
                using var gate = new SemaphoreSlim(8);
                async Task<(Account Account, Playability? Answer)> Ask(Account account)
                {
                    try
                    {
                        await gate.WaitAsync(limit.Token);
                        try
                        {
                            return (account, await _askPlayability(account, universe, limit.Token));
                        }
                        finally
                        {
                            gate.Release();
                        }
                    }
                    catch (Exception ex) when (RobloxApi.IsExpectedError(ex) || ex is OperationCanceledException && !ct.IsCancellationRequested)
                    {
                        return (account, null);
                    }
                }
                var answers = await Task.WhenAll(asking.Select(Ask));
                return answers.Where(x => x.Answer is { Playable: false }).Select(x => (x.Account, PlayabilityWhy(x.Answer!))).ToList();
            }
        }
        catch (OperationCanceledException) when (!ct.IsCancellationRequested)
        {
        }
        return [];
    }

    static string PlayabilityWhy(Playability answer) => answer.Status switch
    {
        2 => L.T("Roblox doesn't take this account's sign-in"),
        3 or 9 => L.T("the game is under review by Roblox"),
        4 or 13 => L.T("the game isn't set up to be played"),
        5 => L.T("the game is private"),
        6 => L.T("only the creator's friends can play it"),
        7 => L.T("only the group's members can play it"),
        8 or 22 => L.T("it can't be played on this device"),
        10 or 21 => L.T("the game has to be bought first"),
        11 => L.T("Roblox has restricted this account"),
        12 => L.T("the game is temporarily unavailable"),
        14 or 15 or 16 => L.T("it isn't available in this region"),
        17 or 18 or 27 => L.T("parental controls block it"),
        19 or 20 or 24 or 26 => L.T("it needs an age check"),
        23 => L.T("the game has no age rating yet"),
        29 => L.T("it needs a trusted friend"),
        30 => L.T("it needs Roblox Plus"),
        31 or 32 or 33 => L.T("only the game's editors or testers can play it"),
        _ when answer.Text is { } said => said.TrimEnd('.'),
        _ => L.F("Roblox says no (status {0})", answer.Status?.ToString(System.Globalization.CultureInfo.InvariantCulture) ?? "?"),
    };

    readonly HashSet<int> _gameBanNoted = new();
    readonly HashSet<int> _noRejoinTold = new();
    readonly Dictionary<int, (Task<RobloxApi.JoinAnswer?> Answer, DateTime At)> _joinChecks = new();
    static readonly TimeSpan JoinCheckRetry = TimeSpan.FromSeconds(30);
    static readonly TimeSpan QueueRecheck = TimeSpan.FromSeconds(60);
    /// <summary>The answer that gave a client its place in a full game's line, shown until the new one.</summary>
    readonly Dictionary<int, RobloxApi.JoinAnswer> _inLine = new();
    /// <summary>
    /// Launches refused (<see cref="RobloxApi.JoinAnswer.Refused"/>) or held for a bot check: not rejoined once they end.
    /// By reference, not value: a launch's start time moves with a set-back clock.
    /// </summary>
    readonly HashSet<PendingLaunch> _joinRefused = new(ReferenceEqualityComparer.Instance);
    static readonly TimeSpan GameBanRecheckLimit = TimeSpan.FromSeconds(6);

    /// <summary>A field, so the checks can answer for Roblox.</summary>
    Func<Account, GameTarget, CancellationToken, Task<RobloxApi.JoinAnswer>> _askJoin =
        (account, target, ct) => RobloxApi.JoinCheckAsync(account.Cookie, target.PlaceId, target.JobId, account.Realm, ct);

    /// <summary>
    /// Asked as a Roblox window asks when joining: a game's ban comes back as its message. The answer is logged after
    /// <paramref name="why"/> (no account, no ticket) as proof of what Roblox answers.
    /// </summary>
    async Task<RobloxApi.JoinAnswer?> CheckJoinAsync(Account account, GameTarget target, string why, CancellationToken ct = default)
    {
        try
        {
            var answer = await _askJoin(account, target, ct);
            // A server named and refused without a ban: that server is gone (RobloxApi.JoinAnswer.ServerGone).
            if (target.JobId is { Length: > 0 } && answer.Status == 12 && !answer.Banned)
                answer = answer with { ServerGone = true };
            Log.Note($"{why}: Roblox answered {answer.Http}, status {answer.Status}: {answer.Message ?? "(no message)"}"
                + (answer.Data is { } data ? $" · statusData {data}" : "") + (answer.Banned ? " (a ban by the game)" : "")
                + (answer.ServerGone ? " (that server is gone)" : ""));
            // No status: the request itself was refused (rate limit, server error, sign-in), so it is asked again.
            return answer.Status is null && !answer.Banned ? null : answer;
        }
        catch (Exception ex)
        {
            Log.Note($"{why}: no answer ({ex.GetType().Name}).");
            return null;
        }
    }

    /// <summary>Status 2 (a join under way): a ban noted for this game is over.</summary>
    bool ForgetBanIfLetIn(Account account, long placeId, RobloxApi.JoinAnswer? answer)
    {
        if (answer is not { Banned: false, Status: 2 } || account.GameBans.RemoveAll(b => SameGame(b.PlaceId, placeId)) == 0)
            return false;
        PersistSoon();
        PushState();
        return true;
    }

    /// <summary>
    /// A game's ban may last a day: before opening, Roblox's join answer says whether a noted one is over. No answer
    /// within <see cref="GameBanRecheckLimit"/> keeps the ban.
    /// </summary>
    async Task RecheckGameBansAsync(List<Account> accounts, GameTarget target, CancellationToken ct)
    {
        var asking = accounts.Where(acc => GameBanOf(acc, target.PlaceId) is not null && acc.Cookie.Length > 0).ToList();
        if (asking.Count == 0)
            return;
        using var limit = CancellationTokenSource.CreateLinkedTokenSource(ct);
        limit.CancelAfter(GameBanRecheckLimit);
        Progress(L.T("Asking Roblox whether the game's ban still stands…"));
        var answers = await Task.WhenAll(asking.Select(acc => CheckJoinAsync(acc, target, "A game's ban noted before, asked before opening", limit.Token)));
        Progress(null);
        for (int i = 0; i < asking.Count; i++)
            ForgetBanIfLetIn(asking[i], target.PlaceId, answers[i]);
    }

    /// <summary>
    /// Noted with the account so auto-rejoin stops trying that game, and notified. Getting in again later drops it
    /// (<see cref="ForgetLiftedGameBans"/>).
    /// </summary>
    /// <param name="since">When the window opened: a ban noted since is this one, seen again after Bloxkeep restarted
    /// with the window still open, and isn't told again.</param>
    void NoteGameBan(Account account, long? placeId, string? message, DateTime since)
    {
        if (placeId is not long place || place <= 0)
            return;
        var known = GameBanOf(account, place);
        if (known is not null && known.At >= since)
        {
            if (known.Message is null && message is not null)
            {
                known.Message = message;
                PersistSoon();
                PushState();
            }
            return;
        }
        // Roblox's join answer carries none of the game's words: the earlier ones are kept.
        message ??= known?.Message;
        string game = _games.GetValueOrDefault(place)?.Name is { Length: > 0 } name ? name : L.F("Place {0}", place);
        account.GameBans.RemoveAll(b => SameGame(b.PlaceId, place));
        account.GameBans.Add(new GameBan { PlaceId = place, GameName = _games.GetValueOrDefault(place)?.Name, At = DateTime.UtcNow, Message = message });
        PersistSoon();
        Notify(message is { Length: > 0 }
            ? L.F("{0} was banned from {1} by the game: {2}", Name(account), game, message)
            : L.F("{0} was banned from {1} by the game.", Name(account), game), "error", "gameBan", account.Id);
        PushState();
    }

    /// <summary>
    /// Only a window the join answer showed stuck on the ban (Join Error 600): one opened into a game that banned the
    /// account once may well get in.
    /// </summary>
    GameBan? BannedWindow(Account account, (int Pid, PendingLaunch Launch) run) =>
        _gameBanNoted.Contains(run.Pid) && _logs.StateOf(run.Pid) is { Status: "joining", JobId: null, PersonStarted: false }
            ? GameBanOf(account, run.Launch.Target?.PlaceId) : null;

    static long RejoinPlace(PendingLaunch launch, RobloxLogWatcher.ClientState? lastLog) =>
        lastLog?.ChosenPlace ?? launch.Target?.PlaceId ?? 0;

    /// <summary>Any place of the game: a game's ban holds in all of them.</summary>
    GameBan? GameBanOf(Account account, long? placeId) =>
        placeId is long place ? account.GameBans.FirstOrDefault(b => SameGame(b.PlaceId, place)) : null;

    void ForgetLiftedGameBans()
    {
        foreach (var (id, info) in _running)
        {
            if (_logs.StateOf(info.Pid) is not { Status: "ingame", PlaceId: long here } || FindAccount(id) is not { GameBans.Count: > 0 } account)
                continue;
            // The game it is in now, not the one it was opened into.
            if (account.GameBans.RemoveAll(b => SameGame(b.PlaceId, here)) > 0)
            {
                PersistSoon();
                PushState();
            }
        }
    }


    void RunDueRejoin(DateTime now)
    {
        // One client at a time, after the wait between accounts. A multi-account launch lets rejoins go while it waits
        // between two, except of the accounts it is about to open.
        if (_rejoinQueue.Count == 0 || _rejoining || _openingHome || (_launchCts is not null && _launchWaiting is null) || now < _sweepBackgroundUntil
            || _pending.Count > 0 || Settling)
            return;
        var due = _rejoinQueue.FirstOrDefault(r => r.Due <= now && _launchWaiting?.Contains(r.AccountId) != true);
        if (due is null || now - _lastSessionStart < RejoinSpacing(due))
            return;
        // Commit limit all but used up: a new window would crash as it opens (2026-10-22: "cannot commit OS memory",
        // OutOfMemoryGraphics) and rejoin into the same crash every 10 s. It waits for room; said once.
        if (_vault?.Data.Settings.MemoryWarnings != false && VirtualMemoryShort(_readMemory(), 1, ClientCommitMb(_lastPlayers?.Players ?? [])))
        {
            _rejoinQueue[_rejoinQueue.IndexOf(due)] = due with { Due = now + MemoryHoldRecheck };
            if (_memoryHeldTold.Add(due.AccountId) && FindAccount(due.AccountId) is { } held)
                Notify(L.F("{0} waits to rejoin: Windows' virtual memory is almost full, so a new Roblox window would crash as it opens. It rejoins once there's room.",
                    Name(held)), "warn", "rejoinFail", held.Id);
            PushState();
            return;
        }
        _memoryHeldTold.Remove(due.AccountId);
        _rejoinQueue.Remove(due);
        _ = RejoinAsync(due);
    }

    /// <summary>The preset's wait between accounts, else Settings'; 3 s at least.</summary>
    TimeSpan RejoinSpacing(RejoinRequest request) =>
        TimeSpan.FromSeconds(Math.Max(3, PresetFor(request.PresetId)?.DelaySeconds ?? _vault?.Data.Settings.LaunchDelaySeconds ?? 0));

    /// <summary>A field, so the checks can answer for Windows.</summary>
    Func<MemoryStatus> _readMemory = SystemMemory.Read;
    static readonly TimeSpan MemoryHoldRecheck = TimeSpan.FromSeconds(30);
    readonly HashSet<Guid> _memoryHeldTold = new();
    /// <summary>How long anti-AFK waits after a window was held on Roblox's bot check (its nudges would take the front).</summary>
    static readonly TimeSpan BotCheckHold = TimeSpan.FromMinutes(5);
    /// <summary>Accounts whose window was hidden when their client ended: the window their rejoin opens is hidden too.</summary>
    readonly HashSet<Guid> _hideWhenIn = new();
    readonly HashSet<Guid> _ranOutOfMemory = new();

    async Task RejoinAsync(RejoinRequest request)
    {
        var vault = _vault;
        var account = FindAccount(request.AccountId);
        // AppController.ServerHop.cs: taken here, even by a rejoin that stops below.
        bool hopping = _hopping.Remove(request.AccountId);
        if (vault is null || account is null || !request.Restart && !RejoinOn(account, request.PresetId) || account.Banned == true
            // Banned by that game since it was queued: only Join Error 600 again. A followed player's game is checked
            // once they are found (FollowTargetAsync).
            || request.Target.Follow is null && GameBanOf(account, request.Target.PlaceId) is not null
            || _running.ContainsKey(account.Id) || _pending.Any(p => p.AccountId == account.Id))
            return;
        _rejoining = true;
        var cts = _rejoinCts = new CancellationTokenSource();
        try
        {
            // Where a followed player is now. Not found (offline, hidden or full server): waits and looks again.
            var target = request.Target;
            if (target.Follow is not null)
            {
                if (await FollowTargetAsync(account, request, cts.Token) is not { } found)
                    return;
                target = found;
            }
            // The preset's "Rejoin server": the server another of its accounts plays in, if it has room.
            else if (!hopping && await TeammateTargetAsync(account, request, cts.Token) is { } teammate)
            {
                target = teammate;
            }
            // Banned while it played (the daily check would say so hours later): a new window would only show the ban
            // every few minutes. No clear answer lets the rejoin go on.
            try
            {
                if (await CheckBanAsync(account) == true)
                {
                    NotifyBanned([account]);
                    Persist();
                    PushState();
                    return;
                }
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
            }
            cts.Token.ThrowIfCancellationRequested();
            var players = RobloxProcesses.GetPlayerPids();
            if (vault.Data.Settings.MultiInstance && !MultiInstance.Armed && players.Length == 0)
                MultiInstance.Enable();
            // Without Multi-Roblox the new client closes the open one, which would then rejoin and close this one.
            if (!MultiInstance.Armed && RobloxProcesses.VisibleWindows(players).Values.Any(w => w.Count > 0))
            {
                Notify(L.F("Didn't rejoin {0}: Multi-Roblox isn't ready, so a new Roblox window would close the one that's open.", Name(account)), "warn", "rejoinFail", account.Id);
                return;
            }
            await StartSessionAsync(account, target, vault.Data.Settings.LaunchVia, cts.Token, rejoin: true, presetId: request.PresetId,
                restart: request.Restart);
            Persist();
            PushState();
        }
        catch (OperationCanceledException) when (cts.IsCancellationRequested)
        {
            // Close all.
        }
        catch (UserError)
        {
            // The account was deleted while the ticket was fetched.
        }
        catch (Exception ex) when ((ex is HttpRequestException or TaskCanceledException or RobloxApiException { RateLimited: true } or RobloxApiException { ServerError: true })
                                   && request.NetworkRetries < MaxNetworkRetries)
        {
            // No connection (often right after sleep) or Roblox busy: try again, each time later; not counted as a
            // rejoin. A refusal is reported below instead: silent retries wouldn't change it.
            var delay = TimeSpan.FromTicks(Math.Min(NetworkRetryDelay.Ticks * (request.NetworkRetries + 1), MaxRetryDelay.Ticks));
            _rejoinQueue.Add(request with { Due = DateTime.UtcNow + delay, NetworkRetries = request.NetworkRetries + 1 });
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex) || ex is Win32Exception or FileNotFoundException)
        {
            string? why = null;
            if (ex is RobloxApiException { CookieInvalid: true })
            {
                // One refused ticket isn't proof of a sign-out (an outage answers 401 for everyone): retried a few
                // times, then said as a refusal, auto-rejoin staying on.
                if (!await ConfirmSignedOutAsync(account))
                {
                    if (request.NetworkRetries < MaxTicketRetries)
                    {
                        var delay = TimeSpan.FromTicks(Math.Min(NetworkRetryDelay.Ticks * (request.NetworkRetries + 1), MaxRetryDelay.Ticks));
                        _rejoinQueue.Add(request with { Due = DateTime.UtcNow + delay, NetworkRetries = request.NetworkRetries + 1 });
                        return;
                    }
                    why = RefusedText;
                }
                else
                {
                    // Back on by itself once signed in again (SignedInAgain).
                    if (account.AutoRejoin)
                        account.RejoinOffBySignOut = true;
                    account.AutoRejoin = false;
                    Persist();
                    PushState();
                }
            }
            Notify(L.F("Couldn't rejoin {0}: {1}", Name(account), why ?? (ex is Win32Exception or FileNotFoundException ? StartErrorText(ex) : ErrorText(ex))), "error", "rejoinFail", account.Id);
        }
        catch (Exception ex)
        {
            // Said and logged rather than the account quietly not coming back.
            Notify(L.F("Couldn't rejoin {0}: {1}", Name(account), ErrorText(ex)), "error", "rejoinFail", account.Id);
        }
        finally
        {
            _rejoining = false;
            if (_rejoinCts == cts)
                _rejoinCts = null;
            cts.Dispose();
        }
    }

    /// <summary>
    /// The log is the better source while current; one stuck in "joining" is left to Roblox's status again, unless
    /// the person started that game (the status check would close it and reopen Bloxkeep's).
    /// </summary>
    bool LogFollowed(int pid) =>
        _logs.StateOf(pid) is { } log && !(log is { Status: "joining", PersonStarted: false } && DateTime.UtcNow - log.At > JoiningTimeout);

    /// <summary>
    /// Bloxkeep's own close makes Roblox log a leave first; that leave is not the person's, so auto-rejoin still brings
    /// the client back. 2026-10-17 (0.743): five clients closed this way logged leaveUGCGameInternal and none came back.
    /// </summary>
    RobloxLogWatcher.ClientState? EndedState(int pid)
    {
        var log = _logs.LastStateOf(pid);
        return log is { Status: "left" } && _closingForRejoin.Contains(pid) ? log with { Status = "disconnected", Reason = null } : log;
    }

    /// <summary>
    /// A client open while its account was out of a game for two presence checks is usually on a kick screen: closed
    /// so auto-rejoin starts a fresh one.
    /// </summary>
    /// <param name="reliable">Accounts whose status came from their own cookie or shows them in a game.</param>
    void DetectDisconnectedSessions(HashSet<long> reliable)
    {
        // Right after sleep everyone looks offline until the network is back; that says nothing about the clients.
        if (Settling)
        {
            _offlineStrikes.Clear();
            return;
        }
        foreach (var (id, info) in _running.ToList())
        {
            var account = FindAccount(id);
            // The log is the better source (RejoinFromLogs); a strike from before it was found says nothing.
            if (LogFollowed(info.Pid))
            {
                _offlineStrikes.Remove(id);
                continue;
            }
            // A status hidden by privacy settings or a failed request proves nothing; wait for a clear answer.
            if (account is not null && !reliable.Contains(account.UserId))
                continue;
            // Nor one a game banned (Join Error 600, log silent): on 2026-10-21 one was closed here as "Lost connection"
            // two minutes after its ban was told.
            if (account is null || !RejoinOn(account, info.Launch.PresetId) || account.Banned == true || info.Launch.Target is null || DateTime.UtcNow - info.Launch.StartedAt < DisconnectGrace
                || GameBanOf(account, RejoinPlace(info.Launch, _logs.StateOf(info.Pid))) is not null
                || !_presence.TryGetValue(account.UserId, out var presence) || presence.Type is 2 or 4)
            {
                _offlineStrikes.Remove(id);
                continue;
            }
            int strikes = _offlineStrikes[id] = _offlineStrikes.GetValueOrDefault(id) + 1;
            if (strikes < 2)
                continue;
            _offlineStrikes.Remove(id);
            _closingForRejoin.Add(info.Pid); // Bloxkeep's own close: never taken for Roblox closing it
            Notify(L.F("{0} dropped out of the game, reason unknown — restarting Roblox…", Name(account)), "info", "drop", account.Id);
            RobloxProcesses.Close(info.Pid);
        }
    }
}
