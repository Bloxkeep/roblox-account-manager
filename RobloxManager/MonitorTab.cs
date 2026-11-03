namespace RobloxManager;

/// <summary>
/// The "Game monitor" tab's live pictures. The page draws the tab (ui/src/monitor/MonitorView.tsx: tile frames, names,
/// buttons, menus) from the windows this sends it (the "monitor" event, every 1.5 s while the tab shows and at once when
/// a window is minimised or opened again), and reports where each tile's picture goes; a page can't show other windows,
/// so <see cref="WindowThumbnails"/> lays each one there. The page withdraws a picture its menu would lie under, and all
/// of them ({ hidden }) whenever the page must show there instead (another tab, a dialog, the lock screen): Windows draws
/// the pictures above the whole page.
/// </summary>
sealed class MonitorTab : IDisposable
{
    readonly Form _host;
    readonly AppController _app;
    /// <summary>The browser control the page's pixels are counted in; null while it is released.</summary>
    readonly Func<Control?> _page;
    readonly WindowThumbnails _pictures;
    readonly System.Windows.Forms.Timer _timer = new() { Interval = 1500 };
    AppController.MonitorArea? _area;
    /// <summary>The windows the page was last sent: only theirs are pictured, whatever handle a report names.</summary>
    HashSet<IntPtr> _known = new();
    bool _shown, _failed;

    public MonitorTab(Form host, AppController app, Func<Control?> page)
    {
        _host = host;
        _app = app;
        _page = page;
        _pictures = new WindowThumbnails(host);
        _pictures.Changed += Refresh;
        _timer.Tick += (_, _) => Refresh();
    }

    /// <summary>The page's monitor tab and its pictures are here (CSS pixels and its pixel ratio), or must not be covered (null).</summary>
    public void SetArea(AppController.MonitorArea? area)
    {
        _area = area;
        Place();
    }

    /// <summary>Puts the pictures where the page last said, or takes them away; also called when the window is minimised,
    /// hidden or back (before the page says so).</summary>
    public void Place()
    {
        if (_area is not { } a || _host.IsDisposed || !_host.Visible || _host.WindowState == FormWindowState.Minimized
            || _page() is not { IsDisposed: false, Visible: true, IsHandleCreated: true } page)
        {
            Withdraw();
            return;
        }
        // CSS pixels times the page's pixel ratio are the browser's device pixels, from its top-left corner. Edges are
        // rounded (halves up, as the browser snaps them: 90 px at 125 % starts at 113, not 112), not the size.
        static int Px(double css, double scale) => (int)Math.Round(css * scale, MidpointRounding.AwayFromZero);
        var origin = _host.PointToClient(page.PointToScreen(Point.Empty));
        Rectangle ToHost(double x, double y, double w, double h)
        {
            var r = Rectangle.FromLTRB(Px(x, a.Scale), Px(y, a.Scale), Px(x + w, a.Scale), Px(y + h, a.Scale));
            r.Offset(origin);
            return r;
        }
        // Never past the browser: a report from before a resize may be larger than the page is now.
        var clip = Rectangle.Intersect(ToHost(a.X, a.Y, a.Width, a.Height), new Rectangle(origin, page.ClientSize));
        if (!_shown)
        {
            _shown = true;
            Refresh();
            _timer.Start();
        }
        var boxes = new Dictionary<IntPtr, Rectangle>();
        foreach (var p in a.Pictures.Where(p => _known.Contains(p.Hwnd)))
            boxes[p.Hwnd] = ToHost(p.X, p.Y, p.Width, p.Height);
        _pictures.Place(boxes, clip);
    }

    /// <summary>Unregistered before hidden: the pictures are Windows' own and would stay on screen without the page.</summary>
    void Withdraw()
    {
        _timer.Stop();
        _shown = false;
        _pictures.Clear();
    }

    /// <summary>The windows open now, to the page (a refresh that fails is logged once, not shown: the next tick tries again).</summary>
    public void Refresh()
    {
        if (!_shown)
            return;
        try
        {
            var clients = _app.MonitorClients();
            _known = clients.Select(c => c.Hwnd).ToHashSet();
            _app.EmitMonitor(clients, _pictures.IsMinimized);
            _failed = false;
        }
        catch (Exception ex)
        {
            if (!_failed)
                Log.Write(ex);
            _failed = true;
        }
    }

    public void Dispose()
    {
        Withdraw();
        _timer.Dispose();
        _pictures.Dispose();
    }
}
