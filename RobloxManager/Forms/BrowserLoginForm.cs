using System.Runtime.InteropServices;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>Sign-in on roblox.com in a throw-away WebView2 profile: each new session cookie goes to <c>onSignedIn</c>,
/// then a fresh profile opens for the next account. The password typed here is never read by the app. Its top (the
/// site, what to do, Retry, Done) is the app's own page (<see cref="ChromeView"/>), apart from the roblox.com one.</summary>
public sealed class BrowserLoginForm : Form
{
    const string ProfilePrefix = "Bloxkeep-login-";
    const string LoginUrl = "https://www.roblox.com/login";
    readonly Themes.Theme _theme = Themes.Current;
    /// <summary>A blocked connection hangs instead of failing, so don't wait for the browser to give up.</summary>
    static readonly TimeSpan SlowAfter = TimeSpan.FromSeconds(12);

    readonly Func<string, Task<string>> _onSignedIn;
    readonly System.Windows.Forms.Timer _poll = new() { Interval = 1000 };
    readonly System.Windows.Forms.Timer _slow = new() { Interval = (int)SlowAfter.TotalMilliseconds };
    readonly ChromeView _chrome;
    WebView2? _web;
    string? _profileDir;
    bool _polling, _closing;
    // What the bar says once the page is up: the instructions, or how the last sign-in went.
    string _readyText = L.T("Sign in to each account in turn. Press Done when finished.");
    string _readyTone = "";
    // What the top shows now.
    string _message = "", _tone = "";
    string? _host;
    bool _retry;

    /// <param name="onSignedIn">Saves the account for a new session cookie and returns a short confirmation.</param>
    public BrowserLoginForm(Func<string, Task<string>> onSignedIn)
    {
        _onSignedIn = onSignedIn;
        Text = L.T("Sign in to Roblox");
        Icon = Ui.AppIcon;
        BackColor = _theme.Background;
        StartPosition = FormStartPosition.CenterParent;
        ShowInTaskbar = false;
        Size = new Size(LogicalToDeviceUnits(1000), LogicalToDeviceUnits(800));

        _chrome = new ChromeView(this, 52);
        _chrome.Act += (what, _) =>
        {
            if (what == "done")
                Close();
            else if (what == "retry")
                Retry();
        };
        Controls.Add(_chrome.Control);

        _poll.Tick += async (_, _) => await PollCookieAsync();
        _slow.Tick += (_, _) =>
        {
            _slow.Stop();
            ShowUnreachable();
        };
        Load += async (_, _) => await OpenFreshBrowserAsync();
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Ui.UseThemedTitleBar(this, _theme);
        _ = StartChromeAsync();
    }

    async Task StartChromeAsync()
    {
        if (await _chrome.StartAsync())
            Push();
    }

    /// <summary>What the top shows now.</summary>
    void Push()
    {
        if (IsDisposed)
            return;
        _chrome.Show(new
        {
            mode = "login",
            tabs = Array.Empty<object>(),
            back = false,
            forward = false,
            host = _host,
            message = _message,
            tone = _tone,
            retry = _retry,
            save = false,
            theme = PageLook.Theme,
            customTheme = PageLook.CustomTheme,
            language = L.Language,
        });
    }

    /// <summary>A new WebView2 with its own empty profile, so the previous account's session can't carry over.</summary>
    async Task OpenFreshBrowserAsync()
    {
        _profileDir = Path.Combine(Path.GetTempPath(), ProfilePrefix + Guid.NewGuid().ToString("N"));
        var web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = BackColor };
        Controls.Add(web);
        Controls.SetChildIndex(web, 0); // fill the space left by the bar
        _web = web;
        try
        {
            var env = await CoreWebView2Environment.CreateAsync(null, _profileDir, WebView2Setup.EnvironmentOptions());
            await web.EnsureCoreWebView2Async(env);
            if (_closing || web.IsDisposed)
                return;
            var core = web.CoreWebView2;
            core.Settings.IsPasswordAutosaveEnabled = false;
            core.Settings.IsGeneralAutofillEnabled = false;
            // DevTools would show the session cookie, which is how cookie thieves talk people into sharing it.
            core.Settings.AreDevToolsEnabled = false;
            // Only Roblox's own pages here: this window shows no address, so another site would pass for the sign-in
            // page. A link elsewhere opens in the person's own browser, and only when they clicked it.
            core.NavigationStarting += (_, e) =>
            {
                if (WebsiteTab.IsRoblox(e.Uri))
                    return;
                e.Cancel = true;
                if (e.IsUserInitiated)
                    WebsiteTab.OpenOutside(e.Uri);
            };
            core.NewWindowRequested += (_, e) =>
            {
                e.Handled = true;
                if (WebsiteTab.IsRoblox(e.Uri))
                    core.Navigate(e.Uri);
                else if (e.IsUserInitiated)
                    WebsiteTab.OpenOutside(e.Uri);
            };
            // The site's name only (never the address), so it's plain this is roblox.com.
            core.SourceChanged += (_, _) =>
            {
                string? host = Uri.TryCreate(core.Source, UriKind.Absolute, out var uri) && WebsiteTab.IsRoblox(core.Source) ? uri.Host : null;
                if (host == _host || _closing)
                    return;
                _host = host;
                Push();
            };
            core.ContentLoading += (_, e) =>
            {
                if (e.IsErrorPage || _closing)
                    return;
                _slow.Stop();
                _retry = false;
                Say(_readyText, _readyTone);
            };
            core.NavigationCompleted += (_, e) =>
            {
                // Canceled = superseded by another navigation (redirects), not a network problem.
                if (e.IsSuccess || e.WebErrorStatus == CoreWebView2WebErrorStatus.OperationCanceled || _closing)
                    return;
                _slow.Stop();
                ShowUnreachable();
            };
            OpenLoginPage(core);
            _poll.Start();
        }
        catch (WebView2RuntimeNotFoundException)
        {
            Ui.Error(this, L.T("Microsoft Edge WebView2 isn't installed. Install it, or add the account with a cookie."));
            Close();
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or UnauthorizedAccessException or ObjectDisposedException)
        {
            if (!_closing)
            {
                Ui.Error(this, L.F("Couldn't open the sign-in browser: {0}", ex.Message));
                Close();
            }
        }
    }

    void OpenLoginPage(CoreWebView2 core)
    {
        _retry = false;
        Say(L.T("Opening the Roblox sign-in page…"), "");
        _slow.Stop();
        _slow.Start();
        core.Navigate(LoginUrl);
    }

    void Retry()
    {
        if (_web?.CoreWebView2 is { } core)
            OpenLoginPage(core);
    }

    void ShowUnreachable()
    {
        if (_closing)
            return;
        _retry = true;
        Say(L.T("Can't reach roblox.com. Check your internet connection, then press Retry."), "bad");
    }

    void Say(string text, string tone)
    {
        _message = text;
        _tone = tone;
        Push();
    }

    async Task PollCookieAsync()
    {
        if (_polling || _web?.CoreWebView2 is not { } core)
            return;
        _polling = true;
        try
        {
            var cookies = await core.CookieManager.GetCookiesAsync("https://www.roblox.com");
            var session = cookies.FirstOrDefault(c => c.Name == ".ROBLOSECURITY" && !string.IsNullOrEmpty(c.Value));
            if (session is null)
                return;
            _poll.Stop();
            await SaveAndContinueAsync(session.Value);
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ObjectDisposedException)
        {
            // Page is navigating or the window is closing; the next tick tries again.
        }
        finally
        {
            _polling = false;
        }
    }

    async Task SaveAndContinueAsync(string cookie)
    {
        string message;
        bool ok = true;
        try
        {
            message = await _onSignedIn(cookie);
        }
        catch (Exception ex) when (ex is UserError || RobloxApi.IsExpectedError(ex))
        {
            message = ex.Message;
            ok = false;
        }
        if (_closing || IsDisposed)
            return;
        _readyTone = ok ? "ok" : "bad";
        _readyText = ok ? message + "  " + L.T("Sign in to the next account, or press Done.") : message;
        Say(_readyText, _readyTone);
        ReleaseBrowser();
        await OpenFreshBrowserAsync();
    }

    void ReleaseBrowser()
    {
        _poll.Stop();
        _slow.Stop();
        if (_web is not null)
        {
            Controls.Remove(_web);
            _web.Dispose();
            _web = null;
        }
        if (_profileDir is { } dir)
            _ = Task.Run(() => DeleteProfileAsync(dir));
        _profileDir = null;
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        _closing = true;
        ReleaseBrowser();
        _poll.Dispose();
        _slow.Dispose();
        base.OnFormClosed(e);
    }

    /// <summary>The browser process exits a moment after the control is disposed, so deletion is retried.</summary>
    static async Task DeleteProfileAsync(string dir)
    {
        for (int i = 0; i < 20 && Directory.Exists(dir); i++)
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

    /// <summary>Removes profiles left behind if the app was closed before deletion finished.</summary>
    public static void CleanupLeftoverProfiles()
    {
        try
        {
            var leftovers = Directory.EnumerateDirectories(Path.GetTempPath(), ProfilePrefix + "*")
                .Concat(Directory.EnumerateDirectories(Path.GetTempPath(), "RobloxManager-login-*"));
            foreach (var dir in leftovers)
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
