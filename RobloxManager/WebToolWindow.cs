using System.Runtime.InteropServices;
using System.Text.Json;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>
/// A small borderless window whose whole face is one of the app's pages (wwwroot/&lt;page&gt;.html, drawing
/// <c>window.BloxkeepUI.&lt;page&gt;(...)</c> from the one ui.js bundle): the macro panel beside the game today, the mini
/// window (MiniWindow), the game overlay and colour picker (OverlayWindow), the dialogs next (the owner's rule of
/// 2026-11-03: nothing the user sees drawn in WinForms). WinForms is only the host: a tool window (not in Alt+Tab), Windows 11's rounded corners in the theme's
/// line colour, the browser of the main window's profile (one shared environment, so one browser and GPU process while
/// the main window's is alive) started on first show and let go a while after the window hides.
/// <para>The page and the host talk in the app page's own messages: the page posts <c>{ cmd, args }</c> ("ready",
/// "size" with its size in CSS pixels, "act" with what was clicked, "drag" to move the window, "resize" to make it
/// taller or shorter by its bottom edge); the host posts
/// <c>{ type: "event", name, data }</c>. The window takes the page's size times its monitor's scale.</para>
/// </summary>
public abstract class WebToolWindow : Form
{
    const int WS_EX_TOOLWINDOW = 0x80, WS_EX_TOPMOST = 0x8, CS_DROPSHADOW = 0x20000;
    const int WM_NCLBUTTONDOWN = 0xA1, HTCAPTION = 2, HTBOTTOM = 15;

    readonly System.Windows.Forms.Timer _release = new();
    WebView2? _web;
    Task? _starting;
    CancellationTokenSource? _startCancel;
    TaskCompletionSource? _ready;
    bool _activate = true;
    /// <summary>The page's size in its own (CSS) pixels, as it last said.</summary>
    Size _page;

    protected WebToolWindow()
    {
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        // The page sizes the window (page size × the monitor's scale); WinForms must not scale it again.
        AutoScaleMode = AutoScaleMode.None;
        Text = Ui.Title;
        Icon = Ui.AppIcon;
        BackColor = Backdrop;
        _release.Tick += (_, _) => ReleaseBrowser();
    }

    /// <summary>The page's file in wwwroot ("panel.html").</summary>
    protected abstract string PageFile { get; }

    /// <summary>Always on top, set in the window's style from its first moment (setting TopMost before Show activates it).</summary>
    protected virtual bool StartsTopMost => false;

    /// <summary>How long the browser is kept after the window hid; shown again meanwhile, it is there at once.</summary>
    protected virtual TimeSpan KeepBrowser => TimeSpan.FromMinutes(1);

    /// <summary>While true the browser is kept even though the window is hidden.</summary>
    protected virtual bool HoldBrowser => false;

    /// <summary>Windows 11's rounded corners and the hairline in the theme's line colour (not over a game: the overlay is square).</summary>
    protected virtual bool Rounded => true;

    /// <summary>Windows' shadow under the window.</summary>
    protected virtual bool Shadow => true;

    /// <summary>The colour shown before the page draws, and under a page that is see-through.</summary>
    protected virtual Color Backdrop => Themes.Current.Surface;

    /// <summary>A click on the page takes the keyboard (it needs it for typing, keys and Esc).</summary>
    protected virtual bool ActivateOnClick => true;

    /// <summary>The page's last size in CSS pixels (empty until it has said).</summary>
    protected Size PageSize => _page;

    /// <summary>The page has started and answered.</summary>
    protected bool BrowserReady => _web?.CoreWebView2 is not null && _ready is { Task.IsCompletedSuccessfully: true };

    protected override CreateParams CreateParams
    {
        get
        {
            var p = base.CreateParams;
            p.ExStyle |= WS_EX_TOOLWINDOW; // not in Alt+Tab
            if (StartsTopMost)
                p.ExStyle |= WS_EX_TOPMOST;
            if (Shadow)
                p.ClassStyle |= CS_DROPSHADOW;
            return p;
        }
    }

    /// <summary>Shown without taking the keyboard when <see cref="ShowQuietly"/> asked so (a game keeps it).</summary>
    protected override bool ShowWithoutActivation => !_activate;

    /// <summary>Whether the next show takes the keyboard.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    protected bool ActivateOnShow
    {
        get => _activate;
        set => _activate = value;
    }

    // ───────────────────────── the browser ─────────────────────────

    /// <summary>Starts the page (once); false when it couldn't (logged; the window stays hidden) or the window went meanwhile.</summary>
    protected async Task<bool> EnsureBrowserAsync()
    {
        _release.Stop();
        while (!BrowserReady)
        {
            if (IsDisposed)
                return false;
            if (_starting is null)
            {
                _startCancel = new CancellationTokenSource();
                _starting = StartBrowserAsync(_startCancel.Token);
            }
            var starting = _starting;
            try
            {
                await starting;
            }
            catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or WebView2RuntimeNotFoundException or TimeoutException or ObjectDisposedException or OperationCanceledException)
            {
                if (_starting == starting)
                {
                    Log.Write(ex);
                    ReleaseBrowser(force: true);
                    return false;
                }
            }
            // Let go while it started (the window disposed, its page failed): the start that came after, if any, is the
            // one that counts. Never this one's failure taking that one's browser down with it.
            if (_starting != starting)
            {
                if (_starting is null)
                    return false;
                continue;
            }
            if (!BrowserReady)
            {
                _starting = null;
                return false;
            }
        }
        return true;
    }

    /// <param name="cancel">Cancelled when the window lets this start go (<see cref="ReleaseBrowser"/>, Dispose).</param>
    async Task StartBrowserAsync(CancellationToken cancel)
    {
        var env = await ShellForm.CreateEnvironmentAsync().WaitAsync(cancel);
        ObjectDisposedException.ThrowIf(IsDisposed, this);
        var web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = Backdrop };
        Controls.Add(web);
        _web = web;
        // The browser starts once its control has a window, and a hidden form's controls get none by themselves.
        _ = Handle;
        _ = web.Handle;
        await web.EnsureCoreWebView2Async(env).WaitAsync(cancel);
        // Disposed or let go while the browser started: nothing of this start is used.
        if (IsDisposed || web.IsDisposed || _web != web || web.CoreWebView2 is not { } core)
            throw new ObjectDisposedException(GetType().Name);
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
        // Its page gone (the renderer crashed or hung, the browser gone): the next show starts it again. Not for a GPU
        // process restart, which the page lives through (ShellForm.FailureOf).
        core.ProcessFailed += (_, e) =>
        {
            if (ShellForm.FailureOf(e.ProcessFailedKind) is BrowserFailure.ReloadPage or BrowserFailure.BrowserGone && IsHandleCreated && !IsDisposed)
                BeginInvoke(() =>
                {
                    if (_web == web)
                        OnBrowserFailed();
                });
        };
        core.WebMessageReceived += (_, e) =>
        {
            string json = e.WebMessageAsJson;
            BeginInvoke(() => OnMessage(json));
        };
        var ready = _ready = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        core.Navigate(ShellForm.Origin + PageFile);
        await ready.Task.WaitAsync(TimeSpan.FromSeconds(15), cancel);
    }

    /// <summary>The page crashed: hidden, and started again on the next show.</summary>
    protected virtual void OnBrowserFailed()
    {
        Hide();
        ReleaseBrowser(force: true);
    }

    /// <summary>The browser goes (its memory with it) unless the window is shown or holds it; the next show starts it again.
    /// A start under way is let go too.</summary>
    protected void ReleaseBrowser(bool force = false)
    {
        _release.Stop();
        if (!force && (Visible || HoldBrowser))
            return;
        _ready = null;
        _starting = null;
        var cancel = _startCancel;
        _startCancel = null;
        if (_web is { } web)
        {
            _web = null;
            Controls.Remove(web);
            web.Dispose();
        }
        cancel?.Cancel();
    }

    protected override void OnVisibleChanged(EventArgs e)
    {
        base.OnVisibleChanged(e);
        if (Visible)
            _release.Stop();
        else if (_web is not null && !IsDisposed)
        {
            _release.Interval = (int)Math.Max(1000, KeepBrowser.TotalMilliseconds);
            _release.Start();
        }
    }

    /// <summary>The browser is let go after <see cref="KeepBrowser"/> again (after <see cref="HoldBrowser"/> turned false).</summary>
    protected void ReleaseLater()
    {
        if (!Visible && _web is not null && !IsDisposed)
        {
            _release.Interval = (int)Math.Max(1000, KeepBrowser.TotalMilliseconds);
            _release.Start();
        }
    }

    /// <summary>The page takes the keyboard (the window was activated: its keys go to the page, not the window).</summary>
    protected void FocusPage()
    {
        if (_web is { IsDisposed: false } web && !web.Focused)
            web.Focus();
    }

    // ───────────────────────── messages ─────────────────────────

    /// <summary>An event to the page, as the app's page gets them: { type: "event", name, data }.</summary>
    protected void SendEvent(string name, object? data)
    {
        if (_web?.CoreWebView2 is not { } core)
            return;
        try
        {
            core.PostWebMessageAsJson(JsonSerializer.Serialize(new { type = "event", name, data }));
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ObjectDisposedException)
        {
            Log.Write(ex);
        }
    }

    /// <summary>The page's messages: { id, cmd, args }.</summary>
    void OnMessage(string json)
    {
        if (IsDisposed)
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
                    _ready?.TrySetResult();
                    OnPageReady();
                    break;
                case "size" when args.ValueKind == JsonValueKind.Object:
                    int w = Num(args, "w"), h = Num(args, "h");
                    if (w > 0 && h > 0)
                    {
                        _page = new Size(w, h);
                        OnPageSize(_page, args);
                    }
                    break;
                case "drag":
                    BeginDrag();
                    break;
                case "resize":
                    BeginResize();
                    break;
                case "act" when args.ValueKind == JsonValueKind.Object:
                    OnAct(Str(args, "what"), args);
                    break;
            }
        }
        catch (JsonException)
        {
        }
    }

    /// <summary>The page has loaded (also again after a crash): push what it draws.</summary>
    protected virtual void OnPageReady()
    {
    }

    /// <summary>The page's size changed (CSS pixels); <paramref name="args"/> has the rest of what it said.</summary>
    protected virtual void OnPageSize(Size css, JsonElement args)
    {
    }

    /// <summary>Something clicked on the page.</summary>
    protected abstract void OnAct(string what, JsonElement args);

    protected static int Num(JsonElement o, string name) => o.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? (int)Math.Ceiling(v.GetDouble()) : 0;

    protected static string Str(JsonElement o, string name) => o.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString()! : "";

    // ───────────────────────── moving and sizing ─────────────────────────

    [DllImport("user32.dll")]
    static extern bool ReleaseCapture();

    [DllImport("user32.dll")]
    static extern IntPtr SendMessage(IntPtr hWnd, int msg, IntPtr wParam, IntPtr lParam);

    /// <summary>
    /// The page's title row pressed and moved: Windows moves the window with the pointer, as by a title bar. (Not CSS
    /// app-region: a drag region swallows the row's right-click, which opens the page's own menu, and a double-click on
    /// it would maximise the borderless window.)
    /// </summary>
    void BeginDrag()
    {
        if (!Visible || (MouseButtons & MouseButtons.Left) == 0)
            return;
        ReleaseCapture();
        SendMessage(Handle, WM_NCLBUTTONDOWN, HTCAPTION, IntPtr.Zero);
        OnDragged();
    }

    /// <summary>The person moved the window by its page (the move has ended).</summary>
    protected virtual void OnDragged()
    {
    }

    /// <summary>
    /// The page's bottom grip pressed: Windows sizes the window by its bottom edge with the pointer, as by a sizing border
    /// (the window has none: its page fills it). The window's minimum and maximum size hold meanwhile.
    /// </summary>
    void BeginResize()
    {
        if (!Visible || (MouseButtons & MouseButtons.Left) == 0)
            return;
        ReleaseCapture();
        SendMessage(Handle, WM_NCLBUTTONDOWN, HTBOTTOM, IntPtr.Zero);
        OnResized();
    }

    /// <summary>The person sized the window by its page (the sizing has ended).</summary>
    protected virtual void OnResized()
    {
    }

    /// <summary>A size in CSS pixels as screen pixels on a monitor of this scale.</summary>
    protected static Size Scaled(Size css, float scale) => new((int)Math.Ceiling(css.Width * scale), (int)Math.Ceiling(css.Height * scale));

    /// <summary>The scale (1 = 96 DPI) of the monitor nearest a point.</summary>
    protected static float ScaleAt(Point p) => DpiAt(p) / 96f;

    /// <summary>The scale of the monitor the window is on now.</summary>
    protected float ScaleNow => IsHandleCreated ? DeviceDpi / 96f : ScaleAt(Location);

    [DllImport("user32.dll")]
    static extern IntPtr MonitorFromPoint(Point pt, uint flags);

    [DllImport("shcore.dll")]
    static extern int GetDpiForMonitor(IntPtr monitor, int type, out uint dpiX, out uint dpiY);

    internal static uint DpiAt(Point p)
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

    /// <summary>
    /// Dragged onto a monitor of another scale: Windows' suggested place is taken (the base does that), then only the size
    /// is set again from the page's size at the new scale. Never the current size times the ratio: with the page's own
    /// size kept apart, a size set twice can't grow or shrink by the ratio twice (TrayWindow's 2026-10-30 lesson).
    /// </summary>
    protected override void OnDpiChanged(DpiChangedEventArgs e)
    {
        base.OnDpiChanged(e);
        if (_page.Width > 0)
            Size = Scaled(_page, e.DeviceDpiNew / 96f);
    }

    // ───────────────────────── look ─────────────────────────

    /// <summary>Windows 11's rounded corners and a border in the theme's line colour; the colour shown before the page draws.</summary>
    protected void Frame()
    {
        BackColor = Backdrop;
        if (_web is { IsDisposed: false } web)
            web.DefaultBackgroundColor = Backdrop;
        if (IsHandleCreated && Rounded)
            Ui.RoundCorners(Handle, Themes.Current);
    }

    const int WM_MOUSEACTIVATE = 0x0021, MA_ACTIVATE = 1;

    protected override void WndProc(ref Message m)
    {
        base.WndProc(ref m);
        if (m.Msg == WM_MOUSEACTIVATE && ActivateOnClick)
            m.Result = MA_ACTIVATE;
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
            _release.Dispose();
            _web?.Dispose();
            _web = null;
            _starting = null;
            _startCancel?.Cancel();
            _startCancel = null;
        }
        base.Dispose(disposing);
    }
}
