using System.Globalization;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// A small always-on-top window listing the running accounts (status, name, game, RAM, Show and Close): mini.html
/// (ui/src/mini/MiniView.tsx) in the app's theme, the same chrome as a macro's panel (until 2026-11 hand-drawn WinForms).
/// It keeps working while the main window is in the tray with its browser released (its own page, the shared browser
/// profile). Esc or ✕ hides it; its place and height are kept in <see cref="Prefs.MiniWindow"/>; its bottom edge sizes it.
/// The page is let go 60 s after it hides.
/// </summary>
sealed class MiniWindow : WebToolWindow
{
    /// <summary>Its width and its heights in CSS pixels (the page's); the window is these times its monitor's scale.</summary>
    const int PageWidth = 300, MinHeight = 200, DefaultHeight = 360, Gap = 16;

    readonly AppController _app;
    readonly Action _openApp;
    readonly System.Windows.Forms.Timer _timer = new() { Interval = 2000 };
    bool _showing, _failed, _placed;
    /// <summary>What the page was last sent: the two-second refresh sends only a change.</summary>
    string? _sent;
    /// <summary>Its height in CSS pixels, as the person left it.</summary>
    int _height = DefaultHeight;

    public event Action? Dismissed;

    public MiniWindow(AppController app, Action openApp)
    {
        _app = app;
        _openApp = openApp;
        // A message box lowers it meanwhile, so it opens in front (Ui.Modal).
        Ui.NoteOnTop(this);
        _timer.Tick += (_, _) => Tick();
    }

    protected override string PageFile => "mini.html";
    protected override bool StartsTopMost => true;
    protected override TimeSpan KeepBrowser => TimeSpan.FromSeconds(60);

    /// <param name="activate">false when reopened at start, so the lock screen keeps the keyboard.</param>
    public async void Open(bool activate)
    {
        Prefs.MiniOpen = true;
        Prefs.Save();
        if (Visible)
        {
            Push();
            if (activate)
                Activate();
            return;
        }
        if (_showing)
            return;
        ActivateOnShow = activate;
        _showing = true;
        if (!await EnsureBrowserAsync() || IsDisposed)
        {
            _showing = false;
            return;
        }
        Push(); // the page answers with its size once it has drawn, and the window shows then (OnPageSize)
    }

    /// <summary>Hidden at the person's request (✕, Esc, Alt+F4, the page's button); forgotten as open, unlike the app's exit.</summary>
    public void Dismiss()
    {
        _showing = false;
        if (!Visible)
            return;
        SaveBounds();
        Prefs.MiniOpen = false;
        Prefs.Save();
        Hide();
        Dismissed?.Invoke();
    }

    /// <summary>A new theme or language: drawn again at once.</summary>
    public void ApplyTheme()
    {
        if (!Visible)
            return;
        Frame();
        Push();
    }

    protected override void OnVisibleChanged(EventArgs e)
    {
        base.OnVisibleChanged(e);
        if (Visible)
            _timer.Start();
        else
            _timer.Stop();
    }

    /// <summary>A refresh that fails is logged (the first of a run of failures), not shown: the next tick tries again.</summary>
    void Tick()
    {
        try
        {
            Push();
            _failed = false;
        }
        catch (Exception ex)
        {
            if (!_failed)
                Log.Write(ex);
            _failed = true;
        }
    }

    /// <summary>What the page draws now: every two seconds while shown, and after each click; sent only when it changed,
    /// except while it opens (the page answers with its size, and the window shows then).</summary>
    void Push()
    {
        if (IsDisposed || !BrowserReady || (!Visible && !_showing))
            return;
        var message = _app.MiniMessage();
        string json = JsonSerializer.Serialize(message);
        if (json == _sent && Visible && !_showing)
            return;
        _sent = json;
        SendEvent("mini", message);
    }

    protected override void OnPageReady()
    {
        _sent = null; // a new page: it has nothing yet
        if (Visible || _showing)
            Push();
    }

    /// <summary>The page has drawn: shown now, where it was left (or at the bottom right of the main screen).</summary>
    protected override void OnPageSize(Size css, JsonElement args)
    {
        if (!_showing || Visible)
            return;
        _showing = false;
        var at = SavedPlace() ?? DefaultPlace();
        float scale = ScaleAt(at.Location);
        Limits(scale);
        var area = Screen.FromPoint(at.Location).WorkingArea;
        var size = new Size(Scaled(new Size(PageWidth, 1), scale).Width, Math.Min(Scaled(new Size(1, _height), scale).Height, area.Height));
        // The place first, then the size: set together on a screen at another scale, Windows' scale change resized it
        // again by the ratio (TrayWindow, 2026-10-30).
        Location = Ui.Inside(new Rectangle(at.Location, size), area, 0);
        Bounds = new Rectangle(Location, size);
        _placed = true;
        Show();
        if (ActivateOnShow)
            Activate();
    }

    /// <summary>Only the height changes, between its least and the screen's.</summary>
    void Limits(float scale)
    {
        int width = Scaled(new Size(PageWidth, 1), scale).Width;
        MinimumSize = Size.Empty;
        MaximumSize = Size.Empty;
        MinimumSize = new Size(width, Scaled(new Size(1, MinHeight), scale).Height);
        // A maximum height of 0 isn't "any" to Windows' size limits: a huge one stands in.
        MaximumSize = new Size(width, 20000);
    }

    /// <summary>Moved to a screen with another scale: the same page size there (WebToolWindow takes Windows' place).</summary>
    protected override void OnDpiChanged(DpiChangedEventArgs e)
    {
        base.OnDpiChanged(e);
        float scale = e.DeviceDpiNew / 96f;
        Limits(scale);
        Size = Scaled(new Size(PageWidth, _height), scale);
    }

    Rectangle DefaultPlace()
    {
        var area = (Screen.PrimaryScreen ?? Screen.AllScreens[0]).WorkingArea;
        var size = Scaled(new Size(PageWidth, _height), ScaleAt(area.Location));
        int margin = Scaled(new Size(Gap, 1), ScaleAt(area.Location)).Width;
        return new Rectangle(area.Right - size.Width - margin, area.Bottom - size.Height - margin, size.Width, size.Height);
    }

    /// <summary>The saved place (its height in CSS pixels), unless no screen shows its top any more (a monitor unplugged).</summary>
    Rectangle? SavedPlace()
    {
        var parts = Prefs.MiniWindow?.Split(',');
        if (parts is not { Length: 4 } || !int.TryParse(parts[0], CultureInfo.InvariantCulture, out int x) || !int.TryParse(parts[1], CultureInfo.InvariantCulture, out int y)
            || !int.TryParse(parts[3], CultureInfo.InvariantCulture, out int h) || h <= 0)
            return null;
        _height = Math.Max(MinHeight, h);
        var size = Scaled(new Size(PageWidth, _height), ScaleAt(new Point(x, y)));
        // The middle half of its title row must be somewhere it can be grabbed.
        var grip = new Rectangle(x + size.Width / 4, y + 2, size.Width / 2, 24);
        return Screen.AllScreens.Any(s => s.WorkingArea.IntersectsWith(grip)) ? new Rectangle(x, y, size.Width, size.Height) : null;
    }

    /// <summary>Noted, not written: the callers save prefs once they are done.</summary>
    void SaveBounds()
    {
        if (!Visible || !_placed)
            return;
        _height = Math.Max(MinHeight, (int)Math.Round(Height / ScaleNow));
        Prefs.MiniWindow = string.Create(CultureInfo.InvariantCulture, $"{Left},{Top},{PageWidth},{_height}");
    }

    protected override void OnDragged()
    {
        SaveBounds();
        Prefs.Save();
    }

    protected override void OnResized()
    {
        SaveBounds();
        Prefs.Save();
    }

    protected override void OnAct(string what, JsonElement args)
    {
        switch (what)
        {
            case "hide":
                Dismiss();
                return;
            case "open":
                _openApp();
                return;
            case "show" when Guid.TryParse(Str(args, "id"), out var id):
                _app.ShowClient(id);
                break;
            case "close" when Guid.TryParse(Str(args, "id"), out var id):
                _app.CloseClient(id);
                break;
            case "closeAll":
                if (args.TryGetProperty("remember", out var r) && r.ValueKind == JsonValueKind.True)
                    _app.SkipConfirm("closeAll");
                _app.CloseAllClients();
                break;
        }
        Push();
    }

    protected override void OnBrowserFailed()
    {
        bool shown = Visible;
        base.OnBrowserFailed();
        if (shown)
            Open(activate: false);
    }

    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        // Alt+F4 hides, like Esc; the app's own exit disposes it.
        if (e.CloseReason == CloseReason.UserClosing)
        {
            e.Cancel = true;
            Dismiss();
            return;
        }
        SaveBounds();
        Prefs.Save();
        base.OnFormClosing(e);
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            // Closed for the exit (not dismissed): its place is kept, and it comes back next start if it was open.
            SaveBounds();
            _timer.Dispose();
        }
        base.Dispose(disposing);
    }
}
