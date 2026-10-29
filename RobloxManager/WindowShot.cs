using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// A picture of a Roblox window's game, for /pc screenshot (AppController.Remote.cs). A window in view, or behind others,
/// is pictured as it is. A minimized one, or one Bloxkeep hid, has nothing to picture: it is shown for a moment the way
/// anti-AFK shows it (all but invisible, click-through, not given the keyboard: <see cref="AntiAfk.Hide"/>), given time
/// to draw, pictured, then minimized or hidden again. Only the game is pictured, not the title bar (an account's name).
/// Tried on the owner's PC (2026-10-29): a minimized window came out whole, in about half a second, nothing seen.
/// </summary>
static class WindowShot
{
    /// <summary>How long a window shown again is given to draw (as anti-AFK gives a minimized one).</summary>
    static readonly TimeSpan DrawTime = TimeSpan.FromMilliseconds(400);

    /// <summary>
    /// How long a picture may take: a Roblox that stops answering meanwhile is given up on (it would hold every /pc
    /// command after it, which are answered one by one).
    /// </summary>
    static readonly TimeSpan PictureTime = TimeSpan.FromSeconds(5);

    const int SW_HIDE = 0, SW_SHOWMINNOACTIVE = 7, SW_SHOWNOACTIVATE = 4;
    // The client area only, and what DirectX draws too (without it, a game's window comes out black).
    const uint PW_CLIENTONLY = 1, PW_RENDERFULLCONTENT = 2;

    [StructLayout(LayoutKind.Sequential)]
    struct RECT
    {
        public int Left, Top, Right, Bottom;
    }

    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsHungAppWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool ShowWindowAsync(IntPtr hWnd, int command);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);

    /// <summary>
    /// The window's game as a JPEG no wider than <paramref name="maxWidth"/>; null when it can't be pictured (gone, not
    /// answering, no size). <paramref name="black"/> says the picture came out all but black (Windows locked, say).
    /// </summary>
    public static async Task<(byte[] Jpeg, bool Black)?> CaptureAsync(IntPtr hWnd, int maxWidth)
    {
        // A window that isn't answering would hold whoever asks it to draw.
        if (hWnd == IntPtr.Zero || !IsWindow(hWnd) || IsHungAppWindow(hWnd))
            return null;
        bool minimized = IsIconic(hWnd), unshown = !IsWindowVisible(hWnd);
        if (!minimized && !unshown)
            return await Timed(hWnd, maxWidth);
        var before = AntiAfk.Hide(hWnd, (IntPtr.Zero, 0), minimized);
        try
        {
            ShowWindowAsync(hWnd, SW_SHOWNOACTIVATE);
            await Task.Delay(DrawTime);
            return IsHungAppWindow(hWnd) ? null : await Timed(hWnd, maxWidth);
        }
        finally
        {
            if (minimized)
                ShowWindowAsync(hWnd, SW_SHOWMINNOACTIVE);
            if (unshown)
                ShowWindowAsync(hWnd, SW_HIDE);
            // Seen again only once it is minimized or hidden again (it would flash up otherwise).
            for (int i = 0; i < 10 && IsWindow(hWnd) && (minimized && !IsIconic(hWnd) || unshown && IsWindowVisible(hWnd)); i++)
                await Task.Delay(50);
            AntiAfk.Show(before);
        }
    }

    /// <summary>The picture, off the window's thread, or null after <see cref="PictureTime"/> (left to finish on its own).</summary>
    static async Task<(byte[] Jpeg, bool Black)?> Timed(IntPtr hWnd, int maxWidth)
    {
        var picture = Task.Run(() => Picture(hWnd, maxWidth));
        return await Task.WhenAny(picture, Task.Delay(PictureTime)) == picture ? await picture : null;
    }

    static (byte[] Jpeg, bool Black)? Picture(IntPtr hWnd, int maxWidth)
    {
        if (!GetClientRect(hWnd, out var r) || r.Right - r.Left < 16 || r.Bottom - r.Top < 16)
            return null;
        int width = r.Right - r.Left, height = r.Bottom - r.Top;
        using var full = new Bitmap(width, height, PixelFormat.Format32bppRgb);
        using (var g = Graphics.FromImage(full))
        {
            IntPtr hdc = g.GetHdc();
            try
            {
                if (!PrintWindow(hWnd, hdc, PW_CLIENTONLY | PW_RENDERFULLCONTENT))
                    return null;
            }
            finally
            {
                g.ReleaseHdc(hdc);
            }
        }
        bool black = AllButBlack(full);
        double scale = Math.Min(1, (double)maxWidth / width);
        using var shown = scale < 1 ? Shrink(full, (int)Math.Round(width * scale), (int)Math.Round(height * scale)) : null;
        return (Jpeg(shown ?? full), black);
    }

    static Bitmap Shrink(Bitmap source, int width, int height)
    {
        var small = new Bitmap(width, height, PixelFormat.Format24bppRgb);
        using var g = Graphics.FromImage(small);
        g.InterpolationMode = InterpolationMode.HighQualityBicubic;
        g.PixelOffsetMode = PixelOffsetMode.HighQuality;
        g.DrawImage(source, 0, 0, width, height);
        return small;
    }

    static byte[] Jpeg(Bitmap picture)
    {
        var codec = ImageCodecInfo.GetImageEncoders().First(e => e.FormatID == ImageFormat.Jpeg.Guid);
        using var quality = new EncoderParameters(1);
        quality.Param[0] = new EncoderParameter(System.Drawing.Imaging.Encoder.Quality, 80L);
        using var stream = new MemoryStream();
        picture.Save(stream, codec, quality);
        return stream.ToArray();
    }

    /// <summary>Nearly every one of 1,600 points across the picture is black.</summary>
    internal static bool AllButBlack(Bitmap picture)
    {
        int lit = 0, seen = 0;
        for (int i = 0; i < 40; i++)
        {
            for (int j = 0; j < 40; j++)
            {
                var c = picture.GetPixel((int)((i + 0.5) * picture.Width / 40), (int)((j + 0.5) * picture.Height / 40));
                seen++;
                if (c.R + c.G + c.B > 30)
                    lit++;
            }
        }
        return lit * 50 < seen;
    }
}
