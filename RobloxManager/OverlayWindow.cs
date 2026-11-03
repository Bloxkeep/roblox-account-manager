using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Drawn over a Roblox window's picture area, following it as it moves: overlay.html (ui/src/overlay/OverlayView.tsx) in
/// a window of ours lying over the game's, so it never sends anything to Roblox: every click lands on it, not on the game.
/// Until 2026-11 it was hand-drawn WinForms (GameOverlay).
/// <list type="bullet">
/// <item>Edit: the game dimmed under the macro's points (numbered dots, the step's number) and areas (labelled boxes
/// with corner handles); a dot is dragged, a box dragged by its inside and sized by its corners.</item>
/// <item>Areas: the same with the areas only (the panel's "Edit areas").</item>
/// <item>Pick: the window's picture as it was a moment ago, frozen, with a magnifier; a click takes the place and its
/// colour.</item>
/// <item>Screen: Pick with no game, over the monitor under the pointer (<see cref="ScreenPicker"/>); the colour only.</item>
/// </list>
/// A bar at its top says what it is for, with Done (or Cancel); Esc or Enter ends it too. It ends by itself when the
/// window goes. The dimming is the window's own opacity: a see-through colour key would let clicks through to the game.
/// The page only says where things went (parts of the picture area); the colour of a click is read here, from the
/// picture itself.
/// </summary>
sealed class OverlayWindow : WebToolWindow
{
    public enum Mode { Edit, Areas, Pick, Screen }

    public sealed record Area(string Name, string Label, Color Colour, MacroRect Rect);

    readonly IntPtr _game;
    readonly Bitmap? _picture;
    readonly string? _pictureUrl;
    readonly System.Windows.Forms.Timer _follow = new() { Interval = 100 };
    List<AreaGeometry.Dot> _dots = new();
    List<Area> _areas = new();
    string _name;
    Rectangle _client;
    bool _done, _opened;

    public Mode Kind { get; }
    /// <summary>The custom theme's colours when that theme is on (AppController.CustomThemeDto), for the bar.</summary>
    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public object? CustomTheme { get; init; }
    /// <summary>A dot let go: its step (0-based) and its new place (parts of the picture area).</summary>
    public event Action<int, double, double>? DotMoved;
    /// <summary>An area let go: its name and its new place.</summary>
    public event Action<string, MacroRect>? AreaChanged;
    /// <summary>Pick and Screen: the place clicked and its colour.</summary>
    public event Action<double, double, string>? Picked;
    /// <summary>Ended by the person (Done, Esc, Enter, the window gone), not by <see cref="End"/> or a pick.</summary>
    public event Action? Done;

    /// <param name="game">The game's window (zero for Screen).</param>
    /// <param name="name">Edit and Areas: the macro's name (empty for a new one).</param>
    /// <param name="picture">Pick and Screen: the picture as it is now (this takes it).</param>
    /// <param name="screen">Screen: the monitor it covers.</param>
    public OverlayWindow(IntPtr game, Mode mode, string name, Bitmap? picture = null, Rectangle? screen = null)
    {
        _game = game;
        _picture = picture;
        _name = name;
        Kind = mode;
        Text = Ui.Title;
        // Editing: the game shows through, dimmed. Picking: the frozen picture covers it whole.
        if (mode is Mode.Edit or Mode.Areas)
            Opacity = 0.62;
        if (picture is not null)
        {
            using var png = new MemoryStream();
            picture.Save(png, ImageFormat.Png);
            _pictureUrl = "data:image/png;base64," + Convert.ToBase64String(png.ToArray());
        }
        if (screen is { } s)
            Bounds = _client = s;
        Ui.NoteOnTop(this);
        _follow.Tick += (_, _) => Follow();
    }

    protected override string PageFile => "overlay.html";
    protected override bool StartsTopMost => true;
    protected override bool Rounded => false;
    protected override bool Shadow => false;
    protected override Color Backdrop => Color.Black;
    protected override TimeSpan KeepBrowser => TimeSpan.FromSeconds(1);

    /// <summary>Starts the page, then shows over the game and takes the keyboard (Esc and Enter are its own, not the
    /// game's). A page that can't start ends it, as Esc would.</summary>
    public async Task OpenAsync()
    {
        if (_opened || IsDisposed)
            return;
        _opened = true;
        if (!await EnsureBrowserAsync() || IsDisposed)
        {
            if (!IsDisposed)
                Close();
            return;
        }
        Push();
        if (Kind == Mode.Screen)
            Show();
        else
            Follow();
        _follow.Enabled = Kind != Mode.Screen;
        if (Visible)
        {
            Activate();
            FocusPage();
        }
    }

    /// <summary>The points and areas to draw (the page's draft again after each change); the page keeps its own while one is dragged.</summary>
    public void Set(List<AreaGeometry.Dot> dots, List<Area> areas, string? name = null)
    {
        _dots = Kind == Mode.Edit ? dots : new();
        _areas = Kind is Mode.Edit or Mode.Areas ? areas : new();
        if (name is not null)
            _name = name;
        Push();
    }

    /// <summary>Closed by the app (the page ended it, the vault locked): <see cref="Done"/> isn't raised.</summary>
    public void End()
    {
        _done = true;
        if (!IsDisposed)
            Close();
    }

    void Push()
    {
        if (!BrowserReady)
            return;
        SendEvent("overlay", new
        {
            mode = Kind switch { Mode.Edit => "edit", Mode.Areas => "areas", Mode.Pick => "pick", _ => "screen" },
            name = _name,
            dots = _dots.Select(d => new { step = d.Step, x = d.X, y = d.Y }),
            areas = _areas.Select(a => new { name = a.Name, label = a.Label, colour = ScreenPicker.Hex(a.Colour), rect = new { x = a.Rect.X, y = a.Rect.Y, w = a.Rect.W, h = a.Rect.H } }),
            picture = _pictureUrl,
            theme = Themes.Current.Id,
            customTheme = CustomTheme,
            language = L.Language,
        });
    }

    protected override void OnPageReady() => Push();

    protected override void OnActivated(EventArgs e)
    {
        base.OnActivated(e);
        FocusPage();
    }

    /// <summary>Parts of the picture area as the page sent them: kept inside it, rounded to 4 places as the app keeps them.</summary>
    static double Part(JsonElement o, string name) =>
        o.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetDouble(out double d) && double.IsFinite(d) ? Math.Round(Math.Clamp(d, 0, 1), 4) : double.NaN;

    protected override void OnAct(string what, JsonElement args)
    {
        switch (what)
        {
            case "done":
            case "cancel":
                Close();
                break;
            case "dot" when Kind == Mode.Edit:
                if (args.TryGetProperty("step", out var s) && s.ValueKind == JsonValueKind.Number && s.TryGetInt32(out int step)
                    && _dots.FindIndex(d => d.Step == step) is int i && i >= 0 && Part(args, "x") is var x && Part(args, "y") is var y && !double.IsNaN(x) && !double.IsNaN(y))
                {
                    _dots[i] = _dots[i] with { X = x, Y = y };
                    DotMoved?.Invoke(step, x, y);
                }
                break;
            case "area" when Kind is Mode.Edit or Mode.Areas:
                string name = Str(args, "name");
                int at = _areas.FindIndex(a => a.Name == name);
                if (at >= 0 && MacroOptions.Rect(new MacroRect { X = Part(args, "x"), Y = Part(args, "y"), W = Part(args, "w"), H = Part(args, "h") }) is { } rect)
                {
                    _areas[at] = _areas[at] with { Rect = rect };
                    AreaChanged?.Invoke(name, rect);
                }
                break;
            case "pick" when Kind is Mode.Pick or Mode.Screen && _picture is { } p:
                double px = Part(args, "x"), py = Part(args, "y");
                if (double.IsNaN(px) || double.IsNaN(py))
                    break;
                // The pixel under the click, from the picture as it was shown (stretched over the window).
                var colour = p.GetPixel(Math.Clamp((int)(px * p.Width), 0, p.Width - 1), Math.Clamp((int)(py * p.Height), 0, p.Height - 1));
                _done = true; // picked: not a cancel
                Picked?.Invoke(px, py, ScreenPicker.Hex(colour));
                Close();
                break;
        }
    }

    /// <summary>Over the game's picture area as it is now; hidden while it is minimised, closed when it is gone.</summary>
    void Follow()
    {
        if (IsDisposed)
            return;
        if (!IsWindow(_game))
        {
            Close();
            return;
        }
        if (IsIconic(_game) || !GetClientRect(_game, out var rc))
        {
            if (Visible)
                Hide();
            return;
        }
        var origin = new POINT();
        ClientToScreen(_game, ref origin);
        var client = new Rectangle(origin.X, origin.Y, rc.Right - rc.Left, rc.Bottom - rc.Top);
        if (client.Width < 2 || client.Height < 2)
            return;
        if (client != _client || Bounds != client)
            Bounds = _client = client;
        if (!Visible && IsHandleCreated)
            Show();
    }

    /// <summary>Never moved to the size the page asks (it asks none) or Windows' idea of a new monitor's size: it follows the game.</summary>
    protected override void OnDpiChanged(DpiChangedEventArgs e)
    {
        if (!_client.IsEmpty)
            Bounds = _client;
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        _follow.Stop();
        base.OnFormClosed(e);
        if (!_done)
        {
            _done = true;
            Done?.Invoke();
        }
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _follow.Dispose();
            _picture?.Dispose();
        }
        base.Dispose(disposing);
    }

    [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct POINT { public int X, Y; }
    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
}

/// <summary>
/// "Pick" with no game open: the monitor under the pointer as it was a moment ago, frozen, with a magnifier, in an
/// <see cref="OverlayWindow"/>; a click takes the colour under the pointer, Esc cancels. Only a picture of the screen:
/// nothing is clicked or sent anywhere. (The WinForms picker covered every monitor at once; a page has one scale, so
/// this covers the one the pointer is on.)
/// </summary>
static class ScreenPicker
{
    /// <summary>The owner steps aside while the screen is pictured, then comes back.</summary>
    public static async void Pick(Form owner, Action<string> picked, object? customTheme = null)
    {
        // A panel stepping aside is not shown again by the panels' tick meanwhile (nor its page let go).
        var panel = owner as PanelWindow;
        bool wasShown = owner.Visible;
        void Back()
        {
            if (panel is not null)
                panel.Suspended = false;
            if (wasShown && !owner.IsDisposed)
                owner.Show();
        }
        try
        {
            if (panel is not null)
                panel.Suspended = true;
            owner.Hide();
            await Task.Delay(200); // the panel gone from the picture
            var screen = Screen.FromPoint(Cursor.Position).Bounds;
            var shot = new Bitmap(screen.Width, screen.Height);
            using (var g = Graphics.FromImage(shot))
                g.CopyFromScreen(screen.Location, Point.Empty, screen.Size);
            var form = new OverlayWindow(IntPtr.Zero, OverlayWindow.Mode.Screen, "", shot, screen) { CustomTheme = customTheme };
            form.Picked += (_, _, colour) => picked(colour);
            form.FormClosed += (_, _) => Back();
            await form.OpenAsync();
        }
        catch (Exception ex)
        {
            Log.Write(ex);
            Back();
        }
    }

    /// <summary>"#rrggbb" of a colour.</summary>
    internal static string Hex(Color c) => $"#{c.R:x2}{c.G:x2}{c.B:x2}";
}
