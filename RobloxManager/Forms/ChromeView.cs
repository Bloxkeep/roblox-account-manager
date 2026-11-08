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
    /// <summary>The open menu under the bar (its place in CSS pixels), or none.</summary>
    RectangleF? _menu;

    /// <summary>Set by a window that places the bar itself (<see cref="WebsiteForm"/>: its menu over the page): called
    /// when the bar's height or its menu changes, instead of the bar taking its height docked.</summary>
    public Action? Relayout { get; set; }

    /// <summary>The bar's own height, in device pixels.</summary>
    public int BarPixels => Scaled(_cssHeight);

    /// <summary>The open menu's place on the bar's page, in device pixels; null when none is open.</summary>
    public Rectangle? MenuPixels => _menu is { } m
        ? Rectangle.FromLTRB((int)Math.Floor(m.Left * _form.DeviceDpi / 96f), (int)Math.Floor(m.Top * _form.DeviceDpi / 96f),
            (int)Math.Ceiling(m.Right * _form.DeviceDpi / 96f), (int)Math.Ceiling(m.Bottom * _form.DeviceDpi / 96f))
        : null;

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
        form.DpiChanged += (_, _) => form.BeginInvoke(() => { if (!_web.IsDisposed) Fit(); });
    }

    public WebView2 Control => _web;

    int Scaled(int css) => (int)Math.Ceiling(css * _form.DeviceDpi / 96f);

    void Fit()
    {
        if (Relayout is { } relayout)
            relayout();
        else
            _web.Height = Scaled(_cssHeight);
    }

    /// <summary>Starts the page (the window's handle must exist); false, logged, when it couldn't.</summary>
    public async Task<bool> StartAsync()
    {
        try
        {
            var env = await AppPages.CreateEnvironmentAsync();
            await _web.EnsureCoreWebView2Async(env);
            if (_web.IsDisposed || _form.IsDisposed || _web.CoreWebView2 is not { } core)
                return false;
            AppPages.Configure(core, env);
            core.WebMessageReceived += (_, e) =>
            {
                string json = e.WebMessageAsJson;
                _form.BeginInvoke(() => OnMessage(json));
            };
            // Its page gone (the renderer crashed or hung): loaded again. The browser gone: nothing is left to load it in,
            // and the window's buttons are this page, so the window closes. A GPU process restart: the page goes on
            // (ShellForm.FailureOf).
            core.ProcessFailed += (_, e) =>
            {
                var failure = ShellForm.FailureOf(e.ProcessFailedKind);
                if (failure is not (BrowserFailure.ReloadPage or BrowserFailure.BrowserGone) || _form.IsDisposed || !_form.IsHandleCreated)
                    return;
                _form.BeginInvoke(() =>
                {
                    _ready = false;
                    if (_form.IsDisposed || _web.IsDisposed)
                        return;
                    if (failure == BrowserFailure.BrowserGone)
                        _form.Close();
                    else if (_web.CoreWebView2 is { } c)
                        c.Navigate(AppPages.Origin + "browser.html");
                });
            };
            core.Navigate(AppPages.Origin + "browser.html");
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
                        Fit();
                    }
                    break;
                // The page's menu opened (its place) or closed (none): the window lets it over the page meanwhile.
                case "menu" when args.ValueKind == JsonValueKind.Object:
                    _menu = Number(args, "x") is { } x && Number(args, "y") is { } y && Number(args, "w") is { } mw and > 0 && Number(args, "h") is { } mh and > 0
                        ? new RectangleF((float)x, (float)y, (float)Math.Min(mw, 2000), (float)Math.Min(mh, 2000))
                        : null;
                    Fit();
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

    static double? Number(JsonElement args, string name) =>
        args.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.GetDouble() is var d && double.IsFinite(d) && d >= 0 ? d : null;
}
