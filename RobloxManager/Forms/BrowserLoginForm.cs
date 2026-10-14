using System.Runtime.InteropServices;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>
/// The user signs in on roblox.com inside a throw-away WebView2 profile. Each time a session cookie appears it is
/// handed to <c>onSignedIn</c>, the profile is deleted and a fresh sign-in page opens for the next account, so
/// many accounts can be added in one go. The password typed here is never read by the app.
/// </summary>
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
    readonly Label _message;
    readonly Button _retry;
    WebView2? _web;
    string? _profileDir;
    bool _polling, _closing;
    // What the bar says once the page is up: the instructions, or how the last sign-in went.
    string _readyText = L.T("Sign in to each account in turn. Press Done when finished.");
    Color _readyColor;

    /// <param name="onSignedIn">Saves the account for a new session cookie and returns a short confirmation.</param>
    public BrowserLoginForm(Func<string, Task<string>> onSignedIn)
    {
        _onSignedIn = onSignedIn;
        _readyColor = _theme.Text2;
        Text = L.T("Sign in to Roblox");
        Icon = Ui.AppIcon;
        BackColor = _theme.Background;
        StartPosition = FormStartPosition.CenterParent;
        ShowInTaskbar = false;
        Size = new Size(LogicalToDeviceUnits(1000), LogicalToDeviceUnits(800));

        _message = new Label
        {
            Dock = DockStyle.Fill,
            AutoSize = false,
            TextAlign = ContentAlignment.MiddleLeft,
            ForeColor = _theme.Text2,
        };
        var done = new Button
        {
            Text = L.T("Done"),
            Dock = DockStyle.Right,
            Width = LogicalToDeviceUnits(96),
            FlatStyle = FlatStyle.Flat,
            BackColor = _theme.Text,
            ForeColor = _theme.Background,
            Cursor = Cursors.Hand,
        };
        done.FlatAppearance.BorderSize = 0;
        done.Click += (_, _) => Close();
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
        _retry.Click += (_, _) => Retry();
        var bar = new Panel { Dock = DockStyle.Top, Height = LogicalToDeviceUnits(52), Padding = new Padding(12, 8, 10, 8), BackColor = _theme.Surface };
        bar.Controls.Add(_message);
        bar.Controls.Add(_retry);
        bar.Controls.Add(done);
        Controls.Add(bar);

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
            core.NewWindowRequested += (_, e) =>
            {
                e.Handled = true;
                core.Navigate(e.Uri);
            };
            core.ContentLoading += (_, e) =>
            {
                if (e.IsErrorPage || _closing)
                    return;
                _slow.Stop();
                _retry.Visible = false;
                Say(_readyText, _readyColor);
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
        _retry.Visible = false;
        Say(L.T("Opening the Roblox sign-in page…"), _theme.Text2);
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
        Say(L.T("Can't reach roblox.com. Check your internet connection, then press Retry."), _theme.Bad);
        _retry.Visible = true;
    }

    void Say(string text, Color color)
    {
        _message.Text = text;
        _message.ForeColor = color;
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
        _readyColor = ok ? _theme.Good : _theme.Bad;
        _readyText = ok ? message + "  " + L.T("Sign in to the next account, or press Done.") : message;
        Say(_readyText, _readyColor);
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
