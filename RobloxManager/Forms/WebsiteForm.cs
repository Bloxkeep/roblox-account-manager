using System.Runtime.InteropServices;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>
/// www.roblox.com in a browser of Bloxkeep's own: signed in as a saved account, or blank for the person to sign in on
/// the site (Save to Bloxkeep then keeps that account). Every window is an InPrivate profile of its own, so several
/// accounts can be open side by side: a session cookie lives only in that profile, in memory, is never written to disk
/// and never reaches the person's Chrome or Edge; a blank window's sign-in is gone when it closes, unless saved. All
/// windows share one browser (one temporary folder in %TEMP%), deleted when the last window closes.
/// Once the window is a saved account's, signing out on the site would end the saved session too, so it is stopped
/// and explained instead.
/// </summary>
public sealed class WebsiteForm : Form
{
    const string FolderPrefix = "Bloxkeep-web-";
    const string HomeUrl = "https://www.roblox.com/home";
    const string LoginUrl = "https://www.roblox.com/login";
    const string CookieName = ".ROBLOSECURITY";
    /// <summary>A blocked connection hangs instead of failing, so don't wait for the browser to give up.</summary>
    static readonly TimeSpan SlowAfter = TimeSpan.FromSeconds(12);

    // The browser all website windows share, and its folder; a new one (new folder) once the last window has closed.
    static Task<CoreWebView2Environment>? _env;
    static string? _envDir;
    static int _windows;

    readonly Themes.Theme _theme = Themes.Current;
    readonly Func<Guid, string, Task<bool>> _onSessionChanged;
    readonly Func<string, RobloxUser, (Guid Id, string Name, string Message)> _save;
    readonly ToolTip _tips = new();
    readonly System.Windows.Forms.Timer _slow = new() { Interval = (int)SlowAfter.TotalMilliseconds };
    // A saved account's: Roblox may hand the site a new session cookie (after "Log out of all other sessions", for one),
    // read back now and then so the account keeps a session that works. A blank window's: who signed in there, soon.
    readonly System.Windows.Forms.Timer _sync = new();
    const int SavedSyncMs = 15000, BlankSyncMs = 2000;
    readonly Label _message;
    readonly Button _retry, _back, _forward, _saveButton;
    readonly WebView2 _web;
    string _cookie;
    /// <summary>Who is signed in on a blank window's site, until saved; null when nobody (or not known yet).</summary>
    RobloxUser? _user;
    /// <summary>Asks Roblox whose a session cookie is; replaced by the checks.</summary>
    Func<string, Task<RobloxUser?>> _identify = cookie => RobloxApi.GetAuthenticatedUserAsync(cookie);
    bool _closing, _synced, _syncing, _askingLogout, _hasWindow;
    /// <summary>Asks whether to close the window after a sign-out was stopped; replaced by the checks.</summary>
    Func<WebsiteForm, bool> _askCloseOnLogout = AskCloseOnLogout;

    /// <summary>The saved account this window is signed in as; null for a blank window not saved (yet).</summary>
    public Guid? AccountId { get; private set; }

    /// <param name="accountId">The saved account to open signed in; null for a blank window at the sign-in page.</param>
    /// <param name="onSessionChanged">
    /// Gets a session cookie a saved account's site holds that differs from the one this window was given, to check and
    /// save it; false = couldn't tell yet (no internet), ask again at the next read.
    /// </param>
    /// <param name="save">Save to Bloxkeep: keeps the account signed in on a blank window and says how that went.</param>
    public WebsiteForm(Guid? accountId, string? name, string cookie, Func<Guid, string, Task<bool>> onSessionChanged,
        Func<string, RobloxUser, (Guid Id, string Name, string Message)> save)
    {
        AccountId = accountId;
        _cookie = cookie;
        _onSessionChanged = onSessionChanged;
        _save = save;
        _sync.Interval = accountId is null ? BlankSyncMs : SavedSyncMs;
        SetName(name);
        Icon = Ui.AppIcon;
        BackColor = _theme.Background;
        StartPosition = FormStartPosition.Manual;
        MinimumSize = new Size(LogicalToDeviceUnits(480), LogicalToDeviceUnits(360));

        _back = NavButton("\uE72B", L.T("Back"), core => core.GoBack());
        _forward = NavButton("\uE72A", L.T("Forward"), core => core.GoForward());
        var reload = NavButton("\uE72C", L.T("Reload"), core => core.Reload());
        _back.Enabled = _forward.Enabled = false;
        _message = new Label
        {
            Dock = DockStyle.Fill,
            AutoSize = false,
            TextAlign = ContentAlignment.MiddleLeft,
            ForeColor = _theme.Text2,
            Padding = new Padding(LogicalToDeviceUnits(8), 0, 0, 0),
        };
        _retry = new Button
        {
            Text = L.T("Retry"),
            Dock = DockStyle.Right,
            Width = LogicalToDeviceUnits(96),
            FlatStyle = FlatStyle.Flat,
            BackColor = _theme.Surface,
            ForeColor = _theme.Text,
            Cursor = Cursors.Hand,
            Visible = false,
        };
        _retry.FlatAppearance.BorderColor = _theme.Line;
        _retry.Click += (_, _) => OpenHome();
        _saveButton = new Button
        {
            Text = L.T("Save to Bloxkeep"),
            Dock = DockStyle.Right,
            Width = LogicalToDeviceUnits(140),
            FlatStyle = FlatStyle.Flat,
            BackColor = _theme.Text,
            ForeColor = _theme.Background,
            Cursor = Cursors.Hand,
            Visible = false,
        };
        _saveButton.FlatAppearance.BorderSize = 0;
        _tips.SetToolTip(_saveButton, L.T("Opens signed in next time. Without saving, closing this window signs it out here."));
        _saveButton.Click += (_, _) => SaveAccount();
        var bar = new Panel { Dock = DockStyle.Top, Height = LogicalToDeviceUnits(40), Padding = new Padding(6, 4, 8, 4), BackColor = _theme.Surface };
        bar.Controls.Add(_message);
        bar.Controls.Add(_retry);
        bar.Controls.Add(_saveButton);
        bar.Controls.Add(reload);
        bar.Controls.Add(_forward);
        bar.Controls.Add(_back);
        _web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = _theme.Background };
        Controls.Add(_web);
        Controls.Add(bar);

        _slow.Tick += (_, _) =>
        {
            _slow.Stop();
            ShowUnreachable();
        };
        _sync.Tick += async (_, _) => await SyncSessionAsync();
        Load += async (_, _) => await OpenAsync();
    }

    /// <summary>
    /// The title: which account this window is signed in as (follows Settings › Hide account names); for a blank window
    /// the name of whoever signed in there (hidden alike), or just Roblox.
    /// </summary>
    public void SetName(string? name)
    {
        name ??= _user is { } user && !Prefs.HideNames ? user.Name : null;
        Text = name is null ? "Roblox" : L.F("{0} — Roblox", name);
    }

    /// <summary>Opens beside the main window, each further window a little lower and to the right.</summary>
    public void PlaceNear(Form owner, int index)
    {
        var area = Screen.FromControl(owner).WorkingArea;
        // On a small screen the window leaves room for all six steps, so the next one still shows below and to the right.
        int stepSize = LogicalToDeviceUnits(32), room = stepSize * 5;
        var size = new Size(Math.Min(LogicalToDeviceUnits(1280), area.Width - room), Math.Min(LogicalToDeviceUnits(860), area.Height - room));
        int step = stepSize * (index % 6);
        Bounds = new Rectangle(
            Math.Clamp(area.Left + (area.Width - size.Width) / 2 + step, area.Left, area.Right - size.Width),
            Math.Clamp(area.Top + (area.Height - size.Height) / 2 + step, area.Top, area.Bottom - size.Height),
            size.Width, size.Height);
    }

    Button NavButton(string glyph, string tip, Action<CoreWebView2> go)
    {
        var button = new Button
        {
            Text = glyph,
            Font = new Font(IconFont, 10f),
            Dock = DockStyle.Left,
            Width = LogicalToDeviceUnits(36),
            FlatStyle = FlatStyle.Flat,
            BackColor = _theme.Surface,
            ForeColor = _theme.Text,
            Cursor = Cursors.Hand,
            TabStop = false,
        };
        button.FlatAppearance.BorderSize = 0;
        _tips.SetToolTip(button, tip);
        button.AccessibleName = tip;
        button.Click += (_, _) =>
        {
            if (_web.CoreWebView2 is { } core)
                go(core);
        };
        return button;
    }

    /// <summary>Windows 11's icon font, or Windows 10's.</summary>
    static readonly string IconFont = FontFamily.Families.Any(f => f.Name == "Segoe Fluent Icons") ? "Segoe Fluent Icons" : "Segoe MDL2 Assets";

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Ui.UseThemedTitleBar(this, _theme);
    }

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
        _windows++;
        _hasWindow = true;
        try
        {
            var env = await SharedBrowser();
            var options = env.CreateCoreWebView2ControllerOptions();
            // A profile of its own per window, kept in memory only: nothing of the session reaches the disk.
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
            if (AccountId is not null && _cookie.Length > 0)
            {
                // Only in this window's profile, as a cookie for this browser session only.
                var session = core.CookieManager.CreateCookie(CookieName, _cookie, ".roblox.com", "/");
                session.IsSecure = true;
                session.IsHttpOnly = true;
                core.CookieManager.AddOrUpdateCookie(session);
            }
            GuardSignOut(core, env);

            core.NewWindowRequested += (_, e) =>
            {
                // Roblox pages stay in this window (the account's); anything else opens in the person's own browser,
                // which has no reason to know this account.
                e.Handled = true;
                if (IsRoblox(e.Uri))
                    core.Navigate(e.Uri);
                else if (Uri.TryCreate(e.Uri, UriKind.Absolute, out var uri) && uri.Scheme is "https" or "http")
                    System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(uri.AbsoluteUri) { UseShellExecute = true })?.Dispose();
            };
            core.HistoryChanged += (_, _) =>
            {
                _back.Enabled = core.CanGoBack;
                _forward.Enabled = core.CanGoForward;
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
                _retry.Visible = false;
                Say("", _theme.Text2);
            };
            core.NavigationCompleted += (_, e) =>
            {
                _slow.Stop();
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
            Ui.Error(this, L.T("Microsoft Edge WebView2 isn't installed. Install it from Microsoft's website, then reopen Bloxkeep."));
            Close();
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or UnauthorizedAccessException or ObjectDisposedException)
        {
            if (!_closing)
            {
                Ui.Error(this, L.F("Couldn't open the Roblox website: {0}", ex.Message));
                Close();
            }
        }
    }

    static bool IsRoblox(string url) =>
        Uri.TryCreate(url, UriKind.Absolute, out var uri) && uri.Scheme == "https"
        && (uri.Host.Equals("roblox.com", StringComparison.OrdinalIgnoreCase) || uri.Host.EndsWith(".roblox.com", StringComparison.OrdinalIgnoreCase));

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
            // "Log out of all other sessions" (logoutfromallsessionsandreauthenticate) goes through: this session gets a
            // new cookie, which SyncSessionAsync saves. A window not saved signs out like any browser.
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
            Close();
    }

    static bool AskCloseOnLogout(WebsiteForm form) =>
        Ui.Modal(() => Dialog.Show(form,
            L.T("Signing out here would also sign this account out of Bloxkeep, and you'd have to add it again. When you're done, just close this window."),
            L.T("Sign-out stopped"), Dialog.Kind.Info, [new Dialog.Choice(L.T("Close window")), new Dialog.Choice(L.T("Keep browsing"))])) == 0;

    void OpenHome()
    {
        if (_web.CoreWebView2 is not { } core)
            return;
        _retry.Visible = false;
        Say(L.T("Opening roblox.com…"), _theme.Text2);
        core.Navigate(AccountId is null && _user is null ? LoginUrl : HomeUrl);
    }

    void ShowUnreachable()
    {
        if (_closing)
            return;
        Say(L.T("Can't reach roblox.com. Check your internet connection, then press Retry."), _theme.Bad);
        _retry.Visible = true;
    }

    void Say(string text, Color color)
    {
        _message.Text = text;
        _message.ForeColor = color;
    }

    /// <summary>Reads the session the site holds; a different one (Roblox renewed it) goes to the app to check and save.</summary>
    async Task SyncSessionAsync()
    {
        if (_syncing || _web.CoreWebView2 is not { } core)
            return;
        _syncing = true;
        try
        {
            var cookies = await core.CookieManager.GetCookiesAsync("https://www.roblox.com");
            string? now = cookies.FirstOrDefault(c => c.Name == CookieName && !string.IsNullOrEmpty(c.Value))?.Value;
            if (AccountId is not { } id)
            {
                await TrackSignInAsync(now);
                return;
            }
            // None: signed out some other way, or not yet; the saved session is left as it is.
            if (now is null || now == _cookie)
                return;
            if (await _onSessionChanged(id, now))
                _cookie = now;
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ObjectDisposedException)
        {
            // The page is navigating or the window is closing; the next read tries again.
        }
        finally
        {
            _syncing = false;
        }
    }

    /// <summary>A blank window: who signed in on the site (or out), shown in the title, with Save to Bloxkeep.</summary>
    async Task TrackSignInAsync(string? now)
    {
        now ??= "";
        if (now == _cookie)
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
                return; // no answer from Roblox: asked again at the next read
            }
        }
        if (_closing || AccountId is not null)
            return;
        _cookie = now;
        _user = user; // a cookie that doesn't work is nobody
        SetName(null);
        _saveButton.Visible = user is not null;
        if (user is not null)
            Say(Prefs.HideNames ? L.T("Signed in.") : L.F("Signed in as {0}.", user.Name), _theme.Text2);
    }

    void SaveAccount()
    {
        if (_user is not { } user || _cookie.Length == 0 || AccountId is not null)
            return;
        try
        {
            var (id, name, message) = _save(_cookie, user);
            // From now on the saved account's window: its sign-out is guarded and a renewed session saved.
            AccountId = id;
            _user = null;
            _sync.Interval = SavedSyncMs;
            _saveButton.Visible = false;
            SetName(name);
            Say(message, _theme.Good);
        }
        catch (UserError ex)
        {
            Say(ex.Message, _theme.Bad);
        }
    }

    /// <summary>Closes without the last read of the session (the vault is locking, or the app quitting).</summary>
    public void CloseNow()
    {
        _synced = true;
        Close();
    }

    protected override async void OnFormClosing(FormClosingEventArgs e)
    {
        base.OnFormClosing(e);
        if (e.Cancel || _synced || AccountId is null || _web.CoreWebView2 is null)
            return;
        // One last read of the session before the window goes, a moment at most.
        e.Cancel = true;
        _synced = true;
        Hide();
        try
        {
            await Task.WhenAny(SyncSessionAsync(), Task.Delay(3000));
        }
        finally
        {
            if (!IsDisposed)
                Close();
        }
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        _closing = true;
        _slow.Dispose();
        _sync.Dispose();
        _tips.Dispose();
        Controls.Remove(_web);
        _web.Dispose();
        if (_hasWindow && --_windows == 0 && _envDir is { } dir)
        {
            // The last window: the shared browser exits a moment after, and its folder goes with it.
            _env = null;
            _envDir = null;
            _ = Task.Run(() => DeleteFolderAsync(dir));
        }
        base.OnFormClosed(e);
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
