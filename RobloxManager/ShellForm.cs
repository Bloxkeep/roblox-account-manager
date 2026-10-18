using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>
/// The app window: hosts the HTML interface embedded in the exe. The page may only load its own files and
/// Roblox CDN images (see the CSP in index.html); every network call goes through <see cref="AppController"/>.
/// The interface is drawn by WebView2 on the graphics card; if the GPU or its driver keeps crashing, or a start
/// never got the page on screen, it switches to drawing on the CPU and reloads without losing the session.
/// With "keep running when closed" on, the close button hides the window to the notification area so auto-rejoin and
/// Multi-Roblox keep working; the tray icon brings it back or quits.
/// </summary>
public sealed class ShellForm : Form
{
    const string Origin = "https://bloxkeep.local/";
    const int MaxAutomaticRestarts = 3;
    /// <summary>
    /// After this long without a browser failure the counts start again: crashes days apart (a GPU reset after a
    /// sleep, a browser ended by an antivirus) aren't a broken driver, nor a reason to stop restarting the page.
    /// </summary>
    static readonly TimeSpan BrowserHealthyAfter = TimeSpan.FromMinutes(30);
    long _lastBrowserFailure = long.MinValue / 2;

    readonly AppController _app;
    // Long enough that a quick look at another program never closes the browser; it is then reopened on return.
    const int ReleaseAfterMs = 5 * 60_000, BusyRecheckMs = 60_000;
    readonly System.Windows.Forms.Timer _releaseTimer = new() { Interval = ReleaseAfterMs };
    readonly System.Windows.Forms.Timer _coverTimer = new() { Interval = 4000 };
    /// <summary>Hides the tray icon again once a notification shown with close-to-tray off is surely gone.</summary>
    readonly System.Windows.Forms.Timer _balloonTimer = new();
    const int BalloonMs = 8000;
    readonly NotifyIcon _tray;
    /// <summary>The tray icon's quick panel (a left click); made with the icon, hidden until then.</summary>
    readonly TrayPanel _panel = new();
    /// <summary>Until when the newest notification balloon may be on screen; the icon isn't hidden before that.</summary>
    DateTime _balloonUntil;
    WebView2? _web;
    /// <summary>The small always-on-top window, made the first time it is opened and kept (hidden) from then on.</summary>
    MiniForm? _mini;
    /// <summary>The live pictures over the page's game monitor tab.</summary>
    readonly MonitorTab _monitor;
    string _themeId;
    int _gpuCrashes, _automaticRestarts;
    bool _restarting, _released, _quitting, _hiding, _minimizeReady, _minimizing;
    /// <summary>The page got on screen at least once this run (so the start marker may go).</summary>
    bool _pageLoaded;
    /// <summary>
    /// The page as it looked when the window went away (JPEG), shown over the window while a released browser
    /// starts again, so coming back looks instant instead of half a second of empty window.
    /// </summary>
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
        // The place, size and maximised state it was left in come back instead (put in OnLoad); the fit above is
        // for a first start, or when that place is on no screen any more.
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
    /// <summary>Comes up without taking the keyboard from the program the person is in.</summary>
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
        // An update in the background that began with the window in the tray: back in the tray, no window. The
        // window (and its browser) is made the first time it is opened from there.
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
        var menu = new TrayMenu { MiniOpen = () => _mini is { Visible: true } };
        menu.MiniClicked += () => SetMiniOpen(null);
        menu.OpenClicked += BringToFront;
        menu.QuitClicked += Quit;
        menu.State = _app.TrayState;
        menu.MultiClicked += on =>
        {
            if (menu.CanSwitchMulti(on))
                _app.TrayMulti(!on);
            else
                BringToFront(); // on: the sidebar's switch, which asks first; locked: the lock screen
        };
        menu.PresetClicked += _app.TrayRunPreset;
        menu.NewRobloxClicked += _app.TrayNewRoblox;
        menu.AccountClicked += _app.TrayOpenAccount;
        menu.CloseAllClicked += () => _app.CloseAllClients();

        // A left click opens the quick panel (the right click, the menu): Windows 11's own tray icons do the same.
        _panel.MiniOpen = () => _mini is { Visible: true };
        _panel.State = _app.TrayState;
        _panel.OpenClicked += BringToFront;
        _panel.MiniClicked += () => SetMiniOpen(null);
        _panel.MultiClicked += on => _app.TrayMulti(!on); // only raised when it can switch from here
        _panel.AccountClicked += _app.TrayOpenAccount;
        _panel.PresetClicked += _app.TrayRunPreset;
        _panel.NewRobloxClicked += _app.TrayNewRoblox;
        _panel.CloseAllClicked += () => _app.CloseAllClients();
        _panel.ShowClientClicked += _app.TrayShowClient;
        _panel.CloseClientClicked += _app.CloseClient;
        var tray = new NotifyIcon { Icon = Ui.AppIcon, Text = Ui.Title, ContextMenuStrip = menu };
        tray.MouseClick += (_, e) =>
        {
            if (e.Button == MouseButtons.Left)
                _panel.Toggle(Cursor.Position);
        };
        menu.Opening += (_, _) => _panel.HidePanel();
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

    /// <summary>
    /// A Windows notification (the tray icon's balloon, which Windows 10/11 show as a toast) while the window is
    /// hidden, minimised or behind another program; false when it is in front, so the page's own toast is enough.
    /// </summary>
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
        // Changes that still can't be written (the file held by an antivirus) would be gone with the window; the
        // person decides, except when Windows itself is shutting down.
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
    }

    /// <summary>
    /// Minimising waits for a picture of the page first (a few dozen milliseconds): a minimised window's page isn't
    /// drawn any more, so it can't be captured afterwards. The minimise is then sent again and goes through.
    /// </summary>
    protected override void WndProc(ref Message m)
    {
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
        // Not a picture of the accounts after a lock, not one in a theme that changed meanwhile (Windows switching
        // light/dark), and not one drawn for another window size or screen scale.
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

    /// <summary>
    /// On the app's start, the logo in the middle of the window while the browser starts, instead of an empty window;
    /// the page's own loading screen takes over once it is drawn. The browser can take a while to start on a slow
    /// PC (or after an update), so the fallback is long.
    /// </summary>
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

    /// <summary>Minimised, or hidden in the notification area.</summary>
    bool InBackground => !Visible || WindowState == FormWindowState.Minimized;

    /// <summary>
    /// This window, or a dialog it owns, is the one the person is working in. Asked of Windows rather than
    /// Form.ActiveForm, which can be null while the keyboard focus sits in the browser's own child window.
    /// </summary>
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

    /// <summary>
    /// In the background, hide the page (the browser throttles it), trim its memory and poll less; after 5 minutes
    /// the browser is closed entirely, leaving only the small core running until the window comes back.
    /// </summary>
    void UpdateBackground()
    {
        if (!IsHandleCreated)
            return;
        bool background = InBackground;
        _app.Background = background;
        if (background)
        {
            _releaseTimer.Interval = ReleaseAfterMs;
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
                // The page stayed open but was hidden, and the browser may need a moment to draw it again: the
                // picture covers that too (the page says when it is visible and drawn). What it kept of its view is
                // stale from here on (a page recreated after a crash must not put it back).
                if (_snapshot is not null && _web is not null)
                    ShowCover();
                _snapshot = null;
                _app.DropKeptView();
            }
        }
        ApplyBackgroundToBrowser();
        _monitor?.Place(); // gone at once while minimised or hidden; back with the page's next report
    }

    /// <summary>In the background the page is hidden (the browser throttles it) and its memory trimmed; also for a browser made while minimised.</summary>
    void ApplyBackgroundToBrowser()
    {
        if (_web?.CoreWebView2 is not { } core)
            return;
        bool background = InBackground;
        _web.Visible = !background;
        core.MemoryUsageTargetLevel = background ? CoreWebView2MemoryUsageTargetLevel.Low : CoreWebView2MemoryUsageTargetLevel.Normal;
    }

    void ReleaseWebView()
    {
        _releaseTimer.Stop();
        if (_web is not { } web || !InBackground || _restarting)
            return;
        // Something half done on the page (a form, a sign-in) would be lost with the browser: look again in a minute.
        if (_app.PageBusy)
        {
            _releaseTimer.Interval = BusyRecheckMs;
            _releaseTimer.Start();
            return;
        }
        _web = null;
        _released = true;
        _app.PageGone();
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
            BrowserFailed(ex);
        }
        finally
        {
            _restarting = false;
        }
    }

    /// <summary>
    /// A browser that didn't start is dropped rather than left as an empty control, and the window counts as released
    /// again, so the next time it comes to the front another one is tried.
    /// </summary>
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

    static Task<CoreWebView2Environment> CreateEnvironmentAsync()
    {
        // Separate profile folders, because a running browser can't be reopened with different switches.
        return CoreWebView2Environment.CreateAsync(null, Path.Combine(Vault.DataDir, Prefs.SafeRendering ? "webview-safe" : "webview"),
            WebView2Setup.MainWindowOptions());
    }

    /// <param name="firstStart">
    /// The app's own start, which the marker file watches (a browser recreated later is not a start that could fail
    /// for lack of a page; the marker left by a quit meanwhile would wrongly switch to CPU drawing).
    /// </param>
    async Task AttachWebViewAsync(CoreWebView2Environment env, bool firstStart = false)
    {
        var web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = Themes.Current.Background };
        Controls.Add(web);
        _cover?.BringToFront();
        _web = web;
        await web.EnsureCoreWebView2Async(env);
        // Only once a browser exists: a start that fails before this (the profile still held by the copy an update
        // replaced) says nothing about the graphics card, and must not leave the marker behind.
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

    static void ServeAsset(CoreWebView2Environment env, CoreWebView2WebResourceRequestedEventArgs e)
    {
        string path = new Uri(e.Request.Uri).AbsolutePath.TrimStart('/');
        var stream = typeof(ShellForm).Assembly.GetManifestResourceStream("wwwroot/" + path);
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
            _ => "application/octet-stream",
        };
        e.Response = env.CreateWebResourceResponse(stream, 200, "OK", $"Content-Type: {type}\r\nCache-Control: no-store");
    }

    /// <summary>The page starts in the right theme, before its first state arrives, so it never flashes dark.</summary>
    static Stream WithTheme(Stream page)
    {
        using var reader = new StreamReader(page);
        string html = reader.ReadToEnd().Replace("<html ", $"<html data-theme=\"{Themes.Current.Id}\" ");
        return new MemoryStream(Encoding.UTF8.GetBytes(html));
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        SystemEvents.UserPreferenceChanged -= OnUserPreferenceChanged;
        // A run whose page did get on screen was a good one, even when it ends mid-way through a browser restart; so
        // was one the person quit (from the tray, before the page was up on a slow PC) or Windows shut down.
        if (_pageLoaded || _quitting || e.CloseReason == CloseReason.WindowsShutDown)
            TryMarkStarting(false);
        _tray.Visible = false;
        _tray.ContextMenuStrip?.Dispose();
        _tray.Dispose();
        _panel.Dispose();
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
