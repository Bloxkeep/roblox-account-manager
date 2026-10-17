using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Keeps Roblox from kicking idle accounts (it does after 20 minutes without input). While nobody is using the PC,
/// each window is brought to the front for a moment and the mouse is moved a few pixels over it and back; then the
/// previous window and cursor position are restored (a minimized window is minimized again). No key is pressed and
/// nothing is clicked. It stops as soon as real input arrives. Windows doesn't allow it while the PC is locked.
/// A window about to be kicked gets it even while the PC is in use, in a pause of a couple of seconds (see
/// <see cref="LastChance"/>).
/// </summary>
static class AntiAfk
{
    /// <summary>
    /// From here a window is nudged at the first minute nobody uses the PC: well inside Roblox's 20 minutes (counted
    /// from joining a server or the last input), so a person at the PC most of the time still leaves one.
    /// </summary>
    public static readonly TimeSpan Interval = TimeSpan.FromMinutes(12);

    /// <summary>
    /// From here, two minutes before Roblox's kick, a window doesn't wait for the PC to be free: it is nudged in the
    /// first pause of <see cref="PauseNeeded"/> (unless a full-screen app is in front).
    /// </summary>
    public static readonly TimeSpan LastChance = TimeSpan.FromMinutes(18);

    /// <summary>How long the PC must have been untouched before a window may be brought to the front.</summary>
    public static readonly TimeSpan UserIdleNeeded = TimeSpan.FromSeconds(60);

    /// <summary>A pause in typing or moving the mouse long enough for a last-chance nudge (about a quarter of a second).</summary>
    public static readonly TimeSpan PauseNeeded = TimeSpan.FromSeconds(2);

    const int Nudge = 3;

    [StructLayout(LayoutKind.Sequential)]
    struct LASTINPUTINFO
    {
        public uint cbSize;
        public uint dwTime;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct POINT
    {
        public int X, Y;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct RECT
    {
        public int Left, Top, Right, Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct MOUSEINPUT
    {
        public int dx, dy;
        public uint mouseData, dwFlags, time;
        public IntPtr dwExtraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct INPUT
    {
        public uint type;
        public MOUSEINPUT mi;
    }

    const uint INPUT_MOUSE = 0, MOUSEEVENTF_MOVE = 0x0001;
    const int SW_RESTORE = 9, SW_SHOWMINNOACTIVE = 7;

    [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LASTINPUTINFO info);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hWnd);
    // Async: ShowWindow on another program's window waits for it to answer, and a Roblox that hangs (loading, paging)
    // would hold Bloxkeep's window and its timer (rejoin, saves) for as long.
    [DllImport("user32.dll")] static extern bool ShowWindowAsync(IntPtr hWnd, int command);
    /// <summary>Windows' own "not responding" (no answer for 5 seconds).</summary>
    [DllImport("user32.dll")] static extern bool IsHungAppWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT point);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint attach, uint attachTo, bool on);
    [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint count, INPUT[] inputs, int size);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("wtsapi32.dll", EntryPoint = "WTSQuerySessionInformationW")]
    static extern bool WTSQuerySessionInformation(IntPtr server, int sessionId, int infoClass, out IntPtr buffer, out int bytes);
    [DllImport("wtsapi32.dll")] static extern void WTSFreeMemory(IntPtr memory);

    const int WTS_CURRENT_SESSION = -1, WTSSessionInfoEx = 25, WTS_SESSIONSTATE_LOCK = 0;

    [DllImport("shell32.dll")] static extern int SHQueryUserNotificationState(out int state);

    const int QUNS_BUSY = 2, QUNS_RUNNING_D3D_FULL_SCREEN = 3, QUNS_PRESENTATION_MODE = 4;

    /// <summary>
    /// A full-screen app (a game, a video, a presentation) is in front, as Windows judges it for its notifications:
    /// a last-chance nudge would interrupt it.
    /// </summary>
    public static bool FullScreenInFront() =>
        SHQueryUserNotificationState(out int state) == 0 && state is QUNS_BUSY or QUNS_RUNNING_D3D_FULL_SCREEN or QUNS_PRESENTATION_MODE;

    /// <summary>
    /// The PC is locked (the sign-in screen is up): no window can come to the front then. Read from WTSINFOEX level 1,
    /// whose session flags follow the level (padded to 8 bytes), the session id and its state.
    /// </summary>
    public static bool Locked()
    {
        if (!WTSQuerySessionInformation(IntPtr.Zero, WTS_CURRENT_SESSION, WTSSessionInfoEx, out var buffer, out int bytes) || buffer == IntPtr.Zero)
            return false;
        try
        {
            return bytes >= 20 && Marshal.ReadInt32(buffer) == 1 && Marshal.ReadInt32(buffer, 16) == WTS_SESSIONSTATE_LOCK;
        }
        finally
        {
            WTSFreeMemory(buffer);
        }
    }

    static uint LastInputTick()
    {
        var info = new LASTINPUTINFO { cbSize = (uint)Marshal.SizeOf<LASTINPUTINFO>() };
        return GetLastInputInfo(ref info) ? info.dwTime : 0;
    }

    /// <summary>The PC's last-input time right after our own nudges, which must not be mistaken for a person.</summary>
    static uint _ownInput;
    /// <summary>The person's last input before those nudges.</summary>
    static uint _personInput;

    /// <summary>
    /// How long since someone last used the mouse or keyboard; our own nudges don't count.
    /// </summary>
    public static TimeSpan IdleTime()
    {
        uint last = LastInputTick();
        if (last == _ownInput)
            last = _personInput;
        return TimeSpan.FromMilliseconds(unchecked((uint)Environment.TickCount - last));
    }

    /// <summary>Someone used the mouse or keyboard within <see cref="UserIdleNeeded"/> (our own nudges don't count).</summary>
    public static bool PersonActive()
    {
        uint last = LastInputTick();
        return last != _ownInput && TimeSpan.FromMilliseconds(unchecked((uint)Environment.TickCount - last)) < UserIdleNeeded;
    }

    /// <summary>
    /// Nudges each window in turn and returns the ones reached. Gives up on the rest as soon as someone touches the PC.
    /// </summary>
    public static async Task<List<IntPtr>> NudgeAsync(IReadOnlyList<IntPtr> windows, Func<bool> keepGoing)
    {
        var reached = new List<IntPtr>();
        IntPtr previous = GetForegroundWindow();
        bool haveCursor = GetCursorPos(out var cursor);
        uint baseline = LastInputTick();
        // Still our last round's input: the person's own is older.
        uint person = baseline == _ownInput ? _personInput : baseline;
        bool sent = false;
        try
        {
            foreach (var hWnd in windows)
            {
                if (!keepGoing() || LastInputTick() != baseline)
                    break; // turned off, or a person is back: hand the PC over right away
                // A window that isn't answering can't be nudged anyway; touching it could only make Bloxkeep wait.
                if (!IsWindow(hWnd) || IsHungAppWindow(hWnd))
                    continue;
                // A minimized window still gets kicked: show it for the moment of the nudge, then minimize it again.
                bool minimized = IsIconic(hWnd);
                if (minimized)
                    ShowWindowAsync(hWnd, SW_RESTORE);
                try
                {
                    if (!Focus(hWnd))
                        continue;
                    await Task.Delay(minimized ? 400 : 150); // a restored Roblox window needs a moment to draw again
                    if (GetForegroundWindow() != hWnd || !GetClientRect(hWnd, out var rect))
                        continue;
                    var center = new POINT { X = (rect.Right - rect.Left) / 2, Y = (rect.Bottom - rect.Top) / 2 };
                    ClientToScreen(hWnd, ref center);
                    SetCursorPos(center.X, center.Y);
                    MoveMouse(Nudge, Nudge);
                    sent = true;
                    await Task.Delay(40);
                    MoveMouse(-Nudge, -Nudge);
                    await Task.Delay(60);
                    // Our own movement counts as input; anything newer than this is a person.
                    baseline = LastInputTick();
                    reached.Add(hWnd);
                }
                finally
                {
                    if (minimized)
                        ShowWindowAsync(hWnd, SW_SHOWMINNOACTIVE);
                }
            }
        }
        finally
        {
            // Only when something was sent: else the last input is the person's, and must stay theirs.
            if (sent)
            {
                _ownInput = baseline;
                _personInput = person;
            }
            if (haveCursor)
                SetCursorPos(cursor.X, cursor.Y);
            if (previous != IntPtr.Zero && IsWindow(previous))
                Focus(previous);
        }
        return reached;
    }

    static void MoveMouse(int dx, int dy)
    {
        var input = new INPUT { type = INPUT_MOUSE, mi = new MOUSEINPUT { dx = dx, dy = dy, dwFlags = MOUSEEVENTF_MOVE } };
        SendInput(1, [input], Marshal.SizeOf<INPUT>());
    }

    /// <summary>
    /// Brings a window to the front. Windows only lets the app that last received input do that, so the input of the
    /// current foreground window's thread is shared for the moment of the switch.
    /// </summary>
    static bool Focus(IntPtr hWnd)
    {
        IntPtr foreground = GetForegroundWindow();
        if (foreground == hWnd)
            return true;
        uint self = GetCurrentThreadId();
        uint other = foreground == IntPtr.Zero ? 0 : GetWindowThreadProcessId(foreground, out _);
        // Not to a program that has stopped answering: sharing its input could leave Bloxkeep waiting on it too.
        bool attached = other != 0 && other != self && !IsHungAppWindow(foreground) && AttachThreadInput(self, other, true);
        try
        {
            BringWindowToTop(hWnd);
            return SetForegroundWindow(hWnd);
        }
        finally
        {
            if (attached)
                AttachThreadInput(self, other, false);
        }
    }
}
