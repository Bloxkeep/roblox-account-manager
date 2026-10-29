namespace RobloxManager;

/// <summary>
/// Keeping saved sign-ins working. Roblox replaces an account's .ROBLOSECURITY every few hours to days and, some months
/// later, refuses the one it replaced (devforum.roblox.com/t/4328913: announced 2026-02-03, enforced from 2026-05-01).
/// A new cookie comes in the Set-Cookie of an answer Roblox gives with the old one (RobloxApi queues it), and is saved
/// to the account once Roblox says it is the same user. Bloxkeep never asks for one (auth's /v1/session/refresh): that
/// ends the session it renews, so an account pasted from a browser or shared with another app would be signed out
/// there (the owner's choice, 2026-10-22).
/// </summary>
public sealed partial class AppController
{
    /// <summary>Tries at asking Roblox whose a renewed cookie is, before it is let go.</summary>
    const int RenewedCheckTries = 4;

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
            // Offline for a moment, or Roblox asked to slow down: asked again a little later, the new one not let go at
            // the first failure.
            // A notice on the account to accept (403 "User is moderated") stays until the person accepts it, so asking
            // again only adds requests (2026-10-29, the owner's PC: 46 in five minutes, every renewal asked up to 4 times).
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex) && attempt < RenewedCheckTries && ex is not RobloxApiException { Moderated: true })
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
            // A request made with the old cookie after Roblox replaced it may have been refused, and the account
            // taken for signed out meanwhile.
            SignedInAgain(account);
        }
        Persist(); // on disk at once: the cookie it replaced may stop working any time now
        PushState();
        Log.Note("Roblox renewed an account's sign-in; saved.");

        bool Holds(Account a) => a.Cookie.Length > 0 && a.Cookie == renewed.Sent && Realm.Normalize(a.Realm) == renewed.Realm;
    }
}
