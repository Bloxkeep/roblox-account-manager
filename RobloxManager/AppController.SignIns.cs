namespace RobloxManager;

/// <summary>
/// Keeping saved sign-ins working. Roblox replaces an account's .ROBLOSECURITY every few hours to days and, some months
/// later, refuses the one it replaced (devforum.roblox.com/t/4328913: announced 2026-02-03, enforced from 2026-05-01).
/// A new cookie comes in the Set-Cookie of an answer Roblox gives with the old one (RobloxApi queues it), and is saved
/// to the account once Roblox says it is the same user. Many of Roblox's endpoints don't send one yet (its staff,
/// 2026-05-19), so a sign-in that wasn't renewed for a week is renewed on purpose: one request, one account at a time.
/// </summary>
public sealed partial class AppController
{
    /// <summary>A sign-in not renewed for this long is renewed on purpose. Roblox gives months; a week leaves plenty.</summary>
    static readonly TimeSpan SignInRenewInterval = TimeSpan.FromDays(7);
    /// <summary>Right after unlocking, status, avatars and the sign-in check go first.</summary>
    static readonly TimeSpan SignInRenewStartDelay = TimeSpan.FromMinutes(2);
    /// <summary>How often the process timer looks for sign-ins due.</summary>
    static readonly TimeSpan SignInRenewLook = TimeSpan.FromMinutes(1);
    static readonly TimeSpan SignInRenewPause = TimeSpan.FromSeconds(5);
    /// <summary>A renewal Roblox refused (or that couldn't be asked) isn't asked again for this long.</summary>
    static readonly TimeSpan SignInRenewRetry = TimeSpan.FromHours(6);
    /// <summary>Tries at asking Roblox whose a renewed cookie is, before it is let go.</summary>
    const int RenewedCheckTries = 4;

    Vault? _renewVault;
    DateTime _nextRenewLook;
    bool _renewing;
    /// <summary>When each account's renewal was last asked for, since this vault opened.</summary>
    readonly Dictionary<Guid, DateTime> _renewAsked = new();

    /// <summary>From the process timer: the cookies Roblox renewed in its answers go to their accounts.</summary>
    void SaveRenewedSignIns()
    {
        if (_vault is not null && RobloxApi.RenewedCookiesWaiting)
            _ = SaveRenewedSignInsAsync();
    }

    async Task SaveRenewedSignInsAsync()
    {
        try
        {
            var saves = new List<Task>();
            while (RobloxApi.TryTakeRenewedCookie(out var renewed))
                saves.Add(SaveRenewedSignInAsync(renewed));
            await Task.WhenAll(saves);
        }
        // Background work must never take the app down.
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    /// <summary>
    /// Saves a renewed cookie to the account(s) the old one was saved for, once Roblox says it signs in that same user:
    /// never another user's sign-in in an account's place. A renewal for a cookie no account holds (deleted, another
    /// vault's, replaced meanwhile) is let go.
    /// </summary>
    async Task SaveRenewedSignInAsync(RenewedCookie renewed)
    {
        var vault = _vault;
        if (vault is null || !vault.Data.Accounts.Any(Holds))
            return;
        RobloxUser? user = null;
        for (int attempt = 1; ; attempt++)
        {
            try
            {
                user = await RobloxApi.GetAuthenticatedUserAsync(renewed.Renewed, renewed.Realm);
                break;
            }
            // Offline for a moment, or Roblox asked to slow down: asked again a little later. The old cookie may
            // already be refused (a renewal on purpose ends it), so the new one isn't let go at the first failure.
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex) && attempt < RenewedCheckTries)
            {
                await Task.Delay(TimeSpan.FromSeconds(10 * attempt * attempt));
                if (_vault != vault)
                    return;
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                Log.Note("Roblox renewed an account's sign-in, but couldn't be asked whose it is; the saved one is kept.");
                return;
            }
        }
        if (_vault != vault)
            return;
        var accounts = vault.Data.Accounts.Where(Holds).ToList();
        if (user is null || accounts.Count == 0 || accounts.All(a => a.UserId != user.Id))
        {
            if (accounts.Count > 0)
                Log.Note("Roblox sent a sign-in for another account, or one that doesn't work; the saved one is kept.");
            return;
        }
        foreach (var account in accounts.Where(a => a.UserId == user.Id))
        {
            account.Cookie = renewed.Renewed;
            account.CookieValid = true;
            account.CookieRenewed = DateTime.Now;
            account.LastChecked = DateTime.Now;
            account.Username = user.Name;
            account.DisplayName = user.DisplayName;
            // A request made with the old cookie just after a renewal on purpose may have been refused, and the
            // account taken for signed out meanwhile.
            SignedInAgain(account);
        }
        Persist(); // on disk at once: the cookie it replaced may stop working any time now
        PushState();
        Log.Note("Roblox renewed an account's sign-in; saved.");

        bool Holds(Account a) => a.Cookie.Length > 0 && a.Cookie == renewed.Sent && Realm.Normalize(a.Realm) == renewed.Realm;
    }

    /// <summary>
    /// Due for a renewal on purpose: signed in (not signed out or banned, as far as Bloxkeep knows), not renewed for
    /// <see cref="SignInRenewInterval"/>, not asked lately, and not open on the website (its tab holds this same
    /// session, which the renewal would end; the site renews its own, and the tab saves that).
    /// </summary>
    bool SignInRenewDue(Account a, DateTime now) =>
        a.Cookie.Length > 0 && a.UserId > 0 && a.CookieValid != false && a.Banned != true
        && (a.CookieRenewed is not { } renewed || DateTime.Now - renewed >= SignInRenewInterval)
        && (!_renewAsked.TryGetValue(a.Id, out var asked) || now - asked >= SignInRenewRetry)
        && !_websites.ContainsKey(a.Id);

    /// <summary>
    /// From the process timer: renews the sign-ins due, once nothing else is asking Roblox with accounts' cookies or
    /// opening Roblox (a request still on its way with the cookie a renewal ends would be refused).
    /// </summary>
    void RenewSignInsIfDue(DateTime now)
    {
        if (_vault is not { } vault)
            return;
        if (_renewVault != vault)
        {
            _renewVault = vault;
            _renewAsked.Clear();
            _nextRenewLook = now + SignInRenewStartDelay;
        }
        if (_renewing || now < _nextRenewLook || !NetworkUp || OpeningRoblox || _checking || _signInCheckRunning || _boot is not null
            || _presenceBusy || _robuxBusy)
            return;
        _nextRenewLook = now + SignInRenewLook;
        var due = vault.Data.Accounts.Where(a => SignInRenewDue(a, now)).ToList();
        if (due.Count > 0)
            _ = RenewSignInsAsync(vault, due);
    }

    async Task RenewSignInsAsync(Vault vault, List<Account> due)
    {
        _renewing = true;
        try
        {
            foreach (var account in due)
            {
                if (_vault != vault || _disposed || !NetworkUp || OpeningRoblox)
                    break;
                // Changed meanwhile: deleted, signed out, renewed by an answer, opened on the website.
                if (!vault.Data.Accounts.Contains(account) || !SignInRenewDue(account, DateTime.UtcNow))
                    continue;
                _renewAsked[account.Id] = DateTime.UtcNow;
                try
                {
                    await RobloxApi.RefreshSessionAsync(account.Cookie, account.Realm);
                    // Asked: not again for a week, whether or not a new cookie came (one that came is saved here).
                    account.CookieRenewed = DateTime.Now;
                    PersistSoon();
                    await SaveRenewedSignInsAsync();
                }
                catch (RobloxApiException ex) when (ex.RateLimited)
                {
                    break; // the rest wait for the next look
                }
                catch (RobloxApiException ex) when (ex.CookieInvalid)
                {
                    if (await ConfirmSignedOutAsync(account))
                        PushState();
                }
                catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                {
                    // Refused or offline: asked again after SignInRenewRetry.
                }
                await Task.Delay(SignInRenewPause);
            }
        }
        // Background work must never take the app down; the next look tries again.
        catch (Exception ex)
        {
            Log.Write(ex);
        }
        finally
        {
            _renewing = false;
        }
    }
}
