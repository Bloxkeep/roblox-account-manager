namespace RobloxManager;

/// <summary>
/// "Switch to another server": each window in a public server reopens into a random other server of its game with room;
/// accounts of one game switched together stay together. Private servers, share links and follows are left as they are.
/// Also: a preset's accounts each in a public server of its own, and windows moved off a server that got crowded
/// (Settings › Staying in game, <see cref="AppSettings.HopAbovePlayers"/>).
/// </summary>
public sealed partial class AppController
{
    /// <summary>Accounts on their way to another server, and since when: their reopening goes there, not to another account's
    /// server. Taken by that reopening (RejoinAsync); one that never comes (the window closed for good, everything closed)
    /// counts no more after <see cref="HopTimeout"/>, so /pc switch isn't refused for ever (2026-10-30 review).</summary>
    readonly Dictionary<Guid, DateTime> _hopping = new();

    static readonly TimeSpan HopTimeout = TimeSpan.FromMinutes(3);

    /// <summary>Some account is still on its way to another server.</summary>
    bool Hopping()
    {
        var now = DateTime.UtcNow;
        foreach (var (id, since) in _hopping.ToList())
        {
            if (now - since > HopTimeout || now < since)
                _hopping.Remove(id);
        }
        return _hopping.Count > 0;
    }

    /// <summary>The accounts' windows in a public server of a game, with that server.</summary>
    List<(Guid Id, (int Pid, PendingLaunch Launch) Run, RobloxLogWatcher.ClientState Log)> InPublicServers(IEnumerable<Guid> ids) =>
        ids.Where(_running.ContainsKey)
            .Select(id => (Id: id, Run: _running[id], Log: _logs.StateOf(_running[id].Pid)))
            .Where(w => w.Run.Launch.Target is { IsPrivate: false, ShareCode: null, Follow: null }
                        && w.Log is { Status: "ingame", PlaceId: not null, JobId.Length: > 0 })
            .Select(w => (w.Id, w.Run, w.Log!))
            .ToList();

    async Task<object?> SwitchServerAsync(Guid[] ids)
    {
        var vault = RequireVault();
        var playing = InPublicServers(ids);
        if (playing.Count == 0)
            throw new UserError(L.T("Only accounts playing in a public server can switch to another one."));
        int moved = 0;
        foreach (var game in playing.GroupBy(w => w.Log.PlaceId!.Value))
        {
            var leaving = game.Select(w => w.Log.JobId!).ToHashSet();
            List<ServerInfo> servers;
            try
            {
                servers = (await RobloxApi.GetServersAsync(game.Key, false, null)).Servers;
            }
            catch (RobloxApiException ex) when (ex.RateLimited)
            {
                Log.Note($"Switch server: place {game.Key}'s server list refused (rate-limited).");
                throw new UserError(L.T("Roblox won't give the server list right now. Try again in a minute."));
            }
            // Locked while Roblox answered: nothing of that vault's is touched any more.
            if (_vault != vault)
                break;
            var fits = servers.Where(s => !leaving.Contains(s.JobId) && s.MaxPlayers - s.Playing >= game.Count()).ToList();
            if (fits.Count == 0)
            {
                Log.Note($"Switch server: no other server of place {game.Key} has room for {game.Count()} ({servers.Count} read).");
                Toast(L.F("No other server of {0} has room right now.", GameName(game.Key)), "warn");
                continue;
            }
            var chosen = fits[Random.Shared.Next(fits.Count)];
            Log.Note($"Switch server: {game.Count()} window(s) of place {game.Key} to a server with {chosen.Playing}/{chosen.MaxPlayers} ({fits.Count} with room).");
            foreach (var (id, run, _) in game)
            {
                if (MoveToServer(id, run, game.Key, chosen.JobId))
                    moved++;
            }
        }
        if (moved > 0)
            Toast(moved == 1 ? L.F("{0} is moving to another server…", Name(FindAccount(playing[0].Id)!))
                : L.F("{0} accounts are moving to another server…", moved), "ok");
        PushState();
        return moved;
    }

    /// <summary>Closes the window to reopen it in that server (auto-rejoin's path); false when it closed or was replaced
    /// meanwhile, leaving nothing to move and no mark that nothing takes.</summary>
    bool MoveToServer(Guid id, (int Pid, PendingLaunch Launch) run, long placeId, string jobId)
    {
        if (!_running.TryGetValue(id, out var current) || current.Pid != run.Pid)
            return false;
        _hopping[id] = DateTime.UtcNow;
        _joinRefused.Remove(run.Launch);
        _restarts[id] = (new GameTarget(placeId, jobId), DateTime.UtcNow);
        _closingForRejoin.Add(run.Pid);
        _closeForReopen(run.Pid, InGameNow(run.Pid));
        return true;
    }

    string GameName(long placeId) => _games.GetValueOrDefault(placeId)?.Name is { Length: > 0 } name ? name : L.T("this game");

    // ---- Each account in a server of its own ----

    /// <summary>For a launch: a public server for each account as it opens, none shared with another of the accounts (the
    /// ones already playing that game included). <paramref name="order"/> is "fewest", "most" or "random". Roblox's list
    /// is read once, at the first account; when it runs out, or can't be read, Roblox picks for the rest.</summary>
    Func<GameTarget, CancellationToken, Task<GameTarget>> ServersApart(string order)
    {
        List<ServerInfo>? servers = null;
        var taken = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        bool said = false;
        return async (target, ct) =>
        {
            if (servers is null)
            {
                Progress(L.T("Finding a server…"));
                servers = new List<ServerInfo>();
                try
                {
                    var (first, next) = await RobloxApi.GetServersAsync(target.PlaceId, order == "fewest", null, ct);
                    servers.AddRange(first);
                    if (next is not null)
                        servers.AddRange((await RobloxApi.GetServersAsync(target.PlaceId, order == "fewest", next, ct)).Servers);
                }
                catch (Exception ex) when (!ct.IsCancellationRequested && RobloxApi.IsExpectedError(ex))
                {
                    // Whatever loaded is used, else Roblox picks.
                    Log.Note($"Servers apart: place {target.PlaceId}'s server list not fully read ({LaunchFailKind(ex)}), {servers.Count} loaded.");
                }
                foreach (var (_, run) in _running)
                {
                    if (_logs.StateOf(run.Pid) is { PlaceId: { } place, JobId: { } job } && place == target.PlaceId)
                        taken.Add(job);
                }
                Log.Note($"Servers apart ({order}): {servers.Count} server(s) of place {target.PlaceId} read, {taken.Count} already taken by open windows.");
            }
            if (ServerApart(servers, taken, order, Random.Shared) is { } chosen)
            {
                taken.Add(chosen.JobId);
                return target with { JobId = chosen.JobId };
            }
            if (!said)
            {
                said = true;
                Log.Note($"Servers apart: ran out of servers with room after {taken.Count} taken; Roblox picks for the rest.");
                Toast(L.T("Not enough servers with room were found for every account to have its own: Roblox picks for the rest."), "warn");
            }
            return target with { JobId = null };
        };
    }

    /// <summary>The next server for an account of its own: one with room that none of <paramref name="taken"/> is in, the
    /// list's first (read emptiest or fullest first) or, with "random", any.</summary>
    internal static ServerInfo? ServerApart(IReadOnlyList<ServerInfo> servers, ISet<string> taken, string order, Random random)
    {
        var fits = servers.Where(s => s.MaxPlayers - s.Playing >= 1 && !taken.Contains(s.JobId)).ToList();
        return fits.Count == 0 ? null : order == "random" ? fits[random.Next(fits.Count)] : fits[0];
    }

    // ---- Off a crowded server ----

    /// <summary>The choices Settings offers for <see cref="AppSettings.HopAbovePlayers"/> (0 = never).</summary>
    internal static readonly int[] HopAboveChoices = [0, 1, 2, 3, 5, 8, 10, 15, 20];
    /// <summary>How often the servers are counted: Roblox's server list allows only a few reads a minute.</summary>
    static readonly TimeSpan CrowdCheckEvery = TimeSpan.FromMinutes(2);
    /// <summary>A window just in is left alone this long: its server may not be in Roblox's list yet, and one that keeps
    /// finding crowded servers doesn't hop without a pause.</summary>
    static readonly TimeSpan CrowdGrace = TimeSpan.FromSeconds(90);
    /// <summary>Pages of 50 read, emptiest first, before a server not seen yet counts as unknown.</summary>
    const int CrowdPages = 4;
    DateTime _nextCrowdCheck;
    bool _crowdBusy;
    /// <summary>Games told to have no quiet server, and when: said again only after a while.</summary>
    readonly Dictionary<long, DateTime> _noQuietServerSaid = new();
    /// <summary>The last failure of the crowd check that was logged (null after a good read).</summary>
    string? _crowdFailNoted;

    /// <summary>Every <see cref="CrowdCheckEvery"/>, when Settings asks for it: the servers of the windows in public servers
    /// are counted from Roblox's list, and a window whose server has more than that many other players moves to one with
    /// that many or fewer, apart from the other accounts (as Switch server does for one). Left until later while a launch,
    /// rejoin, restart or switch is under way.</summary>
    void HopCrowdedClients(DateTime now)
    {
        if (_vault is not { } vault || _crowdBusy || now < _nextCrowdCheck)
            return;
        int above = vault.Data.Settings.HopAbovePlayers;
        if (above <= 0)
            return;
        if (_launchCts is not null || _rejoining || _restarts.Count > 0 || Hopping() || Settling || _updateReopenSince is not null)
            return;
        _nextCrowdCheck = now + CrowdCheckEvery;
        // Not a preset's windows sent to a server on purpose: its own Job ID, or the fullest server ("most").
        var playing = InPublicServers(_running.Keys).Where(w => now - w.Log.At >= CrowdGrace && !_closingForRejoin.Contains(w.Run.Pid)
            && PresetFor(w.Run.Launch.PresetId)?.Server is not ("job" or "most")).ToList();
        if (playing.Count == 0)
            return;
        _crowdBusy = true;
        _ = HopCrowdedAsync(vault, playing, above);
    }

    async Task HopCrowdedAsync(Vault vault, List<(Guid Id, (int Pid, PendingLaunch Launch) Run, RobloxLogWatcher.ClientState Log)> playing, int above)
    {
        try
        {
            foreach (var game in playing.GroupBy(w => w.Log.PlaceId!.Value))
            {
                // The accounts' windows in each server of this game: they don't count as the others.
                var ours = game.GroupBy(w => w.Log.JobId!, StringComparer.OrdinalIgnoreCase).ToDictionary(g => g.Key, g => g.Count(), StringComparer.OrdinalIgnoreCase);
                int most = above + ours.Values.Max();
                var seen = new List<ServerInfo>();
                bool ended = false;
                string? cursor = null;
                for (int page = 0; page < CrowdPages; page++)
                {
                    var (servers, next) = await RobloxApi.GetServersAsync(game.Key, true, cursor);
                    seen.AddRange(servers);
                    cursor = next;
                    if (next is null)
                    {
                        ended = true;
                        break;
                    }
                    // Read emptiest first: past this many players, a server of theirs not seen yet is the more crowded.
                    if (servers.Count > 0 && servers[^1].Playing > most)
                        break;
                }
                if (_vault != vault)
                    return;
                var crowded = CrowdedServers(seen, ended, ours, above);
                _crowdFailNoted = null; // read fine: a later failure is said again
                if (crowded.Count == 0)
                    continue;
                var avoid = new HashSet<string>(ours.Keys, StringComparer.OrdinalIgnoreCase);
                int moved = 0;
                foreach (var (id, run, _) in game.Where(w => crowded.Contains(w.Log.JobId!)))
                {
                    if (QuietServer(seen, above, avoid, Random.Shared) is not { } quiet)
                    {
                        if (!_noQuietServerSaid.TryGetValue(game.Key, out var said) || DateTime.UtcNow - said > TimeSpan.FromMinutes(15))
                        {
                            _noQuietServerSaid[game.Key] = DateTime.UtcNow;
                            Log.Note($"Crowded servers: place {game.Key} has no server with {above} or fewer players ({seen.Count} read); windows stay.");
                            Toast(L.F("No server of {0} has {1} or fewer players right now. Bloxkeep looks again in a few minutes.", GameName(game.Key), above), "warn");
                        }
                        break;
                    }
                    avoid.Add(quiet.JobId);
                    if (MoveToServer(id, run, game.Key, quiet.JobId))
                        moved++;
                }
                if (moved > 0)
                    Log.Note($"Crowded servers: place {game.Key}, {crowded.Count} server(s) above {above} other players; {moved} window(s) moved to quieter ones.");
                if (moved > 0)
                    Toast(L.P(moved, "{0} account's server got crowded, so it moves to a quieter one…",
                        "{0} accounts' servers got crowded, so they move to quieter ones…", moved), "ok");
            }
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            // Rate-limited or offline: counted again next time. Logged when the kind changes, not every 2 minutes.
            string kind = LaunchFailKind(ex);
            if (kind != _crowdFailNoted)
                Log.Note($"Crowded servers: the server list couldn't be read ({_crowdFailNoted = kind}); checked again later.");
        }
        finally
        {
            _crowdBusy = false;
            PushState();
        }
    }

    /// <summary>Which of the accounts' servers have more than <paramref name="above"/> other players, from Roblox's list read
    /// emptiest first (<paramref name="ours"/>: how many of the accounts are in each). One in the list counts its players
    /// less the accounts'. One not in it is fuller than the list went, when the list went past that many; when the list
    /// ended, it is full (Roblox leaves full servers out). Otherwise nothing is known of it and it stays.</summary>
    internal static HashSet<string> CrowdedServers(IReadOnlyList<ServerInfo> seen, bool ended, IReadOnlyDictionary<string, int> ours, int above)
    {
        var crowded = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        int max = seen.Count > 0 ? seen.Max(s => s.MaxPlayers) : 0;
        foreach (var (job, count) in ours)
        {
            var server = seen.FirstOrDefault(s => string.Equals(s.JobId, job, StringComparison.OrdinalIgnoreCase));
            int? others = server is not null ? server.Playing - count
                : seen.Any(s => s.Playing - count > above) ? above + 1
                : ended && max > 0 ? max - count
                : null;
            if (others > above)
                crowded.Add(job);
        }
        return crowded;
    }

    /// <summary>A server with room where, once in, at most <paramref name="above"/> others play, none of <paramref name="avoid"/>; any of them.</summary>
    internal static ServerInfo? QuietServer(IReadOnlyList<ServerInfo> seen, int above, ISet<string> avoid, Random random)
    {
        var fits = seen.Where(s => s.Playing <= above && s.MaxPlayers - s.Playing >= 1 && !avoid.Contains(s.JobId)).ToList();
        return fits.Count == 0 ? null : fits[random.Next(fits.Count)];
    }
}
