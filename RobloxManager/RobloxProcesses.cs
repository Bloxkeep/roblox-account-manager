using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace RobloxManager;

/// <summary>
/// Finds Roblox clients and their windows. Uses process enumeration and window messages; a process handle is
/// opened only to terminate a client that ignores WM_CLOSE, and for the opt-in memory trimming.
/// </summary>
public static class RobloxProcesses
{
    const string PlayerProcessName = "RobloxPlayerBeta";
    const uint WM_SETTEXT = 0x000C, WM_CLOSE = 0x0010, SMTO_ABORTIFHUNG = 0x0002, GW_OWNER = 4;
    const uint SWP_NOZORDER = 0x0004, SWP_NOACTIVATE = 0x0010, SWP_ASYNCWINDOWPOS = 0x4000;
    const int SW_RESTORE = 9, SW_SHOWMINNOACTIVE = 7, SW_SHOWMAXIMIZED = 3, GWL_STYLE = -16, WS_CAPTION = 0x00C00000;
    const uint SYNCHRONIZE = 0x00100000, PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;

    [DllImport("user32.dll")]
    static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    static extern bool IsIconic(IntPtr hWnd);

    [DllImport("user32.dll")]
    static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    static extern bool IsZoomed(IntPtr hWnd);

    [DllImport("user32.dll")]
    static extern bool ShowWindowAsync(IntPtr hWnd, int cmd);

    [DllImport("user32.dll")]
    static extern int GetWindowLong(IntPtr hWnd, int index);

    [DllImport("user32.dll")]
    static extern bool SetWindowPos(IntPtr hWnd, IntPtr insertAfter, int x, int y, int cx, int cy, uint flags);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr OpenProcess(uint access, bool inheritHandle, int processId);

    [DllImport("psapi.dll", SetLastError = true)]
    static extern bool EmptyWorkingSet(IntPtr process);

    [DllImport("kernel32.dll")]
    static extern bool CloseHandle(IntPtr handle);

    delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    [DllImport("user32.dll")]
    static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

    [DllImport("user32.dll")]
    static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll")]
    static extern IntPtr GetWindow(IntPtr hWnd, uint cmd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern int GetWindowTextLength(IntPtr hWnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int maxCount);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint msg, IntPtr wParam, string lParam, uint flags, uint timeoutMs, out IntPtr result);

    [DllImport("user32.dll")]
    static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

    /// <param name="Parent">The process that started it (which may have exited since); 0 when not known.</param>
    /// <param name="Created">When it started (a FILETIME, UTC): a process id Windows gave to a newer process has another one. 0 when not known.</param>
    public readonly record struct PlayerProcess(int Pid, long WorkingSet, int Parent = 0, long Created = 0);

    /// <summary>
    /// Running clients with their RAM use, parent and start time. All come from one system-wide process snapshot (the
    /// one Task Manager takes), so no handle to the Roblox process is opened, and one snapshot answers every question
    /// of a process check.
    /// </summary>
    public static List<PlayerProcess> GetPlayers()
    {
        var players = Snapshot() ?? SnapshotByName();
        lock (_created)
        {
            _created.Clear();
            foreach (var p in players)
            {
                if (p.Created != 0)
                    _created[p.Pid] = p.Created;
            }
        }
        return players;
    }

    /// <summary>When each client of the last snapshot started: a pid acted on later must still be that client (see <see cref="Close"/>).</summary>
    static readonly Dictionary<int, long> _created = new();

    /// <summary>The snapshot through .NET, without parents or start times: where <see cref="Snapshot"/> can't read one.</summary>
    static List<PlayerProcess> SnapshotByName()
    {
        Process[] processes;
        try
        {
            processes = Process.GetProcessesByName(PlayerProcessName);
        }
        catch (Exception ex) when (ex is InvalidOperationException or Win32Exception)
        {
            return [];
        }
        try
        {
            return processes.Select(p => new PlayerProcess(p.Id, WorkingSet(p))).ToList();
        }
        finally
        {
            foreach (var p in processes)
                p.Dispose();
        }
    }

    [DllImport("ntdll.dll")]
    static extern int NtQuerySystemInformation(int infoClass, IntPtr buffer, int length, out int returned);

    const int SystemProcessInformation = 5;
    const int StatusInfoLengthMismatch = unchecked((int)0xC0000004);
    /// <summary>The buffer the last snapshot needed (a few hundred KB), so the next one usually takes one call.</summary>
    static int _snapshotSize = 512 * 1024;

    /// <summary>
    /// The clients from Windows' process list (NtQuerySystemInformation, which Process.GetProcessesByName also reads),
    /// with each one's parent and start time, without a string or object per process on the PC. Read with the 64-bit
    /// layout of SYSTEM_PROCESS_INFORMATION (the same Process uses); null in a 32-bit process or when Windows refuses.
    /// </summary>
    static List<PlayerProcess>? Snapshot()
    {
        if (IntPtr.Size != 8)
            return null;
        const int CreateTime = 32, NameLength = 56, NameBuffer = 64, ProcessId = 80, ParentId = 88, WorkingSetSize = 144, EntryEnd = 152;
        const string Image = PlayerProcessName + ".exe";
        for (int attempt = 0; attempt < 4; attempt++)
        {
            int size = Volatile.Read(ref _snapshotSize);
            IntPtr buffer = Marshal.AllocHGlobal(size);
            try
            {
                int status = NtQuerySystemInformation(SystemProcessInformation, buffer, size, out int needed);
                if (status == StatusInfoLengthMismatch)
                {
                    Volatile.Write(ref _snapshotSize, Math.Max(needed, size) + 64 * 1024);
                    continue;
                }
                if (status < 0 || needed > size)
                    return null;
                var players = new List<PlayerProcess>();
                long end = (long)buffer + needed;
                for (int offset = 0; offset + EntryEnd <= needed;)
                {
                    IntPtr entry = buffer + offset;
                    int length = (ushort)Marshal.ReadInt16(entry, NameLength);
                    IntPtr name = Marshal.ReadIntPtr(entry, NameBuffer);
                    if (length == Image.Length * 2 && (long)name >= (long)buffer && (long)name + length <= end && IsImage(name, Image))
                        players.Add(new PlayerProcess((int)Marshal.ReadInt64(entry, ProcessId), Marshal.ReadInt64(entry, WorkingSetSize),
                            (int)Marshal.ReadInt64(entry, ParentId), Marshal.ReadInt64(entry, CreateTime)));
                    int next = Marshal.ReadInt32(entry, 0);
                    if (next <= 0)
                        break;
                    offset += next;
                }
                return players;
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }
        return null;

        static bool IsImage(IntPtr chars, string image)
        {
            for (int i = 0; i < image.Length; i++)
            {
                if (char.ToUpperInvariant((char)Marshal.ReadInt16(chars, i * 2)) != char.ToUpperInvariant(image[i]))
                    return false;
            }
            return true;
        }
    }

    static long WorkingSet(Process p)
    {
        try
        {
            return p.WorkingSet64;
        }
        catch (InvalidOperationException)
        {
            return 0;
        }
    }

    public static int[] GetPlayerPids() => GetPlayers().Select(p => p.Pid).ToArray();

    /// <summary>Visible top-level windows of several processes, found in a single pass over the desktop's windows.</summary>
    public static Dictionary<int, List<IntPtr>> VisibleWindows(IEnumerable<int> pids)
    {
        var result = pids.Distinct().ToDictionary(pid => pid, _ => new List<IntPtr>());
        if (result.Count == 0)
            return result;
        EnumWindows((hWnd, _) =>
        {
            GetWindowThreadProcessId(hWnd, out uint owner);
            if (result.TryGetValue((int)owner, out var list) && IsWindowVisible(hWnd) && GetWindow(hWnd, GW_OWNER) == IntPtr.Zero)
                list.Add(hWnd);
            return true;
        }, IntPtr.Zero);
        return result;
    }

    static List<IntPtr> TopLevelWindows(int pid) => VisibleWindows([pid])[pid];

    static string GetTitle(IntPtr hWnd)
    {
        int length = GetWindowTextLength(hWnd);
        if (length <= 0)
            return "";
        var sb = new StringBuilder(length + 1);
        GetWindowText(hWnd, sb, sb.Capacity);
        return sb.ToString();
    }

    /// <summary>Renames the client's "Roblox" window so several clients can be told apart in the taskbar.</summary>
    public static void SetWindowTitle(int pid, string title) => SetWindowTitle(TopLevelWindows(pid), title);

    public static void SetWindowTitle(IEnumerable<IntPtr> windows, string title)
    {
        foreach (var hWnd in windows)
        {
            string current = GetTitle(hWnd);
            if (current == title || !current.StartsWith("Roblox", StringComparison.Ordinal))
                continue;
            SendMessageTimeout(hWnd, WM_SETTEXT, IntPtr.Zero, title, SMTO_ABORTIFHUNG, 500, out _);
        }
    }

    /// <summary>A client being closed: when to end it if its windows didn't close, how long to end its children for.</summary>
    /// <param name="created">When it started (0 when not known): a process that has its id later is another one.</param>
    sealed class Closing(DateTime? killAt, DateTime until, IntPtr handle, long created)
    {
        public DateTime? KillAt = killAt;
        public DateTime Until = until;
        public IntPtr Handle = handle;
        public readonly long Created = created;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetProcessTimes(IntPtr process, out long creation, out long exit, out long kernel, out long user);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool QueryFullProcessImageName(IntPtr process, int flags, StringBuilder name, ref int size);

    /// <summary>
    /// Whether <paramref name="pid"/> is still the Roblox client the last snapshot saw under it: the id of a client
    /// that ended a moment ago (between two process checks) may already belong to another program, or a new client.
    /// Asked of the handle when Windows gives one (then the id can't change hands meanwhile), else of a fresh snapshot.
    /// </summary>
    /// <param name="created">When the process with this id now started, as far as it is known (0 when not).</param>
    /// <param name="expected">When the client the last snapshot saw started (0 when not known).</param>
    static bool StillTheClient(int pid, IntPtr handle, out long created, out long expected)
    {
        lock (_created)
            _created.TryGetValue(pid, out expected);
        if (handle != IntPtr.Zero && GetProcessTimes(handle, out created, out _, out _, out _))
        {
            var path = new StringBuilder(1024);
            int size = path.Capacity;
            if (QueryFullProcessImageName(handle, 0, path, ref size))
                return Path.GetFileNameWithoutExtension(path.ToString()).Equals(PlayerProcessName, StringComparison.OrdinalIgnoreCase)
                       && (expected == 0 || created == expected);
        }
        // Roblox's anti-tamper may refuse the handle: Windows' process list says the same, without one.
        if (Snapshot() is { } now)
        {
            var found = now.FirstOrDefault(p => p.Pid == pid);
            created = found.Created;
            return found.Pid == pid && (expected == 0 || found.Created == expected);
        }
        created = expected;
        try
        {
            using var process = Process.GetProcessById(pid);
            return process.ProcessName.Equals(PlayerProcessName, StringComparison.OrdinalIgnoreCase);
        }
        catch (Exception ex) when (ex is ArgumentException or InvalidOperationException)
        {
            return false;
        }
    }

    static readonly Dictionary<int, Closing> _closing = new();
    static Task? _sweeper;
    static readonly TimeSpan CloseGrace = TimeSpan.FromSeconds(2), ChildSweep = TimeSpan.FromSeconds(10), SweepTick = TimeSpan.FromMilliseconds(500);

    /// <summary>
    /// Ends a client for good. Normally its windows are asked to close like their X button would, and the process is
    /// terminated if it is still there 2 s later; <paramref name="force"/> terminates it at once (a frozen client).
    /// When a client closes, Roblox starts a copy of itself in the notification area ("--launch-to-tray", seen in
    /// client 0.742's logs) as a child of the closing client; that copy is ended too, so nothing is left running.
    /// Every client being closed shares one sweep, so closing ten takes no more process snapshots than closing one.
    /// </summary>
    public static void Close(int pid, bool force = false)
    {
        // A real handle, held open until the sweep ends, so Windows can't give this pid to a new process
        // meanwhile (its children would then be someone else's). Zero when Roblox's anti-tamper refuses it.
        IntPtr keep = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        // The client ended by itself since the last process check, and its id went to another program (or a new
        // client): nothing is closed. The process check ends its session.
        if (!StillTheClient(pid, keep, out long created, out long expected))
        {
            if (keep != IntPtr.Zero)
                CloseHandle(keep);
            // Nothing of it is left to close, but the copy it may have started in the notification area as it ended
            // is still swept (only clients started after it).
            lock (_closing)
            {
                if (!_closing.ContainsKey(pid))
                    _closing[pid] = new Closing(null, DateTime.UtcNow + ChildSweep, IntPtr.Zero, expected);
                _sweeper ??= Task.Run(SweepClosingAsync);
            }
            return;
        }
        var windows = force ? [] : TopLevelWindows(pid);
        foreach (var hWnd in windows)
            PostMessage(hWnd, WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
        if (windows.Count == 0)
            Kill(pid);
        var now = DateTime.UtcNow;
        lock (_closing)
        {
            if (_closing.TryGetValue(pid, out var again))
            {
                // Closed a second time: the sweep may be looking at the first entry right now and closes its handle
                // when it is done with it, so that entry is kept and stretched; the handle just opened is the spare.
                if (windows.Count > 0 && !(again.KillAt is { } at && at < now + CloseGrace))
                    again.KillAt = now + CloseGrace;
                again.Until = now + ChildSweep;
                if (keep != IntPtr.Zero)
                    CloseHandle(keep);
            }
            else
                _closing[pid] = new Closing(windows.Count > 0 ? now + CloseGrace : null, now + ChildSweep, keep, created);
            _sweeper ??= Task.Run(SweepClosingAsync);
        }
    }

    /// <summary>One process snapshot per tick for every client being closed: ends the stubborn ones and their children.</summary>
    static async Task SweepClosingAsync()
    {
        while (true)
        {
            await Task.Delay(SweepTick);
            try
            {
                SweepOnce();
            }
            // One bad tick must not end the sweep: the loop would be gone for good and later closes never swept.
            catch (Exception ex)
            {
                Log.Write(ex);
            }
            lock (_closing)
            {
                if (_closing.Count == 0)
                {
                    _sweeper = null;
                    return;
                }
            }
        }
    }

    static void SweepOnce()
    {
        List<KeyValuePair<int, Closing>> items;
        lock (_closing)
            items = _closing.ToList();
        var players = Snapshot();
        Dictionary<int, List<int>> tree;
        HashSet<int> alive;
        if (players is null)
            tree = PlayerChildren(out alive);
        else
        {
            tree = PlayerChildren(players);
            alive = players.Select(p => p.Pid).ToHashSet();
        }
        // A pid of a later process than the client being closed (its id went to a new client) is not touched.
        var created = players?.ToDictionary(p => p.Pid, p => p.Created) ?? [];
        bool Same(int pid, long since) => since == 0 || !created.TryGetValue(pid, out long at) || at == since;
        bool Newer(int child, long since) => since == 0 || !created.TryGetValue(child, out long at) || at >= since;
        var now = DateTime.UtcNow;
        foreach (var (pid, closing) in items)
        {
            // Read under the lock: a second Close of the same client (the UI thread) may be stretching the entry.
            DateTime? killAt;
            DateTime until;
            lock (_closing)
                (killAt, until) = (closing.KillAt, closing.Until);
            if (killAt is { } at && now >= at)
            {
                if (alive.Contains(pid) && Same(pid, closing.Created))
                    Kill(pid);
                lock (_closing)
                {
                    if (closing.KillAt == at)
                        closing.KillAt = null;
                }
            }
            foreach (int child in tree.GetValueOrDefault(pid) ?? [])
            {
                if (Newer(child, closing.Created))
                    Kill(child);
            }
            if (now < until)
                continue;
            lock (_closing)
            {
                // Stretched meanwhile: not done with it yet.
                if (closing.Until != until)
                    continue;
                _closing.Remove(pid);
            }
            if (closing.Handle != IntPtr.Zero)
                CloseHandle(closing.Handle);
        }
    }

    const uint TH32CS_SNAPPROCESS = 0x2;

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct PROCESSENTRY32W
    {
        public uint dwSize, cntUsage, th32ProcessID;
        public IntPtr th32DefaultHeapID;
        public uint th32ModuleID, cntThreads, th32ParentProcessID;
        public int pcPriClassBase;
        public uint dwFlags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
        public string szExeFile;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint processId);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    static extern bool Process32FirstW(IntPtr snapshot, ref PROCESSENTRY32W entry);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    static extern bool Process32NextW(IntPtr snapshot, ref PROCESSENTRY32W entry);

    /// <summary>
    /// Every Roblox client by the process that started it (which may have exited already; the record stays), from a
    /// snapshot <see cref="GetPlayers"/> took, so a process check asks Windows once. A client started before the
    /// running process its parent's id now belongs to isn't that process's child (the id was handed on).
    /// </summary>
    public static Dictionary<int, List<int>> PlayerChildren(IReadOnlyList<PlayerProcess> players)
    {
        // A snapshot without parents (taken through .NET): Windows is asked for them.
        if (players.Count > 0 && players.All(p => p.Parent == 0))
            return PlayerChildren(out _);
        var created = players.ToDictionary(p => p.Pid, p => p.Created);
        var tree = new Dictionary<int, List<int>>();
        foreach (var p in players)
        {
            if (p.Parent == 0 || p.Parent == p.Pid
                || created.TryGetValue(p.Parent, out long parentCreated) && parentCreated != 0 && p.Created != 0 && parentCreated > p.Created)
                continue;
            (tree.TryGetValue(p.Parent, out var children) ? children : tree[p.Parent] = new List<int>()).Add(p.Pid);
        }
        return tree;
    }

    /// <param name="alive">The clients running right now.</param>
    public static Dictionary<int, List<int>> PlayerChildren(out HashSet<int> alive)
    {
        var tree = new Dictionary<int, List<int>>();
        alive = new HashSet<int>();
        IntPtr snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if (snapshot == new IntPtr(-1))
            return tree;
        try
        {
            var entry = new PROCESSENTRY32W { dwSize = (uint)Marshal.SizeOf<PROCESSENTRY32W>() };
            for (bool ok = Process32FirstW(snapshot, ref entry); ok; ok = Process32NextW(snapshot, ref entry))
            {
                if (!entry.szExeFile.Equals(PlayerProcessName + ".exe", StringComparison.OrdinalIgnoreCase))
                    continue;
                int pid = (int)entry.th32ProcessID, parent = (int)entry.th32ParentProcessID;
                alive.Add(pid);
                if (parent != pid)
                    (tree.TryGetValue(parent, out var children) ? children : tree[parent] = new List<int>()).Add(pid);
            }
        }
        finally
        {
            CloseHandle(snapshot);
        }
        return tree;
    }

    /// <summary>Brings a client's window to the front, restoring it if minimised.</summary>
    public static void Focus(int pid)
    {
        var hWnd = MainWindow(pid);
        if (hWnd == IntPtr.Zero)
            return;
        if (IsIconic(hWnd))
            ShowWindowAsync(hWnd, SW_RESTORE);
        SetForegroundWindow(hWnd);
    }

    public static bool IsMinimised(IntPtr hWnd) => IsIconic(hWnd);

    /// <summary>
    /// Maximises a window that has a title bar (Roblox in a window; its fullscreen has none and is left alone), unless it
    /// is minimised or maximised already. A window message only; Windows brings the window to the front with it.
    /// </summary>
    /// <returns>false when it left the window as it was.</returns>
    public static bool Maximise(IntPtr hWnd)
    {
        if (hWnd == IntPtr.Zero || IsIconic(hWnd) || IsZoomed(hWnd) || (GetWindowLong(hWnd, GWL_STYLE) & WS_CAPTION) != WS_CAPTION)
            return false;
        return ShowWindowAsync(hWnd, SW_SHOWMAXIMIZED);
    }

    /// <summary>Minimises a window without bringing another one to the front (a window message only).</summary>
    public static void Minimise(IntPtr hWnd)
    {
        if (hWnd != IntPtr.Zero && !IsIconic(hWnd))
            ShowWindowAsync(hWnd, SW_SHOWMINNOACTIVE);
    }

    public static int ForegroundPid()
    {
        IntPtr hWnd = GetForegroundWindow();
        if (hWnd == IntPtr.Zero)
            return 0;
        GetWindowThreadProcessId(hWnd, out uint pid);
        return (int)pid;
    }

    static IntPtr MainWindow(int pid) => MainWindowOf(TopLevelWindows(pid));

    /// <summary>The game window among a client's visible windows (the one titled "Roblox…"), else its first; zero for none.</summary>
    public static IntPtr MainWindowOf(IReadOnlyList<IntPtr> windows) =>
        windows.FirstOrDefault(h => GetTitle(h).StartsWith("Roblox", StringComparison.Ordinal)) is var main && main != IntPtr.Zero
            ? main
            : windows.FirstOrDefault();

    /// <summary>Tiles the clients' windows over a screen area, in the given order. Window messages only.</summary>
    /// <param name="layout">"fill" spreads them over the whole area; "small" / "medium" / "large" are fixed tiles from the top-left.</param>
    /// <param name="skipMinimized">Leave minimised windows as they are (and give them no tile).</param>
    public static async Task<int> ArrangeGridAsync(IReadOnlyList<int> pids, Rectangle area, string layout = "fill", bool skipMinimized = false)
    {
        var windows = pids.Select(MainWindow).Where(h => h != IntPtr.Zero && !(skipMinimized && IsIconic(h))).ToList();
        if (windows.Count == 0)
            return 0;
        bool restored = false;
        foreach (var hWnd in windows)
        {
            if (IsIconic(hWnd) || IsZoomed(hWnd))
            {
                ShowWindowAsync(hWnd, SW_RESTORE);
                restored = true;
            }
        }
        if (restored)
            await Task.Delay(250); // let the restore finish so it doesn't undo the new size

        var (cols, width, height) = GridCells(windows.Count, area.Size, layout);
        for (int i = 0; i < windows.Count; i++)
        {
            SetWindowPos(windows[i], IntPtr.Zero, area.X + i % cols * width, area.Y + i / cols * height, width, height,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
        }
        return windows.Count;
    }

    /// <summary>
    /// Columns and tile size for <paramref name="count"/> windows. "fill": a near-square grid over the whole area.
    /// Fixed tiles: 8 / 6 / 4 across (small / medium / large) at a 8:7 shape, rows from the top-left, the rest of the
    /// screen left free — and more, smaller tiles only when the rows wouldn't fit on the screen.
    /// </summary>
    public static (int Cols, int Width, int Height) GridCells(int count, Size area, string layout)
    {
        int across = layout switch { "small" => 8, "medium" => 6, "large" => 4, _ => 0 };
        if (across == 0 || count <= 0)
        {
            int c = Math.Max(1, (int)Math.Ceiling(Math.Sqrt(count)));
            int r = Math.Max(1, (int)Math.Ceiling(count / (double)c));
            return (c, area.Width / c, area.Height / r);
        }
        for (int cols = across; ; cols++)
        {
            int width = area.Width / cols, height = width * 7 / 8;
            int rows = (int)Math.Ceiling(count / (double)cols);
            if (rows * height <= area.Height || cols >= 64)
                return (cols, width, height);
        }
    }

    /// <summary>Asks Windows to page out a client's idle memory. Roblox's anti-tamper may refuse the handle.</summary>
    [StructLayout(LayoutKind.Sequential)]
    struct MEMORY_PRIORITY_INFORMATION
    {
        public uint MemoryPriority;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool SetProcessInformation(IntPtr process, int infoClass, ref MEMORY_PRIORITY_INFORMATION info, int size);

    /// <summary>
    /// Low memory priority for a window in the background: Windows takes RAM from it first, and only when it runs short
    /// (the same thing Windows does for background apps). Normal again when it comes to the front.
    /// </summary>
    public static bool SetMemoryPriority(int pid, bool background)
    {
        const uint PROCESS_SET_INFORMATION = 0x0200;
        const int ProcessMemoryPriority = 0, MEMORY_PRIORITY_LOW = 2, MEMORY_PRIORITY_NORMAL = 5;
        IntPtr handle = OpenProcess(PROCESS_SET_INFORMATION, false, pid);
        if (handle == IntPtr.Zero)
            return false;
        try
        {
            var info = new MEMORY_PRIORITY_INFORMATION { MemoryPriority = background ? (uint)MEMORY_PRIORITY_LOW : MEMORY_PRIORITY_NORMAL };
            return SetProcessInformation(handle, ProcessMemoryPriority, ref info, Marshal.SizeOf<MEMORY_PRIORITY_INFORMATION>());
        }
        finally
        {
            CloseHandle(handle);
        }
    }

    public static bool TrimMemory(int pid)
    {
        const uint PROCESS_SET_QUOTA = 0x0100;
        IntPtr handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (handle == IntPtr.Zero)
            return false;
        try
        {
            return EmptyWorkingSet(handle);
        }
        finally
        {
            CloseHandle(handle);
        }
    }

    /// <param name="force">End every client at once instead of closing their windows first.</param>
    /// <param name="forceWhen">Pids to end at once even without <paramref name="force"/> (clients in a game).</param>
    public static int CloseAll(bool force = false, Func<int, bool>? forceWhen = null)
    {
        var pids = GetPlayerPids();
        foreach (var pid in pids)
            Close(pid, force || forceWhen?.Invoke(pid) == true);
        return pids.Length;
    }

    public static void Kill(int pid)
    {
        try
        {
            using var process = Process.GetProcessById(pid);
            process.Kill();
        }
        catch (Exception ex) when (ex is ArgumentException or Win32Exception or InvalidOperationException or NotSupportedException)
        {
        }
    }
}
