namespace RobloxManager;

/// <summary>
/// Play-time statistics: one <see cref="PlaySession"/> per stay in a game by a client Bloxkeep opened (or picked up),
/// kept 90 days. A stay starts when the log says it is joining a server (or at launch, with no log) and ends when the log
/// says the person left or was dropped, or the client quits; a teleport between a game's places is the same stay.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan PlayHistory = TimeSpan.FromDays(90);
    const int MaxPlaySessions = 4000;
    /// <summary>The last month's stays are kept past <see cref="MaxPlaySessions"/> (the 30-day view needs them all), up to this many.</summary>
    const int MaxRecentPlaySessions = 20000;
    static readonly TimeSpan RecentPlay = TimeSpan.FromDays(31);
    const int MaxTopGames = 8;
    /// <summary>A client still without a log after this long is counted from its launch, as playing what it was opened with.</summary>
    static readonly TimeSpan NoLogGrace = TimeSpan.FromSeconds(45);
    /// <summary>The open stay of each account with a client here (the same objects as in the vault's list).</summary>
    readonly Dictionary<Guid, PlaySession> _playing = new();
    /// <summary>Clients that quit while Bloxkeep was locked: how their stay ended, applied at the next unlock.</summary>
    readonly Dictionary<Guid, (string Reason, int? Code, DateTime At)> _endedWhileLocked = new();
    /// <summary>Open stays are written this often, so a crash of Bloxkeep (or of the PC) loses minutes, not hours.</summary>
    static readonly TimeSpan PlaySaveInterval = TimeSpan.FromMinutes(5);
    long _lastPlaySaveTick;
    /// <summary>Process checks come every few seconds; a gap this long means the PC was asleep in between.</summary>
    static readonly TimeSpan SleepGap = TimeSpan.FromSeconds(30);
    /// <summary>When the last process check ran (<see cref="Environment.TickCount64"/>, which counts on through a sleep).</summary>
    long _lastCheckTick;
    /// <summary>When Windows said the PC is going to sleep (0: awake): a check squeezed in before it stops would begin a
    /// stay spanning the sleep. Lapses on its own, in case the sleep was called off.</summary>
    long _asleepTick;
    /// <summary>The PC first went to sleep at this time while Bloxkeep was locked: stays still open then end there.</summary>
    DateTime? _sleptWhileLocked;

    /// <summary>Each check, first: one long after the last means the PC slept, which Windows doesn't always announce (its
    /// word may come after this check). Open stays end at the last check as "sleep"; new ones start no earlier than now.</summary>
    void NoteCheckGap(DateTime now, DateTime lastCheck)
    {
        long tick = Environment.TickCount64;
        bool slept = _lastCheckTick != 0 && tick - _lastCheckTick >= SleepGap.TotalMilliseconds;
        _lastCheckTick = tick;
        if (!slept)
            return;
        _asleepTick = 0;
        if (_resumedAt < now)
            _resumedAt = now;
        if (_vault is null)
            _sleptWhileLocked ??= lastCheck;
        else if (SuspendPlay(lastCheck))
            PersistSoon();
    }

    void TrackPlay(DateTime now)
    {
        if (_vault is not { } vault)
            return;
        if (_asleepTick != 0 && Environment.TickCount64 - _asleepTick < SleepGap.TotalMilliseconds)
            return;
        bool changed = false;
        foreach (var (id, run) in _running)
        {
            var log = _logs.StateOf(run.Pid);
            _playing.TryGetValue(id, out var open);
            if (open is not null)
                open.LastSeenAt = now; // written with the next save, so a crash of Bloxkeep still leaves an end time
            // A client in its game before the PC slept (its stay ended at the sleep) begins a new stay at the wake.
            DateTime SinceWake(DateTime at) => at < _resumedAt ? _resumedAt : at;
            if (log is null)
            {
                // No log (yet): after a while, playing what it was opened with since the launch. A client open before Bloxkeep started
                // has its log looked for first: its stay may be open again already, or have ended and mustn't be counted twice.
                bool before = run.Launch.StartedAt < _started;
                if (open is null && (before ? now - _started >= RobloxLogWatcher.SearchLimit : now - run.Launch.StartedAt >= NoLogGrace)
                    && run.Launch.Target is { PlaceId: > 0 } target)
                {
                    StartPlay(vault, id, run.Pid, target.PlaceId, SinceWake(before ? _started : run.Launch.StartedAt), now);
                    changed = true;
                }
                continue;
            }
            if (log.Status is "left" or "disconnected" or "signedout")
            {
                if (open is not null)
                {
                    ClosePlay(open, log.Status, log.Reason, log.At);
                    _playing.Remove(id);
                    changed = true;
                }
                continue;
            }
            if (log.PlaceId is not > 0)
                continue;
            // A teleport across games is a new stay, known once the new place is looked up (the process check asks for it);
            // until then the stay goes on rather than split for a place of the same game.
            if (open is not null && open.PlaceId != log.PlaceId.Value && !SameGame(open.PlaceId, log.PlaceId.Value))
            {
                if (!HasGame(log.PlaceId.Value, false) && _gameFetchedAt.TryGetValue(log.PlaceId.Value, out var asked) && now - asked < GameRetry)
                    continue;
                ClosePlay(open, "left", null, log.At);
                _playing.Remove(id);
                open = null;
                changed = true;
            }
            if (open is null)
            {
                StartPlay(vault, id, run.Pid, log.PlaceId.Value, SinceWake(log.At), now);
                changed = true;
            }
            else if (open.GameName.Length == 0 && _games.GetValueOrDefault(open.PlaceId) is { Name.Length: > 0 } named)
            {
                // The place was looked up after the stay began (a game moved the player to a place not seen before).
                open.GameName = named.Name;
                open.UniverseId = named.UniverseId;
                changed = true;
            }
        }
        long tick = Environment.TickCount64;
        if (_playing.Count > 0 && tick - _lastPlaySaveTick >= PlaySaveInterval.TotalMilliseconds)
        {
            _lastPlaySaveTick = tick;
            changed = true; // the last-seen times above reach the file (cheap: stays don't count for backups)
        }
        if (changed)
            PersistSoon();
    }

    /// <summary>The PC is going to sleep: every open stay ends now as "sleep". Hours asleep aren't play time and the wake's
    /// disconnect isn't a kick; a client still in its game then begins a new stay.</summary>
    /// <param name="at">When the PC went to sleep: now, or the last process check before a sleep noticed afterwards.</param>
    /// <returns>true when there is something to save.</returns>
    bool SuspendPlay(DateTime at)
    {
        if (_playing.Count == 0)
            return false;
        foreach (var open in _playing.Values)
            ClosePlay(open, "sleep", null, at);
        _playing.Clear();
        return true;
    }

    void StartPlay(Vault vault, Guid accountId, int pid, long placeId, DateTime at, DateTime now)
    {
        var info = _games.GetValueOrDefault(placeId);
        var session = new PlaySession
        {
            AccountId = accountId,
            AccountLabel = FindAccount(accountId)?.Label,
            Pid = pid,
            PlaceId = placeId,
            UniverseId = info?.UniverseId ?? 0,
            GameName = info?.Name ?? "",
            StartedAt = at > now ? now : at,
            LastSeenAt = now,
            PresetId = _running.TryGetValue(accountId, out var run) ? run.Launch.PresetId : null,
        };
        _playing[accountId] = session;
        vault.Data.PlaySessions.Add(session);
        PrunePlay(vault.Data.PlaySessions, now);
        NoteActivity("joined", accountId, placeId, at: session.StartedAt); // Statistics › Activity
    }

    /// <summary>Ends a stay; one its log ended (a leave, a drop, a sign-out) is noted in Statistics › Activity too.</summary>
    void ClosePlay(PlaySession session, string reason, int? code, DateTime at)
    {
        session.EndedAt = at < session.StartedAt ? session.StartedAt : at;
        session.EndReason = reason;
        session.ReasonCode = code;
        if (reason is "left" or "disconnected" or "signedout")
            NoteActivity(reason == "signedout" ? "signedOut" : reason, session.AccountId, session.PlaceId, code, session.EndedAt);
    }

    /// <summary>The client quit: its stay ends as its log says (a leave, a drop), else as closed. While locked the end is
    /// kept for the next unlock.</summary>
    void EndPlay(Guid accountId, RobloxLogWatcher.ClientState? lastLog, DateTime now)
    {
        var (reason, code, at) = lastLog is { Status: "left" or "disconnected" or "signedout" } ? (lastLog.Status == "signedout" ? "left" : lastLog.Status, lastLog.Reason, lastLog.At) : ("closed", (int?)null, now);
        if (_vault is null)
        {
            _endedWhileLocked[accountId] = (reason, code, at);
            return;
        }
        if (!_playing.Remove(accountId, out var open))
            return;
        ClosePlay(open, reason, code, at);
        PersistSoon();
    }

    /// <summary>Before the vault goes away (lock, exit, an update's restart): open stays are left open with their last-seen
    /// time, so a client still running after a restart goes on with its stay (see <see cref="ResumePlaySessions"/>).</summary>
    /// <returns>true when there is something to save.</returns>
    bool NotePlaySessions()
    {
        if (_playing.Count == 0)
            return false;
        var now = DateTime.UtcNow;
        foreach (var session in _playing.Values)
            session.LastSeenAt = now;
        return true;
    }

    /// <summary>After unlocking: a stay left open goes on when its client is still here, the same process (see
    /// <see cref="AdoptOpenSessions"/>); otherwise it ended while Bloxkeep was away, at its last-seen time. If the PC slept
    /// while locked, a stay still going then ended at that sleep.</summary>
    void ResumePlaySessions(Vault vault)
    {
        _playing.Clear();
        var list = vault.Data.PlaySessions;
        bool changed = false;
        foreach (var session in list.Where(s => s.EndedAt is null))
        {
            bool quit = _endedWhileLocked.TryGetValue(session.AccountId, out var ended);
            bool same = _running.TryGetValue(session.AccountId, out var run) && run.Pid == session.Pid;
            if (_sleptWhileLocked is { } slept && slept > session.StartedAt && (quit ? ended.At > slept : same))
                ClosePlay(session, "sleep", null, slept);
            else if (quit)
                ClosePlay(session, ended.Reason, ended.Code, ended.At);
            else if (same && _playing.TryAdd(session.AccountId, session))
                continue;
            else
                ClosePlay(session, "appClosed", null, session.LastSeenAt ?? session.StartedAt);
            changed = true;
        }
        _endedWhileLocked.Clear();
        _sleptWhileLocked = null;
        changed |= PrunePlay(list, DateTime.UtcNow);
        if (changed)
            PersistSoon();
    }

    /// <summary>Stays older than 90 days go, and the oldest finished ones when the list is long: first those older than a
    /// month, so the 30-day view stays whole (accounts rejoining all day make thousands a week).</summary>
    static bool PrunePlay(List<PlaySession> list, DateTime now)
    {
        var cutoff = now - PlayHistory;
        int removed = list.RemoveAll(s => (s.EndedAt ?? now) < cutoff);
        int Trim(int max, Func<PlaySession, bool> may)
        {
            if (list.Count <= max)
                return 0;
            var oldest = list.Where(s => s.EndedAt is not null && may(s)).OrderBy(s => s.StartedAt).Take(list.Count - max).ToHashSet();
            return list.RemoveAll(oldest.Contains);
        }
        var recent = now - RecentPlay;
        removed += Trim(MaxPlaySessions, s => s.EndedAt < recent);
        removed += Trim(MaxRecentPlaySessions, _ => true);
        return removed > 0;
    }

    /// <summary>When icons were last asked for, by game (universe): one Roblox didn't give isn't asked at every refresh.</summary>
    readonly Dictionary<long, DateTime> _playIconAsked = new();
    static readonly TimeSpan PlayIconRetry = TimeSpan.FromMinutes(10);

    /// <summary><see cref="PlayStats"/> for the page. Games in the period that no other view loaded have no icon yet: those
    /// are asked for first, one request per 100 games.</summary>
    async Task<object?> PlayStatsAsync(int days)
    {
        var vault = RequireVault();
        var now = DateTime.UtcNow;
        // A day more than the period: today's figures start at local midnight, up to a day before now.
        var since = now.AddDays(-(days is 7 or 30 ? days : 1) - 1);
        var places = vault.Data.PlaySessions.Where(s => s.EndedAt is not { } ended || ended >= since).Select(s => (s.PlaceId, s.UniverseId))
            .Concat(vault.Data.Settings.Presets.Select(PresetPlaceId).OfType<long>().Select(place => (PlaceId: place, UniverseId: 0L)))
            .Where(p => p.PlaceId > 0 && IconOf(p.PlaceId, p.UniverseId) is null)
            .DistinctBy(p => p.PlaceId)
            .ToList();
        if (places.Count == 0)
            return PlayStats(days);
        // Places whose game isn't known yet: looked up whole (name and icon).
        await LoadGamesAsync(places.Where(p => p.UniverseId == 0 && _games.GetValueOrDefault(p.PlaceId) is null).Take(40));
        if (_playIconAsked.Count > MaxCachedGames)
            _playIconAsked.Clear();
        var universes = places.Select(p => p.UniverseId > 0 ? p.UniverseId : _games.GetValueOrDefault(p.PlaceId)?.UniverseId ?? 0)
            .Where(u => u > 0 && !_iconByUniverse.ContainsKey(u) && !(_playIconAsked.TryGetValue(u, out var at) && now - at < PlayIconRetry))
            .Distinct()
            .Take(100)
            .ToList();
        if (universes.Count > 0)
        {
            foreach (long u in universes)
                _playIconAsked[u] = now;
            try
            {
                foreach (var (id, url) in await RobloxApi.GetGameIconsAsync(universes))
                    _iconByUniverse[id] = url;
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                // The rows keep their placeholder until the next try.
            }
        }
        return PlayStats(days);
    }

    /// <summary>A game's icon from what is known: its place's lookup, or the icon fetched for its universe.</summary>
    string? IconOf(long placeId, long universeId) =>
        _games.GetValueOrDefault(placeId)?.IconUrl
        ?? _iconByUniverse.GetValueOrDefault(universeId > 0 ? universeId : _games.GetValueOrDefault(placeId)?.UniverseId ?? 0);

    /// <summary>The page's figures: today (by hour) or the last 7 or 30 days (by day), in local time. A stay counts for its
    /// part inside the period, one still going up to now.</summary>
    /// <param name="only">The accounts to count (a Discord summary for chosen accounts); null for all.</param>
    /// <param name="until">The period ends here (UTC) instead of now: a Discord summary sent late covers its own day.</param>
    /// <param name="last24Hours">One day is the 24 hours before the end rather than the calendar day (a Discord summary).</param>
    object PlayStats(int days, IReadOnlySet<Guid>? only = null, DateTime? until = null, bool last24Hours = false)
    {
        var vault = RequireVault();
        days = days is 7 or 30 ? days : 1;
        bool hourly = days == 1;
        var now = until is { } stop && stop < DateTime.UtcNow ? stop : DateTime.UtcNow;
        var today = now.ToLocalTime().Date;
        bool rolling = hourly && last24Hours;
        var since = rolling ? now.AddDays(-1) : (hourly ? today : today.AddDays(1 - days)).ToUniversalTime();
        var starts = Enumerable.Range(0, hourly ? 24 : days)
            .Select(i => rolling ? since.AddHours(i) : (hourly ? today.AddHours(i) : today.AddDays(i + 1 - days)).ToUniversalTime()).ToList();
        var bucketSeconds = new double[starts.Count];

        var byAccount = new Dictionary<Guid, (double Seconds, int Sessions, int Kicks, string? Label)>();
        var byGame = new Dictionary<string, (long PlaceId, long UniverseId, string Name, double Seconds, int Sessions)>();
        // A stay begun before its place was looked up has no universe id; one learned later puts it with the rest of its game.
        var universeOf = new Dictionary<long, long>();
        foreach (var s in vault.Data.PlaySessions)
        {
            if (s.UniverseId > 0)
                universeOf[s.PlaceId] = s.UniverseId;
        }
        long UniverseOf(PlaySession s) =>
            s.UniverseId > 0 ? s.UniverseId : universeOf.GetValueOrDefault(s.PlaceId) is > 0 and var u ? u : _games.GetValueOrDefault(s.PlaceId)?.UniverseId ?? 0;
        double total = 0;
        int sessions = 0, kicks = 0;
        // By the preset whose window it was (null: opened otherwise, or before 1.0.5 noted it, or a preset deleted since).
        var byPreset = new Dictionary<Guid, (double Seconds, int Sessions, int Kicks, HashSet<Guid> Accounts)>();
        var noPreset = (Seconds: 0.0, Sessions: 0, Kicks: 0, Accounts: new HashSet<Guid>());
        (double Seconds, PlaySession? Stay) longest = (0, null);
        foreach (var s in vault.Data.PlaySessions)
        {
            if (only is not null && !only.Contains(s.AccountId))
                continue;
            // After the period's end (a late summary): not in it.
            if (until is not null && s.StartedAt >= now)
                continue;
            var end = s.EndedAt is { } ended && ended < now ? ended : now;
            var from = s.StartedAt > since ? s.StartedAt : since;
            if (end <= from && s.StartedAt < since)
                continue;
            double seconds = Math.Max(0, (end - from).TotalSeconds);
            bool kicked = s.EndReason == "disconnected" && s.EndedAt >= since && s.EndedAt <= now;
            total += seconds;
            sessions++;
            if (kicked)
                kicks++;
            var acc = byAccount.GetValueOrDefault(s.AccountId);
            // The newest name recorded for it (the list is oldest first), for an account deleted since.
            byAccount[s.AccountId] = (acc.Seconds + seconds, acc.Sessions + 1, acc.Kicks + (kicked ? 1 : 0), s.AccountLabel ?? acc.Label);
            if (s.PresetId is { } presetId && PresetFor(presetId) is not null)
            {
                var p = byPreset.GetValueOrDefault(presetId, (0, 0, 0, new HashSet<Guid>()));
                p.Accounts.Add(s.AccountId);
                byPreset[presetId] = (p.Seconds + seconds, p.Sessions + 1, p.Kicks + (kicked ? 1 : 0), p.Accounts);
            }
            else
            {
                noPreset.Accounts.Add(s.AccountId);
                noPreset = (noPreset.Seconds + seconds, noPreset.Sessions + 1, noPreset.Kicks + (kicked ? 1 : 0), noPreset.Accounts);
            }
            if (seconds > longest.Seconds)
                longest = (seconds, s);
            // Places of one game count together; a name learned later wins over an empty one.
            long universe = UniverseOf(s);
            string key = universe > 0 ? $"u{universe}" : $"p{s.PlaceId}";
            var game = byGame.GetValueOrDefault(key);
            byGame[key] = (game.PlaceId == 0 ? s.PlaceId : game.PlaceId, universe, s.GameName.Length > 0 ? s.GameName : game.Name ?? "", game.Seconds + seconds, game.Sessions + 1);
            for (int i = 0; i < starts.Count && seconds > 0; i++)
            {
                var bucketEnd = i + 1 < starts.Count ? starts[i + 1] : now;
                var a = from > starts[i] ? from : starts[i];
                var b = end < bucketEnd ? end : bucketEnd;
                if (b > a)
                    bucketSeconds[i] += (b - a).TotalSeconds;
            }
        }

        return new
        {
            days,
            hourly,
            since,
            recorded = vault.Data.PlaySessions.Count > 0,
            totalSeconds = (long)total,
            sessions,
            kicks,
            accounts = byAccount.OrderByDescending(a => a.Value.Seconds).Select(a =>
            {
                var account = FindAccount(a.Key);
                return new
                {
                    id = a.Key,
                    label = account?.Label ?? a.Value.Label,
                    deleted = account is null,
                    username = account?.Username,
                    avatarUrl = account is null ? null : _avatarUrls.GetValueOrDefault(account.UserId),
                    seconds = (long)a.Value.Seconds,
                    a.Value.Sessions,
                    a.Value.Kicks,
                };
            }),
            games = byGame.Values.OrderByDescending(g => g.Seconds).Take(MaxTopGames).Select(g => new
            {
                g.PlaceId,
                name = g.Name.Length > 0 ? g.Name : _games.GetValueOrDefault(g.PlaceId)?.Name is { Length: > 0 } cached ? cached : L.F("Place {0}", g.PlaceId),
                iconUrl = IconOf(g.PlaceId, g.UniverseId),
                seconds = (long)g.Seconds,
                g.Sessions,
            }),
            buckets = starts.Select((start, i) => new { start, seconds = (long)bucketSeconds[i] }),
            presets = byPreset.OrderByDescending(p => p.Value.Seconds).Select(p =>
            {
                var preset = PresetFor(p.Key)!;
                return new
                {
                    id = p.Key, name = preset.Name,
                    iconUrl = PresetPlaceId(preset) is long place ? IconOf(place, 0) : null,
                    seconds = (long)p.Value.Seconds, sessions = p.Value.Sessions, kicks = p.Value.Kicks, accounts = p.Value.Accounts.Count,
                };
            }).Concat(noPreset.Sessions > 0
                ? [new { id = Guid.Empty, name = "", iconUrl = (string?)null, seconds = (long)noPreset.Seconds, sessions = noPreset.Sessions, kicks = noPreset.Kicks, accounts = noPreset.Accounts.Count }]
                : []),
            longest = longest.Stay is { } stay ? new
            {
                seconds = (long)longest.Seconds,
                account = stay.AccountId,
                label = FindAccount(stay.AccountId)?.Label ?? stay.AccountLabel,
                game = stay.GameName.Length > 0 ? stay.GameName : _games.GetValueOrDefault(stay.PlaceId)?.Name,
            } : null,
        };
    }
}
