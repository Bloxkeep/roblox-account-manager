namespace RobloxManager;

/// <summary>
/// Account menu › Open website: www.roblox.com signed in as the account, in a window of Bloxkeep's own (see
/// <see cref="WebsiteForm"/>), one per account, several side by side. The session cookie goes only into that window's
/// in-memory profile, never into the person's own browser.
/// </summary>
public sealed partial class AppController
{
    readonly Dictionary<Guid, WebsiteForm> _websites = new();

    object? OpenWebsite(Guid id)
    {
        var account = RequireAccount(id);
        if (Realm.Normalize(account.Realm) == Realm.Vng)
            throw new UserError(L.T("The website can't be opened for Roblox VNG accounts yet."));
        if (_websites.TryGetValue(id, out var open))
        {
            if (open.WindowState == FormWindowState.Minimized)
                open.WindowState = FormWindowState.Normal;
            open.Activate();
            return null;
        }
        var form = new WebsiteForm(id, NameText(account), account.Cookie, cookie => WebsiteSessionChangedAsync(id, cookie));
        form.PlaceNear(_owner, _websites.Count);
        form.FormClosed += (_, _) =>
        {
            if (_websites.TryGetValue(id, out var f) && f == form)
                _websites.Remove(id);
        };
        _websites[id] = form;
        form.Show();
        return null;
    }

    /// <summary>
    /// The site holds another session cookie than the window was given: Roblox renewed it (after "Log out of all other
    /// sessions", for one), or the person signed in again there. Saved only when Roblox says it is this same account.
    /// </summary>
    /// <returns>False when Roblox couldn't be asked (no internet): the window asks again later.</returns>
    async Task<bool> WebsiteSessionChangedAsync(Guid id, string cookie)
    {
        var vault = _vault;
        if (vault is null || FindAccount(id) is not { } account || account.Cookie == cookie)
            return true;
        RobloxUser? user;
        try
        {
            user = await RobloxApi.GetAuthenticatedUserAsync(cookie, account.Realm);
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            return false;
        }
        if (_vault != vault || account.Cookie == cookie)
            return true;
        if (user is null || user.Id != account.UserId)
        {
            Log.Note("Website window: the site holds a session of another account, or one that doesn't work; the saved one is kept.");
            return true;
        }
        account.Cookie = cookie;
        account.CookieValid = true;
        account.LastChecked = DateTime.Now;
        SignedInAgain(account);
        Persist();
        PushState();
        Log.Note("Website window: Roblox gave the site a new session for the account; saved.");
        return true;
    }

    /// <summary>Closes website windows (all, or these accounts'): the vault is locking, the accounts are deleted, or the app quits.</summary>
    void CloseWebsites(IEnumerable<Guid>? ids = null)
    {
        var which = ids?.ToHashSet();
        foreach (var form in _websites.Where(p => which is null || which.Contains(p.Key)).Select(p => p.Value).ToList())
        {
            if (!form.IsDisposed)
                form.CloseNow();
        }
    }

    /// <summary>The window titles follow a new alias, Hide account names and the language.</summary>
    void RenameWebsites()
    {
        foreach (var (id, form) in _websites)
        {
            if (FindAccount(id) is { } account && !form.IsDisposed)
                form.SetName(NameText(account));
        }
    }
}
