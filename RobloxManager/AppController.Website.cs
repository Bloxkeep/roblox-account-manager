namespace RobloxManager;

/// <summary>
/// www.roblox.com in a browser window of Bloxkeep's own (see <see cref="WebsiteForm"/>), one tab per account, each its
/// own private sign-in: Account menu › Open website opens one signed in as a saved account (one tab per account), and
/// Add account › Open roblox.com (or the window's +) a blank one to sign in on the site, which Save to Bloxkeep then adds
/// to the list. A session cookie goes only into that tab's in-memory profile, never into the person's own browser.
/// </summary>
public sealed partial class AppController
{
    /// <summary>The browser window, while open.</summary>
    WebsiteForm? _webWindow;

    /// <summary>Every website tab open, saved accounts' and blank ones.</summary>
    readonly List<WebsiteTab> _webTabs = new();

    /// <summary>The tab open for a saved account, by account id.</summary>
    readonly Dictionary<Guid, WebsiteTab> _websites = new();

    object? OpenWebsite(Guid id)
    {
        var account = RequireAccount(id);
        if (Realm.Normalize(account.Realm) == Realm.Vng)
            throw new UserError(L.T("The website can't be opened for Roblox VNG accounts yet."));
        if (_websites.TryGetValue(id, out var open) && !open.IsClosed && _webWindow is { IsDisposed: false } window)
        {
            window.SelectTab(open);
            ShowWebWindow(window);
            return null;
        }
        _websites[id] = AddWebsiteTab(id, NameText(account), account.Cookie);
        return null;
    }

    /// <summary>A blank tab at roblox.com's sign-in page: nothing is saved unless the person presses Save to Bloxkeep.</summary>
    object? OpenBlankWebsite()
    {
        RequireVault();
        AddWebsiteTab(null, null, "");
        return null;
    }

    WebsiteTab AddWebsiteTab(Guid? id, string? name, string cookie)
    {
        WebsiteTab tab = null!;
        tab = new WebsiteTab(id, name, cookie, WebsiteSessionChangedAsync, (session, user) => SaveWebsiteAccount(tab, session, user));
        tab.Closed += () =>
        {
            _webTabs.Remove(tab);
            foreach (var key in _websites.Where(p => p.Value == tab).Select(p => p.Key).ToList())
                _websites.Remove(key);
        };
        _webTabs.Add(tab);
        var window = WebWindow();
        window.AddTab(tab);
        ShowWebWindow(window);
        return tab;
    }

    WebsiteForm WebWindow()
    {
        if (_webWindow is { IsDisposed: false } open)
            return open;
        var window = new WebsiteForm
        {
            // The + and Ctrl+T: a blank tab, while Bloxkeep is unlocked (locking closes the window anyway).
            NewBlankTab = () =>
            {
                if (_vault is not null)
                    AddWebsiteTab(null, null, "");
            },
        };
        window.PlaceNear(_owner);
        window.FormClosed += (_, _) =>
        {
            if (_webWindow == window)
                _webWindow = null;
        };
        _webWindow = window;
        return window;
    }

    static void ShowWebWindow(WebsiteForm window)
    {
        if (!window.Visible)
            window.Show();
        if (window.WindowState == FormWindowState.Minimized)
            window.WindowState = FormWindowState.Normal;
        window.Activate();
    }

    /// <summary>Save to Bloxkeep on a blank tab: the account signed in there joins the list (or its saved sign-in is renewed).</summary>
    (Guid Id, string Name, string Message) SaveWebsiteAccount(WebsiteTab tab, string cookie, RobloxUser user)
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
        // The account's tab from now on, unless it has one open already.
        _websites.TryAdd(result.Id, tab);
        return (result.Id, result.Label, L.F(result.Merged ? "{0} was already here — its sign-in was updated." : "{0} saved.", result.Label));
    }

    /// <summary>
    /// The site holds another session cookie than the tab was given: Roblox renewed it (after "Log out of all other
    /// sessions", for one), or the person signed in again there. Saved only when Roblox says it is this same account.
    /// </summary>
    /// <returns>False when Roblox couldn't be asked (no internet): the tab asks again later.</returns>
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
            Log.Note("Website tab: the site holds a session of another account, or one that doesn't work; the saved one is kept.");
            return true;
        }
        account.Cookie = cookie;
        account.CookieValid = true;
        account.LastChecked = DateTime.Now;
        SignedInAgain(account);
        Persist();
        PushState();
        Log.Note("Website tab: Roblox gave the site a new session for the account; saved.");
        return true;
    }

    /// <summary>
    /// Closes website tabs: all of them with their window (the vault is locking, or the app quits; a blank tab's sign-in
    /// goes with it), or the tabs of these accounts (deleted).
    /// </summary>
    void CloseWebsites(IEnumerable<Guid>? ids = null)
    {
        if (ids is null)
        {
            _webWindow?.CloseNow();
            foreach (var tab in _webTabs.ToList())
                tab.CloseNow();
            return;
        }
        var which = ids.ToHashSet();
        foreach (var tab in _websites.Where(p => which.Contains(p.Key)).Select(p => p.Value).ToList())
        {
            tab.CloseNow();
            if (_webWindow is { IsDisposed: false } window)
                _ = window.CloseTabAsync(tab);
        }
    }

    /// <summary>The tab titles follow a new alias, Hide account names and the language.</summary>
    void RenameWebsites()
    {
        foreach (var tab in _webTabs.Where(t => !t.IsClosed))
            tab.SetName(tab.AccountId is { } id && FindAccount(id) is { } account ? NameText(account) : null);
    }
}
