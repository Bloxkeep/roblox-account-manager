using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Drawing.Text;

namespace RobloxManager;

/// <summary>What a play-time summary's picture shows (drawn by <see cref="SummaryCard"/>); every text in Discord's language.</summary>
public sealed class SummaryCardData
{
    /// <summary>"Activity by hour" / "by day", with the period's total on the right.</summary>
    public string ChartTitle { get; init; } = "";
    public string TotalText { get; init; } = "";
    /// <summary>One column per hour (or day); each part is a game (its place in <see cref="Games"/>, -1 for the rest) and its seconds.</summary>
    public List<SummaryCardBucket> Buckets { get; init; } = new();
    public string AccountsTitle { get; init; } = "";
    public List<SummaryCardAccount> Accounts { get; init; } = new();
    /// <summary>"+3 more accounts", under the rows, when not all fit.</summary>
    public string? MoreAccounts { get; init; }
    /// <summary>The games the colours stand for, most played first.</summary>
    public List<string> Games { get; init; } = new();
    public string OtherGames { get; init; } = "";
    /// <summary>Said in place of the chart when nothing was played.</summary>
    public string? Empty { get; init; }
}

public sealed record SummaryCardBucket(string Label, IReadOnlyList<(int Game, double Seconds)> Parts);

public sealed class SummaryCardAccount
{
    public string Name { get; init; } = "";
    /// <summary>A second, quieter line: its stays and drops.</summary>
    public string Detail { get; init; } = "";
    public string Time { get; init; } = "";
    public double Seconds { get; init; }
    public List<(int Game, double Seconds)> Parts { get; init; } = new();
    /// <summary>Roblox's picture of the account (rbxcdn.com only); its initials when there is none or it can't be fetched.</summary>
    public string? AvatarUrl { get; init; }
    public string Initials { get; init; } = "";
    public int Hue { get; init; }
}

/// <summary>A summary's PNG for Discord (attached, so nothing is uploaded elsewhere): play time by hour or day stacked by
/// game, then a row per account with its avatar (Roblox's image servers only) and a bar split by game.</summary>
public static class SummaryCard
{
    // Discord shows it about 450 px wide: drawn at twice that, with text sized to read there.
    const int Width = 1000, Pad = 40, ChartHeight = 230, RowHeight = 86, Avatar = 60;
    static readonly HttpClient Http = new() { Timeout = TimeSpan.FromSeconds(6) };
    static readonly Dictionary<string, byte[]> Avatars = new();

    /// <summary>The games' colours, in order: distinct on Discord's dark and light themes.</summary>
    public static readonly Color[] GameColors =
    [
        Color.FromArgb(0x58, 0x65, 0xF2), Color.FromArgb(0x3B, 0xA5, 0x5D), Color.FromArgb(0xF0, 0xB2, 0x32),
        Color.FromArgb(0xEB, 0x45, 0x9E), Color.FromArgb(0x00, 0xA8, 0xE8), Color.FromArgb(0xE6, 0x7E, 0x22),
    ];
    static readonly Color Other = Color.FromArgb(0x6D, 0x6F, 0x78);
    static readonly Color Back = Color.FromArgb(0x1E, 0x1F, 0x22), Panel = Color.FromArgb(0x2B, 0x2D, 0x31),
        Ink = Color.FromArgb(0xF2, 0xF3, 0xF5), Ink2 = Color.FromArgb(0xB5, 0xBA, 0xC1), Ink3 = Color.FromArgb(0x80, 0x84, 0x8E),
        Grid = Color.FromArgb(0x3A, 0x3C, 0x42);

    public static Color ColorOf(int game) => game >= 0 && game < GameColors.Length ? GameColors[game] : Other;

    public static async Task<byte[]> RenderAsync(SummaryCardData data, CancellationToken ct = default)
    {
        var pictures = new Dictionary<string, Image>();
        try
        {
            foreach (string url in data.Accounts.Select(a => a.AvatarUrl).OfType<string>().Distinct())
            {
                if (await LoadAvatarAsync(url, ct) is { } bytes)
                {
                    try { pictures[url] = Image.FromStream(new MemoryStream(bytes)); }
                    catch (ArgumentException) { } // not a picture System.Drawing reads: initials instead
                }
            }
            return await Task.Run(() => Draw(data, pictures), ct);
        }
        finally
        {
            foreach (var picture in pictures.Values)
                picture.Dispose();
        }
    }

    /// <summary>An avatar's picture from Roblox's CDN (rbxcdn.com only), kept for the next time; null when it can't be had.</summary>
    internal static async Task<byte[]?> LoadAvatarAsync(string url, CancellationToken ct)
    {
        lock (Avatars)
        {
            if (Avatars.TryGetValue(url, out var cached))
                return cached;
        }
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri) || uri.Scheme != Uri.UriSchemeHttps || !uri.Host.EndsWith(".rbxcdn.com", StringComparison.OrdinalIgnoreCase))
            return null;
        try
        {
            byte[] bytes = await Http.GetByteArrayAsync(uri, ct);
            if (bytes.Length > 2_000_000)
                return null;
            lock (Avatars)
            {
                if (Avatars.Count > 200)
                    Avatars.Clear();
                Avatars[url] = bytes;
            }
            return bytes;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or IOException)
        {
            return null;
        }
    }

    static byte[] Draw(SummaryCardData data, Dictionary<string, Image> pictures)
    {
        int rows = data.Accounts.Count;
        int legendRows = data.Games.Count > 0 ? 1 + (data.Games.Count + (data.Games.Count > GameColors.Length ? 1 : 0) - 1) / 3 : 0;
        int height = Pad + 62 + ChartHeight + 46
            + (rows > 0 ? 72 + rows * RowHeight + (data.MoreAccounts is null ? 0 : 44) : 0)
            + (legendRows > 0 ? 24 + legendRows * 46 : 0) + Pad - 10;
        using var bmp = new Bitmap(Width, height, PixelFormat.Format32bppArgb);
        using var g = Graphics.FromImage(bmp);
        g.SmoothingMode = SmoothingMode.AntiAlias;
        g.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
        g.InterpolationMode = InterpolationMode.HighQualityBicubic;
        g.Clear(Color.Transparent);
        using (var back = new SolidBrush(Back))
        using (var card = Rounded(new RectangleF(0, 0, Width, height), 28))
            g.FillPath(back, card);

        using var title = new Font("Segoe UI Semibold", 32, GraphicsUnit.Pixel);
        using var label = new Font("Segoe UI", 26, GraphicsUnit.Pixel);
        using var name = new Font("Segoe UI Semibold", 30, GraphicsUnit.Pixel);
        using var small = new Font("Segoe UI", 25, GraphicsUnit.Pixel);
        using var big = new Font("Segoe UI Semibold", 34, GraphicsUnit.Pixel);
        using var ink = new SolidBrush(Ink);
        using var ink2 = new SolidBrush(Ink2);
        using var ink3 = new SolidBrush(Ink3);

        float y = Pad;
        // Activity by hour (or day), the total on the right.
        g.DrawString(data.ChartTitle, title, ink, Pad, y);
        var totalSize = g.MeasureString(data.TotalText, big);
        g.DrawString(data.TotalText, big, ink, Width - Pad - totalSize.Width, y - 2);
        y += 62;
        var chart = new RectangleF(Pad, y, Width - 2 * Pad, ChartHeight);
        DrawChart(g, chart, data, label, ink2, ink3);
        y += ChartHeight + 46;

        if (rows > 0)
        {
            using (var line = new Pen(Grid, 2))
                g.DrawLine(line, Pad, y, Width - Pad, y);
            y += 18;
            g.DrawString(data.AccountsTitle, title, ink, Pad, y);
            y += 54;
            double most = Math.Max(1, data.Accounts.Max(a => a.Seconds));
            float barLeft = Pad + Avatar + 18 + 270, barRight = Width - Pad - 218;
            foreach (var account in data.Accounts)
            {
                float mid = y + RowHeight / 2f;
                DrawAvatar(g, account, pictures, new RectangleF(Pad, mid - Avatar / 2f, Avatar, Avatar));
                float textLeft = Pad + Avatar + 18, textWidth = barLeft - textLeft - 16;
                g.DrawString(Fit(g, account.Name, name, textWidth), name, ink, textLeft, mid - 37);
                g.DrawString(Fit(g, account.Detail, small, textWidth), small, ink3, textLeft, mid + 3);
                // The bar: its length is the account's share of the most played one's, split by game.
                using (var track = Rounded(new RectangleF(barLeft, mid - 11, barRight - barLeft, 22), 11))
                using (var trackFill = new SolidBrush(Panel))
                    g.FillPath(trackFill, track);
                float full = (float)((barRight - barLeft) * account.Seconds / most), x = barLeft;
                if (full >= 2)
                {
                    using var clip = Rounded(new RectangleF(barLeft, mid - 11, Math.Max(full, 22), 22), 11);
                    var state = g.Save();
                    g.SetClip(clip);
                    foreach (var (game, seconds) in account.Parts.OrderBy(p => p.Game < 0 ? int.MaxValue : p.Game))
                    {
                        float w = (float)(full * seconds / Math.Max(1, account.Seconds));
                        using var fill = new SolidBrush(ColorOf(game));
                        g.FillRectangle(fill, x, mid - 11, w + 1, 22);
                        x += w;
                    }
                    g.Restore(state);
                }
                var timeSize = g.MeasureString(account.Time, name);
                g.DrawString(account.Time, name, ink, Width - Pad - timeSize.Width, mid - timeSize.Height / 2);
                y += RowHeight;
            }
            if (data.MoreAccounts is { } more)
            {
                g.DrawString(more, small, ink3, Pad + Avatar + 18, y + 4);
                y += 44;
            }
        }

        if (legendRows > 0)
        {
            y += 24;
            var items = data.Games.Take(GameColors.Length).Select((game, i) => (game, ColorOf(i))).ToList();
            if (data.Games.Count > GameColors.Length)
                items.Add((data.OtherGames, Other));
            float column = (Width - 2 * Pad) / 3f;
            for (int i = 0; i < items.Count; i++)
            {
                float x = Pad + i % 3 * column, top = y + i / 3 * 46;
                using (var dot = new SolidBrush(items[i].Item2))
                using (var square = Rounded(new RectangleF(x, top + 7, 22, 22), 5))
                    g.FillPath(dot, square);
                g.DrawString(Fit(g, items[i].Item1, label, column - 44), label, ink2, x + 32, top);
            }
        }

        using var png = new MemoryStream();
        bmp.Save(png, ImageFormat.Png);
        return png.ToArray();
    }

    static void DrawChart(Graphics g, RectangleF area, SummaryCardData data, Font label, Brush ink2, Brush ink3)
    {
        using var grid = new Pen(Grid, 1.5f);
        float bottom = area.Bottom - 38; // the labels go under the bars
        float top = area.Top + 10;
        for (int i = 0; i <= 2; i++)
        {
            float lineY = top + (bottom - top) * i / 2f;
            g.DrawLine(grid, area.Left, lineY, area.Right, lineY);
        }
        if (data.Empty is { } empty || data.Buckets.Count == 0)
        {
            string text = data.Empty ?? "";
            var size = g.MeasureString(text, label);
            g.DrawString(text, label, ink2, area.Left + (area.Width - size.Width) / 2, top + (bottom - top - size.Height) / 2);
            return;
        }
        double most = Math.Max(1, data.Buckets.Max(b => b.Parts.Sum(p => p.Seconds)));
        int count = data.Buckets.Count;
        float slot = area.Width / count, bar = Math.Max(4, slot * (count > 12 ? 0.68f : 0.56f));
        int every = count > 12 ? 3 : 1;
        for (int i = 0; i < count; i++)
        {
            var bucket = data.Buckets[i];
            float x = area.Left + i * slot + (slot - bar) / 2, y = bottom;
            double sum = bucket.Parts.Sum(p => p.Seconds);
            if (sum > 0)
            {
                float h = (float)((bottom - top) * sum / most);
                using var column = Rounded(new RectangleF(x, bottom - h, bar, h + 6), Math.Min(7, bar / 2));
                var state = g.Save();
                g.SetClip(column);
                g.SetClip(new RectangleF(x, top - 2, bar, bottom - top + 2), CombineMode.Intersect);
                foreach (var (game, seconds) in bucket.Parts.OrderBy(p => p.Game < 0 ? int.MaxValue : p.Game))
                {
                    float part = (float)(h * seconds / sum);
                    using var fill = new SolidBrush(ColorOf(game));
                    g.FillRectangle(fill, x, y - part, bar, part + 1);
                    y -= part;
                }
                g.Restore(state);
            }
            if (i % every == 0)
            {
                var size = g.MeasureString(bucket.Label, label);
                g.DrawString(bucket.Label, label, ink3, x + bar / 2 - size.Width / 2, bottom + 6);
            }
        }
    }

    static void DrawAvatar(Graphics g, SummaryCardAccount account, Dictionary<string, Image> pictures, RectangleF box)
    {
        using var circle = new GraphicsPath();
        circle.AddEllipse(box);
        if (account.AvatarUrl is { } url && pictures.TryGetValue(url, out var picture))
        {
            var state = g.Save();
            g.SetClip(circle);
            using (var back = new SolidBrush(Panel))
                g.FillEllipse(back, box);
            g.DrawImage(picture, box);
            g.Restore(state);
            return;
        }
        using (var fill = new SolidBrush(FromHue(account.Hue)))
            g.FillPath(fill, circle);
        using var font = new Font("Segoe UI Semibold", 20, GraphicsUnit.Pixel);
        var size = g.MeasureString(account.Initials, font);
        g.DrawString(account.Initials, font, Brushes.White, box.Left + (box.Width - size.Width) / 2, box.Top + (box.Height - size.Height) / 2);
    }

    /// <summary>The page's initials colour (hsl(h 38% 40%)).</summary>
    static Color FromHue(int hue)
    {
        double h = hue % 360 / 60.0, c = (1 - Math.Abs(2 * 0.40 - 1)) * 0.38, xPart = c * (1 - Math.Abs(h % 2 - 1)), m = 0.40 - c / 2;
        var (r, gr, b) = (int)h switch { 0 => (c, xPart, 0.0), 1 => (xPart, c, 0.0), 2 => (0.0, c, xPart), 3 => (0.0, xPart, c), 4 => (xPart, 0.0, c), _ => (c, 0.0, xPart) };
        return Color.FromArgb((int)((r + m) * 255), (int)((gr + m) * 255), (int)((b + m) * 255));
    }

    /// <summary>The text cut with "…" to fit the width.</summary>
    static string Fit(Graphics g, string text, Font font, float width)
    {
        if (g.MeasureString(text, font).Width <= width)
            return text;
        for (int length = text.Length - 1; length > 0; length--)
        {
            string cut = text[..length].TrimEnd() + "…";
            if (g.MeasureString(cut, font).Width <= width)
                return cut;
        }
        return "…";
    }

    static GraphicsPath Rounded(RectangleF r, float radius)
    {
        var path = new GraphicsPath();
        float d = Math.Max(1, Math.Min(radius * 2, Math.Min(r.Width, r.Height)));
        path.AddArc(r.X, r.Y, d, d, 180, 90);
        path.AddArc(r.Right - d, r.Y, d, d, 270, 90);
        path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
        path.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
        path.CloseFigure();
        return path;
    }
}
