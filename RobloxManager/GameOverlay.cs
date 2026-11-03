using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Drawn over a Roblox window's picture area, following it as it moves; a window of ours lying over the game's, so it
/// never sends anything to Roblox: every click lands on it, not on the game.
/// <list type="bullet">
/// <item>Edit: the game dimmed under the macro's points (numbered dots, the step's number) and areas (labelled boxes
/// with corner handles); a dot is dragged, a box dragged by its inside and sized by its corners.</item>
/// <item>Areas: the same with the areas only (the panel's "Edit areas").</item>
/// <item>Pick: the window's picture as it was a moment ago, frozen, with a magnifier; a click takes the place and its
/// colour.</item>
/// </list>
/// A bar at its top says what it is for, with Done; Esc (or Enter) ends it too. It ends by itself when the window goes.
/// </summary>
sealed class GameOverlay : Form
{
    public enum Mode { Edit, Areas, Pick }

    public sealed record Area(string Name, string Label, Color Colour, MacroRect Rect);

    const int Grip = 8, Min = 12, DotRadius = 11, BarH = 34;

    readonly IntPtr _game;
    readonly Bitmap? _picture;
    readonly System.Windows.Forms.Timer _follow = new() { Interval = 100 };
    readonly Font _small = new("Segoe UI", 9f, FontStyle.Bold);
    List<AreaGeometry.Dot> _dots = new();
    List<Area> _areas = new();
    string _title;
    Rectangle _client;
    /// <summary>What is being dragged: a dot (its index) or an area (its index), -1 for none.</summary>
    int _dot = -1, _area = -1;
    AreaGeometry.Grip _grip;
    Point _from, _at;
    Rectangle _start;
    bool _done;

    public Mode Kind { get; }
    /// <summary>A dot let go: its step (0-based) and its new place (parts of the picture area).</summary>
    public event Action<int, double, double>? DotMoved;
    /// <summary>An area let go: its name and its new place.</summary>
    public event Action<string, MacroRect>? AreaChanged;
    /// <summary>Pick: the place clicked and its colour.</summary>
    public event Action<double, double, string>? Picked;
    /// <summary>Ended by the person (Done, Esc, Enter, the window gone), not by <see cref="End"/>.</summary>
    public event Action? Done;

    /// <param name="picture">Pick: the picture area as it is now (this takes it).</param>
    public GameOverlay(IntPtr game, Mode mode, string title, Bitmap? picture = null)
    {
        _game = game;
        _picture = picture;
        _title = title;
        Kind = mode;
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        TopMost = true;
        DoubleBuffered = true;
        KeyPreview = true;
        Font = new Font("Segoe UI", 9.5f, FontStyle.Bold);
        Text = title;
        BackColor = Color.Black;
        Cursor = Cursors.Cross;
        // Editing: the game shows through, dimmed. Picking: the frozen picture covers it whole.
        if (mode != Mode.Pick)
            Opacity = 0.62;
        Ui.NoteOnTop(this);
        _follow.Tick += (_, _) => Follow();
        Follow();
    }

    protected override CreateParams CreateParams
    {
        get
        {
            const int WS_EX_TOOLWINDOW = 0x80;
            var p = base.CreateParams;
            p.ExStyle |= WS_EX_TOOLWINDOW;
            return p;
        }
    }

    /// <summary>The points and areas to draw (the page's draft again after each change). Ignored while one is dragged.</summary>
    public void Set(List<AreaGeometry.Dot> dots, List<Area> areas, string? title = null)
    {
        if (_dot >= 0 || _area >= 0)
            return;
        _dots = Kind == Mode.Edit ? dots : new();
        _areas = Kind == Mode.Pick ? new() : areas;
        if (title is not null)
            Text = _title = title;
        Invalidate();
    }

    /// <summary>Closed by the app (the page ended it, the vault locked): <see cref="Done"/> isn't raised.</summary>
    public void End()
    {
        _done = true;
        if (!IsDisposed)
            Close();
    }

    protected override void OnShown(EventArgs e)
    {
        base.OnShown(e);
        _follow.Start();
        Activate();
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
            _small.Dispose();
        }
        base.Dispose(disposing);
    }

    /// <summary>Over the game's picture area as it is now; hidden while it is minimised, closed when it is gone.</summary>
    void Follow()
    {
        if (!IsWindow(_game))
        {
            if (!IsDisposed)
                Close();
            return;
        }
        if (IsIconic(_game) || !GetClientRect(_game, out var rc))
        {
            if (Visible && !IsDisposed)
                Hide();
            return;
        }
        var origin = new POINT();
        ClientToScreen(_game, ref origin);
        var client = new Rectangle(origin.X, origin.Y, rc.Right - rc.Left, rc.Bottom - rc.Top);
        if (client.Width < 2 || client.Height < 2)
            return;
        if (client != _client || Bounds != client)
        {
            _client = client;
            if (_dot < 0 && _area < 0)
                Bounds = client;
            Invalidate();
        }
        if (!Visible && IsHandleCreated)
            Show();
    }

    Rectangle Local => new(Point.Empty, _client.Size);

    Rectangle Box(Area a) => AreaGeometry.ToScreen(a.Rect, Local);

    /// <summary>The bar at the top and its Done button.</summary>
    (Rectangle Bar, Rectangle Done) TopBar()
    {
        int h = LogicalToDeviceUnits(BarH);
        string done = L.T("Done (Esc)");
        int doneW = TextRenderer.MeasureText(done, Font).Width + LogicalToDeviceUnits(20);
        int textW = TextRenderer.MeasureText(_title, Font).Width + LogicalToDeviceUnits(24);
        int w = Math.Min(Width - LogicalToDeviceUnits(16), textW + doneW + LogicalToDeviceUnits(8));
        var bar = new Rectangle((Width - w) / 2, LogicalToDeviceUnits(8), w, h);
        var button = Kind == Mode.Pick ? Rectangle.Empty
            : new Rectangle(bar.Right - doneW - LogicalToDeviceUnits(4), bar.Y + LogicalToDeviceUnits(4), doneW, h - LogicalToDeviceUnits(8));
        return (bar, button);
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        var g = e.Graphics;
        if (Kind == Mode.Pick && _picture is { } p)
        {
            g.DrawImage(p, Local);
            PaintMagnifier(g, p);
        }
        g.SmoothingMode = SmoothingMode.None;
        int pen = Math.Max(2, DeviceDpi / 48);
        for (int i = 0; i < _areas.Count; i++)
        {
            var a = _areas[i];
            var r = Box(a);
            // The inside shows the game clearer than the dimmed rest.
            using (var fill = new SolidBrush(Color.FromArgb(i == _area ? 120 : 70, a.Colour)))
                g.FillRectangle(fill, r);
            using (var outline = new Pen(a.Colour, pen))
                g.DrawRectangle(outline, r.X, r.Y, r.Width - 1, r.Height - 1);
            var size = TextRenderer.MeasureText(a.Label, Font);
            var tag = new Rectangle(r.X, Math.Max(0, r.Y - size.Height - 2), size.Width + 8, size.Height + 2);
            using (var back = new SolidBrush(a.Colour))
                g.FillRectangle(back, tag);
            TextRenderer.DrawText(g, a.Label, Font, tag, Ink(a.Colour), TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
            using var corner = new SolidBrush(Color.White);
            int s = LogicalToDeviceUnits(Grip);
            foreach (var c in new[] { new Point(r.Left, r.Top), new Point(r.Right, r.Top), new Point(r.Left, r.Bottom), new Point(r.Right, r.Bottom) })
                g.FillRectangle(corner, c.X - s / 2, c.Y - s / 2, s, s);
        }
        g.SmoothingMode = SmoothingMode.AntiAlias;
        int radius = LogicalToDeviceUnits(DotRadius);
        for (int i = 0; i < _dots.Count; i++)
        {
            var at = AreaGeometry.ToScreen(_dots[i].X, _dots[i].Y, Local);
            var circle = new Rectangle(at.X - radius, at.Y - radius, radius * 2, radius * 2);
            using (var fill = new SolidBrush(i == _dot ? Color.FromArgb(255, 196, 0) : Color.FromArgb(37, 99, 235)))
                g.FillEllipse(fill, circle);
            using (var ring = new Pen(Color.White, pen))
                g.DrawEllipse(ring, circle);
            TextRenderer.DrawText(g, (_dots[i].Step + 1).ToString(), _small, circle, i == _dot ? Color.Black : Color.White,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding);
        }
        g.SmoothingMode = SmoothingMode.None;
        var (bar, done) = TopBar();
        using (var back = new SolidBrush(Color.FromArgb(32, 32, 32)))
            g.FillRectangle(back, bar);
        var words = new Rectangle(bar.X + LogicalToDeviceUnits(12), bar.Y, (done.IsEmpty ? bar.Right : done.X) - bar.X - LogicalToDeviceUnits(12), bar.Height);
        TextRenderer.DrawText(g, _title, Font, words, Color.White, TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix);
        if (!done.IsEmpty)
        {
            using (var button = new SolidBrush(Color.FromArgb(37, 99, 235)))
                g.FillRectangle(button, done);
            TextRenderer.DrawText(g, L.T("Done (Esc)"), Font, done, Color.White, TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
        }
    }

    static Color Ink(Color c) => c.GetBrightness() > .55f ? Color.Black : Color.White;

    Color PictureAt(Bitmap p, Point local)
    {
        int x = Width > 0 ? local.X * p.Width / Math.Max(1, Width) : 0, y = Height > 0 ? local.Y * p.Height / Math.Max(1, Height) : 0;
        return p.GetPixel(Math.Clamp(x, 0, p.Width - 1), Math.Clamp(y, 0, p.Height - 1));
    }

    /// <summary>11×11 pixels of the picture around the pointer, 10× each, with the colour under it.</summary>
    void PaintMagnifier(Graphics g, Bitmap p)
    {
        const int span = 11, zoom = 10;
        int side = span * zoom;
        var box = new Rectangle(_at.X + 24, _at.Y + 24, side, side + 26);
        if (box.Right > Width)
            box.X = _at.X - 24 - side;
        if (box.Bottom > Height)
            box.Y = _at.Y - 24 - box.Height;
        int px = Width > 0 ? _at.X * p.Width / Math.Max(1, Width) : 0, py = Height > 0 ? _at.Y * p.Height / Math.Max(1, Height) : 0;
        g.InterpolationMode = InterpolationMode.NearestNeighbor;
        g.PixelOffsetMode = PixelOffsetMode.Half;
        g.DrawImage(p, new Rectangle(box.X, box.Y, side, side), new Rectangle(px - span / 2, py - span / 2, span, span), GraphicsUnit.Pixel);
        g.DrawRectangle(Pens.White, box.X + span / 2 * zoom, box.Y + span / 2 * zoom, zoom, zoom);
        var c = PictureAt(p, _at);
        using (var back = new SolidBrush(c))
            g.FillRectangle(back, box.X, box.Y + side, side, 26);
        TextRenderer.DrawText(g, ScreenPicker.Hex(c), Font, new Rectangle(box.X, box.Y + side, side, 26), Ink(c), TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter);
        g.DrawRectangle(Pens.Black, box.X, box.Y, side, side + 26);
    }

    /// <summary>The topmost area a press takes hold of (the last drawn is on top).</summary>
    (int Index, AreaGeometry.Grip Grip) HitArea(Point p)
    {
        int grip = LogicalToDeviceUnits(Grip);
        for (int i = _areas.Count - 1; i >= 0; i--)
        {
            var g = AreaGeometry.HitTest(Box(_areas[i]), p, grip);
            if (g != AreaGeometry.Grip.None)
                return (i, g);
        }
        return (-1, AreaGeometry.Grip.None);
    }

    protected override void OnMouseDown(MouseEventArgs e)
    {
        base.OnMouseDown(e);
        if (e.Button != MouseButtons.Left || Kind == Mode.Pick)
            return;
        if (TopBar().Done.Contains(e.Location))
        {
            Close();
            return;
        }
        // Dots lie over the areas: they are taken first.
        _dot = AreaGeometry.HitDot(_dots, e.Location, Local, LogicalToDeviceUnits(DotRadius + 3));
        if (_dot < 0)
        {
            (_area, _grip) = HitArea(e.Location);
            if (_area < 0)
                return;
            _start = Box(_areas[_area]);
        }
        _from = e.Location;
        Capture = true;
        Invalidate();
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        _at = e.Location;
        if (Kind == Mode.Pick)
        {
            Invalidate();
            return;
        }
        if (_dot >= 0)
        {
            var (x, y) = AreaGeometry.ToFraction(new Point(Math.Clamp(e.X, 0, Width - 1), Math.Clamp(e.Y, 0, Height - 1)), Local);
            _dots[_dot] = _dots[_dot] with { X = x, Y = y };
            Invalidate();
            return;
        }
        if (_area < 0)
        {
            Cursor = TopBar().Done.Contains(e.Location) ? Cursors.Hand
                : AreaGeometry.HitDot(_dots, e.Location, Local, LogicalToDeviceUnits(DotRadius + 3)) >= 0 ? Cursors.SizeAll
                : HitArea(e.Location).Grip switch
                {
                    AreaGeometry.Grip.Move => Cursors.SizeAll,
                    AreaGeometry.Grip.TopLeft or AreaGeometry.Grip.BottomRight => Cursors.SizeNWSE,
                    AreaGeometry.Grip.TopRight or AreaGeometry.Grip.BottomLeft => Cursors.SizeNESW,
                    _ => Cursors.Cross,
                };
            return;
        }
        var moved = AreaGeometry.Drag(_start, _grip, e.X - _from.X, e.Y - _from.Y, Local, LogicalToDeviceUnits(Min));
        _areas[_area] = _areas[_area] with { Rect = AreaGeometry.ToFraction(moved, Local) };
        Invalidate();
    }

    protected override void OnMouseUp(MouseEventArgs e)
    {
        base.OnMouseUp(e);
        if (Kind == Mode.Pick)
        {
            if (e.Button == MouseButtons.Left && _picture is { } p && Local.Contains(e.Location))
            {
                var (x, y) = AreaGeometry.ToFraction(e.Location, Local);
                string colour = ScreenPicker.Hex(PictureAt(p, e.Location));
                _done = true; // picked: not a cancel
                Picked?.Invoke(x, y, colour);
                Close();
            }
            else if (e.Button == MouseButtons.Right)
                Close();
            return;
        }
        Capture = false;
        if (_dot >= 0)
        {
            var d = _dots[_dot];
            bool moved = e.Location != _from;
            _dot = -1;
            Invalidate();
            if (moved)
                DotMoved?.Invoke(d.Step, d.X, d.Y);
            return;
        }
        if (_area < 0)
            return;
        var a = _areas[_area];
        _area = -1;
        Invalidate();
        AreaChanged?.Invoke(a.Name, a.Rect);
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (e.KeyCode is Keys.Escape or Keys.Enter)
        {
            e.Handled = true;
            Close();
            return;
        }
        base.OnKeyDown(e);
    }

    [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct POINT { public int X, Y; }
    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
}

/// <summary>
/// "Pick" with no game open: the screen as it was a moment ago, frozen, with a magnifier; a click takes the colour under
/// the pointer, Esc cancels. Only a picture of the screen: nothing is clicked or sent anywhere.
/// </summary>
sealed class ScreenPicker : Form
{
    readonly Bitmap _shot;
    readonly Rectangle _screen;
    readonly Action<string> _picked;
    Point _at;

    ScreenPicker(Bitmap shot, Rectangle screen, Action<string> picked)
    {
        _shot = shot;
        _screen = screen;
        _picked = picked;
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        TopMost = true;
        DoubleBuffered = true;
        KeyPreview = true;
        Cursor = Cursors.Cross;
        Bounds = screen;
        Font = new Font("Segoe UI", 9.5f);
        Text = L.T("Pick a colour");
        Ui.NoteOnTop(this);
    }

    /// <summary>The owner steps aside while the screen is pictured, then comes back.</summary>
    public static async void Pick(Form owner, Action<string> picked)
    {
        // A panel stepping aside is not shown again by the panels' tick meanwhile (nor its page let go).
        var panel = owner as PanelWindow;
        try
        {
            bool wasShown = owner.Visible;
            if (panel is not null)
                panel.Suspended = true;
            owner.Hide();
            await Task.Delay(200); // the panel gone from the picture
            var screen = SystemInformation.VirtualScreen;
            var shot = new Bitmap(screen.Width, screen.Height);
            using (var g = Graphics.FromImage(shot))
                g.CopyFromScreen(screen.Location, Point.Empty, screen.Size);
            var form = new ScreenPicker(shot, screen, picked);
            form.FormClosed += (_, _) =>
            {
                if (panel is not null)
                    panel.Suspended = false;
                if (wasShown && !owner.IsDisposed)
                    owner.Show();
                form.Dispose();
            };
            form.Show();
            form.Activate();
        }
        catch (Exception ex)
        {
            Log.Write(ex);
            if (panel is not null)
                panel.Suspended = false;
            if (!owner.IsDisposed)
                owner.Show();
        }
    }

    Color At(Point local) => _shot.GetPixel(Math.Clamp(local.X, 0, _shot.Width - 1), Math.Clamp(local.Y, 0, _shot.Height - 1));

    /// <summary>"#rrggbb" of a colour.</summary>
    internal static string Hex(Color c) => $"#{c.R:x2}{c.G:x2}{c.B:x2}";

    protected override void OnPaint(PaintEventArgs e)
    {
        var g = e.Graphics;
        g.DrawImageUnscaled(_shot, 0, 0);
        // The magnifier: 11×11 pixels around the pointer, 10× each.
        const int span = 11, zoom = 10;
        int side = span * zoom;
        var box = new Rectangle(_at.X + 24, _at.Y + 24, side, side + 26);
        if (box.Right > Width)
            box.X = _at.X - 24 - side;
        if (box.Bottom > Height)
            box.Y = _at.Y - 24 - box.Height;
        g.InterpolationMode = InterpolationMode.NearestNeighbor;
        g.PixelOffsetMode = PixelOffsetMode.Half;
        g.DrawImage(_shot, new Rectangle(box.X, box.Y, side, side), new Rectangle(_at.X - span / 2, _at.Y - span / 2, span, span), GraphicsUnit.Pixel);
        g.DrawRectangle(Pens.White, box.X + span / 2 * zoom, box.Y + span / 2 * zoom, zoom, zoom);
        var c = At(_at);
        using (var back = new SolidBrush(c))
            g.FillRectangle(back, box.X, box.Y + side, side, 26);
        TextRenderer.DrawText(g, Hex(c), Font, new Rectangle(box.X, box.Y + side, side, 26), c.GetBrightness() > .55f ? Color.Black : Color.White,
            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter);
        g.DrawRectangle(Pens.Black, box.X, box.Y, side, side + 26);
        string help = L.T("Click the colour. Esc cancels.");
        var size = TextRenderer.MeasureText(help, Font);
        var primary = Screen.PrimaryScreen?.Bounds ?? _screen;
        var at = new Rectangle(primary.X - _screen.X + (primary.Width - size.Width) / 2 - 8, primary.Y - _screen.Y + 16, size.Width + 16, size.Height + 8);
        using var helpBack = new SolidBrush(Color.FromArgb(32, 32, 32));
        g.FillRectangle(helpBack, at);
        TextRenderer.DrawText(g, help, Font, at, Color.White, TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter);
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        _at = e.Location;
        Invalidate();
    }

    protected override void OnMouseUp(MouseEventArgs e)
    {
        base.OnMouseUp(e);
        if (e.Button == MouseButtons.Left)
        {
            string colour = Hex(At(e.Location));
            Close();
            _picked(colour);
        }
        else
            Close();
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (e.KeyCode == Keys.Escape)
        {
            e.Handled = true;
            Close();
            return;
        }
        base.OnKeyDown(e);
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
            _shot.Dispose();
        base.Dispose(disposing);
    }
}
