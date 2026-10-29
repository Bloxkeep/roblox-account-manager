namespace RobloxManager;

/// <summary>
/// The "Game monitor" tab's live pictures: a web page can't show other windows, so a <see cref="WindowThumbnails"/>
/// lies over the tab's area, which the page reports (<see cref="SetArea"/>) and withdraws (null) whenever the page must
/// show there instead (another tab, a dialog or menu, the lock screen). Windows draws the pictures above every child
/// window, the browser included, so nothing of the page can show through them.
/// </summary>
sealed class MonitorTab : IDisposable
{
    readonly Form _host;
    readonly AppController _app;
    /// <summary>The browser control the page's pixels are counted in; null while it is released.</summary>
    readonly Func<Control?> _page;
    readonly WindowThumbnails _tiles = new() { Visible = false, TabStop = false };
    readonly System.Windows.Forms.Timer _timer = new() { Interval = 1500 };
    readonly TileMenu _menu = new();
    List<AppController.MonitorClient> _clients = new();
    AppController.MonitorArea? _area;
    bool _failed;

    public MonitorTab(Form host, AppController app, Func<Control?> page)
    {
        _host = host;
        _app = app;
        _page = page;
        _tiles.Theme = Themes.Current;
        _tiles.TileClicked += OnTileClicked;
        _timer.Tick += (_, _) => Tick();
        host.Controls.Add(_tiles);
    }

    /// <summary>The page's monitor tab is here (CSS pixels and its pixel ratio), or must not be covered (null).</summary>
    public void SetArea(AppController.MonitorArea? area)
    {
        _area = area;
        Place();
    }

    /// <summary>Puts the pictures where the page last said, or takes them away; also called when the window is minimised,
    /// hidden or back (before the page says so).</summary>
    public void Place()
    {
        if (Bounds() is not { } bounds)
        {
            Withdraw();
            return;
        }
        _tiles.Bounds = bounds;
        if (_tiles.Visible)
            return; // a move or resize: the pictures follow the control by themselves
        ApplyTheme();
        // Shown before going live: a picture is placed from the control's own place and size on the window.
        _tiles.Visible = true;
        _tiles.BringToFront();
        _tiles.Live = true;
        RefreshTiles();
        _timer.Start();
    }

    /// <summary>The tab's area in the window's client pixels, or null when nothing should be shown.</summary>
    Rectangle? Bounds()
    {
        if (_area is not { } a || _host.IsDisposed || !_host.Visible || _host.WindowState == FormWindowState.Minimized
            || _page() is not { IsDisposed: false, Visible: true, IsHandleCreated: true } page)
            return null;
        // CSS pixels times the page's pixel ratio are the browser's device pixels, from its top-left corner. Edges
        // are rounded (halves up, as the browser snaps them: 90 px at 125 % starts at 113, not 112), not the size.
        static int Px(double css, double scale) => (int)Math.Round(css * scale, MidpointRounding.AwayFromZero);
        var r = Rectangle.FromLTRB(Px(a.X, a.Scale), Px(a.Y, a.Scale), Px(a.X + a.Width, a.Scale), Px(a.Y + a.Height, a.Scale));
        var origin = _host.PointToClient(page.PointToScreen(Point.Empty));
        r.Offset(origin);
        // Never past the browser: a report from before a resize may be larger than the page is now.
        r.Intersect(new Rectangle(origin, page.ClientSize));
        return r.Width > 0 && r.Height > 0 ? r : null;
    }

    /// <summary>Unregistered before hidden: the pictures are Windows' own and would stay on screen without the control.</summary>
    void Withdraw()
    {
        _timer.Stop();
        // A click on a tile focuses it; hidden while focused, the keys (the lock screen's password, Esc) would go nowhere.
        if (_tiles.ContainsFocus && _page() is { IsDisposed: false, Visible: true, CanFocus: true } page)
            page.Focus();
        _tiles.Live = false;
        _tiles.Visible = false;
    }

    /// <summary>A new theme from Settings, or Windows switching between light and dark (also noticed at the next refresh).</summary>
    public void ApplyTheme()
    {
        if (_tiles.Theme.Id != Themes.Current.Id)
            _tiles.Theme = Themes.Current;
    }

    /// <summary>A refresh that fails is logged (the first of a run of failures), not shown: a box every 1.5 s would pile up, and the next tick tries again.</summary>
    void Tick()
    {
        try
        {
            RefreshTiles();
            _failed = false;
        }
        catch (Exception ex)
        {
            if (!_failed)
                Log.Write(ex);
            _failed = true;
        }
    }

    /// <summary>Every 1.5 s while shown: the tiles are matched to the windows open now, texts taken fresh (the language and theme can change).</summary>
    void RefreshTiles()
    {
        if (!_tiles.Visible || _tiles.IsDisposed)
            return;
        ApplyTheme();
        _tiles.MinimizedText = L.T("Minimised — no live picture");
        _tiles.EmptyText = L.T("No Roblox window is open.");
        _clients = _app.MonitorClients();
        _tiles.SetItems(_clients.Select(c => new WindowThumbnails.Item(c.Hwnd, c.Label, c.Game, Ram(c.RamMb), $"PID {c.Pid}")).ToList());
    }

    static string Ram(long mb) => mb <= 0 ? "" : mb >= 1024 ? $"{mb / 1024.0:0.0} GB" : $"{mb} MB";

    void OnTileClicked(WindowThumbnails.Item item, MouseButtons button)
    {
        // The client as of the last refresh; its window may have closed meanwhile, which every action survives.
        if (_clients.FirstOrDefault(c => c.Hwnd == item.Hwnd) is not { } client)
            return;
        if (button == MouseButtons.Left)
        {
            RobloxProcesses.Focus(client.Pid);
            return;
        }
        _menu.Show(
            (L.T("Show"), () => RobloxProcesses.Focus(client.Pid)),
            (L.T("Close Roblox"), () => _app.CloseMonitorClient(client)),
            (L.T("Copy PID"), () => Ui.CopyToClipboard(client.Pid.ToString(), secret: false)));
    }

    public void Dispose()
    {
        Withdraw();
        _timer.Dispose();
        _menu.Dispose();
        _tiles.Dispose();
    }

    /// <summary>A tile's right-click menu, drawn like the tray menu; its items are made again each time it opens.</summary>
    sealed class TileMenu : ContextMenuStrip
    {
        public TileMenu()
        {
            ShowImageMargin = false;
            ShowCheckMargin = false;
            DropShadowEnabled = true;
            Font = new Font("Segoe UI", 9.75f);
            Padding = new Padding(4, 5, 4, 5);
        }

        public void Show(params (string Text, Action Run)[] items)
        {
            var old = Items.Cast<ToolStripItem>().ToList();
            Items.Clear();
            foreach (var item in old)
                item.Dispose(); // Clear only unhooks them
            foreach (var (text, run) in items)
            {
                var item = new ToolStripMenuItem(text) { Padding = new Padding(10, 6, 24, 6) };
                item.Click += (_, _) => run();
                Items.Add(item);
            }
            Show(Cursor.Position);
        }

        protected override void OnOpening(System.ComponentModel.CancelEventArgs e)
        {
            var theme = Themes.Current;
            Renderer = new ThemedMenu.Renderer(theme, ThemedMenu.RoundCorners(Handle, theme));
            base.OnOpening(e);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
                Font.Dispose();
            base.Dispose(disposing);
        }
    }
}
