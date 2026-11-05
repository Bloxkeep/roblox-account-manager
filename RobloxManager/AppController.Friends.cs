namespace RobloxManager;

/// <summary>
/// Friend each other: the chosen accounts become Roblox friends, every pair. Where a pair stands is asked first, so one
/// already friends is left alone and a request already sent is only accepted. When Roblox holds a request for a check,
/// it is solved in Bloxkeep's own window, as a join's bot check is (<see cref="AnswerCheckAsync"/>). A pause between
/// requests, so Roblox doesn't rate-limit a dozen accounts. Not yet tried on real accounts (2026-11-05).
/// </summary>
public sealed partial class AppController
{
    bool _friending;
    static readonly TimeSpan FriendPause = TimeSpan.FromSeconds(1.5);

    async Task<object?> MakeFriendsAsync(Guid[] ids)
    {
        var vault = RequireVault();
        var accounts = vault.Data.Accounts.Where(a => ids.Contains(a.Id) && a.UserId > 0 && a.CookieValid != false
            && Realm.Normalize(a.Realm) == Realm.Global).ToList();
        if (accounts.Count < 2)
            throw new UserError(L.T("Pick at least two signed-in roblox.com accounts to friend each other."));
        if (_friending)
            throw new UserError(L.T("Already friending accounts. Wait for it to finish."));
        _friending = true;
        int made = 0, already = 0, failed = 0, pairs = accounts.Count * (accounts.Count - 1) / 2, done = 0;
        try
        {
            for (int i = 0; i < accounts.Count; i++)
            {
                var a = accounts[i];
                var later = accounts.Skip(i + 1).ToList();
                if (later.Count == 0)
                    break;
                Dictionary<long, string> stands;
                try
                {
                    stands = await RobloxApi.FriendStatusesAsync(a.Cookie, a.UserId, later.Select(b => b.UserId).ToList(), a.Realm);
                }
                catch (RobloxApiException ex) when (!ex.RateLimited)
                {
                    stands = []; // unknown: asked anyway, and Roblox says if they are friends already
                    Log.Note($"Friend each other: statuses not read ({ex.Message}).");
                }
                foreach (var b in later)
                {
                    Progress(L.F("Friending accounts… {0}/{1}", ++done, pairs));
                    if (_vault != vault)
                        return null;
                    string stand = stands.GetValueOrDefault(b.UserId, "NotFriends");
                    if (stand == "Friends")
                    {
                        already++;
                        continue;
                    }
                    bool? ok = stand switch
                    {
                        "RequestReceived" => await AcceptAsync(a, b),   // b asked a already
                        "RequestSent" => await AcceptAsync(b, a),       // a asked b already
                        _ => await RequestAsync(a, b) is not { } sent ? null : sent && await AcceptAsync(b, a),
                    };
                    if (ok is null)
                    {
                        Toast(L.F("Stopped: the check was closed. {0} new friends so far.", made), "warn");
                        return null;
                    }
                    if (ok == true)
                        made++;
                    else
                        failed++;
                    await Task.Delay(FriendPause);
                }
            }
        }
        catch (RobloxApiException ex) when (ex.RateLimited)
        {
            Toast(L.F("Roblox is rate-limiting requests; try again shortly. {0} new friends so far.", made), "warn");
            return null;
        }
        finally
        {
            _friending = false;
            Progress(null);
        }
        Toast(L.F("Friend each other: {0} new, {1} already friends, {2} failed.", made, already, failed), failed > 0 ? "warn" : "ok");
        return null;
    }

    /// <summary><paramref name="from"/> asks <paramref name="to"/>; true sent, false refused, null a check closed unsolved.</summary>
    async Task<bool?> RequestAsync(Account from, Account to)
    {
        var answer = await RobloxApi.RequestFriendshipAsync(from.Cookie, to.UserId, from.Realm);
        if (answer.Challenge is { } check)
        {
            if (await PassCheckAsync(from, check) is not { } passed)
                return null;
            answer = await RobloxApi.RequestFriendshipAsync(from.Cookie, to.UserId, from.Realm, passed);
        }
        return answer.Ok;
    }

    /// <summary><paramref name="by"/> accepts the request <paramref name="requester"/> sent it.</summary>
    async Task<bool> AcceptAsync(Account by, Account requester)
    {
        var answer = await RobloxApi.AcceptFriendRequestAsync(by.Cookie, requester.UserId, by.Realm);
        if (answer.Challenge is { } check && await PassCheckAsync(by, check) is { } passed)
            answer = await RobloxApi.AcceptFriendRequestAsync(by.Cookie, requester.UserId, by.Realm, passed);
        return answer.Ok;
    }

    /// <summary>A check solved and handed to Roblox, to carry on the request asked again; null when closed unsolved.</summary>
    async Task<RobloxApi.JoinChallenge?> PassCheckAsync(Account account, RobloxApi.JoinChallenge check)
    {
        if (await AnswerCheckAsync(account, check) is not { } answer)
            return null;
        var solved = check with { MetadataJson = answer };
        try
        {
            return await RobloxApi.ContinueChallengeAsync(account.Realm, account.Cookie, solved);
        }
        catch (RobloxApiException ex) when (!ex.ServerError && !ex.RateLimited)
        {
            return solved; // as a join's: the request asked again carrying the answer
        }
    }
}
