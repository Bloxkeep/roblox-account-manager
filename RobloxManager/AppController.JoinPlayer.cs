using System.Text.Json;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// "Join a player…": the selected accounts open into the server a Roblox user is in. A username is looked up publicly
/// (no cookie). Where the person is comes from Roblox's presence, asked with the selected accounts' own sign-ins one
/// at a time until one is shown the server: Roblox tells it only to people the player's privacy settings let join
/// them. Bloxkeep never works around that (no follow, no friend request); it says what Roblox told it.
/// </summary>
public sealed partial class AppController
{
    const int MaxRecentPlayers = 8;
    static readonly Regex UsernameText = new(@"^[A-Za-z0-9_]{3,20}$", RegexOptions.CultureInvariant);
    static readonly Regex ProfileLink = new(@"roblox\.com/users/(\d{1,19})", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    /// <summary>Who each name typed this session is, so looking the same one up again (a retry) needs no request.</summary>
    readonly Dictionary<string, RobloxUser> _players = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>
    /// {input, ids}: where the player is, as the selected accounts are allowed to see it. Status "found" (with placeId,
    /// jobId and the game's name), "hidden" (in a game, server not shown), "studio", "website" (online, not in a game),
    /// "offline" (or hiding that they're online) or "notFound" (no such user).
    /// </summary>
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

        var viewers = accounts.Where(acc => acc.Cookie.Length > 0 && acc.CookieValid != false && acc.Realm == Realm.Global).ToList();
        if (viewers.Count == 0)
            throw new UserError(L.T("None of the selected accounts is signed in, so Bloxkeep can't ask Roblox where this player is. Sign them in again first."));
        // The most any account was told: a server beats "in a game", which beats Studio, the website, offline.
        static int Told(Presence p) => p switch
        {
            { Type: 2, PlaceId: > 0, JobId.Length: > 0 } => 5,
            { Type: 2 } => 4,
            { Type: 3 } => 3,
            { Type: 1 } => 2,
            _ => 1,
        };
        Presence? seen = null;
        Exception? failed = null;
        foreach (var viewer in viewers)
        {
            try
            {
                if ((await RobloxApi.GetPresenceAsync(viewer.Cookie, [user.Id])).GetValueOrDefault(user.Id) is not { } p)
                    continue;
                if (seen is null || Told(p) > Told(seen))
                    seen = p;
                if (Told(p) == 5)
                    break;
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                failed = ex; // this account couldn't ask; the next one may
                if (ex is RobloxApiException { RateLimited: true })
                    break; // the others would only run into the same limit
            }
            if (_vault != vault)
                throw new UserError(L.T("Bloxkeep was locked."));
        }
        if (seen is null)
        {
            throw new UserError(failed is RobloxApiException { CookieInvalid: true }
                ? L.T("Roblox didn't accept the selected accounts' sign-in. Check their sign-in, then try again.")
                : failed is not null ? ErrorText(failed) : L.T("Roblox didn't say where this player is. Try again in a moment."));
        }
        string status = Told(seen) switch { 5 => "found", 4 => "hidden", 3 => "studio", 2 => "website", _ => "offline" };
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

    /// <summary>
    /// The user a username, user ID or profile link stands for; null when Roblox knows nobody by it. Digits alone are
    /// tried as an ID first, then as a username (Roblox allows those too).
    /// </summary>
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

    /// <summary>
    /// {ids, placeId, jobId, force}: the selected accounts open into the server <see cref="FindPlayerAsync"/> found,
    /// through the usual launch (its warnings and confirmation included). The dock's game box is left as it is.
    /// </summary>
    async Task<object?> JoinServerAsync(JsonElement a)
    {
        var vault = RequireVault();
        var accounts = InLaunchOrder(vault, Ids(a));
        CheckCanLaunch(accounts);
        long placeId = Long(a, "placeId");
        string jobId = Str(a, "jobId").Trim();
        if (placeId <= 0 || !Guid.TryParse(jobId, out _))
            throw new UserError(L.T("That server can't be joined. Look the player up again."));
        return await LaunchAccountsAsync(vault, accounts, new GameTarget(placeId, jobId), Bool(a, "force"), vault.Data.Settings.LaunchDelaySeconds);
    }
}
