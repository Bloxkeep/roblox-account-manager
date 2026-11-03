using System.Drawing;
using System.Drawing.Drawing2D;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// "What it sees": one picture of the game looked at the way the macro's steps look at it, without playing anything.
/// Boxes where its areas are and where it found its colours, pictures and words, and the numbers it decides by
/// (how far the marker is from the target, how well a picture matched). The main tool for tuning colours.
/// </summary>
static class MacroWatch
{
    const int MaxBoxes = 32, MaxNumbers = 12, MaxBlobs = 5;

    /// <summary>A box in fractions of the picture (0..1). Kind: area, target, marker, colour, image, text.</summary>
    public sealed record Box(string Kind, string Label, string Colour, double X, double Y, double W, double H);

    public sealed record Number(string Label, double Value);

    public sealed record Seen(List<Box> Boxes, List<Number> Numbers);

    static Box Fraction(string kind, string label, string colour, RectangleF r, int w, int h) =>
        new(kind, label, colour, Math.Round(r.X / w, 4), Math.Round(r.Y / h, 4), Math.Round(r.Width / w, 4), Math.Round(r.Height / h, 4));

    /// <summary>The macro's looking steps on <paramref name="picture"/>; steps whose board values are missing are left out.</summary>
    public static Seen Look(Bitmap picture, Macro macro, Func<string, JsonElement?> value)
    {
        var seen = new Seen(new(), new());
        int w = picture.Width, h = picture.Height;
        if (w < 2 || h < 2)
            return seen;
        double scale = macro.RecordedSize is { W: > 0 } size ? (double)w / size.W : 1;
        MacroVision.Gray? gray = null;
        MacroVision.Gray Gray() => gray ??= MacroVision.ToGray(picture);
        int n = 0;
        foreach (var raw in macro.Steps)
        {
            n++;
            if (seen.Boxes.Count >= MaxBoxes)
                break;
            MacroStep s;
            try
            {
                s = MacroValues.Resolve(raw, value);
            }
            catch (MacroValueMissing)
            {
                continue;
            }
            string tag = L.F("Step {0}", n);
            switch (s.Kind)
            {
                case "follow" when s.Region is { } region && s.Target is { } target:
                {
                    var area = MacroVision.Area(region, w, h);
                    seen.Boxes.Add(Fraction("area", tag, "#a3a3a3", area, w, h));
                    MacroMarker marker;
                    try
                    {
                        marker = MacroFollow.Marker(s.Marker, value);
                    }
                    catch (MacroValueMissing)
                    {
                        break;
                    }
                    var px = MacroFollow.Pixels.Of(picture, area);
                    var sight = MacroFollow.Look(px, target, marker, s.Tolerance ?? 24);
                    void Line(string kind, string label, string colour, double? x)
                    {
                        if (x is { } at)
                            seen.Boxes.Add(Fraction(kind, label, colour, new RectangleF((float)(area.X + at - 1), area.Y, 3, area.Height), w, h));
                    }
                    Line("target", L.T("Target"), "#22c55e", sight.Target);
                    if (marker.Box)
                    {
                        Line("marker", L.T("Left edge"), "#f59e0b", sight.Left);
                        Line("marker", L.T("Right edge"), "#f59e0b", sight.Right);
                    }
                    Line("marker", L.T("Marker"), "#3b82f6", sight.Marker);
                    if (sight.Target is { } t && sight.Marker is { } m)
                        seen.Numbers.Add(new Number(tag + ": " + L.T("distance (px)"), Math.Round(t - m, 1)));
                    else
                        seen.Numbers.Add(new Number(tag + ": " + (sight.Target is null ? L.T("target not found") : L.T("marker not found")), 0));
                    break;
                }
                case "clickColour" when s.Region is { } region && s.Colour is { } colour:
                {
                    var area = MacroVision.Area(region, w, h);
                    seen.Boxes.Add(Fraction("area", tag, "#a3a3a3", area, w, h));
                    var blobs = MacroFollow.Blobs(MacroFollow.Pixels.Of(picture, area), colour, s.Tolerance ?? 24, s.MinSize ?? 4);
                    foreach (var b in blobs.Take(MaxBlobs))
                        seen.Boxes.Add(Fraction("colour", L.T("Found"), colour, new RectangleF(area.X + b.X, area.Y + b.Y, b.Width, b.Height), w, h));
                    seen.Numbers.Add(new Number(tag + ": " + L.T("patches found"), blobs.Count));
                    break;
                }
                case "waitColour" when s.Colour is { } colour:
                    if (s.Region is { } r)
                    {
                        var area = MacroVision.Area(r, w, h);
                        seen.Boxes.Add(Fraction("area", tag, "#a3a3a3", area, w, h));
                        var blobs = MacroFollow.Blobs(MacroFollow.Pixels.Of(picture, area), colour, s.Tolerance ?? 24, 2);
                        foreach (var b in blobs.Take(MaxBlobs))
                            seen.Boxes.Add(Fraction("colour", L.T("Found"), colour, new RectangleF(area.X + b.X, area.Y + b.Y, b.Width, b.Height), w, h));
                        seen.Numbers.Add(new Number(tag + ": " + L.T("patches found"), blobs.Count));
                    }
                    else if (s.X is { } x && s.Y is { } y)
                        Point(seen, tag, picture, x, y, colour, s.Tolerance ?? 24);
                    break;
                case "waitImage" or "clickImage" when s.Image is { } image:
                    Image(seen, tag, Gray(), image, s.Region, scale, w, h);
                    break;
                case "if" or "loopUntil" when s.Test is { } test:
                    switch (test.Kind)
                    {
                        case "image" when test.Image is { } image:
                            Image(seen, tag, Gray(), image, test.Region, scale, w, h);
                            break;
                        case "colour" when test.X is { } x && test.Y is { } y && test.Colour is { } c:
                            Point(seen, tag, picture, x, y, c, test.Tolerance ?? 24);
                            break;
                    }
                    break;
            }
        }
        if (seen.Numbers.Count > MaxNumbers)
            seen.Numbers.RemoveRange(MaxNumbers, seen.Numbers.Count - MaxNumbers);
        return seen;
    }

    static void Point(Seen seen, string tag, Bitmap picture, double x, double y, string colour, int tolerance)
    {
        int w = picture.Width, h = picture.Height;
        bool yes = MacroVision.ColourAt(picture, x, y, colour, tolerance);
        seen.Boxes.Add(new Box("colour", yes ? L.T("Color matches") : L.T("Color differs"), yes ? "#22c55e" : "#ef4444",
            Math.Round(x - 4.0 / w, 4), Math.Round(y - 4.0 / h, 4), Math.Round(8.0 / w, 4), Math.Round(8.0 / h, 4)));
        seen.Numbers.Add(new Number(tag + ": " + L.T("color matches"), yes ? 1 : 0));
    }

    static void Image(Seen seen, string tag, MacroVision.Gray gray, string image, MacroRect? region, double scale, int w, int h)
    {
        var area = MacroVision.Area(region, gray.W, gray.H);
        if (region is not null)
            seen.Boxes.Add(Fraction("area", tag, "#a3a3a3", area, w, h));
        if (MacroVision.Template(image, scale) is not { } template || MacroVision.Find(gray, template, area) is not { } found)
            return;
        seen.Boxes.Add(Fraction("image", L.F("Match {0}%", (int)Math.Round(found.Score * 100)), found.Score >= 0.9 ? "#22c55e" : "#f59e0b", found.Found, w, h));
        seen.Numbers.Add(new Number(tag + ": " + L.T("picture match"), Math.Round(found.Score, 3)));
    }

    /// <summary>A line under the picture when there is nothing to see: no step looks at the game, or a value is missing.</summary>
    public static string? Note(Macro macro, Seen seen)
    {
        if (seen.Boxes.Count > 0)
            return null;
        bool looks = macro.Steps.Any(s => s.Kind is "follow" or "clickColour" or "waitColour" or "waitImage" or "clickImage" or "waitText"
            || s.Kind is "if" or "loopUntil" && s.Test?.Kind is "image" or "colour" or "text");
        return looks ? L.T("Nothing found: check the colors, areas and settings.") : L.T("No step looks at the game: add a color, picture or text step.");
    }

    /// <summary>The words the macro waits for or checks, with their areas.</summary>
    public static IEnumerable<(string Text, MacroRect? Region)> TextSteps(Macro macro)
    {
        foreach (var s in macro.Steps)
        {
            if (s.Kind == "waitText" && s.Text is { Length: > 0 } text)
                yield return (text, s.Region);
            else if (s.Kind is "if" or "loopUntil" && s.Test is { Kind: "text", Text: { Length: > 0 } t })
                yield return (t, s.Test.Region);
        }
    }

    /// <summary>Boxes around the words the macro looks for, where they are read (text reading takes a while: called at most once a second).</summary>
    public static async Task<List<Box>> ReadTextAsync(Bitmap picture, Macro macro, IMacroDesk desk)
    {
        var boxes = new List<Box>();
        int w = picture.Width, h = picture.Height;
        foreach (var (text, region) in TextSteps(macro).Take(4))
        {
            var area = MacroVision.Area(region, w, h);
            var read = await desk.ReadAsync(picture, area);
            if (read is null)
                break;
            RectangleF? at = null;
            foreach (var lines in read)
            {
                if ((at = MacroOcr.Locate(lines, text)) is not null)
                    break;
            }
            if (at is { IsEmpty: false } box)
                boxes.Add(Fraction("text", text, "#a855f7", box, w, h));
            else if (region is not null)
                boxes.Add(Fraction("area", at is null ? L.F("\"{0}\" not read", text) : text, at is null ? "#ef4444" : "#a855f7", area, w, h));
        }
        return boxes;
    }

    /// <summary>A copy at most <paramref name="maxW"/> wide (the page and the small window need no more).</summary>
    public static Bitmap Scaled(Bitmap picture, int maxW)
    {
        double scale = Math.Min(1, maxW / (double)Math.Max(1, picture.Width));
        int w = Math.Max(1, (int)(picture.Width * scale)), h = Math.Max(1, (int)(picture.Height * scale));
        var small = new Bitmap(w, h);
        using var g = Graphics.FromImage(small);
        g.InterpolationMode = InterpolationMode.HighQualityBilinear;
        g.DrawImage(picture, new Rectangle(0, 0, w, h));
        return small;
    }
}
