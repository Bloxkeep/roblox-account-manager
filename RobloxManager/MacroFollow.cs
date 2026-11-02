using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace RobloxManager;

/// <summary>The colours a follow step keeps on its target: one marker colour, or a box's left and right edges.</summary>
sealed record MacroMarker(string? Colour, string? Left, string? Right)
{
    public bool Box => Left is not null;
}

/// <summary>What a follow step saw in one frame of its area (pixels of the area): null where that colour wasn't there.</summary>
readonly record struct FollowSight(double? Target, double? Marker, double? Left, double? Right);

/// <summary>
/// The maths of the follow and clickColour steps, kept apart from the desktop so the checks run them on made-up
/// pictures and sequences. Colours match when no channel differs by more than the tolerance.
/// </summary>
static class MacroFollow
{
    /// <summary>The marker this close to the target (px) counts as on it: no press either way.</summary>
    public const double DeadZone = 2;
    /// <summary>Predicting looks this many frames ahead (a frame for the picture, one for the game to read the press).</summary>
    public const double LeadFrames = 2;
    /// <summary>Smooth mode: an error of a quarter of the bar, at strength 1, holds for the whole frame.</summary>
    public const double Gain = 4;

    // ───────────────────────── the step's marker ─────────────────────────

    public static void CheckMarker(JsonElement? marker)
    {
        if (marker is not { } m || m.ValueKind == JsonValueKind.Null)
            throw new UserError(L.T("Choose the marker's colour."));
        if (m.ValueKind == JsonValueKind.String)
        {
            NeedColourOrName(m.GetString());
            return;
        }
        if (m.ValueKind != JsonValueKind.Object || !m.TryGetProperty("left", out var left) || !m.TryGetProperty("right", out var right)
            || left.ValueKind != JsonValueKind.String || right.ValueKind != JsonValueKind.String)
            throw new UserError(L.T("Choose the marker's colour."));
        NeedColourOrName(left.GetString());
        NeedColourOrName(right.GetString());
    }

    static void NeedColourOrName(string? text)
    {
        if (text is ['$', .. var name] ? !MacroValues.Name.IsMatch(name) : !IsColour(text))
            throw new UserError(L.T("Choose the marker's colour."));
    }

    static bool IsColour(string? c) => c is { Length: 7 } && c[0] == '#' && c[1..].All(Uri.IsHexDigit);

    /// <summary>The marker's colours, board values put in.</summary>
    /// <exception cref="MacroValueMissing"/>
    public static MacroMarker Marker(JsonElement? marker, Func<string, JsonElement?> value)
    {
        string Colour(string text)
        {
            if (text is not ['$', .. var name])
                return text;
            return value(name) is { ValueKind: JsonValueKind.String } v && IsColour(v.GetString()) ? v.GetString()! : throw new MacroValueMissing(name);
        }
        var m = marker!.Value;
        return m.ValueKind == JsonValueKind.String
            ? new MacroMarker(Colour(m.GetString()!), null, null)
            : new MacroMarker(null, Colour(m.GetProperty("left").GetString()!), Colour(m.GetProperty("right").GetString()!));
    }

    public static (int R, int G, int B) Rgb(string colour) =>
        (Convert.ToInt32(colour.Substring(1, 2), 16), Convert.ToInt32(colour.Substring(3, 2), 16), Convert.ToInt32(colour.Substring(5, 2), 16));

    // ───────────────────────── looking ─────────────────────────

    /// <summary>A picture's pixels as 0xAARRGGBB, row after row.</summary>
    public readonly record struct Pixels(int W, int H, int[] P)
    {
        public static Pixels Of(Bitmap bitmap, Rectangle? area = null)
        {
            var r = Rectangle.Intersect(area ?? new Rectangle(Point.Empty, bitmap.Size), new Rectangle(Point.Empty, bitmap.Size));
            if (r.Width <= 0 || r.Height <= 0)
                return new Pixels(0, 0, []);
            var data = bitmap.LockBits(r, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            try
            {
                var p = new int[r.Width * r.Height];
                for (int y = 0; y < r.Height; y++)
                    Marshal.Copy(data.Scan0 + y * data.Stride, p, y * r.Width, r.Width);
                return new Pixels(r.Width, r.Height, p);
            }
            finally
            {
                bitmap.UnlockBits(data);
            }
        }
    }

    static bool Near(int argb, (int R, int G, int B) c, int tolerance) =>
        Math.Abs(((argb >> 16) & 0xFF) - c.R) <= tolerance && Math.Abs(((argb >> 8) & 0xFF) - c.G) <= tolerance && Math.Abs((argb & 0xFF) - c.B) <= tolerance;

    /// <summary>The middle (x, in pixels of the area) of the pixels of a colour; null when there are none.</summary>
    public static double? Centre(Pixels px, string colour, int tolerance)
    {
        var c = Rgb(colour);
        long sum = 0, count = 0;
        for (int y = 0; y < px.H; y++)
        {
            int row = y * px.W;
            for (int x = 0; x < px.W; x++)
            {
                if (Near(px.P[row + x], c, tolerance))
                {
                    sum += x;
                    count++;
                }
            }
        }
        return count == 0 ? null : (double)sum / count;
    }

    /// <summary>One frame of a follow step: where the target and the marker (or its box's edges) are.</summary>
    public static FollowSight Look(Pixels px, string target, MacroMarker marker, int tolerance)
    {
        double? t = Centre(px, target, tolerance);
        if (!marker.Box)
            return new FollowSight(t, Centre(px, marker.Colour!, tolerance), null, null);
        double? left = Centre(px, marker.Left!, tolerance), right = Centre(px, marker.Right!, tolerance);
        return new FollowSight(t, left is { } l && right is { } r ? (l + r) / 2 : null, left, right);
    }

    /// <summary>On/off: hold while the marker is left of the target, let go once it is right of it; null: on it, keep as is.</summary>
    public static bool? OnOff(double marker, double target, double deadZone = DeadZone) =>
        marker < target - deadZone ? true : marker > target + deadZone ? false : null;

    /// <summary>
    /// Smooth mode: a PD controller. Each frame it says how long to hold (0..frame ms): longer the further the marker is
    /// behind the target, shorter when it is already closing in fast (damping), none once it is past.
    /// </summary>
    public sealed class Pd(double kp, double kd, double width)
    {
        double? _lastError;

        public double Width => width;

        /// <param name="target">Where the target is (px).</param>
        /// <param name="marker">Where the marker is (px), predicted ahead already when asked to.</param>
        public double PressMs(double target, double marker, double frameMs)
        {
            double error = (target - marker) / Math.Max(1, width);
            double change = _lastError is { } last ? error - last : 0; // per frame
            _lastError = error;
            double u = kp * error + kd * change;
            return Math.Clamp(u * Gain, 0, 1) * frameMs;
        }

        public void Reset() => _lastError = null;
    }

    /// <summary>The marker's speed (px a frame) from its last places, and where it will be after <see cref="LeadFrames"/>.</summary>
    public sealed class Predictor
    {
        double? _last;
        double _speed;

        public double Ahead(double marker)
        {
            if (_last is { } last)
                _speed = 0.5 * _speed + 0.5 * (marker - last); // smoothed: one frame's jitter isn't a speed
            _last = marker;
            return marker + _speed * LeadFrames;
        }

        public double Speed => _speed;
    }

    // ───────────────────────── clickColour ─────────────────────────

    /// <summary>
    /// Patches of a colour in the area at least <paramref name="minSize"/> px across, largest first (looked at every
    /// other pixel: a button is never one pixel).
    /// </summary>
    public static List<Rectangle> Blobs(Pixels px, string colour, int tolerance, int minSize)
    {
        var c = Rgb(colour);
        const int step = 2;
        int gw = (px.W + step - 1) / step, gh = (px.H + step - 1) / step;
        var on = new bool[gw * gh];
        for (int gy = 0; gy < gh; gy++)
            for (int gx = 0; gx < gw; gx++)
                on[gy * gw + gx] = Near(px.P[Math.Min(px.H - 1, gy * step) * px.W + Math.Min(px.W - 1, gx * step)], c, tolerance);
        var seen = new bool[on.Length];
        var found = new List<(Rectangle Box, int Count)>();
        var stack = new Stack<int>();
        for (int i = 0; i < on.Length; i++)
        {
            if (!on[i] || seen[i])
                continue;
            int minX = int.MaxValue, minY = int.MaxValue, maxX = -1, maxY = -1, count = 0;
            stack.Push(i);
            seen[i] = true;
            while (stack.Count > 0)
            {
                int j = stack.Pop(), x = j % gw, y = j / gw;
                count++;
                minX = Math.Min(minX, x); maxX = Math.Max(maxX, x);
                minY = Math.Min(minY, y); maxY = Math.Max(maxY, y);
                void Visit(int nx, int ny)
                {
                    if (nx < 0 || ny < 0 || nx >= gw || ny >= gh)
                        return;
                    int k = ny * gw + nx;
                    if (on[k] && !seen[k])
                    {
                        seen[k] = true;
                        stack.Push(k);
                    }
                }
                Visit(x - 1, y); Visit(x + 1, y); Visit(x, y - 1); Visit(x, y + 1);
            }
            var box = new Rectangle(minX * step, minY * step, (maxX - minX + 1) * step, (maxY - minY + 1) * step);
            if (Math.Max(box.Width, box.Height) >= minSize)
                found.Add((box, count));
        }
        return found.OrderByDescending(f => f.Count).Select(f => f.Box).ToList();
    }
}
