namespace RobloxManager;

/// <summary>
/// The daily sign-in check. Roblox ends a saved session without telling anyone (a password change, "log out of
/// everywhere", a security sweep), and the person only finds out when Play fails. So once a day while unlocked,
/// every saved sign-in is verified quietly with the same read-only call the account menu's check uses — one account
/// at a time with a pause between them — and one notification names the accounts that stopped working. The same run
/// (and the account menu's check) also reads each account's public profile for a ban.
/// </summary>
public sealed partial class AppController
{
    /// <summary>A day less a few hours: a check made at 9 am is due again at 9 am tomorrow, not at noon.</summary>
    static readonly TimeSpan SignInCheckInterval = TimeSpan.FromHours(20);
    /// <summary>Right after unlocking, status and avatars are being loaded; the quiet check waits its turn.</summary>
    static readonly TimeSpan SignInCheckStartDelay = TimeSpan.FromSeconds(45);
    static readonly TimeSpan SignInCheckPause = TimeSpan.FromSeconds(2);
    /// <summary>After a run that couldn't finish (offline, rate-limited for good), not again before this.</summary>
    static readonly TimeSpan SignInCheckRetry = TimeSpan.FromHours(1);
    /// <summary>How many accounts the notification names before "and N more".</summary>
    const int SignInCheckNamed = 3;
    /// <summary>A longer name is cut in the notification: a balloon holds about 250 characters in all.</summary>
    const int SignInCheckNameLength = 30;
    bool _signInCheckRunning;
    DateTime _signInCheckRetryAt;
    /// <summary>Accounts the last run couldn't reach: a run that fails on the same ones again still counts as done.</summary>
    readonly HashSet<Guid> _signInCheckFailed = new();
    /// <summary>The last run looked like an outage of Roblox's sign-in service; the next one trusts its rejections.</summary>
    bool _signInCheckOutage;

    object? SetDailySignInCheck(bool enabled)
    {
        Prefs.DailySignInCheck = enabled;
        Prefs.Save();
        PushState();
        return null;
    }

    /// <summary>From the process timer: starts the day's check once it is due and nothing else is checking.</summary>
    void RunSignInCheckIfDue(DateTime now)
    {
        if (!Prefs.DailySignInCheck || _vault is null || _signInCheckRunning || _checking || _boot is not null
            || !NetworkUp || now < _signInCheckRetryAt
            || now - (Prefs.LastSignInCheck ?? DateTime.MinValue) < SignInCheckInterval)
            return;
        _ = DailySignInCheckAsync();
    }

    async Task DailySignInCheckAsync()
    {
        _signInCheckRunning = true;
        try
        {
            await QuietSignInCheckAsync();
        }
        // Background work must never take the app down; the next due time tries again.
        catch (Exception ex)
        {
            Log.Write(ex);
            _signInCheckRetryAt = DateTime.UtcNow + SignInCheckRetry;
        }
        finally
        {
            _signInCheckRunning = false;
        }
    }

    /// <summary>
    /// Whether the run may go on: the vault is still the same (not locked), the person hasn't started a check of
    /// their own (which covers this one) or turned the setting off, and there is a connection to ask over.
    /// </summary>
    bool SignInCheckCanGoOn(Vault vault) =>
        _vault == vault && !_disposed && !_checking && NetworkUp && Prefs.DailySignInCheck;

    async Task QuietSignInCheckAsync()
    {
        var vault = _vault!;
        await Task.Delay(SignInCheckStartDelay);
        // Accounts known to be signed out are skipped (the person was told already), and so are ones checked today
        // by hand: a run cut short by a lock then carries on where it stopped instead of asking Roblox twice.
        var due = vault.Data.Accounts
            .Where(a => a.CookieValid != false && a.Cookie.Length > 0
                        && (a.LastChecked is not { } at || DateTime.Now - at >= SignInCheckInterval))
            .ToList();
        int ok = 0;
        bool changed = false, stopped = false, rateLimited = false;
        var rejected = new List<Account>();
        var failed = new List<Guid>();
        foreach (var account in due)
        {
            if (!SignInCheckCanGoOn(vault))
            {
                stopped = true;
                break;
            }
            if (!vault.Data.Accounts.Contains(account))
                continue; // deleted meanwhile: neither asked about nor named
            for (int attempt = 1; ; attempt++)
            {
                string cookie = account.Cookie;
                try
                {
                    var user = await RobloxApi.GetAuthenticatedUserAsync(cookie, account.Realm);
                    if (!SignInCheckCanGoOn(vault))
                    {
                        stopped = true;
                        break;
                    }
                    // Signed in again meanwhile: the old cookie's answer says nothing about the new one.
                    if (account.Cookie != cookie)
                        break;
                    // A rejection is confirmed below, with a second call, before the account is marked.
                    if (user is null)
                    {
                        rejected.Add(account);
                        break;
                    }
                    ok++;
                    SignInAccepted(account);
                    account.UserId = user.Id;
                    account.Username = user.Name;
                    account.DisplayName = user.DisplayName;
                    account.LastChecked = DateTime.Now;
                    changed = true;
                }
                catch (RobloxApiException ex) when (ex.RateLimited)
                {
                    // Roblox asked to slow down: a long pause, then the same account again; still refused after
                    // that, the whole run waits an hour rather than hearing the same from every account in turn.
                    if (attempt >= 3)
                    {
                        rateLimited = true;
                        break;
                    }
                    await Task.Delay(TimeSpan.FromSeconds(20 * attempt));
                    if (SignInCheckCanGoOn(vault))
                        continue;
                    stopped = true;
                    break;
                }
                catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                {
                    failed.Add(account.Id);
                }
                break;
            }
            // Three rejections before any success: Roblox's sign-in service is most likely down, answering 401 to
            // everyone, and nothing it says now is worth acting on.
            if (stopped || rateLimited || (!_signInCheckOutage && ok == 0 && rejected.Count >= 3))
                break;
            await Task.Delay(SignInCheckPause);
        }

        // Many rejections and few successes is an outage, not a mass sign-out: nothing is marked, and the run is
        // repeated in an hour — that second time trusting what it hears, so a real mass sign-out is still reported.
        // One or two rejections with no account accepted say nothing either way (the confirming call 2 s later would
        // meet the same outage), so they too wait for that second run.
        bool outage = !_signInCheckOutage && rejected.Count > 0 && (ok == 0 || rejected.Count >= 3 && rejected.Count > ok);
        var signedOut = new List<Account>();
        if (!stopped && !outage)
        {
            foreach (var account in rejected)
            {
                await Task.Delay(SignInCheckPause);
                if (!SignInCheckCanGoOn(vault))
                {
                    stopped = true;
                    break;
                }
                if (!vault.Data.Accounts.Contains(account))
                    continue;
                if (!await ConfirmSignedOutAsync(account, trusted: true))
                    continue; // accepted after all, or no clear answer: asked again next time
                signedOut.Add(account);
                account.LastChecked = DateTime.Now;
                changed = true;
            }
        }

        if (!stopped && !outage && !rateLimited)
            changed |= await RecheckSignedOutAsync(vault, signedOut);

        // Bans, from each account's public profile (no cookie): the ones checked above, and the signed-out and banned
        // ones too, so a ban that ends is noticed as well. Roblox asking to slow down ends this part until the next run.
        var banned = new List<Account>();
        if (!stopped && !rateLimited)
        {
            var toCheck = vault.Data.Accounts.Where(a => a.UserId > 0 && (due.Contains(a) || a.CookieValid == false || a.Banned == true)).ToList();
            // Most accounts aren't banned, and roblox.com's list of users leaves banned ones out: one request per 100
            // accounts finds those that aren't. Only the ones it leaves out (a ban, or no answer for them) and the ones
            // banned before are asked about one by one, with the pause. (Roblox VNG's are on another host: one by one.)
            var listed = new HashSet<long>();
            var unbanned = toCheck.Where(a => a.Realm == Realm.Global && a.Banned != true).Select(a => a.UserId).ToList();
            if (unbanned.Count > 1)
            {
                try
                {
                    listed = (await RobloxApi.GetUsersAsync(unbanned)).Keys.ToHashSet();
                }
                catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                {
                    // No list: each is asked about on its own, as before.
                }
            }
            foreach (var account in toCheck)
            {
                if (account.Realm == Realm.Global && account.Banned != true && listed.Contains(account.UserId))
                {
                    account.Banned = false; // as CheckBanAsync has it for one that isn't banned
                    account.BannedSince = null;
                    continue;
                }
                if (!SignInCheckCanGoOn(vault))
                {
                    stopped = true;
                    break;
                }
                try
                {
                    bool? change = await CheckBanAsync(account);
                    if (change is true && vault.Data.Accounts.Contains(account))
                        banned.Add(account);
                    changed |= change is not null;
                }
                catch (RobloxApiException ex) when (ex.RateLimited)
                {
                    break;
                }
                catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                {
                    // No clear answer: asked again next time.
                }
                await Task.Delay(SignInCheckPause);
            }
        }

        if (changed && _vault == vault)
        {
            Persist();
            PushStateIfChanged();
        }
        if (signedOut.Count > 0 && _vault == vault)
            Notify(SignedOutText(signedOut), "warn", "signin");
        if (banned.Count > 0 && _vault == vault)
            NotifyBanned(banned);
        if (stopped)
            return;
        if (outage || rateLimited)
        {
            _signInCheckOutage = outage;
            _signInCheckRetryAt = DateTime.UtcNow + SignInCheckRetry;
            return;
        }
        _signInCheckOutage = false;
        // Accounts that couldn't be reached are tried again in an hour — unless they are the same ones as last time
        // (a Roblox VNG account whose service can't be reached from here), which would mean every hour for good.
        bool sameFailures = failed.Count > 0 && failed.All(_signInCheckFailed.Contains);
        _signInCheckFailed.Clear();
        _signInCheckFailed.UnionWith(failed);
        if (failed.Count > 0 && !sameFailures)
        {
            _signInCheckRetryAt = DateTime.UtcNow + SignInCheckRetry;
            return;
        }
        Prefs.LastSignInCheck = DateTime.UtcNow;
        Prefs.Save();
    }

    /// <summary>
    /// The accounts marked signed out, asked again (quietly: the person was told). One Roblox accepts now was marked in an
    /// outage, or its session came back, and is signed in again here, auto-rejoin back on. Before, they were never asked
    /// again: a sign-out marked in an outage stayed until the person checked or signed in by hand.
    /// </summary>
    /// <param name="markedNow">Marked by this run: just asked.</param>
    /// <returns>Whether any was signed in again.</returns>
    async Task<bool> RecheckSignedOutAsync(Vault vault, List<Account> markedNow)
    {
        bool changed = false;
        foreach (var account in vault.Data.Accounts.Where(a => a.CookieValid == false && a.Cookie.Length > 0 && !markedNow.Contains(a)).ToList())
        {
            if (!SignInCheckCanGoOn(vault))
                break;
            string cookie = account.Cookie;
            try
            {
                var user = await RobloxApi.GetAuthenticatedUserAsync(cookie, account.Realm);
                if (user is not null && account.Cookie == cookie && vault.Data.Accounts.Contains(account) && SignInCheckCanGoOn(vault))
                {
                    SignInAccepted(account);
                    account.UserId = user.Id;
                    account.Username = user.Name;
                    account.DisplayName = user.DisplayName;
                    account.LastChecked = DateTime.Now;
                    changed = true;
                }
            }
            catch (RobloxApiException ex) when (ex.RateLimited)
            {
                break;
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                // No clear answer: asked again next time.
            }
            await Task.Delay(SignInCheckPause);
        }
        return changed;
    }

    /// <summary>The names go in as they are written out (<see cref="Name"/>), so Discord's copy follows its own switch and language.</summary>
    string SignedOutText(List<Account> accounts)
    {
        var named = accounts.Take(SignInCheckNamed).ToList();
        int more = accounts.Count - named.Count;
        var list = new Shown(name =>
        {
            string text = string.Join(", ", named.Select(name).Select(n => n.Length > SignInCheckNameLength ? n[..(SignInCheckNameLength - 1)] + "…" : n));
            // Not L.F: that would replace the sentence the Discord copy is made again from.
            return more > 0 ? text + string.Format(L.T(" and {0} more", _nameStyle?.Language ?? L.Language), more) : text;
        }, NameText);
        return L.P(accounts.Count, "{0} account needs signing in again: {1}", "{0} accounts need signing in again: {1}", accounts.Count, list);
    }

    /// <summary>
    /// Asks Roblox's public profile of the account (no cookie) whether it is banned, and notes the answer: marked when
    /// it is, cleared when it no longer is. A banned account shows so on its card and Play leaves it out.
    /// </summary>
    /// <returns>true when it was found banned just now, false when a ban was cleared, null when nothing changed.</returns>
    async Task<bool?> CheckBanAsync(Account account)
    {
        if (account.UserId <= 0)
            return null;
        long userId = account.UserId;
        bool? banned = await RobloxApi.GetBannedAsync(userId, account.Realm);
        // No clear answer, or the account was signed in as someone else meanwhile: left as it was.
        if (banned is null || account.UserId != userId)
            return null;
        if (banned == true)
        {
            if (account.Banned == true)
                return null;
            account.Banned = true;
            account.BannedSince = DateTime.Now;
            return true;
        }
        bool wasBanned = account.Banned == true;
        account.Banned = false;
        account.BannedSince = null;
        return wasBanned ? false : null;
    }

    /// <summary>One notification for the accounts a check found banned (Settings' sign-in kind; Discord's too).</summary>
    void NotifyBanned(List<Account> banned)
    {
        if (banned.Count == 1)
            Notify(L.F("{0} is banned on Roblox.", Name(banned[0])), "warn", "banned", banned[0].Id);
        else
            Notify(L.P(banned.Count, "{0} account is banned on Roblox: {1}", "{0} accounts are banned on Roblox: {1}", banned.Count, Names(banned)), "warn", "banned");
    }
}
