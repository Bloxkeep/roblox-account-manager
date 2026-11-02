using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// Which window the person's own input reached, for anti-AFK. GetLastInputInfo only says the PC was used: a mouse moved
/// on another monitor counted for the Roblox window in front (2026-10-19: a window was kicked for idling 8 minutes after
/// Bloxkeep last counted input for it). Raw input (RIDEV_INPUTSINK) names each event's device; input with none, sent by a
/// program (anti-AFK's F15, a Vietnamese input method), is left out (seen 2026-10-22). Keys count for the window in
/// front, the mouse only when over it; which key or where is never read. Runs on its own thread: a mouse sends up to
/// thousands of moves a second.
/// </summary>
sealed class RobloxInput : IDisposable
{
    [StructLayout(LayoutKind.Sequential)]
    struct RAWINPUTDEVICE
    {
        public ushort usUsagePage, usUsage;
        public uint dwFlags;
        public IntPtr hwndTarget;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct RAWINPUTHEADER
    {
        public uint dwType, dwSize;
        public IntPtr hDevice, wParam;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct POINT
    {
        public int X, Y;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct MSG
    {
        public IntPtr hwnd;
        public uint message;
        public IntPtr wParam, lParam;
        public uint time;
        public POINT pt;
    }

    [DllImport("user32.dll", SetLastError = true)] static extern bool RegisterRawInputDevices(RAWINPUTDEVICE[] devices, uint count, uint size);
    [DllImport("user32.dll")] static extern uint GetRawInputData(IntPtr input, uint command, out RAWINPUTHEADER data, ref uint size, uint headerSize);
    [DllImport("user32.dll")] static extern uint GetRawInputData(IntPtr input, uint command, byte[] data, ref uint size, uint headerSize);
    [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT point);
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT point);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] static extern int GetMessage(out MSG message, IntPtr hWnd, uint min, uint max);
    [DllImport("user32.dll")] static extern IntPtr DispatchMessage(ref MSG message);
    [DllImport("user32.dll")] static extern bool PostThreadMessage(uint thread, uint message, IntPtr wParam, IntPtr lParam);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();

    const int WM_INPUT = 0x00FF;
    const uint WM_QUIT = 0x0012;
    const uint RID_HEADER = 0x10000005, RID_INPUT = 0x10000003, RIM_TYPEMOUSE = 0, RIM_TYPEKEYBOARD = 1;
    const uint RIDEV_INPUTSINK = 0x100, RIDEV_REMOVE = 0x1;
    const ushort GenericDesktop = 1, Mouse = 2, Keyboard = 6;
    static readonly IntPtr HWND_MESSAGE = -3;

    /// <summary>A mouse sends hundreds of moves a second: where the pointer is, is looked at once in this time.</summary>
    static readonly TimeSpan MouseLook = TimeSpan.FromMilliseconds(250);

    readonly Dictionary<int, DateTime> _reached = new();
    readonly Thread _thread;
    uint _threadId;
    DateTime _mouseLookedAt;

    /// <summary>Windows takes the registration: input is seen. When it doesn't, anti-AFK counts as before.</summary>
    public bool Working { get; private set; }

    public RobloxInput()
    {
        using var ready = new ManualResetEventSlim();
        _thread = new Thread(() => Run(ready)) { IsBackground = true, Name = "Bloxkeep input" };
        _thread.SetApartmentState(ApartmentState.STA);
        _thread.Start();
        ready.Wait(TimeSpan.FromSeconds(5));
    }

    void Run(ManualResetEventSlim ready)
    {
        var window = new Sink(this);
        try
        {
            _threadId = GetCurrentThreadId();
            window.CreateHandle(new CreateParams { Parent = HWND_MESSAGE });
            Working = Register(window.Handle);
        }
        catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or InvalidOperationException)
        {
            Log.Write(ex);
        }
        finally
        {
            ready.Set();
        }
        if (Working)
        {
            while (GetMessage(out var message, IntPtr.Zero, 0, 0) > 0)
                DispatchMessage(ref message);
            Register(IntPtr.Zero);
        }
        if (window.Handle != IntPtr.Zero)
            window.DestroyHandle();
    }

    /// <summary>Mouse and keyboard to this window, also in the background; with no window, that undone.</summary>
    static bool Register(IntPtr target)
    {
        uint flags = target == IntPtr.Zero ? RIDEV_REMOVE : RIDEV_INPUTSINK;
        return RegisterRawInputDevices(
        [
            new RAWINPUTDEVICE { usUsagePage = GenericDesktop, usUsage = Mouse, dwFlags = flags, hwndTarget = target },
            new RAWINPUTDEVICE { usUsagePage = GenericDesktop, usUsage = Keyboard, dwFlags = flags, hwndTarget = target },
        ], 2, (uint)Marshal.SizeOf<RAWINPUTDEVICE>());
    }

    long _personAt;

    /// <summary>When a person last used a keyboard or mouse anywhere (UTC), or null: macros wait while they do.</summary>
    public DateTime? PersonAt => Interlocked.Read(ref _personAt) is var ticks && ticks > 0 ? new DateTime(ticks, DateTimeKind.Utc) : null;

    /// <summary>While set (a macro being recorded), each relative mouse move of a person's mouse (x, y, raw units), on
    /// this thread: Roblox keeps the pointer still while the camera turns, so where the pointer goes says nothing.</summary>
    public Action<int, int>? MouseMoved { get; set; }

    /// <summary>When the person's own input last reached this process's window (UTC), or null.</summary>
    public DateTime? ReachedAt(int pid)
    {
        lock (_reached)
            return _reached.TryGetValue(pid, out var at) ? at : null;
    }

    /// <summary>The process an input reached, or 0: a key goes to the window in front, the mouse only when the pointer is
    /// over it.</summary>
    internal static int Reached(bool keyboard, int front, int underPointer) => front != 0 && (keyboard || underPointer == front) ? front : 0;

    void Note(IntPtr input)
    {
        uint size = (uint)Marshal.SizeOf<RAWINPUTHEADER>();
        if (GetRawInputData(input, RID_HEADER, out var header, ref size, size) == uint.MaxValue)
            return;
        // No device: sent by a program (SendInput), not the person.
        if (header.hDevice == IntPtr.Zero || header.dwType is not (RIM_TYPEMOUSE or RIM_TYPEKEYBOARD))
            return;
        bool keyboard = header.dwType == RIM_TYPEKEYBOARD;
        var now = DateTime.UtcNow;
        Interlocked.Exchange(ref _personAt, now.Ticks);
        if (!keyboard && MouseMoved is { } moved)
            Moved(input, size, moved);
        if (!keyboard)
        {
            if (now - _mouseLookedAt < MouseLook)
                return;
            _mouseLookedAt = now;
        }
        int front = RobloxProcesses.ForegroundPid();
        int pid = Reached(keyboard, front, keyboard ? front : UnderPointer());
        if (pid == 0)
            return;
        lock (_reached)
        {
            if (_reached.Count > 64 && !_reached.ContainsKey(pid))
                _reached.Clear(); // only the last few minutes matter; this keeps the list to the windows in use
            _reached[pid] = now;
        }
    }

    /// <summary>RAWMOUSE after the header: usFlags (2 bytes, then 2 of padding), the button data (4), ulRawButtons (4),
    /// lLastX, lLastY.</summary>
    static void Moved(IntPtr input, uint headerSize, Action<int, int> moved)
    {
        const ushort MOUSE_MOVE_ABSOLUTE = 1;
        uint size = 0;
        GetRawInputData(input, RID_INPUT, null!, ref size, headerSize);
        if (size < headerSize + 20 || size > 256)
            return;
        var data = new byte[size];
        if (GetRawInputData(input, RID_INPUT, data, ref size, headerSize) == uint.MaxValue)
            return;
        int at = (int)headerSize;
        if ((BitConverter.ToUInt16(data, at) & MOUSE_MOVE_ABSOLUTE) != 0)
            return; // a tablet or remote desktop: positions, not moves
        int x = BitConverter.ToInt32(data, at + 12), y = BitConverter.ToInt32(data, at + 16);
        if (x != 0 || y != 0)
            moved(x, y);
    }

    static int UnderPointer()
    {
        if (!GetCursorPos(out var point))
            return 0;
        IntPtr window = WindowFromPoint(point);
        if (window == IntPtr.Zero)
            return 0;
        GetWindowThreadProcessId(window, out uint pid);
        return (int)pid;
    }

    public void Dispose()
    {
        if (_threadId != 0 && _thread.IsAlive)
            PostThreadMessage(_threadId, WM_QUIT, IntPtr.Zero, IntPtr.Zero);
    }

    /// <summary>The message-only window Windows sends the input to.</summary>
    sealed class Sink(RobloxInput owner) : NativeWindow
    {
        protected override void WndProc(ref Message m)
        {
            if (m.Msg == WM_INPUT)
                owner.Note(m.LParam);
            base.WndProc(ref m);
        }
    }
}
