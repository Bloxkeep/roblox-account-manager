using System.Runtime.InteropServices;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace RobloxManager;

/// <summary>
/// The app window: hosts the HTML interface embedded in the exe. The page may only load its own files and
/// Roblox CDN images (see the CSP in index.html); every network call goes through <see cref="AppController"/>.
/// The interface is drawn by WebView2 on the graphics card; if the GPU or its driver keeps crashing, or a start
/// never got the page on screen, it switches to drawing on the CPU and reloads without losing the session.
/// </summary>
public sealed class ShellForm : Form
{
    const string Origin = "https://bloxkeep.local/";
    const string SafeRenderingArgs = "--disable-gpu --disable-gpu-compositing";
    const int MaxAutomaticRestarts = 3;
    static readonly Color Background = Color.FromArgb(10, 12, 17);

    readonly AppController _app;
    readonly System.Windows.Forms.Timer _releaseTimer = new() { Interval = 30_000 };
    WebView2? _web;
    int _gpuCrashes, _automaticRestarts;
    bool _restarting, _released;

    [DllImport("kernel32.dll")]
    static extern IntPtr GetCurrentProcess();

    [DllImport("kernel32.dll")]
    static extern bool SetProcessWorkingSetSize(IntPtr process, IntPtr minimum, IntPtr maximum);

    static string StartMarker => Path.Combine(Vault.DataDir, "starting.flag");

    public ShellForm()
    {
        // Created first: setting Size below already raises OnResize.
        _app = new AppController(this, PostToPage) { SetSafeRendering = safe => RestartWebViewAsync(safe, automatic: false) };
        Text = Ui.Title;
        BackColor = Background;
        StartPosition = FormStartPosition.CenterScreen;
        Size = new Size(LogicalToDeviceUnits(1320), LogicalToDeviceUnits(840));
        MinimumSize = new Size(LogicalToDeviceUnits(980), LogicalToDeviceUnits(620));
        Icon = Ui.AppIcon;
        Load += async (_, _) => await InitAsync();
        _releaseTimer.Tick += (_, _) => ReleaseWebView();
    }

    void PostToPage(string json)
    {
        if (IsDisposed || _web is not { IsDisposed: false } web)
            return;
        try
        {
            web.CoreWebView2?.PostWebMessageAsJson(json);
        }
        catch (Exception ex) when (ex is InvalidOperationException or ObjectDisposedException or COMException)
        {
            // The page is reloading or the window is closing; the page asks for fresh state when it loads.
        }
    }

    public new void BringToFront()
    {
        if (WindowState == FormWindowState.Minimized)
            WindowState = FormWindowState.Normal;
        Activate();
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Ui.UseDarkTitleBar(this, Background);
    }

    /// <summary>
    /// When minimised, hide the page (the browser throttles it), trim its memory and poll less; after 30 seconds
    /// the browser is closed entirely, leaving only the small core running until the window comes back.
    /// </summary>
    protected override void OnResize(EventArgs e)
    {
        base.OnResize(e);
        bool minimized = WindowState == FormWindowState.Minimized;
        _app.Background = minimized;
        if (minimized)
            _releaseTimer.Start();
        else
        {
            _releaseTimer.Stop();
            if (_released && !_restarting)
                _ = RecreateWebViewAsync();
        }
        if (_web?.CoreWebView2 is not { } core)
            return;
        _web.Visible = !minimized;
        core.MemoryUsageTargetLevel = minimized ? CoreWebView2MemoryUsageTargetLevel.Low : CoreWebView2MemoryUsageTargetLevel.Normal;
    }

    void ReleaseWebView()
    {
        _releaseTimer.Stop();
        if (_web is not { } web || WindowState != FormWindowState.Minimized || _restarting)
            return;
        _web = null;
        _released = true;
        Controls.Remove(web);
        web.Dispose();
        // Hand the freed memory back to Windows while the app waits in the background.
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        SetProcessWorkingSetSize(GetCurrentProcess(), -1, -1);
    }

    async Task RecreateWebViewAsync()
    {
        _released = false;
        _restarting = true;
        try
        {
            _app.ResumingPage = true;
            await AttachWebViewAsync(await CreateEnvironmentAsync());
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or WebView2RuntimeNotFoundException)
        {
            Log.Write(ex);
            Ui.Error(this, L.F("Couldn't restart the interface: {0}", ex.Message));
        }
        finally
        {
            _restarting = false;
        }
    }

    async Task InitAsync()
    {
        // The last start never got the page on screen (for example a broken graphics driver): don't use the GPU.
        if (File.Exists(StartMarker) && !Prefs.SafeRendering)
        {
            Prefs.SafeRendering = true;
            Prefs.Save();
        }

        CoreWebView2Environment? env = null;
        bool runtimeMissing = false;
        try
        {
            env = await CreateEnvironmentAsync();
        }
        catch (WebView2RuntimeNotFoundException)
        {
            runtimeMissing = true;
        }
        // Missing runtime: offer Microsoft's installer once, then try again.
        if (runtimeMissing && !await WebView2Setup.TryInstallAsync(this))
        {
            Close();
            return;
        }
        try
        {
            env ??= await CreateEnvironmentAsync();
            await AttachWebViewAsync(env);
        }
        catch (WebView2RuntimeNotFoundException)
        {
            Ui.Error(this, L.T("Microsoft Edge WebView2 isn't installed. Install it from Microsoft's website, then reopen Bloxkeep."));
            Close();
        }
    }

    static Task<CoreWebView2Environment> CreateEnvironmentAsync()
    {
        bool safe = Prefs.SafeRendering;
        var options = safe ? new CoreWebView2EnvironmentOptions(SafeRenderingArgs) : null;
        // Separate profile folders, because a running browser can't be reopened with different switches.
        return CoreWebView2Environment.CreateAsync(null, Path.Combine(Vault.DataDir, safe ? "webview-safe" : "webview"), options);
    }

    async Task AttachWebViewAsync(CoreWebView2Environment env)
    {
        TryMarkStarting(true);
        var web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = Background };
        Controls.Add(web);
        _web = web;
        await web.EnsureCoreWebView2Async(env);
        Configure(web.CoreWebView2, env);
        web.CoreWebView2.Navigate(Origin + "index.html");
    }

    void Configure(CoreWebView2 core, CoreWebView2Environment env)
    {
        var settings = core.Settings;
        settings.AreDevToolsEnabled = false;
        settings.IsStatusBarEnabled = false;
        settings.IsZoomControlEnabled = false;
        settings.AreBrowserAcceleratorKeysEnabled = false;
        settings.IsPasswordAutosaveEnabled = false;
        settings.IsGeneralAutofillEnabled = false;
        settings.IsSwipeNavigationEnabled = false;
        // Only our own embedded page is ever shown, so SmartScreen lookups are wasted work.
        settings.IsReputationCheckingRequired = false;

        // Only offer cut/copy/paste, and only inside text fields.
        core.ContextMenuRequested += (_, e) =>
        {
            if (!e.ContextMenuTarget.IsEditable)
            {
                e.Handled = true;
                return;
            }
            string[] keep = ["cut", "copy", "paste", "selectAll"];
            foreach (var item in e.MenuItems.ToList())
            {
                if (!keep.Contains(item.Name))
                    e.MenuItems.Remove(item);
            }
        };
        core.AddWebResourceRequestedFilter(Origin + "*", CoreWebView2WebResourceContext.All);
        core.WebResourceRequested += (_, e) => ServeAsset(env, e);
        core.NavigationStarting += (_, e) =>
        {
            if (!e.Uri.StartsWith(Origin, StringComparison.OrdinalIgnoreCase))
                e.Cancel = true;
        };
        core.NavigationCompleted += (_, e) =>
        {
            if (e.IsSuccess)
                TryMarkStarting(false);
        };
        core.NewWindowRequested += (_, e) => e.Handled = true;
        core.ProcessFailed += (_, e) => OnProcessFailed(core, e.ProcessFailedKind);
        // Handle commands outside the WebView2 event so a command may open a modal window.
        core.WebMessageReceived += (_, e) =>
        {
            string json = e.WebMessageAsJson;
            BeginInvoke(new Action(async () => await _app.HandleMessageAsync(json)));
        };
    }

    void OnProcessFailed(CoreWebView2 core, CoreWebView2ProcessFailedKind kind)
    {
        switch (kind)
        {
            // The page's renderer died: load the page again; it re-reads the state on load.
            case CoreWebView2ProcessFailedKind.RenderProcessExited or CoreWebView2ProcessFailedKind.RenderProcessUnresponsive:
                BeginInvoke(() =>
                {
                    if (_web?.CoreWebView2 == core)
                        core.Navigate(Origin + "index.html");
                });
                break;
            // Chromium restarts its GPU process by itself; a second crash points at the graphics driver.
            case CoreWebView2ProcessFailedKind.GpuProcessExited:
                if (++_gpuCrashes >= 2 && !Prefs.SafeRendering)
                    BeginInvoke(new Action(async () => await RestartWebViewAsync(safe: true, automatic: true)));
                break;
            case CoreWebView2ProcessFailedKind.BrowserProcessExited:
                BeginInvoke(new Action(async () => await RestartWebViewAsync(Prefs.SafeRendering, automatic: true)));
                break;
        }
    }

    /// <summary>Recreates the browser (optionally switching GPU/CPU drawing). The vault stays unlocked meanwhile.</summary>
    async Task RestartWebViewAsync(bool safe, bool automatic)
    {
        if (_restarting || IsDisposed || (automatic && _automaticRestarts >= MaxAutomaticRestarts))
            return;
        _restarting = true;
        if (automatic)
            _automaticRestarts++;
        try
        {
            if (safe != Prefs.SafeRendering)
            {
                Prefs.SafeRendering = safe;
                Prefs.Save();
            }
            if (_web is { } old)
            {
                _web = null;
                Controls.Remove(old);
                old.Dispose();
            }
            _app.ResumingPage = true;
            await AttachWebViewAsync(await CreateEnvironmentAsync());
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or WebView2RuntimeNotFoundException)
        {
            Log.Write(ex);
            Ui.Error(this, L.F("Couldn't restart the interface: {0}", ex.Message));
        }
        finally
        {
            _restarting = false;
        }
    }

    static void TryMarkStarting(bool starting)
    {
        try
        {
            if (starting)
            {
                Directory.CreateDirectory(Vault.DataDir);
                File.WriteAllText(StartMarker, "");
            }
            else
            {
                File.Delete(StartMarker);
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
    }

    static void ServeAsset(CoreWebView2Environment env, CoreWebView2WebResourceRequestedEventArgs e)
    {
        string path = new Uri(e.Request.Uri).AbsolutePath.TrimStart('/');
        var stream = typeof(ShellForm).Assembly.GetManifestResourceStream("wwwroot/" + path);
        if (stream is null)
        {
            e.Response = env.CreateWebResourceResponse(null, 404, "Not Found", "");
            return;
        }
        string type = Path.GetExtension(path) switch
        {
            ".html" => "text/html; charset=utf-8",
            ".css" => "text/css; charset=utf-8",
            ".js" => "text/javascript; charset=utf-8",
            ".svg" => "image/svg+xml",
            ".png" => "image/png",
            _ => "application/octet-stream",
        };
        e.Response = env.CreateWebResourceResponse(stream, 200, "OK", $"Content-Type: {type}\r\nCache-Control: no-store");
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        _releaseTimer.Dispose();
        _app.Dispose();
        base.OnFormClosed(e);
    }
}
