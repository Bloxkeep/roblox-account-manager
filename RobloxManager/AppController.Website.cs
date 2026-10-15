namespace RobloxManager;

/// <summary>
/// www.roblox.com in windows of Bloxkeep's own (see <see cref="WebsiteForm"/>), several side by side, each its own
/// account: Account menu › Open website opens one signed in as a saved account (one window per account), and
/// Add account › Open roblox.com a blank one to sign in on the site, which Save to Bloxkeep then adds to the list.
/// A session cookie goes only into that window's in-memory profile, never into the person's own browser.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Every website window open, saved accounts' and blank ones.</summary>
    readonly List<WebsiteForm> _webWindows = new();

    /// <summary>The window open for a saved account, by account id.</summary>
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
        _websites[id] = ShowWebsite(id, NameText(account), account.Cookie);
        return null;
    }

    /// <summary>A blank window at roblox.com's sign-in page: nothing is saved unless the person presses Save to Bloxkeep.</summary>
    object? OpenBlankWebsite()
    {
        RequireVault();
        ShowWebsite(null, null, "");
        return null;
    }

    WebsiteForm ShowWebsite(Guid? id, string? name, string cookie)
    {
        WebsiteForm form = null!;
        form = new WebsiteForm(id, name, cookie, WebsiteSessionChangedAsync, (session, user) => SaveWebsiteAccount(form, session, user));
        form.PlaceNear(_owner, _webWindows.Count);
        form.FormClosed += (_, _) =>
        {
            _webWindows.Remove(form);
            foreach (var key in _websites.Where(p => p.Value == form).Select(p => p.Key).ToList())
                _websites.Remove(key);
        };
        _webWindows.Add(form);
        form.Show();
        return form;
    }

    /// <summary>Save to Bloxkeep on a blank window: the account signed in there joins the list (or its saved sign-in is renewed).</summary>
    (Guid Id, string Name, string Message) SaveWebsiteAccount(WebsiteForm form, string cookie, RobloxUser user)
    {
        RequireVault();
        var result = AddOrMerge(new Account
        {
            Cookie = cookie,
            UserId = user.Id,
            Username = user.Name,
            DisplayName = user.DisplayName,
            CookieValid = true,
            LastChecked = DateTime.Now,
        });
        // The account's window from now on, unless it has one open already.
        _websites.TryAdd(result.Id, form);
        return (result.Id, result.Label, L.F(result.Merged ? "{0} was already here — its sign-in was updated." : "{0} saved.", result.Label));
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

    /// <summary>
    /// Closes website windows: all of them (the vault is locking, or the app quits; a blank window's sign-in goes with
    /// it), or the windows of these accounts (deleted).
    /// </summary>
    void CloseWebsites(IEnumerable<Guid>? ids = null)
    {
        var which = ids?.ToHashSet();
        var forms = which is null ? _webWindows.ToList() : _websites.Where(p => which.Contains(p.Key)).Select(p => p.Value).ToList();
        foreach (var form in forms)
        {
            if (!form.IsDisposed)
                form.CloseNow();
        }
    }

    /// <summary>The window titles follow a new alias, Hide account names and the language.</summary>
    void RenameWebsites()
    {
        foreach (var form in _webWindows.Where(f => !f.IsDisposed))
            form.SetName(form.AccountId is { } id && FindAccount(id) is { } account ? NameText(account) : null);
    }
}
