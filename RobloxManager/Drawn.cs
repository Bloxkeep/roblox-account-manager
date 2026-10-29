using System.Drawing.Drawing2D;
using System.Drawing.Text;

namespace RobloxManager;

/// <summary>Drawing helpers so Bloxkeep's native windows look like the page, not stock Windows controls. Sizes are in
/// device pixels; callers scale by their dpi.</summary>
static class Drawn
{
    /// <summary>Windows 11's icon font, or Windows 10's (the same code points for the glyphs used here).</summary>
    public static readonly string IconFont = new InstalledFontCollection().Families.Any(f => f.Name == "Segoe Fluent Icons")
        ? "Segoe Fluent Icons" : "Segoe MDL2 Assets";

    /// <summary>The glyphs Bloxkeep uses, by meaning.</summary>
    public static class Glyph
    {
        public const string Open = "", Mini = "", Play = "", Presets = "", SignIn = "",
            Multi = "", Close = "", Quit = "", Show = "", Lock = "";
    }

    public static Color Mix(Color a, Color b, float amount) => Color.FromArgb(
        (int)(a.R + (b.R - a.R) * amount), (int)(a.G + (b.G - a.G) * amount), (int)(a.B + (b.B - a.B) * amount));

    /// <summary>The page's --accent: the green of Play and of a switch that is on (--accent-ink on it).</summary>
    public static Color Accent(Themes.Theme t) => t.Dark ? Color.FromArgb(0x22, 0xc5, 0x5e) : Color.FromArgb(0x16, 0xa3, 0x4a);

    public static Color AccentInk(Themes.Theme t) => t.Dark ? Color.FromArgb(0x04, 0x20, 0x0d) : Color.White;

    /// <summary>A hover or pressed surface: the text colour washed over the surface, as the page's ghost buttons.</summary>
    public static Color Hover(Themes.Theme t, bool pressed = false) =>
        Mix(t.Surface, t.Text, t.Dark ? (pressed ? .16f : .09f) : (pressed ? .12f : .06f));

    public static GraphicsPath Round(RectangleF r, float radius)
    {
        var path = new GraphicsPath();
        float d = Math.Min(radius * 2, Math.Min(r.Width, r.Height));
        if (d <= 0)
        {
            path.AddRectangle(r);
            return path;
        }
        path.AddArc(r.X, r.Y, d, d, 180, 90);
        path.AddArc(r.Right - d, r.Y, d, d, 270, 90);
        path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
        path.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
        path.CloseFigure();
        return path;
    }

    public static void FillRound(Graphics g, RectangleF r, float radius, Color fill, Color? line = null)
    {
        g.SmoothingMode = SmoothingMode.AntiAlias;
        using var path = Round(r, radius);
        using (var brush = new SolidBrush(fill))
            g.FillPath(brush, path);
        if (line is { } c)
        {
            using var pen = new Pen(c);
            using var inner = Round(new RectangleF(r.X + .5f, r.Y + .5f, r.Width - 1, r.Height - 1), radius);
            g.DrawPath(pen, inner);
        }
    }

    /// <summary>One glyph of <see cref="IconFont"/>, centred in <paramref name="box"/>.</summary>
    public static void Icon(Graphics g, string glyph, Rectangle box, Color color, float sizePx)
    {
        using var font = new Font(IconFont, sizePx, GraphicsUnit.Pixel);
        TextRenderer.DrawText(g, glyph, font, box, color,
            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPadding | TextFormatFlags.NoPrefix | TextFormatFlags.SingleLine);
    }

    /// <summary>A glyph as a small transparent picture, for a menu item's image (drawn with antialiasing: no ClearType on a transparent bitmap).</summary>
    public static Bitmap IconBitmap(string glyph, int sizePx, Color color)
    {
        var bitmap = new Bitmap(sizePx, sizePx);
        using var g = Graphics.FromImage(bitmap);
        g.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
        using var font = new Font(IconFont, sizePx * .8f, GraphicsUnit.Pixel);
        using var brush = new SolidBrush(color);
        using var format = new StringFormat { Alignment = StringAlignment.Center, LineAlignment = StringAlignment.Center };
        g.DrawString(glyph, font, brush, new RectangleF(0, 0, sizePx, sizePx), format);
        return bitmap;
    }

    /// <summary>The page's switch: a pill, its knob right and filled when on.</summary>
    public static void Switch(Graphics g, RectangleF r, bool on, Themes.Theme t, Color onColor)
    {
        g.SmoothingMode = SmoothingMode.AntiAlias;
        float knob = r.Height - 6;
        FillRound(g, r, r.Height / 2, on ? onColor : Mix(t.Surface, t.Text, t.Dark ? .18f : .2f));
        var k = new RectangleF(on ? r.Right - 3 - knob : r.X + 3, r.Y + 3, knob, knob);
        using var brush = new SolidBrush(on ? AccentInk(t) : t.Text);
        g.FillEllipse(brush, k);
    }

    /// <summary>An account's picture in a circle, or its initials on the page's colour for that name.</summary>
    public static void Avatar(Graphics g, RectangleF r, Image? picture, string name)
    {
        g.SmoothingMode = SmoothingMode.AntiAlias;
        using var circle = new GraphicsPath();
        circle.AddEllipse(r);
        if (picture is not null)
        {
            var state = g.Save();
            g.SetClip(circle);
            g.InterpolationMode = InterpolationMode.HighQualityBicubic;
            g.DrawImage(picture, r);
            g.Restore(state);
            return;
        }
        using (var fill = new SolidBrush(WindowIcons.NameColor(name)))
            g.FillPath(fill, circle);
        string letters = WindowIcons.InitialsOf(name);
        using var font = new Font("Segoe UI Semibold", r.Height * (letters.Length > 1 ? .36f : .44f), GraphicsUnit.Pixel);
        TextRenderer.DrawText(g, letters, font, Rectangle.Round(r), Color.White,
            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPadding | TextFormatFlags.NoPrefix | TextFormatFlags.SingleLine);
    }

    /// <summary>An account's picture as a small round image for a menu item.</summary>
    public static Bitmap AvatarBitmap(Image? picture, string name, int sizePx)
    {
        var bitmap = new Bitmap(sizePx, sizePx);
        using var g = Graphics.FromImage(bitmap);
        g.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
        Avatar(g, new RectangleF(0, 0, sizePx - 1, sizePx - 1), picture, name);
        return bitmap;
    }

    /// <summary>Bloxkeep's logo (the app icon) at <paramref name="sizePx"/>, crisp: the icon file's nearest size.</summary>
    public static void Logo(Graphics g, Rectangle box)
    {
        if (Ui.AppIcon is not { } icon)
            return;
        using var sized = new Icon(icon, box.Size);
        g.InterpolationMode = InterpolationMode.HighQualityBicubic;
        using var bitmap = sized.ToBitmap();
        g.DrawImage(bitmap, box);
    }
}
