using System.Diagnostics;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// The main window's entrance on start (owner, 2026-11-08: the window itself should come onto the screen with an
/// effect, not just appear). A see-through stand-in, drawn as the window will look (its colour, corners, start logo),
/// grows from the middle to its full size with a little overshoot while fading in. Then the real window, waiting
/// invisible just above it, fades in over it (only its title text and buttons differ) and the stand-in goes
/// (ShellForm.OpenWindowAsync). A stand-in because a real window can fade but not scale, and Windows' own opening
/// animation is off on many PCs (the owner's too). Clicks go through it to the real window.
/// </summary>
sealed class WindowOpening : NativeWindow, IDisposable
{
    public const double GrowMs = 420;
    const double From = .86;

    readonly Bitmap _plate;
    /// <summary>The window's frame with room around it for the overshoot (which was cut off at the window's edges).</summary>
    readonly Rectangle _frame;
    readonly IntPtr _dc, _dib, _old;
    readonly Bitmap _canvas;

    /// <param name="frame">Where the window shows on screen, without its unseen borders (device pixels).</param>
    /// <param name="plate">The window as it will look at full size (<see cref="Plate"/>); the caller keeps it.</param>
    public WindowOpening(Rectangle frame, Bitmap plate)
    {
        frame.Inflate((int)Math.Ceiling(frame.Width * .01) + 2, (int)Math.Ceiling(frame.Height * .01) + 2);
        _frame = frame;
        _plate = plate;
        var header = new BITMAPINFOHEADER
        {
            biSize = Marshal.SizeOf<BITMAPINFOHEADER>(), biWidth = frame.Width, biHeight = -frame.Height, // top down
            biPlanes = 1, biBitCount = 32,
        };
        IntPtr screen = GetDC(IntPtr.Zero);
        try
        {
            _dc = CreateCompatibleDC(screen);
            _dib = CreateDIBSection(screen, ref header, 0, out IntPtr bits, IntPtr.Zero, 0);
            if (_dc == IntPtr.Zero || _dib == IntPtr.Zero)
                throw new InvalidOperationException("No drawing surface for the opening.");
            _old = SelectObject(_dc, _dib);
            // GDI+ draws straight into the surface Windows shows (premultiplied, as UpdateLayeredWindow wants).
            _canvas = new Bitmap(frame.Width, frame.Height, frame.Width * 4, PixelFormat.Format32bppPArgb, bits);
        }
        finally
        {
            ReleaseDC(IntPtr.Zero, screen);
        }
        const int WS_POPUP = unchecked((int)0x80000000);
        const int WS_EX_LAYERED = 0x80000, WS_EX_TRANSPARENT = 0x20, WS_EX_TOOLWINDOW = 0x80, WS_EX_NOACTIVATE = 0x8000000;
        CreateHandle(new CreateParams
        {
            Style = WS_POPUP,
            ExStyle = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            X = frame.X, Y = frame.Y, Width = frame.Width, Height = frame.Height,
        });
    }

    /// <summary>The window as it will look: its colour, rounded corners and hairline (when <paramref name="radius"/>
    /// is above 0), and what <paramref name="drawClient"/> puts in the client area (relative to the frame).</summary>
    public static Bitmap Plate(Size size, Rectangle client, Color back, Color line, int radius, Action<Graphics, Rectangle> drawClient)
    {
        var plate = new Bitmap(size.Width, size.Height, PixelFormat.Format32bppPArgb);
        using var g = Graphics.FromImage(plate);
        g.SmoothingMode = SmoothingMode.AntiAlias;
        using var path = Rounded(new RectangleF(.5f, .5f, size.Width - 1, size.Height - 1), radius);
        using (var fill = new SolidBrush(back))
            g.FillPath(fill, path);
        if (radius > 0)
        {
            using var pen = new Pen(line);
            g.DrawPath(pen, path);
        }
        g.SmoothingMode = SmoothingMode.None;
        drawClient(g, client);
        return plate;
    }

    static GraphicsPath Rounded(RectangleF r, float radius)
    {
        var path = new GraphicsPath();
        if (radius <= 0)
        {
            path.AddRectangle(r);
            return path;
        }
        float d = radius * 2;
        path.AddArc(r.Left, r.Top, d, d, 180, 90);
        path.AddArc(r.Right - d, r.Top, d, d, 270, 90);
        path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
        path.AddArc(r.Left, r.Bottom - d, d, d, 90, 90);
        path.CloseFigure();
        return path;
    }

    /// <summary>Size (1 = full) and opacity (0–1) at <paramref name="ms"/> into the opening.</summary>
    public static (double Scale, double Alpha) Pose(double ms)
    {
        double t = Math.Clamp(ms / GrowMs, 0, 1);
        // Grows past its size a touch and settles back (an ease-out with overshoot); solid by 60 % of the way.
        const double c = 1.5;
        double back = 1 + (c + 1) * Math.Pow(t - 1, 3) + c * Math.Pow(t - 1, 2);
        double fade = Math.Min(1, t / .6);
        return (From + (1 - From) * back, 1 - Math.Pow(1 - fade, 3));
    }

    /// <summary>Draws the plate at <paramref name="scale"/> around the middle of <paramref name="target"/>.</summary>
    public static void Frame(Graphics g, Bitmap plate, Size target, double scale)
    {
        g.CompositingMode = CompositingMode.SourceCopy;
        g.Clear(Color.Transparent);
        g.InterpolationMode = InterpolationMode.Bilinear;
        g.PixelOffsetMode = PixelOffsetMode.Half;
        float w = (float)(plate.Width * scale), h = (float)(plate.Height * scale);
        g.DrawImage(plate, (target.Width - w) / 2f, (target.Height - h) / 2f, w, h);
    }

    /// <summary>Shows the stand-in just under <paramref name="window"/> and plays the opening; done at full size.</summary>
    public Task GrowAsync(IntPtr window)
    {
        var done = new TaskCompletionSource();
        var clock = Stopwatch.StartNew();
        var timer = new System.Windows.Forms.Timer { Interval = 1 };
        bool shown = false;
        void Step()
        {
            double ms = clock.Elapsed.TotalMilliseconds;
            var (scale, alpha) = Pose(ms);
            using (var g = Graphics.FromImage(_canvas))
                Frame(g, _plate, _frame.Size, scale);
            Show((byte)Math.Round(alpha * 255));
            if (!shown)
            {
                const int SW_SHOWNOACTIVATE = 4;
                const uint SWP_NOSIZE = 0x1, SWP_NOMOVE = 0x2, SWP_NOACTIVATE = 0x10;
                ShowWindow(Handle, SW_SHOWNOACTIVATE);
                SetWindowPos(Handle, window, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE); // under the window
                shown = true;
            }
            if (ms >= GrowMs)
            {
                timer.Dispose();
                done.TrySetResult();
            }
        }
        timer.Tick += (_, _) =>
        {
            try
            {
                Step();
            }
            catch (Exception ex)
            {
                timer.Dispose();
                done.TrySetException(ex);
            }
        };
        timer.Start();
        Step();
        return done.Task;
    }

    void Show(byte alpha)
    {
        const int ULW_ALPHA = 2;
        var at = _frame.Location;
        var size = _frame.Size;
        var from = Point.Empty;
        var blend = new BLENDFUNCTION { BlendOp = 0, SourceConstantAlpha = alpha, AlphaFormat = 1 }; // AC_SRC_OVER, AC_SRC_ALPHA
        UpdateLayeredWindow(Handle, IntPtr.Zero, ref at, ref size, _dc, ref from, 0, ref blend, ULW_ALPHA);
    }

    public void Dispose()
    {
        DestroyHandle();
        _canvas.Dispose();
        SelectObject(_dc, _old);
        DeleteObject(_dib);
        DeleteDC(_dc);
    }

    [StructLayout(LayoutKind.Sequential)]
    struct BITMAPINFOHEADER
    {
        public int biSize, biWidth, biHeight;
        public short biPlanes, biBitCount;
        public int biCompression, biSizeImage, biXPelsPerMeter, biYPelsPerMeter, biClrUsed, biClrImportant;
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    struct BLENDFUNCTION
    {
        public byte BlendOp, BlendFlags, SourceConstantAlpha, AlphaFormat;
    }

    [DllImport("user32.dll")]
    static extern bool UpdateLayeredWindow(IntPtr hwnd, IntPtr hdcDst, ref Point pptDst, ref Size psize, IntPtr hdcSrc,
        ref Point pptSrc, int crKey, ref BLENDFUNCTION pblend, int dwFlags);
    [DllImport("user32.dll")]
    static extern bool ShowWindow(IntPtr hwnd, int cmd);
    [DllImport("user32.dll")]
    static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")]
    static extern IntPtr GetDC(IntPtr hwnd);
    [DllImport("user32.dll")]
    static extern int ReleaseDC(IntPtr hwnd, IntPtr dc);
    [DllImport("gdi32.dll")]
    static extern IntPtr CreateCompatibleDC(IntPtr dc);
    [DllImport("gdi32.dll")]
    static extern IntPtr CreateDIBSection(IntPtr dc, ref BITMAPINFOHEADER header, uint usage, out IntPtr bits, IntPtr section, uint offset);
    [DllImport("gdi32.dll")]
    static extern IntPtr SelectObject(IntPtr dc, IntPtr obj);
    [DllImport("gdi32.dll")]
    static extern bool DeleteObject(IntPtr obj);
    [DllImport("gdi32.dll")]
    static extern bool DeleteDC(IntPtr dc);
}
