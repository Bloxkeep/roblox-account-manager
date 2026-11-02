using System.Diagnostics;
using System.Drawing;
using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// The real desktop for macros: Roblox windows found through Bloxkeep's own bookkeeping, a window's turn the way
/// anti-AFK's nudge takes one (AntiAfk.cs: shown unseen when minimised or hidden, brought forward with the foreground
/// thread's input shared, your window given back after), and input sent with SendInput as scan codes (games read
/// those). Nothing here reads or writes Roblox's memory, hooks it, or posts messages to it.
/// </summary>
sealed class MacroDesk(Func<Guid, (int Pid, int Fps, bool InGame)?> client, Func<DateTime?>? personAt) : IMacroDesk
{
    /// <summary>A person's input this recent holds every run back.</summary>
    public static readonly TimeSpan PersonQuiet = TimeSpan.FromSeconds(5);
    /// <summary>Windows takes a moment to make the window the one in front.</summary>
    static readonly TimeSpan FrontTime = TimeSpan.FromMilliseconds(250);

    public DateTime Now => DateTime.UtcNow;

    public Task Delay(int ms, CancellationToken ct) => ms <= 0 ? Task.CompletedTask : Task.Delay(ms, ct);

    public string? Hold()
    {
        if (AntiAfk.Locked())
            return "locked";
        // Raw input names the device (a program's input has none); GetLastInputInfo with our own sends left out.
        if (AntiAfk.IdleTime() < PersonQuiet || personAt?.Invoke() is { } at && DateTime.UtcNow - at < PersonQuiet)
            return "personActive";
        // A Roblox window in full screen in front is no reason to wait.
        if (AntiAfk.FullScreenInFront() && !IsRoblox(GetForegroundWindow()))
            return "fullscreen";
        return null;
    }

    static bool IsRoblox(IntPtr hWnd)
    {
        if (hWnd == IntPtr.Zero)
            return false;
        GetWindowThreadProcessId(hWnd, out uint pid);
        return RobloxProcesses.GetPlayerPids().Contains((int)pid);
    }

    readonly Dictionary<int, bool> _elevated = new();

    public MacroWindow? Window(Guid accountId)
    {
        if (client(accountId) is not { } c)
            return null;
        var hWnd = RobloxProcesses.MainWindowOf(RobloxProcesses.VisibleWindows([c.Pid])[c.Pid]);
        if (hWnd == IntPtr.Zero || !IsWindow(hWnd))
            return null;
        if (!_elevated.TryGetValue(c.Pid, out bool elevated))
        {
            if (_elevated.Count > 64)
                _elevated.Clear();
            _elevated[c.Pid] = elevated = Elevated(c.Pid);
        }
        return new MacroWindow(accountId, c.Pid, hWnd, ClientOnScreen(hWnd), c.Fps, c.InGame, elevated);
    }

    internal static Rectangle ClientOnScreen(IntPtr hWnd)
    {
        if (!GetClientRect(hWnd, out var r))
            return Rectangle.Empty;
        var origin = new POINT();
        ClientToScreen(hWnd, ref origin);
        return new Rectangle(origin.X, origin.Y, r.Right - r.Left, r.Bottom - r.Top);
    }

    /// <summary>The process runs at a higher integrity level than Bloxkeep (as administrator): Windows drops our input to it.</summary>
    static bool Elevated(int pid)
    {
        try
        {
            using var self = Process.GetCurrentProcess();
            int? theirs = Integrity(pid), ours = Integrity(self.Id);
            return theirs is int t && ours is int o && t > o;
        }
        catch (Exception ex) when (ex is InvalidOperationException or System.ComponentModel.Win32Exception)
        {
            return false;
        }
    }

    static int? Integrity(int pid)
    {
        const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000, TOKEN_QUERY = 0x8;
        const int TokenIntegrityLevel = 25;
        IntPtr process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (process == IntPtr.Zero)
            return null;
        try
        {
            if (!OpenProcessToken(process, TOKEN_QUERY, out IntPtr token))
                return null;
            try
            {
                GetTokenInformation(token, TokenIntegrityLevel, IntPtr.Zero, 0, out int size);
                if (size <= 0)
                    return null;
                IntPtr buffer = Marshal.AllocHGlobal(size);
                try
                {
                    if (!GetTokenInformation(token, TokenIntegrityLevel, buffer, size, out _))
                        return null;
                    // TOKEN_MANDATORY_LABEL: its SID's last sub-authority is the level.
                    IntPtr sid = Marshal.ReadIntPtr(buffer);
                    int count = Marshal.ReadByte(GetSidSubAuthorityCount(sid));
                    return Marshal.ReadInt32(GetSidSubAuthority(sid, (uint)(count - 1)));
                }
                finally
                {
                    Marshal.FreeHGlobal(buffer);
                }
            }
            finally
            {
                CloseHandle(token);
            }
        }
        finally
        {
            CloseHandle(process);
        }
    }

    DateTime _imeCheckedAt;
    bool _ime;

    /// <summary>UniKey or EVKey running (looked for every 30 s): with Telex on they rewrite W, A, O, E, D, S, F, R, X, J, Z.</summary>
    public bool ImeOn()
    {
        if (DateTime.UtcNow - _imeCheckedAt < TimeSpan.FromSeconds(30))
            return _ime;
        _imeCheckedAt = DateTime.UtcNow;
        _ime = false;
        foreach (var name in new[] { "UniKeyNT", "UniKey", "EVKey64", "EVKey32", "EVKey" })
        {
            var found = Process.GetProcessesByName(name);
            foreach (var p in found)
                p.Dispose();
            if (found.Length > 0)
                _ime = true;
        }
        return _ime;
    }

    public async Task<Bitmap?> PictureAsync(MacroWindow window, IMacroTurn? turn) =>
        await WindowShot.ClientAsync(window.Handle, shown: turn is not null);

    /// <summary>A window in view: copied from the screen (a fraction of PrintWindow's time, for many looks a second;
    /// Bloxkeep's own see-through windows aren't in the copy). A window shown unseen isn't on the screen: pictured whole.</summary>
    public async Task<Bitmap?> AreaAsync(MacroWindow window, IMacroTurn turn, Rectangle area)
    {
        if (area.Width <= 0 || area.Height <= 0)
            return null;
        if (turn is Turn { Minimized: false, Unshown: false } t)
        {
            var client = t.Client;
            var shot = new Bitmap(area.Width, area.Height, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
            try
            {
                using var g = Graphics.FromImage(shot);
                g.CopyFromScreen(client.X + area.X, client.Y + area.Y, 0, 0, area.Size);
                return shot;
            }
            catch (Exception) // the screen locked, a secure desktop
            {
                shot.Dispose();
                return null;
            }
        }
        using var whole = await WindowShot.ClientAsync(window.Handle, shown: true);
        if (whole is null)
            return null;
        var r = Rectangle.Intersect(area, new Rectangle(Point.Empty, whole.Size));
        return r.Width > 0 && r.Height > 0 ? whole.Clone(r, whole.PixelFormat) : null;
    }

    public Task<List<List<MacroOcr.Line>>?> ReadAsync(Bitmap picture, Rectangle area)
    {
        // The picture is the caller's: read from a copy on the worker.
        var copy = (Bitmap)picture.Clone();
        return Task.Run(() =>
        {
            using (copy)
                return MacroOcr.Read(copy, area);
        });
    }

    public async Task<IMacroTurn?> BeginTurnAsync(MacroWindow window, CancellationToken ct)
    {
        IntPtr hWnd = window.Handle;
        if (!IsWindow(hWnd) || IsHungAppWindow(hWnd))
            return null;
        uint person = AntiAfk.PersonTick();
        var turn = new Turn(hWnd, GetForegroundWindow(), person, personAt);
        GetCursorPos(out turn.Cursor);
        bool minimized = IsIconic(hWnd), unshown = !IsWindowVisible(hWnd);
        var place = AntiAfk.Place(hWnd);
        // Minimised or hidden: shown unseen (but not click-through: the macro's clicks must land on it).
        turn.Before = minimized || unshown ? AntiAfk.HideForInput(hWnd, place, minimized) : AntiAfk.Note(hWnd, place);
        turn.Minimized = minimized;
        turn.Unshown = unshown;
        if (minimized)
            ShowWindowAsync(hWnd, SW_RESTORE);
        else if (unshown)
            ShowWindowAsync(hWnd, SW_SHOWNOACTIVATE);
        try
        {
            // A zero move first: Windows lets the app that last sent input change the window in front.
            turn.Send(Mouse(0, 0, MOUSEEVENTF_MOVE));
            AntiAfk.Focus(hWnd);
            var waited = Stopwatch.StartNew();
            while (GetForegroundWindow() != hWnd && waited.Elapsed < FrontTime)
            {
                await Task.Delay(10, ct);
                AntiAfk.Focus(hWnd);
            }
            if (GetForegroundWindow() != hWnd)
            {
                await turn.DisposeAsync();
                return null;
            }
            // A restored window needs a moment to draw again (as anti-AFK gives it); one in view, a frame.
            await Task.Delay(minimized || unshown ? 400 : Math.Max(17, window.Fps > 0 ? 1000 / window.Fps : 17), ct);
            turn.Client = ClientOnScreen(hWnd);
            return turn;
        }
        catch
        {
            await turn.DisposeAsync();
            throw;
        }
    }

    /// <summary>A window in front for a macro's input.</summary>
    sealed class Turn(IntPtr hWnd, IntPtr previous, uint person, Func<DateTime?>? personAt) : IMacroTurn
    {
        public POINT Cursor;
        public AntiAfk.Before? Before;
        public bool Minimized, Unshown;
        readonly DateTime _started = DateTime.UtcNow;
        bool _sent, _done;
        /// <summary>GetLastInputInfo's tick when we last sent; input processed well after it is a person's.</summary>
        uint _lastSent = (uint)Environment.TickCount;

        public Rectangle Client { get; set; }

        public bool StillFront() => GetForegroundWindow() == hWnd && IsWindow(hWnd);

        public bool PersonInput()
        {
            // Raw input knows a device's input from a program's; without it, input processed long after our last send.
            if (personAt is not null)
                return personAt() is { } at && at > _started;
            uint last = AntiAfk.LastInputTick();
            return unchecked((int)(last - _lastSent)) > 150;
        }

        public void Send(params INPUT[] inputs)
        {
            SendInput((uint)inputs.Length, inputs, Marshal.SizeOf<INPUT>());
            _sent = true;
            _lastSent = (uint)Environment.TickCount;
        }

        public void Key(MacroKey key, bool up) =>
            Send(Keyboard(key.Vk, key.Scan, KEYEVENTF_SCANCODE | (key.Extended ? KEYEVENTF_EXTENDEDKEY : 0) | (up ? KEYEVENTF_KEYUP : 0)));

        public void Char(char c, bool up) => Send(Keyboard(0, c, KEYEVENTF_UNICODE | (up ? KEYEVENTF_KEYUP : 0)));

        public void MoveTo(Point screen)
        {
            var (x, y) = MacroGeometry.Absolute(screen, VirtualDesktop());
            Send(Mouse(x, y, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK));
        }

        public void MoveBy(int dx, int dy) => Send(Mouse(dx, dy, MOUSEEVENTF_MOVE));

        public void Button(string button, bool up) => Send(Mouse(0, 0, button switch
        {
            "right" => up ? MOUSEEVENTF_RIGHTUP : MOUSEEVENTF_RIGHTDOWN,
            "middle" => up ? MOUSEEVENTF_MIDDLEUP : MOUSEEVENTF_MIDDLEDOWN,
            _ => up ? MOUSEEVENTF_LEFTUP : MOUSEEVENTF_LEFTDOWN,
        }));

        public void Wheel(int notches) => Send(Mouse(0, 0, MOUSEEVENTF_WHEEL, unchecked((uint)(notches * 120))));

        /// <summary>Bloxkeep's own windows made click-through for this turn, with the extended style each had.</summary>
        readonly List<(IntPtr Window, int Style)> _through = new();

        public bool Clear(Point screen)
        {
            for (int i = 0; i < 8; i++)
            {
                IntPtr root = GetAncestor(WindowFromPoint(new POINT { X = screen.X, Y = screen.Y }), GA_ROOT);
                if (root == IntPtr.Zero || root == hWnd)
                    return true;
                GetWindowThreadProcessId(root, out uint pid);
                if (pid != (uint)Environment.ProcessId || _through.Any(w => w.Window == root))
                    return false; // another app's window, or ours that won't let clicks through
                int style = GetWindowLong(root, GWL_EXSTYLE);
                _through.Add((root, style));
                SetWindowLong(root, GWL_EXSTYLE, style | WS_EX_LAYERED | WS_EX_TRANSPARENT);
                // Newly layered, a window shows nothing until it is given its opacity.
                if ((style & WS_EX_LAYERED) == 0)
                    SetLayeredWindowAttributes(root, 0, 255, LWA_ALPHA);
            }
            return false;
        }

        public Point CursorAt() => GetCursorPos(out var p) ? new Point(p.X, p.Y) : Point.Empty;

        /// <summary>The pointer back where it was, your window back in front, the Roblox window back in its place and
        /// state, and our input noted as ours (anti-AFK and the next turn don't take it for a person's).</summary>
        public async ValueTask DisposeAsync()
        {
            if (_done)
                return;
            _done = true;
            foreach (var (window, style) in _through)
            {
                if (IsWindow(window))
                    SetWindowLong(window, GWL_EXSTYLE, style);
            }
            _through.Clear();
            SetCursorPos(Cursor.X, Cursor.Y);
            AntiAfk.GiveBack(previous);
            if (Before is { } before)
            {
                AntiAfk.PutBack(before);
                if (Minimized)
                    ShowWindowAsync(hWnd, SW_SHOWMINNOACTIVE);
                if (Unshown)
                    ShowWindowAsync(hWnd, SW_HIDE);
                for (int i = 0; i < 10 && IsWindow(hWnd) && (Minimized && !IsIconic(hWnd) || Unshown && IsWindowVisible(hWnd)); i++)
                    await Task.Delay(50);
                if (Minimized || Unshown)
                    AntiAfk.Show(before);
            }
            if (_sent)
            {
                await Task.Delay(30); // our last input reaches GetLastInputInfo
                AntiAfk.NoteOwnInput(person);
            }
        }
    }

    static Rectangle VirtualDesktop() =>
        new(GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN), GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));

    static INPUT Keyboard(ushort vk, ushort scan, uint flags) =>
        new() { type = INPUT_KEYBOARD, u = new INPUTUNION { ki = new KEYBDINPUT { wVk = vk, wScan = scan, dwFlags = flags } } };

    static INPUT Mouse(int dx, int dy, uint flags, uint data = 0) =>
        new() { type = INPUT_MOUSE, u = new INPUTUNION { mi = new MOUSEINPUT { dx = dx, dy = dy, dwFlags = flags, mouseData = data } } };

    const uint INPUT_MOUSE = 0, INPUT_KEYBOARD = 1;
    const uint KEYEVENTF_EXTENDEDKEY = 0x1, KEYEVENTF_KEYUP = 0x2, KEYEVENTF_UNICODE = 0x4, KEYEVENTF_SCANCODE = 0x8;
    const uint MOUSEEVENTF_MOVE = 0x1, MOUSEEVENTF_LEFTDOWN = 0x2, MOUSEEVENTF_LEFTUP = 0x4, MOUSEEVENTF_RIGHTDOWN = 0x8, MOUSEEVENTF_RIGHTUP = 0x10,
        MOUSEEVENTF_MIDDLEDOWN = 0x20, MOUSEEVENTF_MIDDLEUP = 0x40, MOUSEEVENTF_WHEEL = 0x800, MOUSEEVENTF_VIRTUALDESK = 0x4000, MOUSEEVENTF_ABSOLUTE = 0x8000;
    const int SM_XVIRTUALSCREEN = 76, SM_YVIRTUALSCREEN = 77, SM_CXVIRTUALSCREEN = 78, SM_CYVIRTUALSCREEN = 79;
    const int SW_RESTORE = 9, SW_SHOWMINNOACTIVE = 7, SW_SHOWNOACTIVATE = 4, SW_HIDE = 0;

    [StructLayout(LayoutKind.Sequential)]
    internal struct POINT
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
    struct KEYBDINPUT
    {
        public ushort wVk, wScan;
        public uint dwFlags, time;
        public IntPtr dwExtraInfo;
    }

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

    [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint count, INPUT[] inputs, int size);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool IsHungAppWindow(IntPtr hWnd);
    [DllImport("user32.dll")] static extern bool ShowWindowAsync(IntPtr hWnd, int command);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
    [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT point);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int index);
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT point);
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr hWnd, uint flags);
    [DllImport("user32.dll", EntryPoint = "GetWindowLongW")] static extern int GetWindowLong(IntPtr hWnd, int index);
    [DllImport("user32.dll", EntryPoint = "SetWindowLongW")] static extern int SetWindowLong(IntPtr hWnd, int index, int value);
    [DllImport("user32.dll")] static extern bool SetLayeredWindowAttributes(IntPtr hWnd, uint key, byte alpha, uint flags);
    const uint GA_ROOT = 2, LWA_ALPHA = 2;
    const int GWL_EXSTYLE = -20, WS_EX_LAYERED = 0x80000, WS_EX_TRANSPARENT = 0x20;
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool GetTokenInformation(IntPtr token, int infoClass, IntPtr info, int length, out int returned);
    [DllImport("advapi32.dll")] static extern IntPtr GetSidSubAuthorityCount(IntPtr sid);
    [DllImport("advapi32.dll")] static extern IntPtr GetSidSubAuthority(IntPtr sid, uint index);
}
