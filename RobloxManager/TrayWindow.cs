using System.Runtime.InteropServices;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>
/// The tray icon's quick panel (a left click) and menu (a right click): the page tray.html (ui/src/Tray.tsx) in a small
/// window of its own, without a frame, above the taskbar by the pointer, in the app's own look (until 2026-10-26 both
/// were drawn by hand in WinForms, which the owner found ugly). Its browser is the main window's (the same profile),
/// started when the tray is first opened and let go a minute after it was last closed, so the panel costs nothing while
/// nobody looks at it; the first open after that takes about a second. While open, the page gets the state again every
/// two seconds. It goes away when something else is clicked, with Esc, or once a command is picked.
/// </summary>
public sealed class TrayWindow : Form
{
    /// <summary>How long the browser is kept after the tray last closed: opened again meanwhile, it is there at once.</summary>
    static readonly TimeSpan KeepBrowser = TimeSpan.FromMinutes(1);

    readonly System.Windows.Forms.Timer _refresh = new() { Interval = 2000 };
    readonly System.Windows.Forms.Timer _release = new() { Interval = (int)KeepBrowser.TotalMilliseconds };
    WebView2? _web;
    Task? _starting;
    TaskCompletionSource? _ready;
    /// <summary>"panel" or "menu"; a new count each time it opens, so the page starts on its first page.</summary>
    string _mode = "panel";
    int _opened;
    Point _anchor;
    /// <summary>The page's size in its own pixels, as it last said.</summary>
    Size _content = new(344, 300);
    bool _showing;
    long _hiddenAt;

    public event Action? OpenClicked;
    public event Action? MiniClicked;
    /// <summary>Multi-Roblox's switch, with whether it is on now (the page offers it only where it can switch from here).</summary>
    public event Action<bool>? MultiClicked;
    public event Action<Guid>? AccountClicked;
    public event Action<Guid>? PresetClicked;
    public event Action? NewRobloxClicked;
    public event Action? CloseAllClicked;
    public event Action<int>? ShowClientClicked;
    public event Action<Guid>? CloseClientClicked;
    public event Action? QuitClicked;

    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<AppController.TrayInfo>? State { get; set; }
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<bool>? MiniOpen { get; set; }
    /// <summary>An account's picture (Roblox's address for it), when known.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<Guid, string?>? PictureOf { get; set; }
    /// <summary>Settings › Appearance › Custom's colours, for a custom theme.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Func<object?>? CustomTheme { get; set; }

    public TrayWindow()
    {
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        TopMost = true;
        StartPosition = FormStartPosition.Manual;
        Text = Ui.Title;
        BackColor = Themes.Current.Surface;
        _refresh.Tick += (_, _) => Push();
        _release.Tick += (_, _) => ReleaseBrowser();
    }

    protected override CreateParams CreateParams
    {
        get
        {
            const int WS_EX_TOOLWINDOW = 0x80, CS_DROPSHADOW = 0x20000;
            var p = base.CreateParams;
            p.ExStyle |= WS_EX_TOOLWINDOW; // not in Alt+Tab
            p.ClassStyle |= CS_DROPSHADOW;
            return p;
        }
    }

    /// <summary>The tray icon was clicked just as the tray went away (the click itself took it away): don't bring it back.</summary>
    public bool RecentlyHidden => Environment.TickCount64 - _hiddenAt < 400;

    /// <summary>Opens the panel or the menu by the tray icon (the pointer at <paramref name="anchor"/>); the same again closes it.</summary>
    public async void Toggle(Point anchor, string mode)
    {
        if (Visible)
        {
            bool same = mode == _mode;
            HideTray();
            if (same)
                return;
        }
        else if (RecentlyHidden && mode == _mode)
            return;
        _mode = mode;
        _anchor = anchor;
        _opened++;
        _showing = true;
        _release.Stop();
        try
        {
            await EnsureBrowserAsync();
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or WebView2RuntimeNotFoundException or TimeoutException)
        {
            Log.Write(ex);
            _showing = false;
            ReleaseBrowser();
            return;
        }
        Push(); // the page answers with its size, and the window shows then (OnSize)
    }

    public void HideTray()
    {
        _showing = false;
        if (!Visible)
            return;
        _hiddenAt = Environment.TickCount64;
        _refresh.Stop();
        Hide();
        _release.Start();
    }

    protected override void OnDeactivate(EventArgs e)
    {
        base.OnDeactivate(e);
        // Only once on screen: the browser starting up in the hidden window can take the activation away too.
        if (Visible)
            HideTray();
    }

    async Task EnsureBrowserAsync()
    {
        if (_web?.CoreWebView2 is not null && _ready is { Task.IsCompletedSuccessfully: true })
            return;
        _starting ??= StartBrowserAsync();
        try
        {
            await _starting;
        }
        finally
        {
            if (_ready is not { Task.IsCompletedSuccessfully: true })
                _starting = null;
        }
    }

    async Task StartBrowserAsync()
    {
        var env = await ShellForm.CreateEnvironmentAsync();
        var web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = Themes.Current.Surface };
        Controls.Add(web);
        _web = web;
        // The browser starts once its control has a window, and a hidden form's controls get none by themselves.
        _ = Handle;
        _ = web.Handle;
        await web.EnsureCoreWebView2Async(env);
        var core = web.CoreWebView2;
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
        core.NavigationStarting += (_, e) => e.Cancel = !e.Uri.StartsWith(ShellForm.Origin, StringComparison.OrdinalIgnoreCase);
        core.NewWindowRequested += (_, e) => e.Handled = true;
        // Loaded: its first "ready" can come before the page is listened to, so the load itself counts as ready too.
        core.NavigationCompleted += (_, e) =>
        {
            if (e.IsSuccess)
                _ready?.TrySetResult();
        };
        // Its page gone (the renderer crashed): the next open starts it again.
        core.ProcessFailed += (_, _) => BeginInvoke(() => { HideTray(); ReleaseBrowser(); });
        core.WebMessageReceived += (_, e) =>
        {
            string json = e.WebMessageAsJson;
            BeginInvoke(() => OnMessage(json));
        };
        _ready = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        core.Navigate(ShellForm.Origin + "tray.html");
        await _ready.Task.WaitAsync(TimeSpan.FromSeconds(15));
    }

    /// <summary>The browser goes a minute after the tray last closed (its memory with it); the next open starts it again.</summary>
    void ReleaseBrowser()
    {
        _release.Stop();
        if (Visible || _showing)
            return;
        _ready = null;
        _starting = null;
        if (_web is { } web)
        {
            _web = null;
            Controls.Remove(web);
            web.Dispose();
        }
    }

    /// <summary>The page's messages, as the app's page sends them: { id, cmd, args }.</summary>
    public void OnMessage(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            string cmd = root.TryGetProperty("cmd", out var c) && c.ValueKind == JsonValueKind.String ? c.GetString()! : "";
            var args = root.TryGetProperty("args", out var a) && a.ValueKind == JsonValueKind.Object ? a : default;
            switch (cmd)
            {
                case "ready":
                    _ready?.TrySetResult();
                    break;
                case "size" when args.ValueKind == JsonValueKind.Object:
                    OnSize(Num(args, "w"), Num(args, "h"), Num(args, "opened"));
                    break;
                case "act" when args.ValueKind == JsonValueKind.Object:
                    Act(args);
                    break;
            }
        }
        catch (JsonException)
        {
        }
    }

    static int Num(JsonElement o, string name) => o.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? (int)Math.Ceiling(v.GetDouble()) : 0;

    static Guid IdOf(JsonElement o) => o.TryGetProperty("id", out var v) && v.ValueKind == JsonValueKind.String && Guid.TryParse(v.GetString(), out var id) ? id : Guid.Empty;

    /// <summary>What was clicked on the page.</summary>
    public void Act(JsonElement a)
    {
        string what = a.TryGetProperty("what", out var w) && w.ValueKind == JsonValueKind.String ? w.GetString()! : "";
        switch (what)
        {
            case "open":
                HideTray();
                OpenClicked?.Invoke();
                break;
            case "mini":
                HideTray();
                MiniClicked?.Invoke();
                break;
            case "multi":
                MultiClicked?.Invoke(a.TryGetProperty("on", out var on) && on.ValueKind == JsonValueKind.True);
                Push();
                break;
            case "account" when IdOf(a) is var id && id != Guid.Empty:
                HideTray();
                AccountClicked?.Invoke(id);
                break;
            case "preset" when IdOf(a) is var id && id != Guid.Empty:
                HideTray();
                PresetClicked?.Invoke(id);
                break;
            case "newRoblox":
                HideTray();
                NewRobloxClicked?.Invoke();
                break;
            case "closeAll":
                // From the panel, after its second click: it stays, showing the windows go. From the menu: done.
                CloseAllClicked?.Invoke();
                if (_mode == "menu")
                    HideTray();
                else
                    Push();
                break;
            case "show" when a.TryGetProperty("pid", out var pid) && pid.ValueKind == JsonValueKind.Number:
                HideTray();
                ShowClientClicked?.Invoke(pid.GetInt32());
                break;
            case "closeClient" when IdOf(a) is var id && id != Guid.Empty:
                CloseClientClicked?.Invoke(id);
                Push();
                break;
            case "quit":
                HideTray();
                QuitClicked?.Invoke();
                break;
            case "hide":
                HideTray();
                break;
        }
    }

    /// <summary>What the page draws: which part, the state now, the theme and the language.</summary>
    public object Message()
    {
        var s = State?.Invoke();
        object? state = s is null ? null : new
        {
            open = s.Open, multi = s.Multi, multiOnly = s.MultiOnly, robloxCount = s.RobloxCount, ramMb = s.RamMb, miniOpen = MiniOpen?.Invoke() == true,
            presets = s.Presets.Select(p => new { id = p.Id, name = p.Name }),
            accounts = (s.Accounts ?? []).Select(x => new { id = x.Id, name = x.Name, running = x.Running, avatar = PictureOf?.Invoke(x.Id) }),
            clients = (s.Clients ?? []).Select(x => new { id = x.Id, name = x.Name, game = x.Game, pid = x.Pid, ramMb = x.RamMb, avatar = PictureOf?.Invoke(x.Id) }),
        };
        return new
        {
            type = "event", name = "tray",
            data = new { mode = _mode, opened = _opened, state, theme = Themes.Current.Id, customTheme = CustomTheme?.Invoke(), language = L.Language },
        };
    }

    void Push()
    {
        if (_web?.CoreWebView2 is not { } core || (!Visible && !_showing))
            return;
        try
        {
            BackColor = Themes.Current.Surface;
            Frame();
            core.PostWebMessageAsJson(JsonSerializer.Serialize(Message()));
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ObjectDisposedException)
        {
            Log.Write(ex);
        }
    }

    /// <summary>
    /// The page's size: the window takes it, by the tray icon; shown now if it was on its way, once the page has drawn
    /// this opening (<paramref name="opened"/>), not what it showed before.
    /// </summary>
    void OnSize(int width, int height, int opened)
    {
        if (width <= 0 || height <= 0 || (_showing && opened != _opened))
            return;
        _content = new Size(width, height);
        if (!_showing && !Visible)
            return;
        Place();
        if (_showing && !Visible)
        {
            _showing = false;
            Show();
            Activate();
            SetForegroundWindow(Handle);
            _web?.Focus();
            _refresh.Start();
        }
        _showing = false;
    }

    /// <summary>
    /// Above the taskbar by the pointer (beside it, when the taskbar is at a side or the top), inside the screen, at the
    /// scale of that screen; a change in height keeps the edge by the taskbar where it is.
    /// </summary>
    void Place()
    {
        var screen = Screen.FromPoint(_anchor);
        Rectangle work = screen.WorkingArea, all = screen.Bounds;
        float scale = DpiAt(_anchor) / 96f;
        int gap = (int)(12 * scale), w = (int)Math.Ceiling(_content.Width * scale), h = Math.Min((int)Math.Ceiling(_content.Height * scale), work.Height - gap * 2);
        int x = Math.Clamp(_anchor.X - w / 2, work.Left + gap, work.Right - w - gap);
        int y = work.Bottom - h - gap;
        if (work.Top > all.Top)
            y = work.Top + gap;
        else if (work.Left > all.Left || work.Right < all.Right)
        {
            x = work.Left > all.Left ? work.Left + gap : work.Right - w - gap;
            y = Math.Clamp(_anchor.Y - h + gap, work.Top + gap, work.Bottom - h - gap);
        }
        Bounds = new Rectangle(x, y, w, h);
    }

    [DllImport("user32.dll")]
    static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    static extern IntPtr MonitorFromPoint(Point pt, uint flags);

    [DllImport("shcore.dll")]
    static extern int GetDpiForMonitor(IntPtr monitor, int type, out uint dpiX, out uint dpiY);

    static uint DpiAt(Point p)
    {
        const uint MONITOR_DEFAULTTONEAREST = 2;
        try
        {
            return GetDpiForMonitor(MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST), 0, out uint dpi, out _) == 0 && dpi > 0 ? dpi : 96;
        }
        catch (Exception ex) when (ex is DllNotFoundException or EntryPointNotFoundException)
        {
            return 96;
        }
    }

    /// <summary>Windows 11's rounded corners and a border in the theme's line colour.</summary>
    void Frame()
    {
        if (IsHandleCreated)
            ThemedMenu.RoundCorners(Handle, Themes.Current);
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Frame();
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _refresh.Dispose();
            _release.Dispose();
            _web?.Dispose();
        }
        base.Dispose(disposing);
    }
}
