namespace RobloxManager;

/// <summary>
/// "Switch to another server": each window in a public server reopens into a random other server of its game with room;
/// accounts of one game switched together stay together. Private servers, share links and follows are left as they are.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Accounts on their way to another server: their reopening goes there, not to another account's server.</summary>
    readonly HashSet<Guid> _hopping = new();

    async Task<object?> SwitchServerAsync(Guid[] ids)
    {
        RequireVault();
        var playing = ids.Where(_running.ContainsKey)
            .Select(id => (Id: id, Run: _running[id], Log: _logs.StateOf(_running[id].Pid)))
            .Where(w => w.Run.Launch.Target is { IsPrivate: false, ShareCode: null, Follow: null }
                        && w.Log is { Status: "ingame", PlaceId: not null, JobId.Length: > 0 })
            .ToList();
        if (playing.Count == 0)
            throw new UserError(L.T("Only accounts playing in a public server can switch to another one."));
        int moved = 0;
        foreach (var game in playing.GroupBy(w => w.Log!.PlaceId!.Value))
        {
            var leaving = game.Select(w => w.Log!.JobId!).ToHashSet();
            List<ServerInfo> servers;
            try
            {
                servers = (await RobloxApi.GetServersAsync(game.Key, false, null)).Servers;
            }
            catch (RobloxApiException ex) when (ex.RateLimited)
            {
                throw new UserError(L.T("Roblox won't give the server list right now. Try again in a minute."));
            }
            var fits = servers.Where(s => !leaving.Contains(s.JobId) && s.MaxPlayers - s.Playing >= game.Count()).ToList();
            if (fits.Count == 0)
            {
                Toast(L.F("No other server of {0} has room right now.", GameName(game.Key)), "warn");
                continue;
            }
            var chosen = fits[Random.Shared.Next(fits.Count)];
            foreach (var (id, run, _) in game)
            {
                _hopping.Add(id);
                _joinRefused.Remove(run.Launch);
                _restarts[id] = (new GameTarget(game.Key, chosen.JobId), DateTime.UtcNow);
                _closingForRejoin.Add(run.Pid);
                _closeForReopen(run.Pid, InGameNow(run.Pid));
                moved++;
            }
        }
        if (moved > 0)
            Toast(moved == 1 ? L.F("{0} is moving to another server…", Name(FindAccount(playing[0].Id)!))
                : L.F("{0} accounts are moving to another server…", moved), "ok");
        PushState();
        return moved;
    }

    string GameName(long placeId) => _games.GetValueOrDefault(placeId)?.Name is { Length: > 0 } name ? name : L.T("this game");
}
