using System.Runtime.InteropServices;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>
/// One tab of Bloxkeep's browser (<see cref="WebsiteForm"/>): www.roblox.com signed in as a saved account, or blank for
/// signing in on the site (Save to Bloxkeep keeps that account). Each tab is its own InPrivate profile: the session cookie
/// lives only there, in memory, never on disk or in the person's Chrome or Edge. All tabs share one browser (a %TEMP%
/// folder deleted when the last tab closes). A saved account's sign-out on the site would end the saved session too,
/// so it is stopped and explained.
/// </summary>
public sealed class WebsiteTab : UserControl
{
    const string FolderPrefix = "Bloxkeep-web-";
    const string HomeUrl = "https://www.roblox.com/home";
    const string LoginUrl = "https://www.roblox.com/login";
    const string CookieName = ".ROBLOSECURITY";
    /// <summary>A blocked connection hangs instead of failing, so don't wait for the browser to give up.</summary>
    static readonly TimeSpan SlowAfter = TimeSpan.FromSeconds(12);

    // The browser all tabs share, and its folder; a new one (new folder) once the last tab has closed.
    static Task<CoreWebView2Environment>? _env;
    static string? _envDir;
    static int _tabs;

    readonly Themes.Theme _theme = Themes.Current;
    readonly Func<Guid, string, Task<bool>> _onSessionChanged;
    readonly Func<string, RobloxUser, (Guid Id, string Name, string Message)> _save;
    readonly System.Windows.Forms.Timer _slow = new() { Interval = (int)SlowAfter.TotalMilliseconds };
    // A saved account's tab reads back a cookie Roblox renewed (e.g. after "Log out of all other sessions"); a blank tab's
    // learns who signed in.
    readonly System.Windows.Forms.Timer _sync = new();
    const int SavedSyncMs = 15000, BlankSyncMs = 2000;
    readonly WebView2 _web;
    string _cookie;
    /// <summary>Who is signed in on a blank tab's site, until saved; null when nobody (or not known yet).</summary>
    RobloxUser? _user;
    /// <summary>Replaced by the checks.</summary>
    Func<string, Task<RobloxUser?>> _identify = cookie => RobloxApi.GetAuthenticatedUserAsync(cookie);
    bool _closing, _synced, _syncing, _askingLogout, _hasBrowser, _tornDown;
    /// <summary>Goes up at each switch to another account (<see cref="SwitchToAsync"/>): a read of the session begun
    /// before it, or an older switch still on its way, is left.</summary>
    int _generation;
    /// <summary>
    /// A session Roblox couldn't be asked about (no answer, too many questions, a notice on the account): asked again
    /// after a while, not at every read (2026-10-30 review: a blank tab asked every 2 s, ~30 times a minute, while
    /// Roblox answered "User is moderated", and harder still while it said "too many"). A notice to accept waits for the
    /// next page the tab shows (accepting it loads one).
    /// </summary>
    string? _failedSession;
    DateTime _askAgainAt;
    int _failures;
    static readonly TimeSpan FirstWait = TimeSpan.FromSeconds(30), LongestWait = TimeSpan.FromSeconds(60);
    Func<WebsiteTab, bool> _askCloseOnLogout = AskCloseOnLogout;

    /// <summary>The saved account this tab is signed in as; null for a blank tab not saved (yet).</summary>
    public Guid? AccountId { get; private set; }

    /// <summary>What the tab shows: the account's name (or whoever signed in on a blank tab), else Roblox.</summary>
    public string Title { get; private set; } = "Roblox";

    /// <summary>The title changed (signed in on a blank tab, saved, renamed, Hide account names).</summary>
    public event Action? TitleChanged;

    /// <summary>The tab wants to go: its browser couldn't start, or the person chose Close tab after a stopped sign-out.</summary>
    public event Action? CloseRequested;

    public event Action? Closed;

    public bool IsClosed => _tornDown;

    // What the window's top (WebsiteForm's ChromeView) shows for this tab: words and switches only, never the page.

    /// <summary>The account's Roblox avatar for its tab (none while names are hidden); set by the app.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public string? Avatar
    {
        get => _avatar;
        set
        {
            if (value == _avatar)
                return;
            _avatar = value;
            TitleChanged?.Invoke();
        }
    }
    string? _avatar;

    public bool CanGoBack { get; private set; }
    public bool CanGoForward { get; private set; }
    /// <summary>The page's host name (www.roblox.com), shown so it's plain which site this is; null before one loads.</summary>
    public string? Host { get; private set; }
    /// <summary>The status line, and its colour: "", "ok" or "bad".</summary>
    public string Message { get; private set; } = "";
    public string Tone { get; private set; } = "";
    public bool ShowRetry { get; private set; }
    /// <summary>Save to Bloxkeep: a blank tab someone signed in on.</summary>
    public bool CanSave { get; private set; }

    /// <summary>Something the window's top shows changed (back/forward, the site, the status line, Retry, Save).</summary>
    public event Action? StateChanged;

    void Changed() => StateChanged?.Invoke();

    /// <param name="accountId">The saved account to open signed in; null for a blank tab at the sign-in page.</param>
    /// <param name="onSessionChanged">Gets a renewed cookie of a saved account's site to check and save; false = couldn't
    /// tell yet (no internet), ask again at the next read.</param>
    /// <param name="save">Save to Bloxkeep: keeps the account signed in on a blank tab and says how that went.</param>
    public WebsiteTab(Guid? accountId, string? name, string cookie, Func<Guid, string, Task<bool>> onSessionChanged,
        Func<string, RobloxUser, (Guid Id, string Name, string Message)> save)
    {
        AccountId = accountId;
        _cookie = cookie;
        _onSessionChanged = onSessionChanged;
        _save = save;
        _sync.Interval = accountId is null ? BlankSyncMs : SavedSyncMs;
        SetName(name);
        BackColor = _theme.Background;
        Dock = DockStyle.Fill;

        _web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = _theme.Background };
        Controls.Add(_web);

        _slow.Tick += (_, _) =>
        {
            _slow.Stop();
            ShowUnreachable();
        };
        _sync.Tick += async (_, _) => await SyncSessionAsync();
    }

    /// <summary>Starts the browser even while hidden: a tab hidden at once (another opened right after) would otherwise
    /// never get its Load.</summary>
    public void Start()
    {
        if (_hasBrowser || _tornDown)
            return;
        // Window handles even while hidden (each needs its parent's, made on the way): the browser draws into them.
        _ = Handle;
        _ = _web.Handle;
        _ = OpenAsync();
    }

    /// <summary>The title: the account's name (follows Hide account names), whoever signed in on a blank tab, or Roblox.</summary>
    public void SetName(string? name)
    {
        name ??= _user is { } user && !Prefs.HideNames ? user.Name : null;
        string title = name ?? "Roblox";
        if (title == Title)
            return;
        Title = title;
        TitleChanged?.Invoke();
    }

    public void FocusPage()
    {
        if (!_web.IsDisposed)
            _web.Focus();
    }

    // The window's top asks for these (WebsiteForm): only to the page this tab already shows.

    public void GoBack() => _web.CoreWebView2?.GoBack();

    public void GoForward() => _web.CoreWebView2?.GoForward();

    public void Reload() => _web.CoreWebView2?.Reload();

    /// <summary>Retry after roblox.com couldn't be reached: its home (or sign-in) page again.</summary>
    public void Retry() => OpenHome();

    /// <summary>Save to Bloxkeep, on a blank tab someone signed in on.</summary>
    public void Save() => SaveAccount();

    static Task<CoreWebView2Environment> SharedBrowser()
    {
        if (_env is null || _env.IsFaulted || _env.IsCanceled)
        {
            _envDir = Path.Combine(Path.GetTempPath(), FolderPrefix + Guid.NewGuid().ToString("N"));
            _env = CoreWebView2Environment.CreateAsync(null, _envDir, WebView2Setup.EnvironmentOptions());
        }
        return _env;
    }

    async Task OpenAsync()
    {
        if (_hasBrowser || _tornDown)
            return;
        _tabs++;
        _hasBrowser = true;
        try
        {
            var env = await SharedBrowser();
            var options = env.CreateCoreWebView2ControllerOptions();
            // A profile of its own per tab, kept in memory only: nothing of the session reaches the disk.
            options.ProfileName = "w" + Guid.NewGuid().ToString("N");
            options.IsInPrivateModeEnabled = true;
            await _web.EnsureCoreWebView2Async(env, options);
            if (_closing || _web.IsDisposed)
                return;
            var core = _web.CoreWebView2;
            core.Settings.IsPasswordAutosaveEnabled = false;
            core.Settings.IsGeneralAutofillEnabled = false;
            // DevTools would show the session cookie, which is how cookie thieves talk people into sharing it.
            core.Settings.AreDevToolsEnabled = false;
            AddSession(core);
            GuardSignOut(core, env);

            core.NewWindowRequested += (_, e) =>
            {
                // Roblox pages stay in this tab; anything else opens in the person's own browser, without this account.
                e.Handled = true;
                if (IsRoblox(e.Uri))
                    core.Navigate(e.Uri);
                else
                    OpenOutside(e.Uri);
            };
            // The site's name only (never the address, which can carry a one-time code), so it's plain this is roblox.com.
            core.SourceChanged += (_, _) =>
            {
                string? host = Uri.TryCreate(core.Source, UriKind.Absolute, out var uri) && IsRoblox(core.Source) ? uri.Host : null;
                if (host == Host)
                    return;
                Host = host;
                Changed();
            };
            core.HistoryChanged += (_, _) =>
            {
                CanGoBack = core.CanGoBack;
                CanGoForward = core.CanGoForward;
                Changed();
            };
            // Only Roblox's own pages here, as in the sign-in window (BrowserLoginForm): this window shows no address, so
            // another site, signed-in looking, would pass for Roblox's (2026-10-30 review: a link or a redirect off the
            // site loaded inside it). A link elsewhere opens in the person's own browser, and only when they clicked it.
            core.NavigationStarting += (_, e) =>
            {
                if (MayOpenHere(e.Uri))
                    return;
                e.Cancel = true;
                if (e.IsUserInitiated)
                    OpenOutside(e.Uri);
            };
            core.NavigationStarting += (_, e) =>
            {
                _slow.Stop();
                if (e.Cancel)
                    return;
                _slow.Start();
            };
            core.ContentLoading += (_, e) =>
            {
                if (e.IsErrorPage || _closing)
                    return;
                _slow.Stop();
                ShowRetry = false;
                Say("", "");
            };
            core.NavigationCompleted += (_, e) =>
            {
                _slow.Stop();
                // A notice on the account may have been accepted on the way (Failed): asked again at the next read.
                if (e.IsSuccess && _askAgainAt == DateTime.MaxValue)
                    _askAgainAt = DateTime.UtcNow;
                // Canceled = superseded by another navigation (redirects); a status code = Roblox answered (a 404 page).
                if (e.IsSuccess || e.HttpStatusCode > 0 || e.WebErrorStatus == CoreWebView2WebErrorStatus.OperationCanceled || _closing)
                    return;
                ShowUnreachable();
            };
            OpenHome();
            _sync.Start();
        }
        catch (WebView2RuntimeNotFoundException)
        {
            Ui.Error(FindForm(), L.T("Microsoft Edge WebView2 isn't installed. Install it from Microsoft's website, then reopen Bloxkeep."));
            CloseRequested?.Invoke();
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or UnauthorizedAccessException or ObjectDisposedException)
        {
            if (!_closing)
            {
                Ui.Error(FindForm(), L.F("Couldn't open the Roblox website: {0}", ex.Message));
                CloseRequested?.Invoke();
            }
        }
    }

    /// <summary>A saved account's session, only in this tab's profile, as a cookie for this browser session only.</summary>
    void AddSession(CoreWebView2 core)
    {
        if (AccountId is null || _cookie.Length == 0)
            return;
        var session = core.CookieManager.CreateCookie(CookieName, _cookie, ".roblox.com", "/");
        session.IsSecure = true;
        session.IsHttpOnly = true;
        core.CookieManager.AddOrUpdateCookie(session);
    }

    /// <summary>
    /// The account picker on the window's top: this tab signs in as another saved account, on the page it shows (its
    /// home when that isn't Roblox's). The account it was is read once more first, as when a tab closes, then its
    /// sign-in goes from the tab's profile: cookies and the site's storage, so nothing of it is left for the next one.
    /// </summary>
    public async Task SwitchToAsync(Guid id, string name, string cookie, string? avatar)
    {
        if (_tornDown)
            return;
        int generation = ++_generation;
        if (AccountId is not null && _web.CoreWebView2 is not null)
            await Task.WhenAny(SyncSessionAsync(), Task.Delay(3000));
        if (generation != _generation || _tornDown)
            return;
        AccountId = id;
        _cookie = cookie;
        _user = null;
        CanSave = false;
        _failedSession = null;
        _askAgainAt = default;
        _failures = 0;
        _sync.Interval = SavedSyncMs;
        StartUrl = null;
        Avatar = avatar;
        SetName(name);
        // Not started yet: it signs in as this account when it is (OpenAsync).
        if (_web.CoreWebView2 is not { } core)
        {
            Changed();
            return;
        }
        string? here = IsRoblox(core.Source) ? core.Source : null;
        try
        {
            _sync.Stop();
            core.Stop();
            Say(L.T("Opening roblox.com…"), "");
            await core.Profile.ClearBrowsingDataAsync(CoreWebView2BrowsingDataKinds.Cookies | CoreWebView2BrowsingDataKinds.AllDomStorage);
            if (generation != _generation || _tornDown)
                return;
            AddSession(core);
            ShowRetry = false;
            core.Navigate(here ?? HomeUrl);
            _sync.Start();
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ObjectDisposedException)
        {
            if (!_tornDown)
                Say(L.F("Couldn't open the Roblox website: {0}", ex.Message), "bad");
        }
    }

    internal static bool IsRoblox(string url) =>
        Uri.TryCreate(url, UriKind.Absolute, out var uri) && uri.Scheme == "https"
        && (uri.Host.Equals("roblox.com", StringComparison.OrdinalIgnoreCase) || uri.Host.EndsWith(".roblox.com", StringComparison.OrdinalIgnoreCase));

    /// <summary>
    /// What this tab may show: Roblox's pages; Play's roblox-player: link (Windows asks which app opens it, as in any
    /// browser); a blank page, or one Bloxkeep writes in itself (the checks do; a page can't go to a data: address).
    /// </summary>
    internal static bool MayOpenHere(string url) =>
        IsRoblox(url) || (url.IndexOf(':') is > 0 and var colon && url[..colon].ToLowerInvariant() is "roblox-player" or "roblox" or "about" or "data");

    /// <summary>A web link, in the person's own browser; anything else (a file, another scheme) isn't opened.</summary>
    internal static void OpenOutside(string url)
    {
        if (Uri.TryCreate(url, UriKind.Absolute, out var uri) && uri.Scheme is "https" or "http")
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(uri.AbsoluteUri) { UseShellExecute = true })?.Dispose();
    }

    /// <summary>The site's sign-out: it would end this session on Roblox's side, and with it the account saved in Bloxkeep.</summary>
    static bool IsSignOut(string url) =>
        Uri.TryCreate(url, UriKind.Absolute, out var uri) && IsRoblox(url)
        && (uri.Host.Equals("auth.roblox.com", StringComparison.OrdinalIgnoreCase)
                ? uri.AbsolutePath.TrimEnd('/') is var path && (path.Equals("/v1/logout", StringComparison.OrdinalIgnoreCase) || path.Equals("/v2/logout", StringComparison.OrdinalIgnoreCase))
                : uri.AbsolutePath.StartsWith("/logout", StringComparison.OrdinalIgnoreCase));

    void GuardSignOut(CoreWebView2 core, CoreWebView2Environment env)
    {
        const string Filter = "https://auth.roblox.com/v*/logout*";
        try
        {
            // Also requests from workers and frames, on runtimes that can say so.
            core.AddWebResourceRequestedFilter(Filter, CoreWebView2WebResourceContext.All, CoreWebView2WebResourceRequestSourceKinds.All);
        }
        catch (Exception ex) when (ex is NotImplementedException or InvalidCastException or COMException)
        {
            core.AddWebResourceRequestedFilter(Filter, CoreWebView2WebResourceContext.All);
        }
        core.WebResourceRequested += (_, e) =>
        {
            // "Log out of all other sessions" goes through (SyncSessionAsync saves the new cookie). An unsaved tab signs out freely.
            if (AccountId is null || !IsSignOut(e.Request.Uri) || !e.Request.Method.Equals("POST", StringComparison.OrdinalIgnoreCase))
                return;
            e.Response = env.CreateWebResourceResponse(null, 403, "Forbidden", "Content-Type: application/json");
            BeginInvoke(SignOutStopped);
        };
        core.NavigationStarting += (_, e) =>
        {
            if (AccountId is null || !IsSignOut(e.Uri))
                return;
            e.Cancel = true;
            BeginInvoke(SignOutStopped);
        };
    }

    void SignOutStopped()
    {
        if (_askingLogout || _closing || IsDisposed)
            return;
        _askingLogout = true;
        bool close;
        try
        {
            close = _askCloseOnLogout(this);
        }
        finally
        {
            _askingLogout = false;
        }
        if (close && !IsDisposed)
            CloseRequested?.Invoke();
    }

    static bool AskCloseOnLogout(WebsiteTab tab) =>
        Ui.Modal(() => Dialog.Show(tab.FindForm(),
            L.T("Signing out here would also sign this account out of Bloxkeep, and you'd have to add it again. When you're done, just close this tab."),
            L.T("Sign-out stopped"), Dialog.Kind.Info, [new Dialog.Choice(L.T("Close tab")), new Dialog.Choice(L.T("Keep browsing"))])) == 0;

    void OpenHome()
    {
        if (_web.CoreWebView2 is not { } core)
            return;
        ShowRetry = false;
        Say(L.T("Opening roblox.com…"), "");
        core.Navigate(AccountId is null && _user is null ? LoginUrl : StartUrl ?? HomeUrl);
    }

    /// <summary>Where a saved account's tab opens instead of its home page (Appeal: Roblox's Violations & Appeals).</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public string? StartUrl { get; set; }

    /// <summary>An open tab goes to <paramref name="url"/>, a page on roblox.com.</summary>
    public void Go(string url)
    {
        StartUrl = url;
        if (_web.CoreWebView2 is { } core)
            core.Navigate(url);
    }

    void ShowUnreachable()
    {
        if (_closing)
            return;
        ShowRetry = true;
        Say(L.T("Can't reach roblox.com. Check your internet connection, then press Retry."), "bad");
    }

    void Say(string text, string tone)
    {
        Message = text;
        Tone = tone;
        Changed();
    }

    /// <summary>Reads the session the site holds; a different one (Roblox renewed it) goes to the app to check and save.</summary>
    async Task SyncSessionAsync()
    {
        if (_syncing || _web.CoreWebView2 is not { } core)
            return;
        _syncing = true;
        int generation = _generation;
        try
        {
            var cookies = await core.CookieManager.GetCookiesAsync("https://www.roblox.com");
            string? now = cookies.FirstOrDefault(c => c.Name == CookieName && !string.IsNullOrEmpty(c.Value))?.Value;
            // Switched to another account meanwhile: what was read may be either one's.
            if (generation != _generation)
                return;
            if (AccountId is not { } id)
            {
                await TrackSignInAsync(now);
                return;
            }
            // None: signed out some other way, or not yet; the saved session is left as it is.
            if (now is null || now == _cookie || Waiting(now))
                return;
            bool saved = await _onSessionChanged(id, now);
            if (generation != _generation)
                return;
            if (saved)
                _cookie = now;
            else
                Failed(now, moderated: false);
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ObjectDisposedException)
        {
            // The page is navigating or the tab is closing; the next read tries again.
        }
        finally
        {
            _syncing = false;
        }
    }

    /// <summary>A blank tab: who signed in on the site (or out), shown as its title, with Save to Bloxkeep.</summary>
    async Task TrackSignInAsync(string? now)
    {
        now ??= "";
        if (now == _cookie || Waiting(now))
            return;
        RobloxUser? user = null;
        if (now.Length > 0)
        {
            try
            {
                user = await _identify(now);
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                Failed(now, moderated: ex is RobloxApiException { Moderated: true });
                return; // no answer from Roblox: asked again later
            }
        }
        if (_closing || AccountId is not null)
            return;
        _cookie = now;
        _user = user; // a cookie that doesn't work is nobody
        SetName(null);
        CanSave = user is not null;
        if (user is not null)
            Say(Prefs.HideNames ? L.T("Signed in.") : L.F("Signed in as {0}.", user.Name), "");
        else
            Changed();
    }

    bool Waiting(string session) => session == _failedSession && DateTime.UtcNow < _askAgainAt;

    void Failed(string session, bool moderated)
    {
        _failures = session == _failedSession ? _failures + 1 : 1;
        _failedSession = session;
        var wait = _failures > 1 ? LongestWait : FirstWait;
        _askAgainAt = moderated ? DateTime.MaxValue : DateTime.UtcNow + wait;
    }

    void SaveAccount()
    {
        if (_user is not { } user || _cookie.Length == 0 || AccountId is not null)
            return;
        try
        {
            var (id, name, message) = _save(_cookie, user);
            // From now on the saved account's tab: its sign-out is guarded and a renewed session saved.
            AccountId = id;
            _user = null;
            _sync.Interval = SavedSyncMs;
            CanSave = false;
            SetName(name);
            Say(message, "ok");
        }
        catch (UserError ex)
        {
            Say(ex.Message, "bad");
        }
    }

    /// <summary>Closes after one last read of a saved account's session (a moment at most).</summary>
    public async Task CloseAsync()
    {
        if (_tornDown)
            return;
        if (!_synced && AccountId is not null && _web.CoreWebView2 is not null)
        {
            _synced = true;
            await Task.WhenAny(SyncSessionAsync(), Task.Delay(3000));
        }
        TearDown();
    }

    /// <summary>Closes without the last read of the session (the vault is locking, or the app quitting).</summary>
    public void CloseNow()
    {
        _synced = true;
        TearDown();
    }

    void TearDown()
    {
        if (_tornDown)
            return;
        _tornDown = _closing = true;
        _slow.Dispose();
        _sync.Dispose();
        Controls.Remove(_web);
        _web.Dispose();
        if (_hasBrowser && --_tabs == 0 && _envDir is { } dir)
        {
            // The last tab: the shared browser exits a moment after, and its folder goes with it.
            _env = null;
            _envDir = null;
            _ = Task.Run(() => DeleteFolderAsync(dir));
        }
        Closed?.Invoke();
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
            TearDown();
        base.Dispose(disposing);
    }

    /// <summary>The browser process exits a moment after the control is disposed, so deletion is retried.</summary>
    static async Task DeleteFolderAsync(string dir)
    {
        for (int i = 0; i < 30 && Directory.Exists(dir); i++)
        {
            try
            {
                Directory.Delete(dir, recursive: true);
                return;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                await Task.Delay(500);
            }
        }
    }

    /// <summary>Removes folders left behind if the app was closed before deletion finished.</summary>
    public static void CleanupLeftovers()
    {
        try
        {
            foreach (var dir in Directory.EnumerateDirectories(Path.GetTempPath(), FolderPrefix + "*"))
            {
                try { Directory.Delete(dir, recursive: true); }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { }
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
    }
}
