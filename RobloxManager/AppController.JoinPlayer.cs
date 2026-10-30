using System.Text.Json;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>"Join a player…": the selected accounts open into a Roblox user's server. A username is looked up publicly; where
/// they are comes from presence asked with each selected account's own sign-in until one is shown the server (Roblox shows it
/// only to people the player's privacy lets join). Bloxkeep never works around that (no follow, no friend request).</summary>
public sealed partial class AppController
{
    const int MaxRecentPlayers = 8;
    static readonly Regex UsernameText = new(@"^[A-Za-z0-9_]{3,20}$", RegexOptions.CultureInvariant);
    static readonly Regex ProfileLink = new(@"roblox\.com/users/(\d{1,19})", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    /// <summary>Who each name typed this session is, so looking the same one up again (a retry) needs no request.</summary>
    readonly Dictionary<string, RobloxUser> _players = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>{input, ids}: where the player is, as the selected accounts may see it: "found" (with placeId, jobId, game name),
    /// "hidden", "studio", "website", "offline" or "notFound".</summary>
    async Task<object?> FindPlayerAsync(JsonElement a)
    {
        var vault = RequireVault();
        var accounts = InLaunchOrder(vault, Ids(a));
        if (accounts.Count == 0)
            throw new UserError(L.T("Select at least one account."));
        string input = Str(a, "input").Trim().TrimStart('@').Trim();
        var user = await LookUpPlayerAsync(input);
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));
        if (user is null)
            return new { status = "notFound", name = input };
        RememberPlayer(vault.Data.Settings, user.Name);

        var viewers = accounts.Where(CanAskPresence).ToList();
        if (viewers.Count == 0)
            throw new UserError(L.T("None of the selected accounts is signed in, so Bloxkeep can't ask Roblox where this player is. Sign them in again first."));
        var (seen, failed) = await WherePlayerIsAsync(user.Id, viewers, vault);
        if (seen is null)
        {
            throw new UserError(failed is RobloxApiException { CookieInvalid: true }
                ? L.T("Roblox didn't accept the selected accounts' sign-in. Check their sign-in, then try again.")
                : failed is not null ? ErrorText(failed) : L.T("Roblox didn't say where this player is. Try again in a moment."));
        }
        string status = PlayerStatus(seen);
        string? game = null;
        if (status == "found")
        {
            try
            {
                game = (await GetGameInfoAsync(seen.PlaceId!.Value))?.Name;
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                // The name is an extra: the server can be joined without it.
            }
        }
        return new
        {
            status,
            userId = user.Id,
            name = user.Name,
            displayName = user.DisplayName,
            placeId = status == "found" ? seen.PlaceId : null,
            jobId = status == "found" ? seen.JobId : null,
            game,
        };
    }

    /// <summary>An account that can ask Roblox where someone is: signed in to Roblox (not Roblox VNG, whose ids are another count).</summary>
    static bool CanAskPresence(Account account) => account.Cookie.Length > 0 && account.CookieValid != false && account.Realm == Realm.Global;

    /// <summary>How much Roblox told about where a player is: a server beats "in a game", which beats Studio, the website, offline.</summary>
    static int PresenceRank(Presence p) => p switch
    {
        { Type: 2, PlaceId: > 0, JobId.Length: > 0 } => 5,
        { Type: 2 } => 4,
        { Type: 3 } => 3,
        { Type: 1 } => 2,
        _ => 1,
    };

    static string PlayerStatus(Presence p) => PresenceRank(p) switch { 5 => "found", 4 => "hidden", 3 => "studio", 2 => "website", _ => "offline" };

    /// <summary>Where a player is, asked with each account's own sign-in in turn until one is shown a server. The most any was told,
    /// or null with the last failure when none could ask. Bloxkeep locked meanwhile: a UserError.</summary>
    async Task<(Presence? Seen, Exception? Failed)> WherePlayerIsAsync(long userId, IReadOnlyList<Account> viewers, Vault vault, CancellationToken ct = default)
    {
        Presence? seen = null;
        Exception? failed = null;
        foreach (var viewer in viewers)
        {
            ct.ThrowIfCancellationRequested();
            try
            {
                if ((await RobloxApi.GetPresenceAsync(viewer.Cookie, [userId], ct: ct)).GetValueOrDefault(userId) is { } p)
                {
                    if (seen is null || PresenceRank(p) > PresenceRank(seen))
                        seen = p;
                    if (PresenceRank(p) == 5)
                        break;
                }
            }
            catch (Exception ex) when (!ct.IsCancellationRequested && RobloxApi.IsExpectedError(ex))
            {
                failed = ex; // this account couldn't ask; the next one may
                if (ex is RobloxApiException { RateLimited: true })
                    break; // the others would only run into the same limit
            }
            if (_vault != vault)
                throw new UserError(L.T("Bloxkeep was locked."));
        }
        return (seen, failed);
    }

    /// <summary>The user a username, user ID or profile link stands for, or null. Digits alone: tried as an ID, then a username.</summary>
    async Task<RobloxUser?> LookUpPlayerAsync(string input)
    {
        if (input.Length == 0)
            throw new UserError(L.T("Type the player's Roblox username or user ID."));
        if (_players.TryGetValue(input, out var known))
            return known;
        long id = 0;
        if (ProfileLink.Match(input) is { Success: true } link)
            long.TryParse(link.Groups[1].Value, out id);
        else if (input.Length <= 19 && input.All(char.IsAsciiDigit))
            long.TryParse(input, out id);
        else if (!UsernameText.IsMatch(input))
            throw new UserError(L.T("A Roblox username is 3 to 20 letters, digits or _ (the username, not the display name). A user ID or a profile link works too."));
        RobloxUser? user = null;
        // A refused or rate-limited lookup says so, instead of reading as "no such user".
        if (id > 0)
            user = (await RobloxApi.GetUsersAsync([id], strict: true)).GetValueOrDefault(id);
        if (user is null && UsernameText.IsMatch(input))
            user = await RobloxApi.GetUserByNameAsync(input);
        if (user is not null)
        {
            if (_players.Count >= 50)
                _players.Clear();
            _players[input] = user;
        }
        return user;
    }

    /// <summary>The last few names looked up, newest first; kept in the vault, since they are other people's names.</summary>
    void RememberPlayer(AppSettings s, string name)
    {
        if (s.RecentPlayers.FirstOrDefault() == name)
            return;
        s.RecentPlayers.RemoveAll(n => n.Equals(name, StringComparison.OrdinalIgnoreCase));
        s.RecentPlayers.Insert(0, name);
        if (s.RecentPlayers.Count > MaxRecentPlayers)
            s.RecentPlayers.RemoveRange(MaxRecentPlayers, s.RecentPlayers.Count - MaxRecentPlayers);
        PersistSoon();
        PushState();
    }

    object? ForgetPlayer(string name)
    {
        if (RequireVault().Data.Settings.RecentPlayers.RemoveAll(n => n.Equals(name, StringComparison.OrdinalIgnoreCase)) > 0)
            PersistSoon();
        PushState();
        return null;
    }

    /// <summary>{ids, placeId, jobId, force, follow, followUserId, followName}: the selected accounts open into the server
    /// <see cref="FindPlayerAsync"/> found, through the usual launch. With follow, their rejoins follow that player (see
    /// <see cref="FollowTargetAsync"/>); the box's choice is remembered.</summary>
    async Task<object?> JoinServerAsync(JsonElement a)
    {
        var vault = RequireVault();
        var accounts = InLaunchOrder(vault, Ids(a));
        CheckCanLaunch(accounts);
        var target = JoinTarget(a);
        if (Has(a, "follow", JsonValueKind.True) || Has(a, "follow", JsonValueKind.False))
        {
            vault.Data.Settings.FollowJoinedPlayer = Bool(a, "follow");
            PersistSoon();
        }
        return await LaunchAccountsAsync(vault, accounts, target, Bool(a, "force"), vault.Data.Settings.LaunchDelaySeconds);
    }

    /// <summary>The server <see cref="JoinServerAsync"/> opens, following the player it found when its box was ticked.</summary>
    static GameTarget JoinTarget(JsonElement a)
    {
        long placeId = Long(a, "placeId");
        string jobId = Str(a, "jobId").Trim();
        if (placeId <= 0 || !Guid.TryParse(jobId, out _))
            throw new UserError(L.T("That server can't be joined. Look the player up again."));
        long followId = Long(a, "followUserId");
        string followName = Str(a, "followName").Trim();
        return new GameTarget(placeId, jobId)
        {
            Follow = Bool(a, "follow") && followId > 0 ? new FollowedPlayer(followId, followName.Length > 0 ? followName : followId.ToString()) : null,
        };
    }

    // ───────── following a player on rejoin ─────────

    /// <summary>A rejoin waiting for its followed player (offline, a hidden or full server) looks for them again after this.</summary>
    static readonly TimeSpan FollowRecheck = TimeSpan.FromSeconds(60);
    /// <summary>Why each account waits for its followed player, as last said: said again only when the reason changes.</summary>
    readonly Dictionary<Guid, string> _followWaitTold = new();
    /// <summary>How many of your signed-in accounts a following rejoin asks where the player is (its own first).</summary>
    const int FollowViewers = 3;
    /// <summary>A followed player last seen outside a server you can join is taken as still there this long, for every account
    /// following them.</summary>
    static readonly TimeSpan FollowSeenFor = TimeSpan.FromSeconds(30);
    readonly Dictionary<long, (Vault Vault, DateTime At, Presence? Seen)> _followSeen = new();

    /// <summary>Where a following rejoin goes: the player's server now, as Roblox shows it to this account (first) or another signed-in
    /// one, checked with Roblox's join answer. When they can't be followed yet (offline, website, Studio, hidden, full or ended server,
    /// a game that bans or refuses this account), the rejoin waits <see cref="FollowRecheck"/>, said once per reason, and null comes
    /// back. No answer at all (no connection, rate limit) throws, so the rejoin's usual retry takes it.</summary>
    async Task<GameTarget?> FollowTargetAsync(Account account, RejoinRequest request, CancellationToken ct)
    {
        var vault = RequireVault();
        var follow = request.Target.Follow!;
        string player = follow.Name;
        // A few accounts at most, and a player not in a server they can see is remembered a little while: ten accounts following
        // someone offline each asked with every account every minute, about 100 presence requests a minute (review, 2026-10-30).
        var viewers = new[] { account }.Concat(vault.Data.Accounts.Where(other => other.Id != account.Id)).Where(CanAskPresence)
            .Take(FollowViewers).ToList();
        if (viewers.Count == 0)
            return Wait("none", L.F("{0} waits to join {1}: none of your accounts is signed in, so Bloxkeep can't ask Roblox where {1} is. Bloxkeep looks again every minute.", Name(account), player));
        Presence? seen;
        if (_followSeen.TryGetValue(follow.UserId, out var known) && known.Vault == vault && DateTime.UtcNow - known.At < FollowSeenFor)
            seen = known.Seen;
        else
        {
            (seen, var failed) = await WherePlayerIsAsync(follow.UserId, viewers, vault, ct);
            if (seen is null && failed is not null)
                throw failed;
            // Only "not in a server yet": a server found is asked afresh each time, since the player may have moved on.
            if (seen is null || PresenceRank(seen) < 5)
                _followSeen[follow.UserId] = (vault, DateTime.UtcNow, seen);
            else
                _followSeen.Remove(follow.UserId);
        }
        string status = seen is null ? "offline" : PlayerStatus(seen);
        if (status != "found")
        {
            return Wait(status, status switch
            {
                "hidden" => L.F("{0} waits to join {1}: {1} is in a game, but Roblox doesn't show which server to your accounts (their privacy settings). Bloxkeep looks again every minute.", Name(account), player),
                "studio" => L.F("{0} waits to join {1}: {1} is in Roblox Studio, not in a game. Bloxkeep looks again every minute.", Name(account), player),
                "website" => L.F("{0} waits to join {1}: {1} is on the Roblox website, not in a game. Bloxkeep looks again every minute.", Name(account), player),
                _ => L.F("{0} waits to join {1}: {1} is offline, or hides being online. Bloxkeep looks again every minute.", Name(account), player),
            });
        }
        var target = new GameTarget(seen!.PlaceId!.Value, seen.JobId) { Follow = follow };
        // A game that has banned this account: its server would only show the ban (Join Error 600).
        if (GameBanOf(account, target.PlaceId) is not null)
            return Wait("banned", L.F("{0} waits to join {1}: the game {1} is in has banned this account. Bloxkeep looks again every minute.", Name(account), player));
        // Roblox's own join answer: a full or ended server or a game refusing this account would only open a failing window.
        // No answer (refused request, no network): the window opens and its join tells as usual.
        var answer = await CheckJoinAsync(account, target, "Following a player for a rejoin", ct);
        ct.ThrowIfCancellationRequested();
        if (answer is { Full: true })
            return Wait("full", L.F("{0} waits to join {1}: {1}'s server is full. Bloxkeep looks again every minute.", Name(account), player));
        if (answer is { Ended: true })
            return Wait("ended", L.F("{0} waits to join {1}: {1}'s server has just ended. Bloxkeep looks again every minute.", Name(account), player));
        if (answer is { Refused: true })
            return Wait("refused", L.F("{0} waits to join {1}: Roblox won't let this account into the game {1} is in. Bloxkeep looks again every minute.", Name(account), player));
        if (_followWaitTold.Remove(account.Id))
            Toast(L.F("{0} found {1} again — rejoining their server…", Name(account), player), "info");
        return target;

        GameTarget? Wait(string reason, string text)
        {
            _rejoinQueue.RemoveAll(r => r.AccountId == account.Id);
            _rejoinQueue.Add(request with { Due = DateTime.UtcNow + FollowRecheck });
            if (!_followWaitTold.TryGetValue(account.Id, out string? told) || told != reason)
            {
                _followWaitTold[account.Id] = reason;
                Notify(text, "warn", "rejoinFail", account.Id);
            }
            PushState();
            return null;
        }
    }
}
