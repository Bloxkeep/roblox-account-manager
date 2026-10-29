using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>
/// The app window: hosts the HTML interface embedded in the exe. The page may only load its own files and
/// Roblox CDN images (see the CSP in index.html); every network call goes through <see cref="AppController"/>.
/// If the GPU driver keeps crashing, or a start never got the page on screen, it reloads drawing on the CPU.
/// With "keep running when closed" on, closing hides it to the tray so auto-rejoin and Multi-Roblox keep working.
/// </summary>
public sealed class ShellForm : Form
{
    internal const string Origin = "https://bloxkeep.local/";
    const int MaxAutomaticRestarts = 3;
    /// <summary>Failure counts reset after this long: crashes days apart (a GPU reset after sleep, an antivirus) aren't a broken driver.</summary>
    static readonly TimeSpan BrowserHealthyAfter = TimeSpan.FromMinutes(30);
    long _lastBrowserFailure = long.MinValue / 2;

    readonly AppController _app;
    // In the background the browser sleeps and stays, so the window comes back at once. When Windows is short of memory
    // (Roblox windows opening meanwhile), it is closed instead, looked at every half minute, and made again on return.
    const int TightReleaseAfterMs = 30_000, BusyRecheckMs = 60_000;
    readonly System.Windows.Forms.Timer _releaseTimer = new() { Interval = TightReleaseAfterMs };
    /// <summary>When the window last went to the background (minimised or hidden), in Environment.TickCount64.</summary>
    long _backgroundSince;
    bool _wasBackground;
    readonly System.Windows.Forms.Timer _coverTimer = new() { Interval = 4000 };
    /// <summary>Hides the tray icon again once a notification shown with close-to-tray off is surely gone.</summary>
    readonly System.Windows.Forms.Timer _balloonTimer = new();
    const int BalloonMs = 8000;
    readonly NotifyIcon _tray;
    readonly TrayWindow _trayWindow = new();
    /// <summary>Until when the newest notification balloon may be on screen; the icon isn't hidden before that.</summary>
    DateTime _balloonUntil;
    WebView2? _web;
    /// <summary>The small always-on-top window, made the first time it is opened and kept (hidden) from then on.</summary>
    MiniForm? _mini;
    readonly MonitorTab _monitor;
    string _themeId;
    int _gpuCrashes, _automaticRestarts;
    bool _restarting, _released, _quitting, _hiding, _minimizeReady, _minimizing;
    /// <summary>The page got on screen at least once this run (so the start marker may go).</summary>
    bool _pageLoaded;
    /// <summary>The page as it looked when the window went away (JPEG), shown while a released browser starts again.</summary>
    byte[]? _snapshot;
    bool _snapshotUnlocked;
    string? _snapshotTheme;
    PictureBox? _cover;
    bool _coverSkipLogged;

    const int WM_SYSCOMMAND = 0x0112, SC_MINIMIZE = 0xF020, SW_RESTORE = 9;

    [DllImport("user32.dll")]
    static extern bool PostMessage(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    static extern bool ShowWindow(IntPtr hwnd, int cmd);

    [DllImport("kernel32.dll")]
    static extern IntPtr GetCurrentProcess();

    [DllImport("kernel32.dll")]
    static extern bool SetProcessWorkingSetSize(IntPtr process, IntPtr minimum, IntPtr maximum);

    [DllImport("user32.dll")]
    static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    static extern IntPtr GetAncestor(IntPtr hwnd, uint flags);

    static string StartMarker => Path.Combine(Vault.DataDir, "starting.flag");

    /// <param name="resume">What the previous run handed over when it quit for an update in the background (see <see cref="Relaunch"/>).</param>
    public ShellForm(Relaunch.State? resume = null)
    {
        // Created first: setting Size below already raises OnResize.
        _app = new AppController(this, PostToPage)
        {
            SetSafeRendering = safe => RestartWebViewAsync(safe, automatic: false),
            AppearanceChanged = ApplyAppearance,
            HotkeysChanged = RegisterHotkeys,
            ShowNotification = ShowNotification,
            PageShown = RemoveCover,
            SetMiniOpen = open => SetMiniOpen(open),
            SetMonitorArea = area => _monitor?.SetArea(area),
            QuitForUpdate = Quit,
            // After the command's answer has gone back to the page: closing to the tray takes a picture of it first.
            WindowControl = action => BeginInvoke(() =>
            {
                if (IsDisposed)
                    return;
                if (action == "close")
                    Close();
                else if (action == "minimize")
                    WindowState = FormWindowState.Minimized;
                else
                    WindowState = WindowState == FormWindowState.Maximized ? FormWindowState.Normal : FormWindowState.Maximized;
            }),
            WindowNow = () => (!Visible ? "hidden" : WindowState == FormWindowState.Minimized ? "minimized" : "shown",
                Visible && WindowState != FormWindowState.Minimized && InFront, _mini is { Visible: true }),
        };
        if (resume is not null)
        {
            _app.Resume(resume);
            _resumeWindow = resume.Window;
            _startHidden = resume.Window == "hidden";
            _showQuietly = !resume.Foreground;
            _frontAtStart = GetForegroundWindow();
        }
        _monitor = new MonitorTab(this, _app, () => _web);
        _themeId = Themes.Current.Id;
        Text = Ui.Title;
        BackColor = Themes.Current.Background;
        StartPosition = FormStartPosition.CenterScreen;
        // Fit the screen the window opens on (the one with the mouse, like CenterScreen): on a 1366×768 or 1080p
        // laptop at 125–150 % the preferred size would put the launch dock under the taskbar.
        var area = Screen.FromPoint(Cursor.Position).WorkingArea.Size;
        var preferred = new Size(LogicalToDeviceUnits(1320), LogicalToDeviceUnits(840));
        MinimumSize = Fit(new Size(LogicalToDeviceUnits(980), LogicalToDeviceUnits(620)), area);
        Size = Fit(preferred, area);
        if (preferred.Width > area.Width || preferred.Height > area.Height)
            WindowState = FormWindowState.Maximized;
        // The saved place, size and maximised state win (put in OnLoad); the fit above is for a first start or a lost screen.
        if (SavedPlace(out _))
        {
            StartPosition = FormStartPosition.Manual;
            WindowState = FormWindowState.Normal;
        }
        Icon = Ui.AppIcon;
        Load += async (_, _) => await InitAsync();
        _releaseTimer.Tick += (_, _) => ReleaseWebView();
        _coverTimer.Tick += (_, _) => RemoveCover(); // the page never said it was drawn
        _balloonTimer.Tick += (_, _) =>
        {
            _balloonTimer.Stop();
            HideTrayIfIdle();
        };
        _tray = CreateTrayIcon();
        _tray.Visible = Prefs.CloseToTray;
        SystemEvents.UserPreferenceChanged += OnUserPreferenceChanged;
        // The mini window comes back where it was, without taking the keyboard from the lock screen.
        Shown += (_, _) =>
        {
            if (Prefs.MiniOpen)
                SetMiniOpen(true, activate: false);
            if (_resumeWindow is not null)
                TellIfLockedAfterUpdate();
            // Back after an update in the background while another program was in front: that one keeps the keyboard
            // (a minimised or maximised window is activated as it shows, so the keyboard goes back) and stays on top.
            if (_showQuietly)
            {
                _showQuietly = false;
                IntPtr front = _frontAtStart;
                if (front != IntPtr.Zero && front != Handle && IsWindow(front))
                {
                    if (GetForegroundWindow() == Handle)
                        SetForegroundWindow(front);
                    SetWindowPos(Handle, front, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
            }
        };
    }

    /// <summary>The program in front when this run started after an update in the background.</summary>
    readonly IntPtr _frontAtStart;

    /// <summary>How the window comes back after an update in the background: hidden, minimized or shown (null on a normal start).</summary>
    readonly string? _resumeWindow;
    /// <summary>Stays in the tray at start (it was there when the update in the background began).</summary>
    bool _startHidden;
    bool _showQuietly;

    protected override bool ShowWithoutActivation => _showQuietly;

    /// <summary>Back after an update but locked: said once, since the window may be in the tray and a scheduled preset waits for the unlock.</summary>
    void TellIfLockedAfterUpdate()
    {
        if (_toldLockedAfterUpdate || _app.LockedAfterUpdate() is not { } text)
            return;
        _toldLockedAfterUpdate = true;
        BeginInvoke(() => ShowNotification(text, "warn"));
    }
    bool _toldLockedAfterUpdate;

    protected override void SetVisibleCore(bool value)
    {
        // Updated while in the tray: back in the tray; the window and its browser are made when first opened.
        if (value && _startHidden)
        {
            _startHidden = false;
            // Quiet only for the restart itself: opened from the tray later, the window comes to the front as usual.
            _showQuietly = false;
            if (!IsHandleCreated)
                CreateHandle();
            _tray.Visible = true;
            _app.Background = true;
            if (Prefs.MiniOpen)
                BeginInvoke(() => SetMiniOpen(true, activate: false));
            TellIfLockedAfterUpdate();
            value = false;
        }
        // Into the tray: the browser hears it is hidden while the window still shows. Told after (OnVisibleChanged), it
        // never was (a hidden window's control doesn't pass it on): its page kept drawing as if on screen, and the
        // picture over the window on its way back waited 4 seconds for a "drawn again" that never came.
        if (!value && _web is { Visible: true } web)
            web.Visible = false;
        base.SetVisibleCore(value);
    }

    const uint SWP_NOSIZE = 0x1, SWP_NOMOVE = 0x2, SWP_NOACTIVATE = 0x10;

    [DllImport("user32.dll")]
    static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int cx, int cy, uint flags);

    [DllImport("user32.dll")]
    static extern bool SetForegroundWindow(IntPtr hwnd);

    [DllImport("user32.dll")]
    static extern bool IsWindow(IntPtr hwnd);

    /// <summary>Shows or hides the mini window (null toggles); the page is told, since its button shows the state.</summary>
    bool SetMiniOpen(bool? open, bool activate = true)
    {
        bool show = open ?? _mini is not { Visible: true };
        if (show)
        {
            if (_mini is null)
            {
                _mini = new MiniForm(_app, BringToFront);
                _mini.Dismissed += _app.PushState;
            }
            _mini.Open(activate);
            _app.PushState();
        }
        else
            _mini?.Dismiss(); // which pushes the state itself
        return show;
    }

    static Size Fit(Size size, Size area) => new(Math.Min(size.Width, area.Width), Math.Min(size.Height, area.Height));

    NotifyIcon CreateTrayIcon()
    {
        // The quick panel (a left click) and the menu (a right click), drawn by the tray's page (TrayWindow).
        var tw = _trayWindow;
        tw.MiniOpen = () => _mini is { Visible: true };
        tw.State = _app.TrayState;
        tw.PictureOf = _app.TrayPictureUrl;
        tw.CustomTheme = _app.TrayCustomTheme;
        tw.OpenClicked += BringToFront;
        tw.MiniClicked += () => SetMiniOpen(null);
        tw.QuitClicked += Quit;
        tw.MultiClicked += on => _app.TrayMulti(!on); // only offered where it can switch from here
        tw.AccountClicked += _app.TrayOpenAccount;
        tw.PresetClicked += _app.TrayRunPreset;
        tw.NewRobloxClicked += _app.TrayNewRoblox;
        tw.CloseAllClicked += () => _app.CloseAllClients();
        tw.ShowClientClicked += _app.TrayShowClient;
        tw.CloseClientClicked += _app.CloseClient;
        var tray = new NotifyIcon { Icon = Ui.AppIcon, Text = Ui.Title };
        tray.MouseClick += (_, e) =>
        {
            if (e.Button is MouseButtons.Left or MouseButtons.Right)
                tw.Toggle(Cursor.Position, e.Button == MouseButtons.Left ? "panel" : "menu");
        };
        // A notification shown while the icon is normally hidden (close-to-tray off) takes the icon with it — but
        // not while a newer balloon is still showing (Windows closes the one it replaced right after).
        tray.BalloonTipClicked += (_, _) =>
        {
            BringToFront();
            _balloonUntil = default;
            HideTrayIfIdle();
        };
        tray.BalloonTipClosed += (_, _) => HideTrayIfIdle();
        return tray;
    }

    /// <summary>A Windows notification (tray balloon, shown as a toast) while the window is hidden, minimised or behind
    /// another program; false when it is in front, so the page's own toast is enough.</summary>
    bool ShowNotification(string text, string kind)
    {
        if (IsDisposed || _quitting || (!InBackground && InFront))
            return false;
        _balloonUntil = DateTime.UtcNow.AddMilliseconds(BalloonMs);
        _tray.Visible = true;
        _tray.ShowBalloonTip(BalloonMs, Ui.Title, text, kind switch { "error" => ToolTipIcon.Error, "warn" => ToolTipIcon.Warning, _ => ToolTipIcon.None });
        // The closed event may come early (dismissed) or not at all: the timer hides the icon in the end.
        _balloonTimer.Stop();
        _balloonTimer.Interval = BalloonMs + 1000;
        _balloonTimer.Start();
        return true;
    }

    bool BalloonShowing => DateTime.UtcNow < _balloonUntil;

    /// <summary>Back to the icon's normal state, unless a notification is still on screen.</summary>
    void HideTrayIfIdle()
    {
        if (!IsDisposed && !BalloonShowing)
            _tray.Visible = Prefs.CloseToTray;
    }

    /// <summary>Closes for real, even with "keep running when closed" on.</summary>
    public void Quit()
    {
        _quitting = true;
        Close();
    }

    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        SavePlace();
        // Windows shutting down and Task Manager still close the app; the uninstaller and the tray menu use Quit.
        if (!_quitting && Prefs.CloseToTray && e.CloseReason == CloseReason.UserClosing)
        {
            e.Cancel = true;
            _ = HideToTrayAsync();
            return;
        }
        // Changes that can't be written yet (file held by an antivirus) would be lost: ask, unless Windows is shutting down.
        if (!_app.SaveBeforeClosing() && e.CloseReason != CloseReason.WindowsShutDown
            && !Ui.Modal(() => Dialog.Confirm(this, L.T("Couldn't save your changes. Quit anyway and lose them?"),
                L.T("Quit anyway"), L.T("Keep Bloxkeep open"), danger: true)))
        {
            e.Cancel = true;
            _quitting = false;
            return;
        }
        base.OnFormClosing(e);
    }

    async Task HideToTrayAsync()
    {
        if (_hiding)
            return;
        _hiding = true;
        int brought = _broughtToFront;
        try
        {
            await TakeSnapshotAsync(); // while the page is still on screen
        }
        finally
        {
            _hiding = false;
        }
        // Opened from the tray while the picture was taken: the person changed their mind.
        if (_quitting || IsDisposed || _broughtToFront != brought)
            return;
        _tray.Visible = true;
        Hide();
        if (Prefs.TrayHintShown)
            return;
        Prefs.TrayHintShown = true;
        Prefs.Save();
        _tray.ShowBalloonTip(5000, Ui.Title, L.T("Bloxkeep is still running. Right-click this icon to quit."), ToolTipIcon.None);
    }

    /// <summary>A new theme or close-to-tray choice from Settings, or Windows switching between light and dark.</summary>
    void ApplyAppearance()
    {
        var theme = Themes.Current;
        _themeId = theme.Id;
        BackColor = theme.Background;
        if (IsHandleCreated)
            Ui.UseThemedTitleBar(this, theme);
        if (_web is { IsDisposed: false } web)
            web.DefaultBackgroundColor = theme.Background;
        _mini?.ApplyTheme();
        _monitor.ApplyTheme();
        _tray.Visible = Prefs.CloseToTray || BalloonShowing;
    }

    void OnUserPreferenceChanged(object? sender, UserPreferenceChangedEventArgs e)
    {
        if (e.Category != UserPreferenceCategory.General || Prefs.Theme != Themes.FollowWindows || IsDisposed || !IsHandleCreated)
            return;
        BeginInvoke(() =>
        {
            if (Themes.Current.Id == _themeId)
                return;
            ApplyAppearance();
            _app.PushState();
        });
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

    /// <summary>How often the window was asked to the front, so a hide under way can tell it was overtaken.</summary>
    int _broughtToFront;

    public new void BringToFront()
    {
        _broughtToFront++;
        if (!Visible)
            Show();
        // Back to the size it had, maximised or not (setting WindowState = Normal would un-maximise it).
        if (WindowState == FormWindowState.Minimized && IsHandleCreated)
            ShowWindow(Handle, SW_RESTORE);
        Activate();
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Ui.UseThemedTitleBar(this, Themes.Current);
        RegisterHotkeys();
    }

    const int WM_HOTKEY = 0x0312, MOD_ALT = 0x1, MOD_CONTROL = 0x2, MOD_NOREPEAT = 0x4000;
    [System.Runtime.InteropServices.DllImport("user32.dll")] static extern bool RegisterHotKey(IntPtr hWnd, int id, int modifiers, int key);
    [System.Runtime.InteropServices.DllImport("user32.dll")] static extern bool UnregisterHotKey(IntPtr hWnd, int id);

    /// <summary>Ctrl+Alt+H and Ctrl+Alt+A from any app while the switch is on; one another app holds is reported in Settings.</summary>
    void RegisterHotkeys()
    {
        if (!IsHandleCreated)
            return;
        UnregisterHotKey(Handle, AppController.HotkeyHide);
        UnregisterHotKey(Handle, AppController.HotkeyArrange);
        var taken = new List<string>();
        if (Prefs.GlobalHotkeys)
        {
            if (!RegisterHotKey(Handle, AppController.HotkeyHide, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, (int)Keys.H))
                taken.Add("Ctrl+Alt+H");
            if (!RegisterHotKey(Handle, AppController.HotkeyArrange, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, (int)Keys.A))
                taken.Add("Ctrl+Alt+A");
        }
        _app.HotkeysTaken = taken;
    }

    /// <summary>Minimising waits for a picture of the page first (a few dozen ms): a minimised window's page isn't
    /// drawn, so it can't be captured afterwards. The minimise is then sent again.</summary>
    protected override void WndProc(ref Message m)
    {
        if (m.Msg == WM_HOTKEY)
        {
            _app.OnHotkey((int)m.WParam);
            return;
        }
        if (m.Msg == WM_SYSCOMMAND && ((int)m.WParam & 0xFFF0) == SC_MINIMIZE)
        {
            // Consumed whichever way the minimise goes through, so a leftover never skips the next picture.
            bool ready = _minimizeReady;
            _minimizeReady = false;
            if (!ready && _web is { Visible: true } && !_released)
            {
                if (!_minimizing) // a second click while the picture is taken waits for the same one
                    _ = MinimizeAfterSnapshotAsync();
                return;
            }
        }
        base.WndProc(ref m);
    }

    async Task MinimizeAfterSnapshotAsync()
    {
        _minimizing = true;
        try
        {
            await TakeSnapshotAsync();
        }
        finally
        {
            _minimizing = false;
        }
        if (IsDisposed || !IsHandleCreated || WindowState == FormWindowState.Minimized)
            return;
        _minimizeReady = true;
        PostMessage(Handle, WM_SYSCOMMAND, SC_MINIMIZE, IntPtr.Zero);
    }

    async Task TakeSnapshotAsync()
    {
        if (_web?.CoreWebView2 is not { } core || !_web.Visible || WindowState == FormWindowState.Minimized)
            return;
        try
        {
            var stream = new MemoryStream();
            var capture = core.CapturePreviewAsync(CoreWebView2CapturePreviewImageFormat.Jpeg, stream);
            // Never hold the window up for long if the browser is busy.
            if (await Task.WhenAny(capture, Task.Delay(500)) != capture)
            {
                // The browser may still be writing to the stream: it goes when the capture ends (a fault logged there).
                _ = capture.ContinueWith(t =>
                {
                    if (t.IsFaulted)
                        Log.Write(t.Exception!);
                    stream.Dispose();
                }, TaskScheduler.Default);
                return;
            }
            await capture;
            _snapshot = stream.Length > 0 ? stream.ToArray() : null;
            _snapshotUnlocked = _app.Unlocked;
            _snapshotTheme = Themes.Current.Id;
            stream.Dispose();
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ObjectDisposedException)
        {
            _snapshot = null;
        }
    }

    /// <summary>Puts the last picture of the page over the window until the new page has drawn itself.</summary>
    void ShowCover()
    {
        var bytes = _snapshot;
        _snapshot = null;
        // Not a picture from before a lock, in a theme changed since (Windows light/dark), or for another size or scale.
        if (bytes is null || _snapshotUnlocked != _app.Unlocked || _snapshotTheme != Themes.Current.Id)
            return;
        try
        {
            var image = Image.FromStream(new MemoryStream(bytes));
            // The browser may hand the picture back at another scale than the window's pixels (screen scaling, two
            // monitors with different scaling): the same shape is stretched to fit, anything else is left out.
            bool same = image.Size == ClientSize;
            bool sameShape = ClientSize.Height > 0 && image.Height > 0
                && Math.Abs((double)image.Width / image.Height - (double)ClientSize.Width / ClientSize.Height) < 0.02;
            if (!same && !sameShape)
            {
                if (!_coverSkipLogged)
                    Log.Note($"Window picture skipped: {image.Width}x{image.Height} for a {ClientSize.Width}x{ClientSize.Height} window.");
                _coverSkipLogged = true; // once per run is enough to see why
                image.Dispose();
                return;
            }
            PutCover(image, same ? PictureBoxSizeMode.Normal : PictureBoxSizeMode.StretchImage);
        }
        catch (ArgumentException)
        {
        }
    }

    /// <summary>Covers the window with <paramref name="image"/> until the page says it is drawn (or <paramref name="fallbackMs"/> pass).</summary>
    void PutCover(Image image, PictureBoxSizeMode mode, int fallbackMs = 4000)
    {
        RemoveCover();
        _cover = new PictureBox { Dock = DockStyle.Fill, Image = image, SizeMode = mode, BackColor = Themes.Current.Background };
        Controls.Add(_cover);
        _cover.BringToFront();
        _coverTimer.Interval = fallbackMs;
        _coverTimer.Start();
    }

    /// <summary>At start, the logo in the window until the page's loading screen is drawn. The browser can be slow to
    /// start on a slow PC or after an update, so the fallback is long.</summary>
    void ShowStartCover()
    {
        if (Ui.AppIcon is not { } icon)
            return;
        int size = LogicalToDeviceUnits(72);
        using var big = new Icon(icon, new Size(size, size));
        PutCover(big.ToBitmap(), PictureBoxSizeMode.CenterImage, fallbackMs: 15_000);
    }

    void RemoveCover()
    {
        _coverTimer.Stop();
        if (_cover is not { } cover)
            return;
        _cover = null;
        Controls.Remove(cover);
        cover.Image?.Dispose();
        cover.Dispose();
    }

    bool InBackground => !Visible || WindowState == FormWindowState.Minimized;

    /// <summary>This window or a dialog it owns is active. Asked of Windows: Form.ActiveForm can be null while the
    /// keyboard focus sits in the browser's own child window.</summary>
    bool InFront
    {
        get
        {
            const uint GA_ROOTOWNER = 3;
            if (Form.ActiveForm is not null)
                return true;
            IntPtr front = GetForegroundWindow();
            return front != IntPtr.Zero && IsHandleCreated && GetAncestor(front, GA_ROOTOWNER) == Handle;
        }
    }

    protected override void OnResize(EventArgs e)
    {
        base.OnResize(e);
        UpdateBackground();
        // Maximised or restored with the title bar's button (no ResizeEnd for that): remembered for the next start.
        if (WindowState != _lastState)
        {
            _lastState = WindowState;
            if (WindowState != FormWindowState.Minimized)
                SavePlace();
        }
    }

    protected override void OnResizeEnd(EventArgs e)
    {
        base.OnResizeEnd(e);
        SavePlace();
    }

    protected override void OnLoad(EventArgs e)
    {
        base.OnLoad(e);
        RestorePlace();
        // An update in the background found the window minimised: it comes back that way.
        if (_resumeWindow == "minimized")
            WindowState = FormWindowState.Minimized;
    }

    void RestorePlace()
    {
        // Only now, with the window real: moved to a screen with another scale, it then gets a proper dpi change.
        if (!SavedPlace(out var saved))
            return;
        var area = Screen.FromRectangle(saved).WorkingArea;
        Location = saved.Location;
        Size = new Size(Math.Clamp(saved.Width, MinimumSize.Width, Math.Max(MinimumSize.Width, area.Width)),
            Math.Clamp(saved.Height, MinimumSize.Height, Math.Max(MinimumSize.Height, area.Height)));
        if (Prefs.MainMaximized)
            WindowState = FormWindowState.Maximized;
        _lastState = WindowState;
    }

    FormWindowState _lastState = FormWindowState.Normal;

    /// <summary>The saved place, unless no screen shows the middle of its title bar any more (a monitor unplugged).</summary>
    static bool SavedPlace(out Rectangle r)
    {
        r = default;
        var parts = Prefs.MainWindow?.Split(',');
        if (parts is not { Length: 4 } || !int.TryParse(parts[0], out int x) || !int.TryParse(parts[1], out int y)
            || !int.TryParse(parts[2], out int w) || !int.TryParse(parts[3], out int h) || w <= 0 || h <= 0)
            return false;
        r = new Rectangle(x, y, w, h);
        var grip = new Rectangle(x + w / 4, y + 2, w / 2, 24);
        return Screen.AllScreens.Any(s => s.WorkingArea.IntersectsWith(grip));
    }

    /// <summary>Remembers the size it has when not maximised (also while maximised or minimised) and whether it is maximised.</summary>
    void SavePlace()
    {
        if (!IsHandleCreated || !Visible)
            return;
        var normal = WindowState == FormWindowState.Normal ? Bounds : RestoreBounds;
        if (normal.Width <= 0 || normal.Height <= 0)
            return;
        string place = $"{normal.X},{normal.Y},{normal.Width},{normal.Height}";
        // Minimised says nothing about maximised: the last known state is kept.
        bool maximized = WindowState == FormWindowState.Minimized ? Prefs.MainMaximized : WindowState == FormWindowState.Maximized;
        if (place == Prefs.MainWindow && maximized == Prefs.MainMaximized)
            return;
        Prefs.MainWindow = place;
        Prefs.MainMaximized = maximized;
        Prefs.Save();
    }

    protected override void OnVisibleChanged(EventArgs e)
    {
        base.OnVisibleChanged(e);
        UpdateBackground();
    }

    /// <summary>In the background, hide the page, put it to sleep (no timers or drawing, memory back to Windows) and poll
    /// less; it wakes in a few dozen ms, where recreating took about 2.5 s (Tests/perf/TrayBench). Only when Windows is
    /// short of memory is the browser closed entirely.</summary>
    void UpdateBackground()
    {
        if (!IsHandleCreated)
            return;
        bool background = InBackground;
        if (background && !_wasBackground)
            _backgroundSince = Environment.TickCount64;
        _wasBackground = background;
        _app.Background = background;
        if (background)
        {
            _releaseTimer.Interval = TightReleaseAfterMs;
            _releaseTimer.Start();
        }
        else
        {
            _releaseTimer.Stop();
            if (_released && !_restarting)
            {
                ShowCover();
                _ = RecreateWebViewAsync();
            }
            else
            {
                // The browser may need a moment to draw the hidden page again: the picture covers that too. What it kept
                // of its view is stale from here on (a page recreated after a crash must not put it back).
                if (_snapshot is not null && _web is not null)
                    ShowCover();
                _snapshot = null;
                _app.DropKeptView();
            }
        }
        ApplyBackgroundToBrowser();
        _monitor?.Place(); // gone at once while minimised or hidden; back with the page's next report
    }

    /// <summary>In the background the page is hidden and put to sleep; also for a browser made while minimised. Shown, it is awake (showing wakes it).</summary>
    void ApplyBackgroundToBrowser()
    {
        if (_web?.CoreWebView2 is not { } core)
            return;
        bool background = InBackground;
        _web.Visible = !background;
        if (background)
        {
            _ = SleepAsync(core);
            return;
        }
        if (core.MemoryUsageTargetLevel != CoreWebView2MemoryUsageTargetLevel.Normal)
            core.MemoryUsageTargetLevel = CoreWebView2MemoryUsageTargetLevel.Normal;
        _app.PageAsleep = false; // what it missed, and a fresh state
    }

    /// <summary>TrySuspend on the hidden page; nothing is sent to it meanwhile (AppController.PageAsleep), so nothing wakes
    /// it early. A page that won't sleep keeps getting states, with its memory level low instead.</summary>
    async Task SleepAsync(CoreWebView2 core)
    {
        // The browser hears it is hidden on a later turn of the message loop; asked to sleep before that, it refuses.
        await Task.Delay(300);
        if (!InBackground || _web?.CoreWebView2 != core || core.IsSuspended)
            return;
        _app.PageAsleep = true;
        bool asleep = false;
        try
        {
            asleep = await core.TrySuspendAsync();
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ObjectDisposedException)
        {
            // Shown again meanwhile, or the browser went: nothing to put to sleep.
        }
        if (asleep || !InBackground || _web?.CoreWebView2 != core)
            return;
        _app.PageAsleep = false;
        core.MemoryUsageTargetLevel = CoreWebView2MemoryUsageTargetLevel.Low;
    }

    void ReleaseWebView()
    {
        _releaseTimer.Stop();
        if (_web is not { } web || !InBackground || _restarting)
            return;
        long hidden = Environment.TickCount64 - _backgroundSince;
        var memory = SystemMemory.Read();
        if (ReleaseWait(hidden, memory) is > 0 and int wait)
        {
            _releaseTimer.Interval = wait;
            _releaseTimer.Start();
            return;
        }
        // Something half done on the page (a form, a sign-in) would be lost with the browser: look again in a minute.
        if (_app.PageBusy)
        {
            _releaseTimer.Interval = BusyRecheckMs;
            _releaseTimer.Start();
            return;
        }
        Log.Note($"Interface closed in the background, Windows short of memory: {memory.CommitAvailableMb} MB of virtual memory (and {memory.PageFileRoomMb} MB for the page files to grow) and {memory.AvailableMb} MB of RAM left.");
        _web = null;
        _released = true;
        _app.PageGone();
        Controls.Remove(web);
        web.Dispose();
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        SetProcessWorkingSetSize(GetCurrentProcess(), -1, -1);
    }

    /// <summary>When to look again, 0 to close the browser now (Windows short of memory). Asleep it holds about 180 MB of
    /// virtual memory but next to no RAM.</summary>
    static int ReleaseWait(long hiddenMs, MemoryStatus memory) => AppController.MemoryTight(memory) ? 0 : TightReleaseAfterMs;

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
            BrowserFailed(ex);
        }
        finally
        {
            _restarting = false;
        }
    }

    /// <summary>Drops a browser that didn't start and marks the window released, so coming to the front tries again.</summary>
    void BrowserFailed(Exception ex)
    {
        DropWebView();
        _app.PageGone();
        _released = true;
        RemoveCover();
        Ui.Error(this, L.F("Couldn't restart the interface: {0}", ex.Message));
    }

    async Task InitAsync()
    {
        // The last start never got the page on screen (for example a broken graphics driver): don't use the GPU.
        if (File.Exists(StartMarker) && !Prefs.SafeRendering)
        {
            Prefs.SafeRendering = true;
            Prefs.Save();
        }
        ShowStartCover(); // before the slowest step, making the browser environment

        bool offeredInstall = false;
        // The browser profile may still be held by the copy this one replaced (an update's restart) for a moment.
        for (int attempt = 1; ; attempt++)
        {
            try
            {
                CoreWebView2Environment env;
                try
                {
                    env = await CreateEnvironmentAsync();
                }
                // Missing runtime: offer Microsoft's installer once, then try again.
                catch (WebView2RuntimeNotFoundException) when (!offeredInstall)
                {
                    offeredInstall = true;
                    if (!await WebView2Setup.TryInstallAsync(this))
                    {
                        Quit();
                        return;
                    }
                    env = await CreateEnvironmentAsync();
                }
                await AttachWebViewAsync(env, firstStart: true);
                return;
            }
            catch (WebView2RuntimeNotFoundException)
            {
                Ui.Error(this, L.T("Microsoft Edge WebView2 isn't installed. Install it from Microsoft's website, then reopen Bloxkeep."));
                Quit();
                return;
            }
            catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or UnauthorizedAccessException)
            {
                DropWebView();
                if (attempt < 3 && !IsDisposed)
                {
                    await Task.Delay(1000);
                    continue;
                }
                Log.Write(ex);
                Ui.Error(this, L.F("Couldn't open the interface: {0}", ex.Message));
                Quit();
                return;
            }
        }
    }

    internal static Task<CoreWebView2Environment> CreateEnvironmentAsync()
    {
        // Separate profile folders, because a running browser can't be reopened with different switches.
        return CoreWebView2Environment.CreateAsync(null, Vault.CacheFolder(Prefs.SafeRendering ? "webview-safe" : "webview"),
            WebView2Setup.MainWindowOptions());
    }

    /// <param name="firstStart">The app's own start, which the marker file watches. Not for a browser recreated later: a
    /// quit meanwhile would leave the marker and wrongly switch to CPU drawing.</param>
    async Task AttachWebViewAsync(CoreWebView2Environment env, bool firstStart = false)
    {
        var web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = Themes.Current.Background };
        Controls.Add(web);
        _cover?.BringToFront();
        _web = web;
        await web.EnsureCoreWebView2Async(env);
        // Only once a browser exists: a start failing before (profile held by the copy an update replaced) says nothing about the GPU.
        if (firstStart)
            TryMarkStarting(true);
        Configure(web.CoreWebView2, env);
        web.CoreWebView2.Navigate(Origin + "index.html");
        ApplyBackgroundToBrowser(); // made while minimised: hidden and trimmed from the start
        // The previous browser took the keyboard focus with it; typing (the password on the lock screen) must reach
        // the new one without a click first. Only while this window is the active one, so no other program loses it.
        if (Form.ActiveForm == this && !web.IsDisposed)
            web.Focus();
    }

    /// <summary>Removes a browser that failed to start, so the next try doesn't add a second one.</summary>
    void DropWebView()
    {
        if (_web is not { } web)
            return;
        _web = null;
        Controls.Remove(web);
        web.Dispose();
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
            else if (_web?.CoreWebView2 == core)
                _monitor.SetArea(null); // a reload (the renderer crashed): the new page starts on the lock or loading screen
        };
        core.NavigationCompleted += (_, e) =>
        {
            if (!e.IsSuccess)
                return;
            _pageLoaded = true;
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
        long now = Environment.TickCount64;
        if (now - _lastBrowserFailure > (long)BrowserHealthyAfter.TotalMilliseconds)
        {
            _gpuCrashes = 0;
            _automaticRestarts = 0;
        }
        _lastBrowserFailure = now;
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
            // Before the old page is gone: a state pushed in between would reach the new page before its getState.
            _app.PageGone();
            DropWebView();
            _app.ResumingPage = true;
            await AttachWebViewAsync(await CreateEnvironmentAsync());
        }
        catch (Exception ex) when (ex is COMException or InvalidOperationException or ArgumentException or WebView2RuntimeNotFoundException)
        {
            Log.Write(ex);
            BrowserFailed(ex);
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

    internal static void ServeAsset(CoreWebView2Environment env, CoreWebView2WebResourceRequestedEventArgs e)
    {
        string path = new Uri(e.Request.Uri).AbsolutePath.TrimStart('/');
        // The custom theme's picture: only a file of that name, read whole so no handle stays open (a new picture deletes the old one).
        string? picture = path.StartsWith("user/", StringComparison.Ordinal) && Themes.IsPictureName(path[5..]) ? Path.Combine(Vault.PicturesDir, path[5..]) : null;
        // One Vault.Tidy couldn't move yet is still at the top.
        if (picture is not null && !File.Exists(picture))
            picture = Path.Combine(Vault.DataDir, path[5..]);
        Stream? stream;
        try
        {
            stream = path.StartsWith("user/", StringComparison.Ordinal)
                ? picture is not null && File.Exists(picture) ? new MemoryStream(File.ReadAllBytes(picture)) : null
                : typeof(ShellForm).Assembly.GetManifestResourceStream("wwwroot/" + path);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            stream = null; // the picture went away meanwhile: the theme's colour shows instead
        }
        if (stream is null)
        {
            e.Response = env.CreateWebResourceResponse(null, 404, "Not Found", "");
            return;
        }
        if (path == "index.html")
            stream = WithTheme(stream);
        string type = Path.GetExtension(path) switch
        {
            ".html" => "text/html; charset=utf-8",
            ".css" => "text/css; charset=utf-8",
            ".js" => "text/javascript; charset=utf-8",
            ".svg" => "image/svg+xml",
            ".png" => "image/png",
            ".jpg" or ".jpeg" => "image/jpeg",
            ".webp" => "image/webp",
            ".gif" => "image/gif",
            ".bmp" => "image/bmp",
            ".mp4" => "video/mp4",
            ".webm" => "video/webm",
            _ => "application/octet-stream",
        };
        e.Response = env.CreateWebResourceResponse(stream, 200, "OK", $"Content-Type: {type}\r\nCache-Control: no-store");
    }

    /// <summary>The page starts in the right theme, before its first state arrives, so it never flashes dark.</summary>
    static Stream WithTheme(Stream page)
    {
        using var reader = new StreamReader(page);
        var theme = Themes.Current;
        // A light custom theme takes Light's words and lines (app.css): from the first frame, not once its colours come.
        string tone = theme is { Id: Themes.Custom, Dark: false } ? " data-tone=\"light\"" : "";
        string html = reader.ReadToEnd().Replace("<html ", $"<html data-theme=\"{theme.Id}\"{tone} ");
        return new MemoryStream(Encoding.UTF8.GetBytes(html));
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        SystemEvents.UserPreferenceChanged -= OnUserPreferenceChanged;
        // A run whose page got on screen was good, even ending mid browser restart; so was one quit by the person or Windows.
        if (_pageLoaded || _quitting || e.CloseReason == CloseReason.WindowsShutDown)
            TryMarkStarting(false);
        _tray.Visible = false;
        _tray.ContextMenuStrip?.Dispose();
        _tray.Dispose();
        _trayWindow.Dispose();
        // Closed for the exit (not dismissed), so it comes back next start if it was open.
        _mini?.Dispose();
        _releaseTimer.Dispose();
        _balloonTimer.Dispose();
        RemoveCover();
        _coverTimer.Dispose();
        _monitor.Dispose();
        _app.Dispose();
        base.OnFormClosed(e);
    }
}
