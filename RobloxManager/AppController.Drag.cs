using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// A Roblox window's edge dragged by the person goes smaller than Roblox's own least size (2026-10-23): Roblox holds its
/// window at 800 × 600 in its own units whenever it is sized by hand, so a window Arrange made small (a 4 × 4 grid, a
/// size of one's own) grew back as soon as an edge was dragged, and couldn't be dragged in past that size. A WinEvent
/// hook (in Bloxkeep's own process: nothing goes into Roblox's) sees each drag of a Roblox window start and end; at the
/// end, the window gets the size the mouse asked for (<see cref="RobloxProcesses.AfterDrag"/>), given the way Arrange
/// gives its tiles. A move (the title bar) is left to Windows.
/// </summary>
public sealed partial class AppController
{
    const uint EVENT_SYSTEM_MOVESIZESTART = 0x000A, EVENT_SYSTEM_MOVESIZEEND = 0x000B, WINEVENT_SKIPOWNPROCESS = 2;
    IntPtr _dragHook;
    WinEventProc? _dragProc;
    /// <summary>The Roblox window being dragged now: its place and the mouse where the drag started.</summary>
    (IntPtr Window, Rectangle Start, Point Grab)? _drag;

    [StructLayout(LayoutKind.Sequential)]
    struct POINT { public int X, Y; }
    [DllImport("user32.dll")]
    static extern bool GetCursorPos(out POINT point);

    void WatchDrags()
    {
        _dragProc = (_, eventType, hwnd, idObject, _, _, _) =>
        {
            if (_disposed || idObject != 0 || hwnd == IntPtr.Zero)
                return;
            if (eventType == EVENT_SYSTEM_MOVESIZESTART)
            {
                _drag = RobloxProcesses.IsPlayerWindow(hwnd) && RobloxProcesses.WindowRect(hwnd) is { } start && GetCursorPos(out var grab)
                    ? (hwnd, start, new Point(grab.X, grab.Y))
                    : null;
                return;
            }
            if (_drag is not { } drag || drag.Window != hwnd)
                return;
            _drag = null;
            if (GetCursorPos(out var release))
                _ = KeepDraggedSizeAsync(drag.Window, drag.Start, drag.Grab, new Point(release.X, release.Y));
        };
        _dragHook = SetWinEventHook(EVENT_SYSTEM_MOVESIZESTART, EVENT_SYSTEM_MOVESIZEEND, IntPtr.Zero, _dragProc, 0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);
    }

    /// <summary>
    /// The drag's end: the size the move or the mouse meant, if Roblox kept another; again a moment later, should Roblox
    /// size its window once more as it finishes (it does after a resize, as it lays its game out again).
    /// </summary>
    static async Task KeepDraggedSizeAsync(IntPtr hwnd, Rectangle start, Point grab, Point release)
    {
        int border = RobloxProcesses.ResizeBorder(hwnd);
        for (int i = 0; i < 2; i++)
        {
            await Task.Delay(i == 0 ? 30 : 350);
            if (RobloxProcesses.WindowRect(hwnd) is not { } now
                || RobloxProcesses.AfterDrag(start, now, grab, release, border) is not { } meant)
                return;
            RobloxProcesses.PlaceWindow(hwnd, meant);
        }
    }

    void StopWatchingDrags()
    {
        if (_dragHook != IntPtr.Zero)
            UnhookWinEvent(_dragHook);
        _dragHook = IntPtr.Zero;
    }
}
