using System.ComponentModel;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// A grid of captioned tiles of other programs' windows, live as DWM thumbnails (what the taskbar shows on hover):
/// Windows draws each frame straight into this control's top-level window, so nothing is copied or polled. A minimised
/// window has no live frame, so its tile shows a note. Knows nothing of the app; the layout is pure arithmetic
/// (<see cref="TileRects"/>) so it can be checked without a window.
/// </summary>
/// <remarks>A thumbnail exists only while this control has a handle, <see cref="Live"/> is on and its window is in the
/// item list; every other path unregisters it, so none leaks or points at a dead window.</remarks>
sealed class WindowThumbnails : ScrollableControl
{
    /// <summary>A window to show: its handle, the caption's two lines, and a short tag at the right of each line.</summary>
    public sealed record Item(IntPtr Hwnd, string Title, string Subtitle, string Tag, string Tag2);

    /// <summary>Tile spacing and the smallest tile, in logical pixels (scaled by the dpi in <see cref="TileRects"/>).</summary>
    public const int TilePad = 10, MinTileWidth = 200, MinTileHeight = 150;
    const int Inset = 6, Radius = 10;
    const TextFormatFlags LineFlags = TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix | TextFormatFlags.SingleLine | TextFormatFlags.NoPadding;

    const uint DWM_TNP_RECTDESTINATION = 0x1, DWM_TNP_RECTSOURCE = 0x2, DWM_TNP_OPACITY = 0x4, DWM_TNP_VISIBLE = 0x8, DWM_TNP_SOURCECLIENTAREAONLY = 0x10;

    [StructLayout(LayoutKind.Sequential)]
    struct RECT
    {
        public int Left, Top, Right, Bottom;
        public RECT(Rectangle r) => (Left, Top, Right, Bottom) = (r.Left, r.Top, r.Right, r.Bottom);
    }

    [StructLayout(LayoutKind.Sequential)]
    struct SIZE
    {
        public int cx, cy;
    }

    // BOOLs are 4-byte ints so the struct is blittable and 48 bytes, as dwmapi.h lays it out.
    [StructLayout(LayoutKind.Sequential)]
    struct DWM_THUMBNAIL_PROPERTIES
    {
        public uint dwFlags;
        public RECT rcDestination;
        public RECT rcSource;
        public byte opacity;
        public int fVisible;
        public int fSourceClientAreaOnly;
    }

    [DllImport("dwmapi.dll")]
    static extern int DwmRegisterThumbnail(IntPtr destination, IntPtr source, out IntPtr thumbnail);

    [DllImport("dwmapi.dll")]
    static extern int DwmUnregisterThumbnail(IntPtr thumbnail);

    [DllImport("dwmapi.dll")]
    static extern int DwmUpdateThumbnailProperties(IntPtr thumbnail, ref DWM_THUMBNAIL_PROPERTIES properties);

    [DllImport("dwmapi.dll")]
    static extern int DwmQueryThumbnailSourceSize(IntPtr thumbnail, out SIZE size);

    [DllImport("user32.dll")]
    static extern bool IsWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    static extern bool IsIconic(IntPtr hWnd);

    [DllImport("user32.dll")]
    static extern bool IsWindowVisible(IntPtr hWnd);

    /// <summary>One window's tile. Thumb is zero while no thumbnail is registered for it.</summary>
    sealed class Tile(Item item)
    {
        public Item Item = item;
        public IntPtr Thumb;
        public Rectangle Bounds, Picture;
        public bool Minimized;
    }

    readonly List<Tile> _tiles = new();
    Font _titleFont, _smallFont;
    Themes.Theme _theme = Themes.Current;
    bool _live;
    int _hover = -1;

    /// <summary>A tile was clicked with this button (right for a menu); the item as last given to <see cref="SetItems"/>.</summary>
    public event Action<Item, MouseButtons>? TileClicked;

    /// <summary>Shown in a tile instead of the picture while its window is minimised.</summary>
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public string MinimizedText { get; set; } = "";

    /// <summary>Shown in the middle when there are no items.</summary>
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public string EmptyText { get; set; } = "";

    public WindowThumbnails()
    {
        SetStyle(ControlStyles.OptimizedDoubleBuffer | ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.ResizeRedraw, true);
        AutoScroll = true;
        (_titleFont, _smallFont) = MakeFonts();
    }

    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public Themes.Theme Theme
    {
        get => _theme;
        set
        {
            _theme = value;
            BackColor = value.Background;
            Invalidate();
        }
    }

    /// <summary>Off (the default, and while the owner is hidden or minimised) every thumbnail is unregistered so Windows
    /// composes nothing; the items are kept and registered again when it goes on.</summary>
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public bool Live
    {
        get => _live;
        set
        {
            if (_live == value)
                return;
            _live = value;
            if (value)
                RegisterAll();
            else
                UnregisterAll();
            Relayout();
        }
    }

    // ───────────────────────────── layout ─────────────────────────────

    static int Scale(int logical, int dpi) => (int)Math.Round(logical * dpi / 96.0);

    /// <summary>Columns: 1, 2, 2×2, 3×2, 3×3, 4×3, 4×4, then a square-ish grid; fewer when tiles would be narrower than
    /// <see cref="MinTileWidth"/> (a tall narrow window stacks them).</summary>
    public static int Columns(int count, int width, int dpi)
    {
        if (count <= 1)
            return 1;
        int cols = count switch { 2 => 2, <= 4 => 2, <= 6 => 3, <= 9 => 3, <= 16 => 4, _ => (int)Math.Ceiling(Math.Sqrt(count)) };
        int pad = Scale(TilePad, dpi), minW = Scale(MinTileWidth, dpi);
        while (cols > 1 && (width - pad * (cols + 1)) / cols < minW)
            cols--;
        return cols;
    }

    /// <summary>The tiles' rectangles in device pixels. Rows share the height; tiles never go below
    /// <see cref="MinTileHeight"/>, and then <paramref name="contentHeight"/> exceeds the client (the owner scrolls).</summary>
    public static Rectangle[] TileRects(int count, Size client, int dpi, out int contentHeight)
    {
        contentHeight = 0;
        if (count <= 0)
            return [];
        int pad = Scale(TilePad, dpi), minW = Scale(MinTileWidth, dpi), minH = Scale(MinTileHeight, dpi);
        int cols = Columns(count, client.Width, dpi), rows = (count + cols - 1) / cols;
        int tileW = Math.Max(minW, (client.Width - pad * (cols + 1)) / cols);
        int tileH = Math.Max(minH, (client.Height - pad * (rows + 1)) / rows);
        contentHeight = pad + rows * (tileH + pad);
        var rects = new Rectangle[count];
        for (int i = 0; i < count; i++)
            rects[i] = new Rectangle(pad + i % cols * (tileW + pad), pad + i / cols * (tileH + pad), tileW, tileH);
        return rects;
    }

    /// <summary>The largest rectangle of the source's shape inside <paramref name="box"/>, centred (a 16:9 game in a square tile is letterboxed).</summary>
    public static Rectangle Fit(Size source, Rectangle box)
    {
        if (source.Width <= 0 || source.Height <= 0 || box.Width <= 0 || box.Height <= 0)
            return box;
        double scale = Math.Min((double)box.Width / source.Width, (double)box.Height / source.Height);
        int w = Math.Max(1, (int)(source.Width * scale)), h = Math.Max(1, (int)(source.Height * scale));
        return new Rectangle(box.X + (box.Width - w) / 2, box.Y + (box.Height - h) / 2, w, h);
    }

    int CaptionHeight() =>
        TextRenderer.MeasureText("Ag", _titleFont).Height + TextRenderer.MeasureText("Ag", Font).Height + Scale(Inset, DeviceDpi) * 2 + Scale(2, DeviceDpi);

    /// <summary>Places the tiles for the current size and scroll position and moves every thumbnail with them.</summary>
    public void Relayout()
    {
        int dpi = DeviceDpi, inset = Scale(Inset, dpi);
        var rects = TileRects(_tiles.Count, ClientSize, dpi, out int contentHeight);
        // The scrollbar first (it narrows the client); the resize that follows lays out again for the new width.
        var minSize = new Size(0, contentHeight > ClientSize.Height ? contentHeight : 0);
        if (AutoScrollMinSize != minSize)
        {
            AutoScrollMinSize = minSize;
            rects = TileRects(_tiles.Count, ClientSize, dpi, out _);
        }
        int captionH = CaptionHeight(), scrollY = AutoScrollPosition.Y;
        for (int i = 0; i < _tiles.Count; i++)
        {
            var tile = _tiles[i];
            var r = rects[i];
            r.Offset(0, scrollY);
            tile.Bounds = r;
            tile.Picture = new Rectangle(r.X + inset, r.Y + captionH, r.Width - inset * 2, r.Height - captionH - inset);
        }
        UpdateThumbnails();
        Invalidate();
    }

    // ───────────────────────────── thumbnails ─────────────────────────────

    /// <summary>The top-level window the thumbnails are drawn into (DWM accepts no child window as destination).</summary>
    IntPtr Destination => FindForm() is { IsHandleCreated: true } form ? form.Handle : IntPtr.Zero;

    /// <summary>The windows to show, in tile order; existing tiles keep their thumbnail. Call every second or two: it also
    /// follows minimising and windows that have gone.</summary>
    public void SetItems(IReadOnlyList<Item> items)
    {
        var keep = new Dictionary<IntPtr, Tile>();
        foreach (var tile in _tiles)
            keep.TryAdd(tile.Item.Hwnd, tile);
        var next = new List<Tile>(items.Count);
        foreach (var item in items)
        {
            if (item.Hwnd == IntPtr.Zero || next.Any(t => t.Item.Hwnd == item.Hwnd))
                continue;
            if (keep.Remove(item.Hwnd, out var tile))
                tile.Item = item;
            else
            {
                tile = new Tile(item);
                if (_live)
                    Register(tile);
            }
            next.Add(tile);
        }
        foreach (var gone in keep.Values)
            Unregister(gone);
        _tiles.Clear();
        _tiles.AddRange(next);
        if (_hover >= _tiles.Count)
            _hover = -1;
        Relayout();
    }

    void Register(Tile tile)
    {
        if (tile.Thumb != IntPtr.Zero || !IsHandleCreated)
            return;
        IntPtr destination = Destination;
        if (destination == IntPtr.Zero || !IsWindow(tile.Item.Hwnd))
            return;
        // Fails for a window that just closed or one of another desktop session: the tile then shows its caption only.
        if (DwmRegisterThumbnail(destination, tile.Item.Hwnd, out var thumb) != 0 || thumb == IntPtr.Zero)
        {
            tile.Thumb = IntPtr.Zero;
            return;
        }
        tile.Thumb = thumb;
        // Until this is set the source size answers for the whole frame (title bar and borders), and the first
        // layout would fit the picture to the wrong shape.
        var props = new DWM_THUMBNAIL_PROPERTIES { dwFlags = DWM_TNP_SOURCECLIENTAREAONLY, fSourceClientAreaOnly = 1 };
        DwmUpdateThumbnailProperties(thumb, ref props);
    }

    static void Unregister(Tile tile)
    {
        if (tile.Thumb == IntPtr.Zero)
            return;
        // Fails harmlessly once the destination window is gone: Windows dropped the thumbnail with it.
        DwmUnregisterThumbnail(tile.Thumb);
        tile.Thumb = IntPtr.Zero;
    }

    void RegisterAll()
    {
        foreach (var tile in _tiles)
            Register(tile);
    }

    void UnregisterAll()
    {
        foreach (var tile in _tiles)
            Unregister(tile);
    }

    /// <summary>Fits every thumbnail into its tile's picture area, hidden while its window is minimised or the tile is scrolled out of view.</summary>
    void UpdateThumbnails()
    {
        if (!_live || !IsHandleCreated)
            return;
        var client = ClientRectangle;
        // Thumbnail rectangles are relative to the destination window's client area, not to this control.
        var origin = FindForm() is { IsHandleCreated: true } form ? form.PointToClient(PointToScreen(Point.Empty)) : Point.Empty;
        foreach (var tile in _tiles)
        {
            if (!IsWindow(tile.Item.Hwnd))
            {
                // Gone since the last list: the next SetItems drops the tile; never keep a thumbnail of a dead window.
                Unregister(tile);
                tile.Minimized = false;
                continue;
            }
            // A hidden window has no live frame either (Windows shows a grey placeholder for it).
            tile.Minimized = IsIconic(tile.Item.Hwnd) || !IsWindowVisible(tile.Item.Hwnd);
            if (tile.Thumb == IntPtr.Zero)
                Register(tile);
            if (tile.Thumb == IntPtr.Zero)
                continue;
            var props = new DWM_THUMBNAIL_PROPERTIES
            {
                dwFlags = DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
                opacity = 255,
                fSourceClientAreaOnly = 1,
            };
            bool sized = DwmQueryThumbnailSourceSize(tile.Thumb, out var size) == 0 && size.cx > 0 && size.cy > 0;
            var source = sized ? new Size(size.cx, size.cy) : new Size(16, 9);
            var picture = Fit(source, tile.Picture);
            var visible = Rectangle.Intersect(picture, client);
            if (tile.Minimized || tile.Picture.Width < 24 || tile.Picture.Height < 24 || visible.IsEmpty)
            {
                // Hidden, but still given a real rectangle: an empty one is refused as an argument.
                props.fVisible = 0;
                props.rcDestination = new RECT(new Rectangle(origin.X + tile.Bounds.X, origin.Y + tile.Bounds.Y, 1, 1));
            }
            else
            {
                props.fVisible = 1;
                var cut = visible;
                visible.Offset(origin);
                props.rcDestination = new RECT(visible);
                // The part of the window shown, given every time since Windows keeps the last one: a tile half scrolled
                // out shows the matching half, not a squashed whole. (Counted from the client area, as the picture is.)
                if (sized)
                {
                    props.dwFlags |= DWM_TNP_RECTSOURCE;
                    double sx = (double)source.Width / picture.Width, sy = (double)source.Height / picture.Height;
                    props.rcSource = cut.Size == picture.Size
                        ? new RECT(new Rectangle(0, 0, source.Width, source.Height))
                        : new RECT(new Rectangle(
                            (int)((cut.X - picture.X) * sx), (int)((cut.Y - picture.Y) * sy),
                            Math.Max(1, (int)(cut.Width * sx)), Math.Max(1, (int)(cut.Height * sy))));
                }
            }
            if (DwmUpdateThumbnailProperties(tile.Thumb, ref props) != 0)
                Unregister(tile); // the window went away between the two calls; registered again next time if not
        }
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        if (_live)
        {
            RegisterAll();
            Relayout();
        }
    }

    protected override void OnHandleDestroyed(EventArgs e)
    {
        // The thumbnails belong to this handle's top-level window; a recreated handle registers them again.
        UnregisterAll();
        base.OnHandleDestroyed(e);
    }

    protected override void OnResize(EventArgs e)
    {
        base.OnResize(e);
        if (IsHandleCreated)
            Relayout();
    }

    protected override void OnLocationChanged(EventArgs e)
    {
        base.OnLocationChanged(e);
        // Moved without a resize: the pictures are placed in the window's pixels, not this control's, so move them too.
        if (IsHandleCreated)
            Relayout();
    }

    protected override void OnScroll(ScrollEventArgs se)
    {
        base.OnScroll(se);
        Relayout();
    }

    protected override void OnMouseWheel(MouseEventArgs e)
    {
        base.OnMouseWheel(e);
        Relayout(); // the wheel scrolls without OnScroll; the pictures must follow all the same
    }

    /// <summary>Sized in pixels for this control's own dpi: the form's font follows a screen change by itself, these must be made again.</summary>
    (Font Title, Font Small) MakeFonts() =>
        (new Font("Segoe UI Semibold", 9.5f * DeviceDpi / 72f, GraphicsUnit.Pixel), new Font("Segoe UI", 8.5f * DeviceDpi / 72f, GraphicsUnit.Pixel));

    protected override void OnDpiChangedAfterParent(EventArgs e)
    {
        base.OnDpiChangedAfterParent(e);
        var old = (_titleFont, _smallFont);
        (_titleFont, _smallFont) = MakeFonts();
        old._titleFont.Dispose();
        old._smallFont.Dispose();
        Relayout();
    }

    // ───────────────────────────── mouse ─────────────────────────────

    int TileAt(Point p)
    {
        for (int i = 0; i < _tiles.Count; i++)
        {
            if (_tiles[i].Bounds.Contains(p))
                return i;
        }
        return -1;
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        SetHover(TileAt(e.Location));
    }

    protected override void OnMouseLeave(EventArgs e)
    {
        base.OnMouseLeave(e);
        SetHover(-1);
    }

    void SetHover(int index)
    {
        if (index == _hover)
            return;
        // Only the two tiles whose border changes are redrawn (the pictures are Windows' own, untouched either way).
        if (_hover >= 0 && _hover < _tiles.Count)
            Invalidate(_tiles[_hover].Bounds);
        _hover = index;
        if (_hover >= 0)
            Invalidate(_tiles[_hover].Bounds);
        Cursor = _hover >= 0 ? Cursors.Hand : Cursors.Default;
    }

    protected override void OnMouseDown(MouseEventArgs e)
    {
        base.OnMouseDown(e);
        Focus(); // so the wheel scrolls the grid, not whatever had the keyboard
    }

    protected override void OnMouseClick(MouseEventArgs e)
    {
        base.OnMouseClick(e);
        int i = TileAt(e.Location);
        if (i >= 0 && (e.Button == MouseButtons.Left || e.Button == MouseButtons.Right))
            TileClicked?.Invoke(_tiles[i].Item, e.Button);
    }

    // ───────────────────────────── paint ─────────────────────────────

    static Color Mix(Color a, Color b, float amount) => Color.FromArgb(
        (int)(a.R + (b.R - a.R) * amount), (int)(a.G + (b.G - a.G) * amount), (int)(a.B + (b.B - a.B) * amount));

    static GraphicsPath Rounded(Rectangle r, int radius)
    {
        var path = new GraphicsPath();
        int d = radius * 2;
        path.AddArc(r.X, r.Y, d, d, 180, 90);
        path.AddArc(r.Right - d, r.Y, d, d, 270, 90);
        path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
        path.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
        path.CloseFigure();
        return path;
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        var g = e.Graphics;
        g.Clear(_theme.Background);
        if (_tiles.Count == 0)
        {
            TextRenderer.DrawText(g, EmptyText, Font, ClientRectangle, _theme.Text2,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.WordBreak | TextFormatFlags.NoPrefix);
            return;
        }
        int dpi = DeviceDpi, inset = Scale(Inset, dpi), radius = Scale(Radius, dpi), gap = Scale(8, dpi);
        using var surface = new SolidBrush(_theme.Surface);
        using var picture = new SolidBrush(_theme.Background);
        using var line = new Pen(_theme.Line);
        using var hot = new Pen(Mix(_theme.Line, _theme.Text, .35f));
        for (int i = 0; i < _tiles.Count; i++)
        {
            var tile = _tiles[i];
            var r = tile.Bounds;
            if (!r.IntersectsWith(e.ClipRectangle))
                continue;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            var frame = new Rectangle(r.X, r.Y, r.Width - 1, r.Height - 1);
            using (var path = Rounded(frame, radius))
            {
                g.FillPath(surface, path);
                g.DrawPath(i == _hover ? hot : line, path);
            }
            g.SmoothingMode = SmoothingMode.None;
            // The picture area in the page colour: the live picture covers it, or is letterboxed on it.
            if (tile.Picture.Width > 0 && tile.Picture.Height > 0)
                g.FillRectangle(picture, tile.Picture);

            var item = tile.Item;
            int y = r.Y + inset, left = r.X + inset, width = r.Width - inset * 2;
            Caption(g, item.Title, _titleFont, _theme.Text, item.Tag, left, y, width, gap);
            y += TextRenderer.MeasureText(g, "Ag", _titleFont).Height + Scale(2, dpi);
            Caption(g, item.Subtitle, Font, _theme.Text2, item.Tag2, left, y, width, gap);

            if (tile.Minimized || tile.Thumb == IntPtr.Zero)
            {
                // No live frame: minimised (worth a note), or it couldn't be registered (an empty tile).
                if (tile.Minimized && tile.Picture.Height > 0)
                    TextRenderer.DrawText(g, MinimizedText, Font, tile.Picture, _theme.Text2,
                        TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.WordBreak | TextFormatFlags.NoPrefix);
            }
        }
    }

    /// <summary>One caption line: text at the left, ending in "…" before the tag at the right.</summary>
    void Caption(Graphics g, string text, Font font, Color color, string tag, int x, int y, int width, int gap)
    {
        int h = TextRenderer.MeasureText(g, "Ag", font).Height;
        // Measured without the ellipsis flag: with it, an unbounded measure comes back a few pixels wide ("9…" for "900 MB").
        int tagW = string.IsNullOrEmpty(tag) ? 0 : TextRenderer.MeasureText(g, tag, _smallFont, Size.Empty, LineFlags & ~TextFormatFlags.EndEllipsis).Width;
        if (tagW > 0)
        {
            int tagH = TextRenderer.MeasureText(g, "Ag", _smallFont).Height;
            TextRenderer.DrawText(g, tag, _smallFont, new Rectangle(x + width - tagW, y + (h - tagH) / 2, tagW, tagH), _theme.Text2, LineFlags | TextFormatFlags.Right);
            width -= tagW + gap;
        }
        if (width > 0)
            TextRenderer.DrawText(g, text, font, new Rectangle(x, y, width, h), color, LineFlags | TextFormatFlags.Left);
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            UnregisterAll();
            _tiles.Clear();
            _titleFont.Dispose();
            _smallFont.Dispose();
        }
        base.Dispose(disposing);
    }
}
