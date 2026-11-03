using System.Globalization;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// A macro's panel beside the game: panel.html (ui/src/panel/PanelView.tsx) in a small always-on-top tool window, in the
/// app's theme (until 2026-11 hand-drawn WinForms, whose choice list spilled out of the window). The page draws Run/Stop,
/// the status, the tabs and the settings the maker let the player change; what is clicked comes back as an "act" and
/// goes to <see cref="AppController"/>. Placed right of the game (else left of it), never over it; a place the person
/// dragged it to is kept per macro in <see cref="Prefs.PanelWindows"/>. Folded, it is a small pill.
/// </summary>
public sealed class PanelWindow : WebToolWindow
{
    const int Gap = 8;

    readonly AppController _app;
    bool _showing, _pill, _placed;
    /// <summary>What the page was last sent, and the theme its frame was last drawn in: pushed again only when changed.</summary>
    string? _sent;
    Themes.Theme? _framed;

    public Guid MacroId { get; }

    /// <summary>The person closed it (✕, Esc, Alt+F4).</summary>
    public event Action? Dismissed;

    /// <summary>Shown by itself (its game started), not by the person: an update may go on while it shows.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public bool AutoShown { get; set; }

    /// <summary>Stepped aside for a moment (the screen colour picker): not shown again by the panels' tick meanwhile, and its
    /// page kept. Back and still hidden, the page goes after the keep time as usual.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public bool Suspended
    {
        get => _suspended;
        set
        {
            _suspended = value;
            if (!value)
                ReleaseLater();
        }
    }

    bool _suspended;

    /// <summary>The window in front before this one took the keyboard (a Roblox window the focus may go back to).</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public IntPtr Before { get; set; }

    public PanelWindow(AppController app, Guid macroId)
    {
        _app = app;
        MacroId = macroId;
        // Lowered while a message box shows, so the box opens in front of it (as the mini window and the overlay).
        Ui.NoteOnTop(this);
    }

    protected override string PageFile => "panel.html";
    protected override bool StartsTopMost => true;
    protected override TimeSpan KeepBrowser => TimeSpan.FromSeconds(60);
    /// <summary>
    /// A panel stepped aside keeps its page. A folded one no longer does once hidden (its game ended): it held its page
    /// for the whole session (audit B13). Shown, folded or not, the page stays anyway (ReleaseBrowser).
    /// </summary>
    protected override bool HoldBrowser => Suspended;

    public bool Pill => _pill;

    /// <summary>Shown (or brought forward); <paramref name="activate"/> false: by itself, the game keeps the keyboard.</summary>
    public async void Open(bool activate)
    {
        if (Visible)
        {
            Push();
            if (activate)
                Activate();
            return;
        }
        ActivateOnShow = activate;
        _showing = true;
        if (!await EnsureBrowserAsync() || IsDisposed)
        {
            _showing = false;
            return;
        }
        Push(); // the page answers with its size, and the window shows then (OnPageSize)
    }

    /// <summary>Hidden by the app (its game ended, the macro deleted): not counted as the person closing it.</summary>
    public void HideQuietly()
    {
        _showing = false;
        if (Visible)
            Hide();
    }

    public void Dismiss()
    {
        HideQuietly();
        Dismissed?.Invoke();
    }

    /// <summary>What the page draws now, from the app: every tick, but only sent when it changed (or while it opens: the page
    /// answers each one with its size, and the window shows then).</summary>
    public void Push()
    {
        if (IsDisposed || !BrowserReady || (!Visible && !_showing))
            return;
        if (Themes.Current is var theme && theme != _framed)
        {
            _framed = theme;
            Frame();
        }
        var message = _app.PanelMessage(MacroId, _pill);
        string json = JsonSerializer.Serialize(message);
        if (json == _sent && Visible && !_showing)
            return;
        _sent = json;
        SendEvent("panel", message);
    }

    /// <summary>A change refused or a Run that couldn't start: said under its row (name) or in the status line (null).</summary>
    public void Error(string? name, string text) => SendEvent("panelError", new { name, text });

    protected override void OnPageReady()
    {
        _sent = null; // a new page: it has nothing yet
        if (Visible || _showing)
            Push();
    }

    protected override void OnPageSize(Size css, JsonElement args)
    {
        if (!Visible && !_showing)
            return;
        _pill = args.TryGetProperty("pill", out var p) && p.ValueKind == JsonValueKind.True;
        float scale = Visible ? ScaleNow : ScaleAt(PlaceFor(Scaled(css, 1)).Location);
        var size = Scaled(css, scale);
        // Never taller than the screen it is on (the rows scroll inside).
        var work = Screen.FromPoint(Visible ? Location : PlaceFor(size).Location).WorkingArea;
        size.Height = Math.Min(size.Height, work.Height - (int)(24 * scale));
        if (!Visible)
        {
            var at = PlaceFor(size);
            // The place first, then the size: set together on a screen at another scale, Windows' scale change resized it
            // again by the ratio (TrayWindow, 2026-10-30).
            Location = at.Location;
            Bounds = new Rectangle(at.Location, size);
            _placed = true;
            _showing = false;
            Show();
            if (ActivateOnShow)
                Activate();
            _app.PanelShown(this);
        }
        else if (Size != size)
            Size = size; // folded, unfolded, a tab with more rows: the top-left stays
    }

    /// <summary>Where it shows: its saved place (on a screen), else by the game.</summary>
    Rectangle PlaceFor(Size size)
    {
        if (Prefs.PanelWindows.TryGetValue(MacroId.ToString(), out var saved) && Parse(saved) is { } at
            && Screen.AllScreens.Any(s => s.WorkingArea.IntersectsWith(new Rectangle(at.X + size.Width / 4, at.Y + 2, Math.Max(1, size.Width / 2), 24))))
            return new Rectangle(Ui.Inside(new Rectangle(at, size), Screen.FromPoint(at).WorkingArea, 8), size);
        return ByGame(_app.PanelGameRect(MacroId), size, _app.PanelsShownBesides(this));
    }

    /// <summary>
    /// Right of the game + 8 px, top-aligned, when it fits that monitor's work area; else left of it; else the old place
    /// (the work area's right edge, 80 px down). Never over the game: Roblox's player list is at its top right, and a
    /// macro's click under a Bloxkeep window makes that window click-through for a moment, which can blank its page.
    /// Other panels already there push it down, 8 px apart.
    /// </summary>
    internal static Rectangle ByGame(Rectangle? game, Size size, IReadOnlyList<Rectangle> others)
    {
        Rectangle work;
        Point at;
        if (game is { Width: > 0, Height: > 0 } g)
        {
            work = Screen.FromRectangle(g).WorkingArea;
            if (g.Right + Gap + size.Width <= work.Right)
                at = new Point(g.Right + Gap, Math.Max(work.Top, g.Top));
            else if (g.Left - Gap - size.Width >= work.Left)
                at = new Point(g.Left - Gap - size.Width, Math.Max(work.Top, g.Top));
            else
                at = new Point(work.Right - size.Width - 16, work.Top + 80);
        }
        else
        {
            work = (Screen.PrimaryScreen ?? Screen.AllScreens[0]).WorkingArea;
            at = new Point(work.Right - size.Width - 16, work.Top + 80);
        }
        return Stack(new Rectangle(at, size), others, work);
    }

    /// <summary>Moved down below every panel it would cover, 8 px apart, kept on the work area.</summary>
    internal static Rectangle Stack(Rectangle r, IReadOnlyList<Rectangle> others, Rectangle work)
    {
        for (int i = 0; i < 20; i++)
        {
            var hit = others.Where(o => o.IntersectsWith(r)).Select(o => o.Bottom).DefaultIfEmpty(int.MinValue).Max();
            if (hit == int.MinValue)
                break;
            r.Y = hit + Gap;
        }
        if (r.Bottom > work.Bottom)
            r.Y = Math.Max(work.Top, work.Bottom - r.Height);
        return r;
    }

    static Point? Parse(string text)
    {
        var p = text.Split(',');
        return p.Length >= 2 && int.TryParse(p[0], CultureInfo.InvariantCulture, out int x) && int.TryParse(p[1], CultureInfo.InvariantCulture, out int y) ? new Point(x, y) : null;
    }

    /// <summary>Dragged by its title: that place is kept for this macro.</summary>
    protected override void OnDragged()
    {
        if (!Visible || !_placed)
            return;
        Prefs.PanelWindows[MacroId.ToString()] = string.Create(CultureInfo.InvariantCulture, $"{Left},{Top}");
        Prefs.Save();
    }

    /// <summary>"Snap to game": the kept place forgotten, placed by the game again.</summary>
    public void Snap()
    {
        if (Prefs.PanelWindows.Remove(MacroId.ToString()))
            Prefs.Save();
        if (!Visible)
            return;
        var r = ByGame(_app.PanelGameRect(MacroId), Size, _app.PanelsShownBesides(this));
        Location = r.Location;
    }

    protected override void OnAct(string what, JsonElement args) => _app.PanelAct(this, what, args);

    protected override void OnBrowserFailed()
    {
        bool shown = Visible;
        base.OnBrowserFailed();
        if (shown)
            Open(activate: false);
    }

    const int WM_ACTIVATE = 0x0006, WA_INACTIVE = 0;

    protected override void WndProc(ref Message m)
    {
        if (m.Msg == WM_ACTIVATE && ((int)m.WParam & 0xFFFF) != WA_INACTIVE)
            _app.PanelActivated(this, m.LParam);
        base.WndProc(ref m); // a click on it takes the keyboard (WebToolWindow)
    }

    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        // Alt+F4 (and Windows asking it to close) only hides it; the app disposes it itself (Dispose, never Close).
        if (e.CloseReason == CloseReason.UserClosing)
        {
            e.Cancel = true;
            Dismiss();
            return;
        }
        base.OnFormClosing(e);
    }

    [DllImport("user32.dll")]
    static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);

    const uint SWP_NOSIZE = 0x1, SWP_NOMOVE = 0x2, SWP_NOACTIVATE = 0x10;
    static readonly IntPtr HWND_TOPMOST = new(-1);

    /// <summary>On top of everything (a Roblox window, Bloxkeep or a panel is in front), or just below <paramref name="front"/>
    /// (another program came forward: it covers the panel, the panel still covers what it covered before).</summary>
    public void Layer(bool onTop, IntPtr front)
    {
        if (!Visible || !IsHandleCreated)
            return;
        SetWindowPos(Handle, onTop ? HWND_TOPMOST : front, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE);
    }
}
