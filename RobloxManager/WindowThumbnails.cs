using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Live pictures of other programs' windows as DWM thumbnails (what the taskbar shows on hover), each laid exactly in a
/// rectangle of a window of ours: Windows draws every frame straight into that window, above its page, so nothing is
/// copied or polled. The game monitor's page draws everything else (frames, names, buttons, menus) and says where each
/// picture goes (<see cref="Place"/>); this draws nothing itself. A minimised window has no live frame: its picture is
/// hidden, and <see cref="Changed"/> tells so the page can say it.
/// </summary>
/// <remarks>A thumbnail exists only while its window has a rectangle from the last <see cref="Place"/>; every other path
/// unregisters it, so none leaks or points at a dead window.</remarks>
sealed class WindowThumbnails : IDisposable
{
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

    /// <summary>One window's picture. Thumb is zero while no thumbnail is registered for it.</summary>
    sealed class Picture
    {
        public IntPtr Thumb;
        /// <summary>Where the page wants it, in the destination window's client pixels.</summary>
        public Rectangle Box;
        public bool Minimized;
        /// <summary>The window's size when its picture was last placed (empty while unknown).</summary>
        public Size Shown;
    }

    readonly Form _host;
    readonly Dictionary<IntPtr, Picture> _pictures = new();
    /// <summary>The part of the host the pictures may show in (the page's monitor area: a picture scrolled half out is cut).</summary>
    Rectangle _clip;

    /// <summary>
    /// A picture shows the part of its window that had the window's size when placed: a window that grew (Arrange, brought
    /// to the front) showed its top-left corner zoomed in until the next refresh (owner, 2026-11-02). So each window's
    /// size and minimising are looked at this often, and the pictures placed again at once.
    /// </summary>
    readonly System.Windows.Forms.Timer _watch = new() { Interval = 50 };

    /// <summary>A window was minimised or opened again: the page says so in its tile.</summary>
    public event Action? Changed;

    public WindowThumbnails(Form host)
    {
        _host = host;
        _watch.Tick += (_, _) => Follow();
    }

    /// <summary>A window has no live picture now (minimised or hidden), as of the last look.</summary>
    public bool IsMinimized(IntPtr hwnd) => _pictures.TryGetValue(hwnd, out var p) ? p.Minimized : IsIconic(hwnd) || !IsWindowVisible(hwnd);

    /// <summary>
    /// Each window's picture in its rectangle (the host's client pixels), cut to <paramref name="clip"/>; a window not
    /// given loses its picture (scrolled away, under a menu, closed).
    /// </summary>
    public void Place(IReadOnlyDictionary<IntPtr, Rectangle> boxes, Rectangle clip)
    {
        _clip = clip;
        foreach (var gone in _pictures.Keys.Where(h => !boxes.ContainsKey(h)).ToList())
        {
            Unregister(_pictures[gone]);
            _pictures.Remove(gone);
        }
        foreach (var (hwnd, box) in boxes)
        {
            if (!_pictures.TryGetValue(hwnd, out var p))
                _pictures[hwnd] = p = new Picture { Minimized = IsIconic(hwnd) || !IsWindowVisible(hwnd) };
            p.Box = box;
        }
        Update();
        _watch.Enabled = _pictures.Count > 0;
    }

    /// <summary>Every picture taken away (the page shows something else there).</summary>
    public void Clear()
    {
        _watch.Stop();
        foreach (var p in _pictures.Values)
            Unregister(p);
        _pictures.Clear();
    }

    /// <summary>The largest rectangle of the source's shape inside <paramref name="box"/>, centred (a 16:9 game in a square box is letterboxed).</summary>
    public static Rectangle Fit(Size source, Rectangle box)
    {
        if (source.Width <= 0 || source.Height <= 0 || box.Width <= 0 || box.Height <= 0)
            return box;
        double scale = Math.Min((double)box.Width / source.Width, (double)box.Height / source.Height);
        int w = Math.Max(1, (int)(source.Width * scale)), h = Math.Max(1, (int)(source.Height * scale));
        return new Rectangle(box.X + (box.Width - w) / 2, box.Y + (box.Height - h) / 2, w, h);
    }

    void Register(IntPtr hwnd, Picture p)
    {
        if (p.Thumb != IntPtr.Zero || !_host.IsHandleCreated || !IsWindow(hwnd))
            return;
        // Fails for a window that just closed or one of another desktop session: its tile then shows no picture.
        if (DwmRegisterThumbnail(_host.Handle, hwnd, out var thumb) != 0 || thumb == IntPtr.Zero)
            return;
        p.Thumb = thumb;
        // Until this is set the source size answers for the whole frame (title bar and borders), and the first
        // placing would fit the picture to the wrong shape.
        var props = new DWM_THUMBNAIL_PROPERTIES { dwFlags = DWM_TNP_SOURCECLIENTAREAONLY, fSourceClientAreaOnly = 1 };
        DwmUpdateThumbnailProperties(thumb, ref props);
    }

    static void Unregister(Picture p)
    {
        if (p.Thumb == IntPtr.Zero)
            return;
        // Fails harmlessly once the destination window is gone: Windows dropped the thumbnail with it.
        DwmUnregisterThumbnail(p.Thumb);
        p.Thumb = IntPtr.Zero;
    }

    /// <summary>Fits every picture into its box, hidden while its window is minimised or the box is out of the clip.</summary>
    void Update()
    {
        foreach (var (hwnd, p) in _pictures)
        {
            if (!IsWindow(hwnd))
            {
                // Gone since the page's last report: never keep a thumbnail of a dead window.
                Unregister(p);
                continue;
            }
            // A hidden window has no live frame either (Windows shows a grey placeholder for it).
            p.Minimized = IsIconic(hwnd) || !IsWindowVisible(hwnd);
            if (p.Thumb == IntPtr.Zero)
                Register(hwnd, p);
            if (p.Thumb == IntPtr.Zero)
                continue;
            var props = new DWM_THUMBNAIL_PROPERTIES
            {
                dwFlags = DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
                opacity = 255,
                fSourceClientAreaOnly = 1,
            };
            bool sized = DwmQueryThumbnailSourceSize(p.Thumb, out var size) == 0 && size.cx > 0 && size.cy > 0;
            var source = sized ? new Size(size.cx, size.cy) : new Size(16, 9);
            p.Shown = sized ? source : Size.Empty;
            var picture = Fit(source, p.Box);
            var visible = Rectangle.Intersect(picture, _clip);
            if (p.Minimized || p.Box.Width < 24 || p.Box.Height < 24 || visible.IsEmpty)
            {
                // Hidden, but still given a real rectangle: an empty one is refused as an argument.
                props.fVisible = 0;
                props.rcDestination = new RECT(new Rectangle(p.Box.X, p.Box.Y, 1, 1));
            }
            else
            {
                props.fVisible = 1;
                props.rcDestination = new RECT(visible);
                // The part of the window shown, given every time since Windows keeps the last one: a picture half scrolled
                // out shows the matching half, not a squashed whole. (Counted from the client area, as the picture is.)
                if (sized)
                {
                    props.dwFlags |= DWM_TNP_RECTSOURCE;
                    double sx = (double)source.Width / picture.Width, sy = (double)source.Height / picture.Height;
                    props.rcSource = visible.Size == picture.Size
                        ? new RECT(new Rectangle(0, 0, source.Width, source.Height))
                        : new RECT(new Rectangle(
                            (int)((visible.X - picture.X) * sx), (int)((visible.Y - picture.Y) * sy),
                            Math.Max(1, (int)(visible.Width * sx)), Math.Max(1, (int)(visible.Height * sy))));
                }
            }
            if (DwmUpdateThumbnailProperties(p.Thumb, ref props) != 0)
                Unregister(p); // the window went away between the two calls; registered again next time if not
        }
    }

    /// <summary>A window resized, minimised or opened again since its picture was placed: placed again now.</summary>
    void Follow()
    {
        bool moved = false, minimized = false;
        foreach (var (hwnd, p) in _pictures)
        {
            if (!IsWindow(hwnd))
                continue;
            bool min = IsIconic(hwnd) || !IsWindowVisible(hwnd);
            if (min != p.Minimized)
            {
                minimized = true;
                continue;
            }
            if (min || p.Thumb == IntPtr.Zero)
                continue;
            var size = DwmQueryThumbnailSourceSize(p.Thumb, out var s) == 0 && s.cx > 0 && s.cy > 0 ? new Size(s.cx, s.cy) : Size.Empty;
            if (size != p.Shown)
                moved = true;
        }
        if (moved || minimized)
            Update();
        if (minimized)
            Changed?.Invoke();
    }

    public void Dispose()
    {
        _watch.Dispose();
        Clear();
    }
}
