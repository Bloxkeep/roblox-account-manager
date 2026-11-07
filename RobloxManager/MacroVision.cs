using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// What a macro sees in a window's picture: a cut-out image found again (normalised cross-correlation on grey levels,
/// a coarse pass over a shrunk picture, then the best few places looked at closely), a colour at a point, and whether
/// the picture has stopped changing. Text is <see cref="MacroOcr"/>'s. Pictures are the window's client area at full size.
/// </summary>
static class MacroVision
{
    /// <summary>A grey picture: one byte a pixel, row after row.</summary>
    internal sealed class Gray(int w, int h, byte[] pixels)
    {
        public int W { get; } = w;
        public int H { get; } = h;
        public byte[] P { get; } = pixels;
        public byte this[int x, int y] => P[y * W + x];
    }

    /// <summary>The picture in grey; <paramref name="area"/> (pixels) cuts it first.</summary>
    public static Gray ToGray(Bitmap bitmap, Rectangle? area = null)
    {
        var r = Rectangle.Intersect(area ?? new Rectangle(0, 0, bitmap.Width, bitmap.Height), new Rectangle(0, 0, bitmap.Width, bitmap.Height));
        if (r.Width <= 0 || r.Height <= 0)
            return new Gray(0, 0, []);
        var data = bitmap.LockBits(r, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        try
        {
            var row = new byte[r.Width * 4];
            var pixels = new byte[r.Width * r.Height];
            for (int y = 0; y < r.Height; y++)
            {
                Marshal.Copy(data.Scan0 + y * data.Stride, row, 0, row.Length);
                for (int x = 0, o = y * r.Width; x < r.Width; x++)
                    pixels[o + x] = (byte)((row[x * 4 + 2] * 77 + row[x * 4 + 1] * 150 + row[x * 4] * 29) >> 8);
            }
            return new Gray(r.Width, r.Height, pixels);
        }
        finally
        {
            bitmap.UnlockBits(data);
        }
    }

    /// <summary>An area (0..1 of the client) in pixels of a <paramref name="w"/> × <paramref name="h"/> picture; all of it when null.</summary>
    public static Rectangle Area(MacroRect? r, int w, int h)
    {
        if (r is null)
            return new Rectangle(0, 0, w, h);
        int x = Math.Clamp((int)Math.Floor(r.X * w), 0, w), y = Math.Clamp((int)Math.Floor(r.Y * h), 0, h);
        int right = Math.Clamp((int)Math.Ceiling((r.X + r.W) * w), x, w), bottom = Math.Clamp((int)Math.Ceiling((r.Y + r.H) * h), y, h);
        return Rectangle.FromLTRB(x, y, right, bottom);
    }

    /// <summary>Pixels back to 0..1 of a <paramref name="w"/> × <paramref name="h"/> client.</summary>
    public static MacroRect Fraction(Rectangle r, int w, int h) =>
        new() { X = (double)r.X / w, Y = (double)r.Y / h, W = (double)r.Width / w, H = (double)r.Height / h };

    /// <summary>
    /// Where <paramref name="template"/> best matches in <paramref name="area"/> of <paramref name="picture"/>, and how well
    /// (1 = the same, under 0.5 = not there); null when it doesn't fit in the area.
    /// </summary>
    public static (double Score, Rectangle Found)? Find(Gray picture, Gray template, Rectangle area)
    {
        area = Rectangle.Intersect(area, new Rectangle(0, 0, picture.W, picture.H));
        if (template.W < 1 || template.H < 1 || area.Width < template.W || area.Height < template.H)
            return null;
        // Coarse pass: shrunk so the template is about 12 px on its short side, and the work stays bounded.
        int step = Math.Max(1, Math.Min(template.W, template.H) / 12);
        while (step < 16 && (double)(area.Width / step) * (area.Height / step) * (template.W / step) * (template.H / step) > 30e6)
            step++;
        var cut = Crop(picture, area);
        // The picture and template at each size looked at, the coarsest first, halving down to full size.
        var levels = new Dictionary<int, (Gray Hay, Gray Tpl)>();
        (Gray Hay, Gray Tpl) Level(int s)
        {
            if (!levels.TryGetValue(s, out var level))
                levels[s] = level = (Shrink(cut, s), Shrink(template, s));
            return level;
        }
        var candidates = new List<(int X, int Y)>();
        var (hay, tpl) = Level(step);
        if (hay.W >= tpl.W && hay.H >= tpl.H && tpl.W > 0 && tpl.H > 0)
        {
            var scores = Correlate(hay, tpl, new Rectangle(0, 0, hay.W - tpl.W + 1, hay.H - tpl.H + 1));
            int keep = 6, apart = Math.Max(1, Math.Min(tpl.W, tpl.H) / 2);
            foreach (var (_, x, y) in scores.OrderByDescending(s => s.Score))
            {
                if (candidates.Count >= keep)
                    break;
                if (candidates.All(c => Math.Abs(c.X - x) >= apart || Math.Abs(c.Y - y) >= apart))
                    candidates.Add((x, y));
            }
        }
        else
        {
            step = 1;
            candidates.Add((0, 0));
        }
        // Each followed down the sizes, looking a little around where it was at the size before.
        double best = double.NegativeInfinity;
        Point at = default;
        foreach (var (cx, cy) in candidates)
        {
            int x = cx, y = cy, s = step;
            double score = double.NegativeInfinity;
            do
            {
                int next = s == step && step == 1 ? 1 : Math.Max(1, s / 2);
                var (h, t) = Level(next);
                int reach = next == 1 && s == 1 ? 3 : 2;
                int x0 = x * s / next, y0 = y * s / next;
                var near = Rectangle.Intersect(Rectangle.FromLTRB(x0 - reach, y0 - reach, x0 + reach + 1, y0 + reach + 1),
                    new Rectangle(0, 0, h.W - t.W + 1, h.H - t.H + 1));
                score = double.NegativeInfinity;
                foreach (var (sc, px, py) in Correlate(h, t, near))
                {
                    if (sc > score)
                        (score, x, y) = (sc, px, py);
                }
                s = next;
            }
            while (s > 1);
            if (score > best)
                (best, at) = (score, new Point(area.X + x, area.Y + y));
        }
        return double.IsNegativeInfinity(best) ? null : (Math.Clamp(best, -1, 1), new Rectangle(at, new Size(template.W, template.H)));
    }

    /// <summary>Normalised cross-correlation of <paramref name="t"/> at each top-left in <paramref name="places"/>. A flat
    /// template (one colour) is scored by how close and how flat the picture is there instead.</summary>
    static List<(double Score, int X, int Y)> Correlate(Gray p, Gray t, Rectangle places)
    {
        var result = new List<(double, int, int)>(Math.Max(0, places.Width * places.Height));
        int n = t.W * t.H;
        long sumT = 0, sumT2 = 0;
        foreach (byte v in t.P)
        {
            sumT += v;
            sumT2 += v * v;
        }
        double meanT = (double)sumT / n, varT = sumT2 - (double)sumT * sumT / n;
        bool flat = varT < n * 4.0; // under 2 grey levels of spread
        for (int y = places.Top; y < places.Bottom; y++)
        {
            for (int x = places.Left; x < places.Right; x++)
            {
                long sumI = 0, sumI2 = 0, cross = 0;
                for (int ty = 0; ty < t.H; ty++)
                {
                    int po = (y + ty) * p.W + x, to = ty * t.W;
                    for (int tx = 0; tx < t.W; tx++)
                    {
                        int i = p.P[po + tx];
                        sumI += i;
                        sumI2 += i * i;
                        cross += i * t.P[to + tx];
                    }
                }
                double varI = sumI2 - (double)sumI * sumI / n;
                double score;
                if (flat)
                    score = 1 - (Math.Abs((double)sumI / n - meanT) + Math.Sqrt(Math.Max(0, varI) / n)) / 255;
                else if (varI < n * 4.0)
                    score = 0;
                else
                    score = (cross - (double)sumI * meanT) / Math.Sqrt(varI * varT);
                result.Add((score, x, y));
            }
        }
        return result;
    }

    internal static Gray Crop(Gray g, Rectangle r)
    {
        r = Rectangle.Intersect(r, new Rectangle(0, 0, g.W, g.H));
        if (r.X == 0 && r.Y == 0 && r.Width == g.W && r.Height == g.H)
            return g;
        var pixels = new byte[r.Width * r.Height];
        for (int y = 0; y < r.Height; y++)
            Array.Copy(g.P, (r.Y + y) * g.W + r.X, pixels, y * r.Width, r.Width);
        return new Gray(r.Width, r.Height, pixels);
    }

    /// <summary>Each <paramref name="step"/> × <paramref name="step"/> block averaged into one pixel.</summary>
    internal static Gray Shrink(Gray g, int step)
    {
        if (step <= 1)
            return g;
        int w = g.W / step, h = g.H / step;
        var pixels = new byte[Math.Max(0, w * h)];
        int area = step * step;
        for (int y = 0; y < h; y++)
        {
            for (int x = 0; x < w; x++)
            {
                int sum = 0;
                for (int dy = 0; dy < step; dy++)
                {
                    int o = (y * step + dy) * g.W + x * step;
                    for (int dx = 0; dx < step; dx++)
                        sum += g.P[o + dx];
                }
                pixels[y * w + x] = (byte)(sum / area);
            }
        }
        return new Gray(w, h, pixels);
    }

    /// <summary>Bilinear, to any size: a template cut from a window of another size, scaled to this one.</summary>
    internal static Gray Resize(Gray g, int w, int h)
    {
        if (w == g.W && h == g.H)
            return g;
        w = Math.Max(1, w);
        h = Math.Max(1, h);
        var pixels = new byte[w * h];
        for (int y = 0; y < h; y++)
        {
            double sy = Math.Clamp((y + 0.5) * g.H / h - 0.5, 0, g.H - 1);
            int y0 = (int)sy, y1 = Math.Min(y0 + 1, g.H - 1);
            double fy = sy - y0;
            for (int x = 0; x < w; x++)
            {
                double sx = Math.Clamp((x + 0.5) * g.W / w - 0.5, 0, g.W - 1);
                int x0 = (int)sx, x1 = Math.Min(x0 + 1, g.W - 1);
                double fx = sx - x0;
                double top = g[x0, y0] * (1 - fx) + g[x1, y0] * fx, bottom = g[x0, y1] * (1 - fx) + g[x1, y1] * fx;
                pixels[y * w + x] = (byte)Math.Round(top * (1 - fy) + bottom * fy);
            }
        }
        return new Gray(w, h, pixels);
    }

    static readonly Dictionary<string, Gray> Templates = new();
    /// <summary>error.log: the length of the last picture that couldn't be read.</summary>
    static int _badTemplate = -1;

    /// <summary>A step's picture (PNG, base64) in grey, scaled by <paramref name="scale"/> (the window's width over the
    /// one it was cut from); null when it can't be read.</summary>
    public static Gray? Template(string base64, double scale)
    {
        Gray? gray;
        lock (Templates)
            Templates.TryGetValue(base64, out gray);
        if (gray is null)
        {
            try
            {
                using var stream = new MemoryStream(Convert.FromBase64String(base64));
                using var bitmap = new Bitmap(stream);
                gray = ToGray(bitmap);
            }
            catch (Exception ex) when (ex is FormatException or ArgumentException or ExternalException)
            {
                // Not cached, so looked at again each look: written once per picture in a row.
                if (Interlocked.Exchange(ref _badTemplate, base64.Length) != base64.Length)
                    Log.Note($"Macro: a step's picture ({base64.Length} characters) can't be read ({ex.GetType().Name}); it is never found.");
                return null;
            }
            lock (Templates)
            {
                if (Templates.Count > 64)
                    Templates.Clear();
                Templates[base64] = gray;
            }
        }
        // A few percent off is left as it is: resampling blurs more than it helps.
        return Math.Abs(scale - 1) < 0.03 ? gray : Resize(gray, (int)Math.Round(gray.W * scale), (int)Math.Round(gray.H * scale));
    }

    /// <summary>The colour at a point (0..1 of the picture) is within <paramref name="tolerance"/> of <paramref name="colour"/>
    /// ("#rrggbb") on every channel.</summary>
    public static bool ColourAt(Bitmap picture, double x, double y, string colour, int tolerance)
    {
        var c = picture.GetPixel(Math.Clamp((int)(x * picture.Width), 0, picture.Width - 1), Math.Clamp((int)(y * picture.Height), 0, picture.Height - 1));
        int want = Convert.ToInt32(colour[1..], 16);
        return Math.Abs(c.R - (want >> 16 & 0xFF)) <= tolerance && Math.Abs(c.G - (want >> 8 & 0xFF)) <= tolerance && Math.Abs(c.B - (want & 0xFF)) <= tolerance;
    }

    /// <summary>A small grey copy of an area, to tell whether the picture is still changing.</summary>
    public static Gray Signature(Gray picture, Rectangle area)
    {
        var cut = Crop(picture, area);
        int step = Math.Max(1, cut.W / 64);
        return Shrink(cut, step);
    }

    /// <summary>The mean difference per pixel of two signatures (0..255); 255 when their sizes differ (the window changed size).</summary>
    public static double Difference(Gray a, Gray b)
    {
        if (a.W != b.W || a.H != b.H || a.P.Length == 0)
            return 255;
        long sum = 0;
        for (int i = 0; i < a.P.Length; i++)
            sum += Math.Abs(a.P[i] - b.P[i]);
        return (double)sum / a.P.Length;
    }

    /// <summary>A signature this close to the last one counts as unchanged (a blinking cursor, video noise).</summary>
    public const double StillDifference = 1.5;
}
