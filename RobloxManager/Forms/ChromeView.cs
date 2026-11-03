using System.Runtime.InteropServices;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>The theme and language a page of the app's draws in, as the app's own page gets them.</summary>
static class PageLook
{
    public static string Theme => Themes.Current.Id;

    /// <summary>A custom theme's colours, without its picture (these pages are plain).</summary>
    public static object CustomTheme => new
    {
        background = Prefs.Custom.Background,
        accent = Prefs.Custom.Accent,
        fade = Prefs.Custom.Fade,
        blur = Prefs.Custom.Blur,
        picture = (string?)null,
        video = false,
    };
}

/// <summary>
/// The top of Bloxkeep's browser windows (<see cref="WebsiteForm"/>, <see cref="BrowserLoginForm"/>): browser.html
/// (ui/src/browser/BrowserChrome.tsx) in a WebView2 of the app's own profile docked above the roblox.com one, in the
/// app's theme (until 2026-11 WinForms buttons and labels). It is another browser than the roblox.com pages' (their
/// own throw-away profiles): it gets only what it draws (names as the app shows them, the site's host name, the
/// status words), never a cookie, a page of the site or what is typed there, and it can only ask for the window's own
/// buttons (select, close, back…), never send a page anywhere.
/// </summary>
sealed class ChromeView
{
    readonly Form _form;
    readonly WebView2 _web;
    bool _ready;
    int _cssHeight;
    object? _last;

    /// <summary>Something clicked on the page: its name and the rest of what it said.</summary>
    public event Action<string, JsonElement>? Act;

    /// <param name="cssHeight">Its height until the page has said (CSS pixels), so the page below doesn't jump.</param>
    public ChromeView(Form form, int cssHeight)
    {
        _form = form;
        _cssHeight = cssHeight;
        _web = new WebView2 { Dock = DockStyle.Top, DefaultBackgroundColor = Themes.Current.Background, TabStop = false };
        _web.Height = Scaled(cssHeight);
        // Dragged onto a monitor of another scale: its height again from the page's, once Windows has rescaled the form.
        form.DpiChanged += (_, _) => form.BeginInvoke(() => { if (!_web.IsDisposed) _web.Height = Scaled(_cssHeight); });
    }

    public WebView2 Control => _web;

    int Scaled(int css) => (int)Math.Ceiling(css * _form.DeviceDpi / 96f);

    /// <summary>Starts the page (the window's handle must exist); false, logged, when it couldn't.</summary>
    public async Task<bool> StartAsync()
    {
        try
        {
            var env = await ShellForm.CreateEnvironmentAsync();
            await _web.EnsureCoreWebView2Async(env);
            if (_web.IsDisposed)
                return false;
            var core = _web.CoreWebView2;
            var settings = core.Settings;
            settings.AreDevToolsEnabled = false;
            settings.IsStatusBarEnabled = false;
            settings.IsZoomControlEnabled = false;
            settings.AreBrowserAcceleratorKeysEnabled = false;
            settings.IsPasswordAutosaveEnabled = false;
            settings.IsGeneralAutofillEnabled = false;
            settings.IsSwipeNavigationEnabled = false;
            settings.IsReputationCheckingRequired = false;
            settings.AreDefaultContextMenusEnabled = false;
            core.AddWebResourceRequestedFilter(ShellForm.Origin + "*", CoreWebView2WebResourceContext.All);
            core.WebResourceRequested += (_, e) => ShellForm.ServeAsset(env, e);
            // Only the app's own page, ever.
            core.NavigationStarting += (_, e) => e.Cancel = !e.Uri.StartsWith(ShellForm.Origin, StringComparison.OrdinalIgnoreCase);
            core.NewWindowRequested += (_, e) => e.Handled = true;
            core.WebMessageReceived += (_, e) =>
            {
                string json = e.WebMessageAsJson;
                _form.BeginInvoke(() => OnMessage(json));
            };
            // Its page gone (the renderer crashed): loaded again.
            core.ProcessFailed += (_, _) => _form.BeginInvoke(() =>
            {
                _ready = false;
                if (!_web.IsDisposed && _web.CoreWebView2 is { } c)
                    c.Navigate(ShellForm.Origin + "browser.html");
            });
            core.Navigate(ShellForm.Origin + "browser.html");
            return true;
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or WebView2RuntimeNotFoundException or ObjectDisposedException or UnauthorizedAccessException)
        {
            Log.Write(ex);
            return false;
        }
    }

    /// <summary>What the page draws now (a "browser" event); kept for a page that loads (again) later.</summary>
    public void Show(object data)
    {
        _last = data;
        if (_ready)
            Post(data);
    }

    void Post(object data)
    {
        if (_web.IsDisposed || _web.CoreWebView2 is not { } core)
            return;
        try
        {
            core.PostWebMessageAsJson(JsonSerializer.Serialize(new { type = "event", name = "browser", data }));
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ObjectDisposedException)
        {
            Log.Write(ex);
        }
    }

    void OnMessage(string json)
    {
        if (_web.IsDisposed)
            return;
        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            string cmd = root.TryGetProperty("cmd", out var c) && c.ValueKind == JsonValueKind.String ? c.GetString()! : "";
            var args = root.TryGetProperty("args", out var a) && a.ValueKind == JsonValueKind.Object ? a : default;
            switch (cmd)
            {
                case "ready":
                    _ready = true;
                    if (_last is not null)
                        Post(_last);
                    break;
                case "size" when args.ValueKind == JsonValueKind.Object:
                    if (args.TryGetProperty("h", out var h) && h.ValueKind == JsonValueKind.Number && (int)Math.Ceiling(h.GetDouble()) is > 0 and < 400 and var css)
                    {
                        _cssHeight = css;
                        _web.Height = Scaled(css);
                    }
                    break;
                case "act" when args.ValueKind == JsonValueKind.Object:
                    string what = args.TryGetProperty("what", out var w) && w.ValueKind == JsonValueKind.String ? w.GetString()! : "";
                    Act?.Invoke(what, args);
                    break;
            }
        }
        catch (JsonException)
        {
        }
    }
}
