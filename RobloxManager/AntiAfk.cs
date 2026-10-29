using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Keeps Roblox from kicking idle accounts (after 20 minutes without input). While the PC is idle, each window is made
/// all but invisible and click-through, given the keyboard and one F15 press, then your window is put back in front
/// (topmost for an instant, in case Windows won't hand back the keyboard), the Roblox window goes back to its place,
/// and only then is it visible again. The cursor doesn't move; real input stops it; impossible while locked.
/// </summary>
static class AntiAfk
{
    /// <summary>
    /// Well inside Roblox's 20 minutes, so a person at the PC most of the time still leaves an idle minute for it.
    /// </summary>
    public static readonly TimeSpan Interval = TimeSpan.FromMinutes(12);

    /// <summary>
    /// Two minutes before the kick a window doesn't wait for the PC to be free: it is nudged in the first pause of
    /// <see cref="PauseNeeded"/> (unless a full-screen app is in front).
    /// </summary>
    public static readonly TimeSpan LastChance = TimeSpan.FromMinutes(18);

    public static readonly TimeSpan UserIdleNeeded = TimeSpan.FromSeconds(60);

    public static readonly TimeSpan PauseNeeded = TimeSpan.FromSeconds(2);

    /// <summary>F15: on no keyboard and used by no game or common app, but input all the same.</summary>
    const ushort NudgeKey = 0x7E; // VK_F15

    /// <summary>1 of 255: 0 could make Windows treat the window as not shown at all.</summary>
    internal const byte HiddenAlpha = 1;

    [StructLayout(LayoutKind.Sequential)]
    struct LASTINPUTINFO
    {
        public uint cbSize;
        public uint dwTime;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct MOUSEINPUT
    {
        public int dx, dy;
        public uint mouseData, dwFlags, time;
        public IntPtr dwExtraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct KEYBDINPUT
    {
        public ushort wVk, wScan;
        public uint dwFlags, time;
        public IntPtr dwExtraInfo;
    }

    /// <summary>Holds a mouse input too, so its size is the one SendInput expects.</summary>
    [StructLayout(LayoutKind.Explicit)]
    struct INPUTUNION
    {
        [FieldOffset(0)] public MOUSEINPUT mi;
        [FieldOffset(0)] public KEYBDINPUT ki;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct INPUT
    {
        public uint type;
        public INPUTUNION u;
    }

    /// <summary>The size of Windows' INPUT (40 bytes in a 64-bit process, 28 in a 32-bit one): SendInput refuses any other.</summary>
    internal static int InputSize => Marshal.SizeOf<INPUT>();

    const uint INPUT_KEYBOARD = 1, KEYEVENTF_KEYUP = 0x0002, MAPVK_VK_TO_VSC = 0;
    const int SW_RESTORE = 9, SW_SHOWMINNOACTIVE = 7, SW_SHOWNOACTIVATE = 4, SW_HIDE = 0;
    const int GWL_EXSTYLE = -20, WS_EX_TOPMOST = 0x8, WS_EX_TRANSPARENT = 0x20, WS_EX_LAYERED = 0x80000;
    const uint LWA_ALPHA = 0x2, GW_HWNDPREV = 3;
    const uint SWP_NOSIZE = 0x1, SWP_NOMOVE = 0x2, SWP_NOACTIVATE = 0x10, SWP_NOOWNERZORDER = 0x200;
    // Not async: a window put back under yours while yours is still always-on-top would become always-on-top itself.
    // So a window that has stopped answering isn't moved (the move waits for its answer).
    const uint Restack = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    static readonly IntPtr HWND_TOPMOST = -1, HWND_NOTOPMOST = -2;
    const uint RDW_INVALIDATE = 0x1, RDW_ERASE = 0x4, RDW_ALLCHILDREN = 0x80, RDW_FRAME = 0x400;
    const int DWMWA_TRANSITIONS_FORCEDISABLED = 3;

    [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LASTINPUTINFO info);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hWnd);
    // Async: ShowWindow waits for the window to answer, and a hung Roblox would hold Bloxkeep's timer (rejoin, saves).
    [DllImport("user32.dll")] static extern bool ShowWindowAsync(IntPtr hWnd, int command);
    /// <summary>Windows' own "not responding" (no answer for 5 seconds).</summary>
    [DllImport("user32.dll")] static extern bool IsHungAppWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] static extern IntPtr GetShellWindow();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr hWnd, System.Text.StringBuilder name, int size);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint attach, uint attachTo, bool on);
    [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint count, INPUT[] inputs, int size);
    [DllImport("user32.dll")] static extern uint MapVirtualKey(uint code, uint mapType);
    [DllImport("user32.dll", EntryPoint = "GetWindowLongW")] static extern int GetWindowLong(IntPtr hWnd, int index);
    [DllImport("user32.dll", EntryPoint = "SetWindowLongW")] static extern int SetWindowLong(IntPtr hWnd, int index, int value);
    [DllImport("user32.dll")] static extern bool GetLayeredWindowAttributes(IntPtr hWnd, out uint key, out byte alpha, out uint flags);
    [DllImport("user32.dll")] static extern bool SetLayeredWindowAttributes(IntPtr hWnd, uint key, byte alpha, uint flags);
    [DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr hWnd, uint command);
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] static extern bool RedrawWindow(IntPtr hWnd, IntPtr update, IntPtr region, uint flags);
    [DllImport("dwmapi.dll")] static extern int DwmSetWindowAttribute(IntPtr hWnd, int attribute, ref int value, int size);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("wtsapi32.dll", EntryPoint = "WTSQuerySessionInformationW")]
    static extern bool WTSQuerySessionInformation(IntPtr server, int sessionId, int infoClass, out IntPtr buffer, out int bytes);
    [DllImport("wtsapi32.dll")] static extern void WTSFreeMemory(IntPtr memory);

    const int WTS_CURRENT_SESSION = -1, WTSSessionInfoEx = 25, WTS_SESSIONSTATE_LOCK = 0;

    [DllImport("shell32.dll")] static extern int SHQueryUserNotificationState(out int state);

    const int QUNS_BUSY = 2, QUNS_RUNNING_D3D_FULL_SCREEN = 3, QUNS_PRESENTATION_MODE = 4;

    /// <summary>As Windows judges it for its notifications (a game, a video, a presentation).</summary>
    public static bool FullScreenInFront() =>
        SHQueryUserNotificationState(out int state) == 0 && state is QUNS_BUSY or QUNS_RUNNING_D3D_FULL_SCREEN or QUNS_PRESENTATION_MODE;

    /// <summary>
    /// No window can come to the front then. Read from WTSINFOEX level 1: the level (padded to 8 bytes), the session
    /// id, then its state.
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

    /// <summary>Our own nudges don't count.</summary>
    public static TimeSpan IdleTime()
    {
        uint last = LastInputTick();
        if (last == _ownInput)
            last = _personInput;
        return TimeSpan.FromMilliseconds(unchecked((uint)Environment.TickCount - last));
    }

    /// <summary>Input within <see cref="UserIdleNeeded"/>; our own nudges don't count.</summary>
    public static bool PersonActive()
    {
        uint last = LastInputTick();
        return last != _ownInput && TimeSpan.FromMilliseconds(unchecked((uint)Environment.TickCount - last)) < UserIdleNeeded;
    }

    /// <summary>A Roblox window as it was before a nudge; Above is the window it goes back under.</summary>
    internal sealed record Before(IntPtr Window, int ExStyle, bool HadAttributes, uint Key, byte Alpha, uint Flags, IntPtr Above, int Depth, bool Minimized);

    /// <summary>
    /// Returns the windows reached; gives up on the rest as soon as someone touches the PC. <paramref name="press"/>
    /// (tests only) stands in for the key press, so a test never types on the desktop.
    /// </summary>
    public static async Task<List<IntPtr>> NudgeAsync(IReadOnlyList<IntPtr> windows, Func<bool> keepGoing, Func<IntPtr, Task>? press = null)
    {
        press ??= PressKeyAsync;
        var reached = new List<IntPtr>();
        IntPtr previous = GetForegroundWindow();
        uint baseline = LastInputTick();
        // Still our last round's input: the person's own is older.
        uint person = baseline == _ownInput ? _personInput : baseline;
        bool sent = false;
        // Read before any of them moves.
        var places = windows.Distinct().Where(IsWindow).ToDictionary(w => w, Place);
        var hidden = new List<Before>();
        // Windows Bloxkeep hid, shown for the nudge.
        var unseen = new List<IntPtr>();
        try
        {
            foreach (var hWnd in windows)
            {
                if (!keepGoing() || LastInputTick() != baseline)
                    break; // turned off, or a person is back: hand the PC over right away
                // Touching a hung window could only make Bloxkeep wait.
                if (!IsWindow(hWnd) || IsHungAppWindow(hWnd))
                    continue;
                // Unseen and click-through first, so neither its coming to the front nor a click at that moment shows.
                // A minimized window still gets kicked: restored for the nudge, then minimized again.
                bool minimized = IsIconic(hWnd);
                // A window Bloxkeep hid (RobloxProcesses.HideWindows) is shown for the nudge, still unseen, then hidden.
                bool unshown = !IsWindowVisible(hWnd);
                hidden.Add(Hide(hWnd, places.GetValueOrDefault(hWnd), minimized));
                if (minimized)
                    ShowWindowAsync(hWnd, SW_RESTORE);
                else if (unshown)
                    ShowWindowAsync(hWnd, SW_SHOWNOACTIVATE);
                try
                {
                    if (!Focus(hWnd))
                        continue;
                    await Task.Delay(minimized ? 400 : 150); // a restored Roblox window needs a moment to draw again
                    // The key goes to whichever window has the keyboard, and a person's keys would mix with it.
                    if (GetForegroundWindow() != hWnd || LastInputTick() != baseline || !keepGoing())
                        continue;
                    await press(hWnd);
                    sent = true;
                    await Task.Delay(60);
                    // Our own key counts as input; anything newer than this is a person.
                    baseline = LastInputTick();
                    reached.Add(hWnd);
                }
                finally
                {
                    if (minimized)
                        ShowWindowAsync(hWnd, SW_SHOWMINNOACTIVE);
                    if (unshown)
                    {
                        ShowWindowAsync(hWnd, SW_HIDE);
                        unseen.Add(hWnd);
                    }
                }
            }
        }
        finally
        {
            // Else the last input is the person's, and must stay theirs.
            if (sent)
            {
                _ownInput = baseline;
                _personInput = person;
            }
            // Nothing touched (turned off, a person back, no window answering): the window you're in is left alone.
            if (hidden.Count > 0)
                GiveBack(previous);
            // Highest in the stack first, so one that goes under another finds it there; seen again only once in place
            // (a minimized one once minimized again).
            foreach (var before in hidden.OrderBy(b => b.Depth))
                PutBack(before);
            for (int i = 0; i < 10 && (hidden.Any(b => b.Minimized && IsWindow(b.Window) && !IsIconic(b.Window))
                                       || unseen.Any(w => IsWindow(w) && IsWindowVisible(w))); i++)
                await Task.Delay(50);
            foreach (var before in hidden)
                Show(before);
        }
        return reached;
    }

    /// <summary>
    /// The nearest shown, not-topmost window above, and the depth. Hidden ones don't count: a program's hidden
    /// input-method window rides along with its front window, so going back under one would leave the window in front.
    /// </summary>
    static (IntPtr Above, int Depth) Place(IntPtr hWnd)
    {
        IntPtr above = IntPtr.Zero;
        int depth = 0;
        for (IntPtr w = GetWindow(hWnd, GW_HWNDPREV); w != IntPtr.Zero && depth < 5000; w = GetWindow(w, GW_HWNDPREV), depth++)
            if (above == IntPtr.Zero && IsWindowVisible(w) && (GetWindowLong(w, GWL_EXSTYLE) & WS_EX_TOPMOST) == 0)
                above = w;
        return (above, depth);
    }

    /// <summary>A minimized one also loses Windows' restore animation, which the taskbar would show.</summary>
    internal static Before Hide(IntPtr hWnd, (IntPtr Above, int Depth) place, bool minimized)
    {
        int ex = GetWindowLong(hWnd, GWL_EXSTYLE);
        uint key = 0, flags = 0;
        byte alpha = 255;
        bool had = (ex & WS_EX_LAYERED) != 0 && GetLayeredWindowAttributes(hWnd, out key, out alpha, out flags);
        var before = new Before(hWnd, ex, had, key, alpha, flags, place.Above, place.Depth, minimized);
        SetWindowLong(hWnd, GWL_EXSTYLE, ex | WS_EX_LAYERED | WS_EX_TRANSPARENT);
        SetLayeredWindowAttributes(hWnd, 0, HiddenAlpha, LWA_ALPHA);
        if (minimized)
            Animations(hWnd, false);
        return before;
    }

    /// <summary>
    /// Only the bits <see cref="Hide"/> set are touched. A hung window is left to <see cref="Recover"/>: a change of
    /// style waits for the window's answer.
    /// </summary>
    internal static void Show(Before b)
    {
        if (!IsWindow(b.Window) || IsHungAppWindow(b.Window))
            return;
        const int ours = WS_EX_LAYERED | WS_EX_TRANSPARENT;
        SetWindowLong(b.Window, GWL_EXSTYLE, (GetWindowLong(b.Window, GWL_EXSTYLE) & ~ours) | (b.ExStyle & ours));
        if (b.HadAttributes)
            SetLayeredWindowAttributes(b.Window, b.Key, b.Alpha, b.Flags);
        if (b.Minimized)
            Animations(b.Window, true);
        RedrawWindow(b.Window, IntPtr.Zero, IntPtr.Zero, RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN);
    }

    static void Animations(IntPtr hWnd, bool on)
    {
        int disabled = on ? 0 : 1;
        DwmSetWindowAttribute(hWnd, DWMWA_TRANSITIONS_FORCEDISABLED, ref disabled, sizeof(int));
    }

    internal static void PutBack(Before b)
    {
        if (b.Minimized || b.Above == IntPtr.Zero || !IsWindow(b.Window) || !IsWindow(b.Above) || IsHungAppWindow(b.Window))
            return;
        SetWindowPos(b.Window, b.Above, 0, 0, 0, 0, Restack | SWP_NOOWNERZORDER);
    }

    /// <summary>
    /// Windows may refuse to hand the keyboard back (a click landed meanwhile): made topmost for an instant, your
    /// window comes back on top all the same.
    /// </summary>
    static void GiveBack(IntPtr previous)
    {
        if (previous == IntPtr.Zero || !IsWindow(previous))
            return;
        bool lift = !IsDesktop(previous) && IsWindowVisible(previous) && !IsIconic(previous) && (GetWindowLong(previous, GWL_EXSTYLE) & WS_EX_TOPMOST) == 0
            && !IsHungAppWindow(previous);
        if (lift)
            SetWindowPos(previous, HWND_TOPMOST, 0, 0, 0, 0, Restack);
        Focus(previous);
        if (lift)
            SetWindowPos(previous, HWND_NOTOPMOST, 0, 0, 0, 0, Restack); // first of the usual windows
    }

    /// <summary>Its icons or the wallpaper behind them: never lifted over the windows.</summary>
    static bool IsDesktop(IntPtr hWnd)
    {
        if (hWnd == GetShellWindow())
            return true;
        var name = new System.Text.StringBuilder(32);
        return GetClassName(hWnd, name, name.Capacity) > 0 && name.ToString() is "Progman" or "WorkerW";
    }

    static async Task PressKeyAsync(IntPtr _)
    {
        ushort scan = (ushort)MapVirtualKey(NudgeKey, MAPVK_VK_TO_VSC);
        INPUT Key(uint flags) => new() { type = INPUT_KEYBOARD, u = new INPUTUNION { ki = new KEYBDINPUT { wVk = NudgeKey, wScan = scan, dwFlags = flags } } };
        SendInput(1, [Key(0)], InputSize);
        await Task.Delay(30); // held for a moment, like a real key
        SendInput(1, [Key(KEYEVENTF_KEYUP)], InputSize);
    }

    /// <summary>
    /// Undoes a nudge cut short by Bloxkeep closing; told apart by <see cref="HiddenAlpha"/>, which nothing else uses.
    /// </summary>
    public static void Recover(IEnumerable<IntPtr> windows)
    {
        const int ours = WS_EX_LAYERED | WS_EX_TRANSPARENT;
        foreach (var w in windows)
        {
            int ex = GetWindowLong(w, GWL_EXSTYLE);
            if ((ex & ours) != ours || !GetLayeredWindowAttributes(w, out _, out byte alpha, out uint flags) || (flags & LWA_ALPHA) == 0 || alpha != HiddenAlpha
                || IsHungAppWindow(w))
                continue;
            SetWindowLong(w, GWL_EXSTYLE, ex & ~ours);
            Animations(w, true);
            RedrawWindow(w, IntPtr.Zero, IntPtr.Zero, RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN);
        }
    }

    /// <summary>
    /// Windows only lets the app that last received input do it, so the foreground thread's input is shared meanwhile.
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
