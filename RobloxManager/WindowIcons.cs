using System.Drawing;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Each account's Roblox window carries the account's picture as its icon (the taskbar, Alt+Tab, the title bar), so the
/// windows are told apart at a glance; the account's initials on its colour when it has no picture (yet). Roblox keeps
/// an icon set on its window (seen on 0.743); a window that drops it gets it again at the next tick. Given back
/// (Roblox's own icon) when the setting is turned off. UI thread only.
/// </summary>
static class WindowIcons
{
    [DllImport("user32.dll")]
    static extern bool SendMessageTimeout(IntPtr hWnd, int msg, IntPtr wParam, IntPtr lParam, int flags, int timeout, out IntPtr result);
    [DllImport("user32.dll")]
    static extern bool DestroyIcon(IntPtr icon);
    const int WM_GETICON = 0x7F, WM_SETICON = 0x80, ICON_SMALL = 0, ICON_BIG = 1, SMTO_ABORTIFHUNG = 2;
    static readonly IntPtr ICON_SMALL_W = 0, ICON_BIG_W = 1;

    /// <summary>The icons made for a client, and each of its windows' own icons from before (put back when they go).</summary>
    sealed record Made(IntPtr Big, IntPtr Small, string Key, Dictionary<IntPtr, (IntPtr Big, IntPtr Small)> Own);
    static readonly Dictionary<int, Made> ByPid = new();

    /// <param name="key">What the picture shows (the picture's address, or the initials and colour): drawn again when it changes.</param>
    public static void Apply(int pid, IEnumerable<IntPtr> windows, string key, Func<Bitmap> draw)
    {
        if (!ByPid.TryGetValue(pid, out var made) || made.Key != key)
        {
            using var picture = draw();
            var next = new Made(IconOf(picture, 48), IconOf(picture, 24), key, made?.Own ?? new());
            foreach (var hWnd in windows)
                Put(hWnd, next);
            if (made is not null)
                Destroy(made);
            ByPid[pid] = next;
            return;
        }
        foreach (var hWnd in windows)
        {
            if (Get(hWnd, ICON_BIG) != made.Big)
                Put(hWnd, made);
        }
    }

    /// <summary>The windows' own icons back (the setting turned off, Bloxkeep closing).</summary>
    public static void Remove(int pid, IEnumerable<IntPtr> windows)
    {
        if (!ByPid.Remove(pid, out var made))
            return;
        foreach (var hWnd in windows)
        {
            var own = made.Own.GetValueOrDefault(hWnd);
            Set(hWnd, own.Big, own.Small);
        }
        Destroy(made);
    }

    /// <summary>The icons of clients that have closed go.</summary>
    public static void Forget(IReadOnlySet<int> alive)
    {
        foreach (int pid in ByPid.Keys.Where(pid => !alive.Contains(pid)).ToList())
        {
            Destroy(ByPid[pid]);
            ByPid.Remove(pid);
        }
    }

    public static IEnumerable<int> Pids => ByPid.Keys.ToList();

    /// <summary>Puts the made icons on a window, noting first the icons it had of its own (once: later it has ours).</summary>
    static void Put(IntPtr hWnd, Made made)
    {
        if (!made.Own.ContainsKey(hWnd))
            made.Own[hWnd] = (Get(hWnd, ICON_BIG), Get(hWnd, ICON_SMALL));
        Set(hWnd, made.Big, made.Small);
    }

    static void Set(IntPtr hWnd, IntPtr big, IntPtr small)
    {
        SendMessageTimeout(hWnd, WM_SETICON, ICON_BIG_W, big, SMTO_ABORTIFHUNG, 300, out _);
        SendMessageTimeout(hWnd, WM_SETICON, ICON_SMALL_W, small, SMTO_ABORTIFHUNG, 300, out _);
    }

    static IntPtr Get(IntPtr hWnd, int which) =>
        SendMessageTimeout(hWnd, WM_GETICON, (IntPtr)which, IntPtr.Zero, SMTO_ABORTIFHUNG, 300, out IntPtr icon) ? icon : IntPtr.Zero;

    static void Destroy(Made made)
    {
        DestroyIcon(made.Big);
        DestroyIcon(made.Small);
    }

    /// <summary>The picture in a circle, as the app draws avatars, at an icon's size.</summary>
    static IntPtr IconOf(Bitmap picture, int size)
    {
        using var bitmap = new Bitmap(size, size, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
        using (var g = Graphics.FromImage(bitmap))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.InterpolationMode = InterpolationMode.HighQualityBicubic;
            g.PixelOffsetMode = PixelOffsetMode.HighQuality;
            using var circle = new GraphicsPath();
            circle.AddEllipse(0, 0, size - 1, size - 1);
            g.SetClip(circle);
            g.DrawImage(picture, new Rectangle(0, 0, size, size));
        }
        return bitmap.GetHicon();
    }

    /// <summary>An account without a picture: up to two letters of its name on its own colour (the page's hue).</summary>
    public static Bitmap Initials(string name)
    {
        string letters = new string(name.Where(char.IsLetterOrDigit).Take(2).ToArray()).ToUpperInvariant();
        if (letters.Length == 0)
            letters = "?";
        var bitmap = new Bitmap(96, 96);
        using var g = Graphics.FromImage(bitmap);
        g.SmoothingMode = SmoothingMode.AntiAlias;
        g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;
        g.Clear(FromHue(Hue(name), 0.45, 0.42));
        using var font = new Font("Segoe UI Semibold", letters.Length > 1 ? 34 : 40, FontStyle.Regular, GraphicsUnit.Pixel);
        using var brush = new SolidBrush(Color.White);
        var size = g.MeasureString(letters, font);
        g.DrawString(letters, font, brush, (96 - size.Width) / 2, (96 - size.Height) / 2);
        return bitmap;
    }

    /// <summary>The same hue the page gives a name (app.js hue).</summary>
    static int Hue(string s)
    {
        uint h = 7;
        foreach (char c in s)
            h = unchecked(h * 31 + c);
        return (int)(h % 360);
    }

    static Color FromHue(int hue, double s, double l)
    {
        double c = (1 - Math.Abs(2 * l - 1)) * s, x = c * (1 - Math.Abs(hue / 60.0 % 2 - 1)), m = l - c / 2;
        (double r, double g, double b) = (hue / 60) switch { 0 => (c, x, 0d), 1 => (x, c, 0d), 2 => (0d, c, x), 3 => (0d, x, c), 4 => (x, 0d, c), _ => (c, 0d, x) };
        return Color.FromArgb((int)((r + m) * 255), (int)((g + m) * 255), (int)((b + m) * 255));
    }
}
