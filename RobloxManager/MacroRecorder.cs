using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>Something recorded: a key or mouse button going down or up, a wheel turn, a relative mouse move (raw
/// units) or the pointer's place, <see cref="Ms"/> after the recording began.</summary>
/// <param name="Kind">"keyDown", "keyUp", "down", "up", "wheel", "move" (relative) or "pos" (where the pointer is).</param>
sealed record MacroEvent(int Ms, string Kind, string? Key = null, string? Button = null, Point At = default, int Dx = 0, int Dy = 0, int Notches = 0)
{
    /// <summary>For a click: a picture of where it clicked (PNG, base64), when one was taken.</summary>
    public string? Image { get; set; }
}

/// <summary>
/// Recorded input made into a macro's steps: a key pressed alone becomes a tap with its hold time, keys pressed together
/// Hold and Release; clicks a click where they were (two quick ones a double click); a right-button drag the camera
/// turning (in pieces, so its path is kept); the wheel a scroll; the time between them waits (under 30 ms dropped).
/// </summary>
static class MacroRecording
{
    /// <summary>Two clicks this close in time and place are a double click.</summary>
    const int DoubleMs = 400, DoublePx = 6;
    /// <summary>A camera drag is cut into pieces this long at most.</summary>
    const int CameraPiece = 400;
    const int MinWait = 30;

    /// <summary>A step made, from when to when it plays (the waits go between), and where (a click).</summary>
    sealed record Act(int Start, int End, MacroStep Step, Point At = default);

    /// <param name="place">A screen point as 0..1 of the window's client area; null for one outside it.</param>
    public static List<MacroStep> ToSteps(IReadOnlyList<MacroEvent> recorded, Func<Point, (double X, double Y)?> place)
    {
        var events = recorded.OrderBy(e => e.Ms).ToList();
        var actions = new List<Act>();
        var used = new HashSet<int>();
        var held = new HashSet<string>();
        for (int i = 0; i < events.Count; i++)
        {
            if (used.Contains(i))
                continue;
            var e = events[i];
            switch (e.Kind)
            {
                case "keyDown":
                {
                    if (e.Key is null || held.Contains(e.Key))
                        break; // the keyboard's own repeat while held
                    int up = events.FindIndex(i + 1, x => x.Kind == "keyUp" && x.Key == e.Key);
                    bool alone = up > 0 && e.Key != "Alt" && events.Skip(i + 1).Take(up - i - 1).All(x => x.Kind is "move" or "pos" || (x.Kind == "keyDown" && x.Key == e.Key));
                    if (alone)
                    {
                        actions.Add(new(e.Ms, events[up].Ms, new MacroStep { Kind = "key", Key = e.Key, Ms = Math.Clamp(events[up].Ms - e.Ms, 30, 5000) }));
                        for (int j = i + 1; j <= up; j++)
                            used.Add(j);
                    }
                    else
                    {
                        held.Add(e.Key);
                        actions.Add(new(e.Ms, e.Ms, new MacroStep { Kind = "down", Key = e.Key }));
                    }
                    break;
                }
                case "keyUp":
                    if (e.Key is not null && held.Remove(e.Key))
                        actions.Add(new(e.Ms, e.Ms, new MacroStep { Kind = "up", Key = e.Key }));
                    break;
                case "down" when e.Button == "right":
                {
                    int up = events.FindIndex(i + 1, x => x.Kind == "up" && x.Button == "right");
                    int end = up > 0 ? events[up].Ms : events[^1].Ms;
                    var moves = events.Skip(i + 1).Take((up > 0 ? up : events.Count) - i - 1).Where(x => x.Kind == "move").ToList();
                    if (up > 0)
                        used.Add(up);
                    foreach (int j in Enumerable.Range(i + 1, Math.Max(0, (up > 0 ? up : events.Count) - i - 1)).Where(j => events[j].Kind == "move"))
                        used.Add(j);
                    if (moves.Sum(m => Math.Abs(m.Dx) + Math.Abs(m.Dy)) < 4)
                    {
                        if (place(e.At) is { } p)
                            actions.Add(new(e.Ms, end, new MacroStep { Kind = "click", X = Round(p.X), Y = Round(p.Y), Button = "right", Image = e.Image }));
                        break;
                    }
                    // In pieces: a drag one way then back would add up to nothing.
                    int pieceStart = e.Ms;
                    int dx = 0, dy = 0;
                    foreach (var m in moves)
                    {
                        if (m.Ms - pieceStart > CameraPiece && (dx != 0 || dy != 0))
                        {
                            actions.Add(new(pieceStart, m.Ms, Camera(dx, dy, m.Ms - pieceStart)));
                            (pieceStart, dx, dy) = (m.Ms, 0, 0);
                        }
                        dx += m.Dx;
                        dy += m.Dy;
                    }
                    if (dx != 0 || dy != 0)
                        actions.Add(new(pieceStart, end, Camera(dx, dy, end - pieceStart)));
                    break;
                }
                case "down":
                {
                    int up = events.FindIndex(i + 1, x => x.Kind == "up" && x.Button == e.Button);
                    if (up > 0)
                        used.Add(up);
                    if (place(e.At) is not { } p)
                        break;
                    int end = up > 0 ? events[up].Ms : e.Ms;
                    // A second quick click on the same place: the first becomes a double click.
                    if (actions.Count > 0 && actions[^1] is { Step.Kind: "click" } prev && prev.Step.Button == e.Button && prev.Step.Double != true
                        && e.Ms - prev.End <= DoubleMs && Math.Abs(prev.At.X - e.At.X) <= DoublePx && Math.Abs(prev.At.Y - e.At.Y) <= DoublePx)
                    {
                        prev.Step.Double = true;
                        actions[^1] = prev with { End = end };
                        break;
                    }
                    actions.Add(new(e.Ms, end, new MacroStep { Kind = "click", X = Round(p.X), Y = Round(p.Y), Button = e.Button ?? "left", Image = e.Image }, e.At));
                    break;
                }
                case "wheel":
                {
                    int notches = e.Notches, end = e.Ms;
                    for (int j = i + 1; j < events.Count && events[j].Kind is "wheel" or "move" or "pos" && events[j].Ms - end <= 200; j++)
                    {
                        if (events[j].Kind != "wheel")
                            continue;
                        notches += events[j].Notches;
                        end = events[j].Ms;
                        used.Add(j);
                    }
                    if (notches != 0)
                        actions.Add(new(e.Ms, end, new MacroStep { Kind = "scroll", Notches = Math.Clamp(notches, -50, 50) }));
                    break;
                }
            }
        }
        // Still held when the recording ended: let go at the end.
        int last = events.Count > 0 ? events[^1].Ms : 0;
        foreach (string key in held)
            actions.Add(new(last, last, new MacroStep { Kind = "up", Key = key }));

        var steps = new List<MacroStep>();
        int at = -1;
        foreach (var a in actions.OrderBy(a => a.Start))
        {
            if (at >= 0 && a.Start - at >= MinWait)
                steps.Add(new MacroStep { Kind = "wait", Ms = Math.Min(a.Start - at, MacroRules.MaxWaitMs) });
            steps.Add(a.Step);
            at = Math.Max(at, a.End);
        }
        return steps.Count > MacroRules.MaxSteps ? steps.Take(MacroRules.MaxSteps).ToList() : steps;
    }

    static MacroStep Camera(int dx, int dy, int ms) =>
        new() { Kind = "camera", Dx = Math.Clamp(dx, -100_000, 100_000), Dy = Math.Clamp(dy, -100_000, 100_000), Ms = Math.Clamp(ms, 16, 10_000), Button = "right" };

    static double Round(double v) => Math.Round(Math.Clamp(v, 0, 1), 4);

    // ───────── TinyTask ─────────

    /// <summary>The frame TinyTask's screen positions are taken as (it records the whole screen, not a window).</summary>
    public static readonly Size TinyTaskFrame = new(1920, 1080);

    /// <summary>
    /// A TinyTask recording (.rec): Windows' journal records (EVENTMSG) one after another, 20 bytes each as the 32-bit
    /// TinyTask writes them: message, paramL, paramH, time (ms, GetTickCount), window. Keys: paramL's low byte is the
    /// virtual key, its next byte the scan code; paramH's bit 15 the extended flag. Mouse: paramL, paramH the screen
    /// position. The wheel's turn isn't in the record: one notch up each.
    /// </summary>
    public static List<MacroEvent> ReadTinyTask(byte[] file)
    {
        if (file.Length < 20 || file.Length % 20 != 0)
            throw new UserError(L.T("That isn't a TinyTask recording."));
        var events = new List<MacroEvent>();
        uint? first = null;
        Point last = default;
        bool rightDown = false;
        for (int o = 0; o + 20 <= file.Length; o += 20)
        {
            uint message = BitConverter.ToUInt32(file, o), paramL = BitConverter.ToUInt32(file, o + 4), paramH = BitConverter.ToUInt32(file, o + 8);
            uint time = BitConverter.ToUInt32(file, o + 12), window = BitConverter.ToUInt32(file, o + 16);
            first ??= time;
            int ms = (int)Math.Clamp(unchecked(time - first.Value), 0, int.MaxValue);
            var at = new Point(unchecked((int)paramL), unchecked((int)paramH));
            switch (message)
            {
                case 0x100 or 0x104 or 0x101 or 0x105:
                    if (MacroKeys.FromScan((ushort)(paramL >> 8 & 0xFF), (paramH & 0x8000) != 0, (ushort)(paramL & 0xFF)) is { } key)
                        events.Add(new(ms, message is 0x100 or 0x104 ? "keyDown" : "keyUp", key.Name));
                    break;
                case 0x200:
                    if (rightDown && (at.X != last.X || at.Y != last.Y))
                        events.Add(new(ms, "move", Dx: at.X - last.X, Dy: at.Y - last.Y));
                    last = at;
                    break;
                case 0x201 or 0x204 or 0x207:
                    last = at;
                    rightDown |= message == 0x204;
                    events.Add(new(ms, "down", Button: message == 0x201 ? "left" : message == 0x204 ? "right" : "middle", At: at));
                    break;
                case 0x202 or 0x205 or 0x208:
                    rightDown &= message != 0x205;
                    events.Add(new(ms, "up", Button: message == 0x202 ? "left" : message == 0x205 ? "right" : "middle", At: at));
                    break;
                case 0x20A:
                    short delta = unchecked((short)(window >> 16));
                    events.Add(new(ms, "wheel", Notches: delta < 0 ? -1 : 1));
                    break;
                default:
                    if (message is not (>= 0x100 and <= 0x20E))
                        throw new UserError(L.T("That isn't a TinyTask recording."));
                    break;
            }
        }
        return events;
    }

    /// <summary>A TinyTask recording as a macro (not saved), its places as parts of a 1920 × 1080 screen.</summary>
    public static Macro FromTinyTask(byte[] file, string name)
    {
        var frame = TinyTaskFrame;
        var steps = ToSteps(ReadTinyTask(file), p => (p.X / (double)frame.Width, p.Y / (double)frame.Height));
        if (steps.Count == 0)
            throw new UserError(L.T("That recording has nothing Bloxkeep can play."));
        return new Macro { Name = name, Steps = steps, RecordedSize = new MacroSize { W = frame.Width, H = frame.Height }, JitterMs = 0, JitterPx = 0 };
    }

    /// <summary>A square of about <paramref name="side"/> px around <paramref name="at"/> (client pixels), as PNG base64.</summary>
    public static string? Cut(Bitmap picture, Point at, int side = 64)
    {
        var r = Rectangle.Intersect(new Rectangle(at.X - side / 2, at.Y - side / 2, side, side), new Rectangle(0, 0, picture.Width, picture.Height));
        if (r.Width < 8 || r.Height < 8)
            return null;
        using var cut = picture.Clone(r, PixelFormat.Format24bppRgb);
        using var stream = new MemoryStream();
        cut.Save(stream, ImageFormat.Png);
        return Convert.ToBase64String(stream.ToArray());
    }
}

/// <summary>
/// Records a person's input into one Roblox window: low-level keyboard and mouse hooks in Bloxkeep's own process (never
/// in Roblox), on a thread of their own, kept only while recording, taking only what goes to that window while it is in
/// front. Our own input (injected) is left out. F8 ends it; after 10 minutes it ends by itself.
/// </summary>
sealed class MacroRecorder : IDisposable
{
    public static readonly TimeSpan Countdown = TimeSpan.FromSeconds(3), MaxLength = TimeSpan.FromMinutes(10);

    public Guid AccountId { get; }
    public DateTime StartsAt { get; }
    /// <summary>The client area when recording began.</summary>
    public Rectangle Client { get; }
    readonly IntPtr _window;
    readonly int _pid;
    readonly List<MacroEvent> _events = new();
    readonly List<Task> _pictures = new();
    readonly SynchronizationContext? _ui;
    readonly long _startTick;
    readonly Thread _thread;
    uint _threadId;
    IntPtr _keyboardHook, _mouseHook;
    HookProc? _keyboardProc, _mouseProc;
    readonly HashSet<string> _down = new();
    bool _rightDown;
    Point _lastPoint;
    readonly RobloxInput? _raw;
    int _actions;
    /// <summary>error.log: F8 was written already.</summary>
    bool _f8Noted;

    /// <summary>F8 was pressed (on the window's thread).</summary>
    public System.Action? StopAsked { get; set; }
    /// <summary>Something new was recorded (on the window's thread).</summary>
    public System.Action? Changed { get; set; }

    /// <summary>Roughly how many steps so far (taps, clicks, scrolls).</summary>
    public int Steps => Volatile.Read(ref _actions);

    public MacroRecorder(Guid accountId, int pid, IntPtr window, RobloxInput? raw)
    {
        AccountId = accountId;
        _pid = pid;
        _window = window;
        _raw = raw is { Working: true } ? raw : null;
        Client = MacroDesk.ClientOnScreen(window);
        StartsAt = DateTime.UtcNow + Countdown;
        _startTick = Environment.TickCount64 + (long)Countdown.TotalMilliseconds;
        _ui = SynchronizationContext.Current;
        if (_raw is not null)
            _raw.MouseMoved = OnRawMove;
        using var ready = new ManualResetEventSlim();
        _thread = new Thread(() => Run(ready)) { IsBackground = true, Name = "Bloxkeep macro recorder" };
        _thread.Start();
        bool readyInTime = ready.Wait(TimeSpan.FromSeconds(5));
        if (_keyboardHook == IntPtr.Zero || _mouseHook == IntPtr.Zero)
        {
            Log.Note($"Macro recording: didn't start on window {pid}: keyboard hook {(_keyboardHook != IntPtr.Zero ? "set" : "refused")}, mouse hook "
                + $"{(_mouseHook != IntPtr.Zero ? "set" : "refused")}{(readyInTime ? "" : ", its thread took over 5 s")}.");
            Dispose();
            throw new UserError(L.T("Windows didn't let Bloxkeep record. Try again."));
        }
        Log.Note($"Macro recording: started on window {pid} ({Client.Width}x{Client.Height}), raw mouse input {(_raw is null ? "off" : "on")}"
            + $"{(_ui is null ? "; no UI thread, so F8 and the step count won't reach the app" : "")}.");
    }

    void Run(ManualResetEventSlim ready)
    {
        _threadId = GetCurrentThreadId();
        _keyboardProc = OnKeyboard;
        _mouseProc = OnMouse;
        IntPtr module = GetModuleHandle(null);
        _keyboardHook = SetWindowsHookEx(WH_KEYBOARD_LL, _keyboardProc, module, 0);
        int keyboardError = _keyboardHook == IntPtr.Zero ? Marshal.GetLastWin32Error() : 0;
        _mouseHook = SetWindowsHookEx(WH_MOUSE_LL, _mouseProc, module, 0);
        int mouseError = _mouseHook == IntPtr.Zero ? Marshal.GetLastWin32Error() : 0;
        if (keyboardError != 0 || mouseError != 0)
            Log.Note($"Macro recording: SetWindowsHookEx failed (keyboard error {keyboardError}, mouse error {mouseError}).");
        ready.Set();
        if (_keyboardHook != IntPtr.Zero && _mouseHook != IntPtr.Zero)
        {
            while (GetMessage(out var message, IntPtr.Zero, 0, 0) > 0)
                DispatchMessage(ref message);
        }
        if (_keyboardHook != IntPtr.Zero)
            UnhookWindowsHookEx(_keyboardHook);
        if (_mouseHook != IntPtr.Zero)
            UnhookWindowsHookEx(_mouseHook);
    }

    int Now => (int)(Environment.TickCount64 - _startTick);

    /// <summary>Input for the recorded window: it is the one in front, and the countdown is over.</summary>
    bool Ours()
    {
        if (Environment.TickCount64 < _startTick)
            return false;
        IntPtr front = GetForegroundWindow();
        GetWindowThreadProcessId(front, out uint pid);
        return pid == _pid;
    }

    void Add(MacroEvent e, bool action)
    {
        lock (_events)
            _events.Add(e);
        if (action)
        {
            Interlocked.Increment(ref _actions);
            _ui?.Post(_ => Changed?.Invoke(), null);
        }
    }

    // Quick, on the hooks' thread: Windows drops a hook that takes long (LowLevelHooksTimeout).
    IntPtr OnKeyboard(int code, IntPtr wParam, IntPtr lParam)
    {
        if (code >= 0)
        {
            var k = Marshal.PtrToStructure<KBDLLHOOKSTRUCT>(lParam);
            bool up = (k.flags & LLKHF_UP) != 0, injected = (k.flags & LLKHF_INJECTED) != 0;
            if (!injected && k.vkCode == VK_F8)
            {
                if (!up)
                    _ui?.Post(_ =>
                    {
                        // Written on the window's thread: the hook must stay quick. Once: a held F8 repeats.
                        if (!_f8Noted)
                        {
                            _f8Noted = true;
                            Log.Note($"Macro recording: F8 pressed{(StopAsked is null ? ", but nothing listens for it" : "")}.");
                        }
                        StopAsked?.Invoke();
                    }, null);
            }
            else if (!injected && Ours() && MacroKeys.FromScan((ushort)k.scanCode, (k.flags & LLKHF_EXTENDED) != 0, (ushort)k.vkCode) is { } key)
            {
                if (!up && _down.Add(key.Name))
                    Add(new MacroEvent(Now, "keyDown", key.Name), action: true);
                else if (up && _down.Remove(key.Name))
                    Add(new MacroEvent(Now, "keyUp", key.Name), action: false);
            }
        }
        return CallNextHookEx(IntPtr.Zero, code, wParam, lParam);
    }

    IntPtr OnMouse(int code, IntPtr wParam, IntPtr lParam)
    {
        if (code >= 0)
        {
            var m = Marshal.PtrToStructure<MSLLHOOKSTRUCT>(lParam);
            int message = (int)wParam;
            if ((m.flags & LLMHF_INJECTED) == 0 && (Ours() || message is WM_LBUTTONUP or WM_RBUTTONUP or WM_MBUTTONUP))
            {
                var at = new Point(m.pt.X, m.pt.Y);
                switch (message)
                {
                    case WM_MOUSEMOVE:
                        // Without raw input, the camera's turn is guessed from where the pointer goes.
                        if (_rightDown && _raw is null && (at.X != _lastPoint.X || at.Y != _lastPoint.Y))
                            Add(new MacroEvent(Now, "move", Dx: at.X - _lastPoint.X, Dy: at.Y - _lastPoint.Y), action: false);
                        _lastPoint = at;
                        break;
                    case WM_LBUTTONDOWN or WM_RBUTTONDOWN or WM_MBUTTONDOWN:
                        string button = message == WM_LBUTTONDOWN ? "left" : message == WM_RBUTTONDOWN ? "right" : "middle";
                        _rightDown |= button == "right";
                        _lastPoint = at;
                        var down = new MacroEvent(Now, "down", Button: button, At: at);
                        Add(down, action: true);
                        if (button != "middle")
                            _ui?.Post(_ => Picture(down), null);
                        break;
                    case WM_LBUTTONUP or WM_RBUTTONUP or WM_MBUTTONUP:
                        _rightDown &= message != WM_RBUTTONUP;
                        Add(new MacroEvent(Now, "up", Button: message == WM_LBUTTONUP ? "left" : message == WM_RBUTTONUP ? "right" : "middle", At: at), action: false);
                        break;
                    case WM_MOUSEWHEEL:
                        int notches = (short)(m.mouseData >> 16) / 120;
                        if (notches != 0)
                            Add(new MacroEvent(Now, "wheel", Notches: notches), action: true);
                        break;
                }
            }
        }
        return CallNextHookEx(IntPtr.Zero, code, wParam, lParam);
    }

    /// <summary>A person's raw mouse move (RobloxInput's thread): the camera's turn while the right button is held.</summary>
    void OnRawMove(int dx, int dy)
    {
        if (_rightDown && Ours())
            Add(new MacroEvent(Now, "move", Dx: dx, Dy: dy), action: false);
    }

    /// <summary>A picture of where a click landed, for "click by image" (on the window's thread).</summary>
    void Picture(MacroEvent click)
    {
        var at = new Point(click.At.X - Client.X, click.At.Y - Client.Y);
        async Task Take()
        {
            using var picture = await WindowShot.ClientAsync(_window, shown: true);
            if (picture is not null)
                click.Image = MacroRecording.Cut(picture, at);
        }
        lock (_pictures)
        {
            if (_pictures.Count < 100)
                _pictures.Add(Take());
        }
    }

    /// <summary>Stops recording; the steps recorded, as parts of the client area it had when recording began.</summary>
    public async Task<List<MacroStep>> FinishAsync()
    {
        Dispose();
        Task[] pictures;
        lock (_pictures)
            pictures = _pictures.ToArray();
        await Task.WhenAny(Task.WhenAll(pictures), Task.Delay(3000));
        List<MacroEvent> events;
        lock (_events)
            events = _events.ToList();
        var client = Client;
        var steps = MacroRecording.ToSteps(events, p => client.Contains(p) ? ((p.X - client.X) / (double)client.Width, (p.Y - client.Y) / (double)client.Height) : null);
        int failed = pictures.Count(t => t.IsFaulted), unfinished = pictures.Count(t => !t.IsCompleted);
        Log.Note($"Macro recording: finished on window {_pid} with {events.Count} events, {steps.Count} steps; click pictures {pictures.Length}"
            + $"{(failed > 0 ? $", {failed} failed ({pictures.First(t => t.IsFaulted).Exception!.GetBaseException().GetType().Name})" : "")}"
            + $"{(unfinished > 0 ? $", {unfinished} not done in 3 s" : "")}.");
        return steps;
    }

    bool _disposed;

    public void Dispose()
    {
        if (_disposed)
            return;
        _disposed = true;
        if (_raw is not null && _raw.MouseMoved == OnRawMove)
            _raw.MouseMoved = null;
        if (_threadId != 0 && _thread.IsAlive)
        {
            PostThreadMessage(_threadId, WM_QUIT, IntPtr.Zero, IntPtr.Zero);
            if (!_thread.Join(TimeSpan.FromSeconds(2)))
                Log.Note("Macro recording: its hook thread didn't end in 2 s.");
        }
    }

    delegate IntPtr HookProc(int code, IntPtr wParam, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)]
    struct KBDLLHOOKSTRUCT
    {
        public uint vkCode, scanCode, flags, time;
        public IntPtr dwExtraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct MSLLHOOKSTRUCT
    {
        public POINT pt;
        public uint mouseData, flags, time;
        public IntPtr dwExtraInfo;
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

    const int WH_KEYBOARD_LL = 13, WH_MOUSE_LL = 14;
    const uint LLKHF_EXTENDED = 0x01, LLKHF_INJECTED = 0x10, LLKHF_UP = 0x80, LLMHF_INJECTED = 0x01, VK_F8 = 0x77, WM_QUIT = 0x12;
    const int WM_MOUSEMOVE = 0x200, WM_LBUTTONDOWN = 0x201, WM_LBUTTONUP = 0x202, WM_RBUTTONDOWN = 0x204, WM_RBUTTONUP = 0x205,
        WM_MBUTTONDOWN = 0x207, WM_MBUTTONUP = 0x208, WM_MOUSEWHEEL = 0x20A;

    [DllImport("user32.dll", SetLastError = true)] static extern IntPtr SetWindowsHookEx(int id, HookProc proc, IntPtr module, uint thread);
    [DllImport("user32.dll")] static extern bool UnhookWindowsHookEx(IntPtr hook);
    [DllImport("user32.dll")] static extern IntPtr CallNextHookEx(IntPtr hook, int code, IntPtr wParam, IntPtr lParam);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern IntPtr GetModuleHandle(string? name);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern int GetMessage(out MSG message, IntPtr hWnd, uint min, uint max);
    [DllImport("user32.dll")] static extern IntPtr DispatchMessage(ref MSG message);
    [DllImport("user32.dll")] static extern bool PostThreadMessage(uint thread, uint message, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
}
