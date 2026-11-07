using System.Diagnostics;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Short words for error.log about where windows are and how they look on screen, so a report like "nothing showed" or
/// "all full screen" can be read from the log instead of guessed (owner, 2026-11-07: three window bugs in a day, each
/// found only from a video). Process ids only, never account names: the log may be sent to us.
/// </summary>
static class WindowReport
{
    /// <summary>"1224×957 normal, screen 2 of 2" / "maximized" / "full screen" / "minimized" / "gone".</summary>
    public static string Describe(IntPtr hWnd)
    {
        if (hWnd == IntPtr.Zero || !IsWindow(hWnd))
            return "gone";
        if (IsIconic(hWnd))
            return "minimized";
        if (!GetWindowRect(hWnd, out var r))
            return "unknown";
        var rect = Rectangle.FromLTRB(r.Left, r.Top, r.Right, r.Bottom);
        var screens = Screen.AllScreens;
        var screen = Screen.FromRectangle(rect);
        string state = IsZoomed(hWnd) ? "maximized" : rect.Contains(screen.Bounds) ? "full screen" : "normal";
        return $"{rect.Width}×{rect.Height} {state}, screen {Array.IndexOf(screens, screen) + 1} of {screens.Length}"
            + (IsWindowVisible(hWnd) ? "" : ", hidden");
    }

    /// <summary>How many of the windows are in each state: "4 maximized, 2 normal".</summary>
    public static string States(IEnumerable<IntPtr> windows) =>
        string.Join(", ", windows.Select(h => Describe(h) switch
        {
            var d when d.Contains("maximized") => "maximized",
            var d when d.Contains("full screen") => "full screen",
            var d when d.Contains("normal") => "normal",
            var d => d,
        }).GroupBy(s => s).Select(g => $"{g.Count()} {g.Key}"));

    /// <summary>
    /// After an arrange: each window in <paramref name="area"/>, restored, and seen at its middle (not under another app);
    /// the others named with what's wrong. "5 in place" or "3 in place; 41316 maximized; 52168 under chrome".
    /// </summary>
    public static string Placed(IReadOnlyList<(int Pid, IntPtr Window)> windows, Rectangle area)
    {
        var wrong = new List<string>();
        int ok = 0;
        foreach (var (pid, hWnd) in windows)
        {
            string? why = null;
            if (hWnd == IntPtr.Zero || !IsWindow(hWnd))
                why = "gone";
            else if (IsIconic(hWnd))
                why = "minimized";
            else if (!IsWindowVisible(hWnd))
                why = "hidden";
            else if (IsZoomed(hWnd))
                why = "maximized";
            else if (GetWindowRect(hWnd, out var r))
            {
                var middle = new Point((r.Left + r.Right) / 2, (r.Top + r.Bottom) / 2);
                if (!area.Contains(middle))
                    why = $"off the screen asked for ({Describe(hWnd)})";
                else if (CoveredBy(hWnd, middle) is { } over)
                    why = $"under {over}";
            }
            if (why is null)
                ok++;
            else
                wrong.Add($"{pid} {why}");
        }
        return wrong.Count == 0 ? $"{ok} in place" : $"{ok} in place; " + string.Join("; ", wrong);
    }

    /// <summary>The program whose window shows at <paramref name="point"/> instead of <paramref name="hWnd"/>; null when it
    /// shows, or another Roblox window does (overlapping layouts lay them over each other on purpose).</summary>
    static string? CoveredBy(IntPtr hWnd, Point point)
    {
        var top = WindowFromPoint(new POINT { X = point.X, Y = point.Y });
        if (top == IntPtr.Zero)
            return null;
        var root = GetAncestor(top, GA_ROOT);
        if (root == hWnd || root == IntPtr.Zero)
            return null;
        GetWindowThreadProcessId(root, out uint pid);
        try
        {
            using var p = Process.GetProcessById((int)pid);
            if (p.ProcessName.Equals("RobloxPlayerBeta", StringComparison.OrdinalIgnoreCase))
                return null;
            return pid == Environment.ProcessId ? "Bloxkeep" : p.ProcessName;
        }
        catch (Exception ex) when (ex is ArgumentException or InvalidOperationException)
        {
            return "another app";
        }
    }

    const uint GA_ROOT = 2;
    [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct POINT { public int X, Y; }
    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsZoomed(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT point);
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr hWnd, uint flags);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
}
