using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Keeps Roblox from kicking idle accounts (it does after 20 minutes without input). While nobody is using the PC,
/// each window is brought to the front for a moment and the mouse is moved a few pixels over it and back; then the
/// previous window and cursor position are restored (a minimized window is minimized again). No key is pressed and
/// nothing is clicked. It stops as soon as real input arrives. Windows doesn't allow it while the PC is locked.
/// </summary>
static class AntiAfk
{
    /// <summary>Well inside Roblox's 20 minutes (counted from joining a server or the last input).</summary>
    public static readonly TimeSpan Interval = TimeSpan.FromMinutes(15);

    /// <summary>How long the PC must have been untouched before a window may be brought to the front.</summary>
    public static readonly TimeSpan UserIdleNeeded = TimeSpan.FromSeconds(60);

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
    [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr hWnd, int command);
    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT point);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint attach, uint attachTo, bool on);
    [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint count, INPUT[] inputs, int size);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();

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
                if (!IsWindow(hWnd))
                    continue;
                // A minimized window still gets kicked: show it for the moment of the nudge, then minimize it again.
                bool minimized = IsIconic(hWnd);
                if (minimized)
                    ShowWindow(hWnd, SW_RESTORE);
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
                        ShowWindow(hWnd, SW_SHOWMINNOACTIVE);
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
        bool attached = other != 0 && other != self && AttachThreadInput(self, other, true);
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
