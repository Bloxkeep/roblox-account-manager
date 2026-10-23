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
    const uint SWP_NOSIZE = 0x0001, SWP_NOMOVE = 0x0002, SWP_NOZORDER = 0x0004, SWP_NOACTIVATE = 0x0010, SWP_NOSENDCHANGING = 0x0400,
        SWP_ASYNCWINDOWPOS = 0x4000;
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
    /// <param name="Private">The memory it holds for itself (its private bytes, its share of Windows' commit limit); 0 when not known.</param>
    public readonly record struct PlayerProcess(int Pid, long WorkingSet, int Parent = 0, long Created = 0, long Private = 0);

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
            return processes.Select(p => new PlayerProcess(p.Id, WorkingSet(p), Private: PrivateBytes(p))).ToList();
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
        const int CreateTime = 32, NameLength = 56, NameBuffer = 64, ProcessId = 80, ParentId = 88, WorkingSetSize = 144,
            PrivatePageCount = 200, EntryEnd = 208;
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
                            (int)Marshal.ReadInt64(entry, ParentId), Marshal.ReadInt64(entry, CreateTime), Marshal.ReadInt64(entry, PrivatePageCount)));
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

    static long PrivateBytes(Process p)
    {
        try
        {
            return p.PrivateMemorySize64;
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
    /// The executable path as supplied at launch. QueryFullProcessImageName resolves junctions, hiding which alias
    /// was used; the command line retains it. Only the first argument is returned, never tickets in later arguments.
    /// </summary>
    public static string? LaunchPath(int pid, long expectedCreation)
    {
        string? line = CommandLineOf(pid, expectedCreation)?.TrimStart();
        if (string.IsNullOrEmpty(line))
            return null;
        int end = line[0] == '"' ? line.IndexOf('"', 1) : line.IndexOfAny([' ', '\t']);
        if (line[0] == '"' && end < 1)
            return null;
        string path = line[0] == '"' ? line[1..end] : end < 0 ? line : line[..end];
        return Path.IsPathFullyQualified(path) ? path : null;
    }

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

    /// <summary>The smallest and largest window the "custom" layout gives.</summary>
    public static readonly Size LeastWindow = new(160, 120), MostWindow = new(7680, 4320);

    /// <summary>A layout Arrange windows knows (see <see cref="Settings.WindowLayout"/>): grids up to 8 × 8.</summary>
    public static bool IsLayout(string layout) =>
        layout is "fill" or "small" or "medium" or "large" or "custom" || GridOf(layout) is not null;

    /// <summary>A grid layout's columns and rows ("3x2"), else null.</summary>
    static (int Cols, int Rows)? GridOf(string layout) =>
        layout.Length == 3 && layout[1] == 'x' && layout[0] is >= '1' and <= '8' && layout[2] is >= '1' and <= '8'
            ? (layout[0] - '0', layout[2] - '0') : null;

    /// <summary>Tiles the clients' windows over a screen area, in the given order. Window messages only.</summary>
    /// <param name="layout">"fill" spreads them over the whole area; a grid ("2x2") gives each one of its cells from the
    /// top-left; "custom" gives each window <paramref name="custom"/>; "small" / "medium" / "large" are fixed tiles.</param>
    /// <param name="skipMinimized">Leave minimised windows as they are (and give them no tile).</param>
    /// <param name="app">Bloxkeep's window, when Arrange windows was clicked in it: the windows come over every other app
    /// (<see cref="RaiseAsync"/>). Zero leaves the order of the PC's windows as it is (tiling after a launch).</param>
    public static Task<int> ArrangeGridAsync(IReadOnlyList<int> pids, Rectangle area, string layout = "fill", bool skipMinimized = false,
        IntPtr app = default, Size custom = default) =>
        ArrangeAsync(pids.Select(MainWindow).Where(h => h != IntPtr.Zero && !(skipMinimized && IsIconic(h))).ToList(), area, layout, app, custom);

    /// <summary>
    /// Tiles these windows (see <see cref="ArrangeGridAsync"/>). Roblox keeps its window at least 800 × 600 in its own
    /// units (1224 × 957 pixels on a 150 % screen) by answering the "changing" message a move sends: tiles smaller than
    /// that left the windows at that size, over each other and past the screen's edge (5 windows on a 150 % laptop
    /// screen, 2026-10-21). The tiles are given without that message (SWP_NOSENDCHANGING), so Roblox takes them and draws
    /// the game at that size (tried on the owner's PC, 853 × 764, the same day). A window that grows back all the same is
    /// spread evenly over the area instead (<see cref="OverlapSpots"/>), each over the one before it, so every title bar
    /// shows; so are more windows than a grid has cells, or "custom" windows too many to fit side by side.
    /// </summary>
    static async Task<int> ArrangeAsync(List<IntPtr> windows, Rectangle area, string layout, IntPtr app = default, Size custom = default)
    {
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

        var (cols, width, height) = GridCells(windows.Count, area.Size, layout, custom);
        var cell = new Size(width, height);
        if ((windows.Count + cols - 1) / cols * height <= area.Height)
        {
            for (int i = 0; i < windows.Count; i++)
            {
                SetWindowPos(windows[i], IntPtr.Zero, area.X + i % cols * width, area.Y + i / cols * height, width, height,
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSENDCHANGING | SWP_ASYNCWINDOWPOS);
            }
        }
        else
            await StackAsync(windows, OverlapSpots(windows.Count, area, cell), cell, SWP_NOSENDCHANGING);
        if (await KeptSizeAsync(windows, cell) is { } size)
            await StackAsync(windows, OverlapSpots(windows.Count, area, size), size);
        if (app != IntPtr.Zero)
            await RaiseAsync(windows, app);
        return windows.Count;
    }

    /// <summary>
    /// The windows at these spots, overlapping: the last one keeps its place among the PC's other windows; each one before
    /// it goes just under the next, in turn (each moves on its own window's thread), so a later one lies over an earlier
    /// one's body, not its title bar.
    /// </summary>
    static async Task StackAsync(List<IntPtr> windows, List<Point> spots, Size size, uint flags = 0)
    {
        for (int i = windows.Count - 1; i >= 0; i--)
        {
            bool last = i == windows.Count - 1;
            SetWindowPos(windows[i], last ? IntPtr.Zero : windows[i + 1], spots[i].X, spots[i].Y, size.Width, size.Height,
                flags | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS | (last ? SWP_NOZORDER : 0));
            await Task.Delay(40);
        }
    }

    /// <summary>
    /// The windows over every other app at once (clicking each grouped taskbar button in turn was the only way,
    /// 2026-10-21): each just under the next, the last just under <paramref name="app"/>, Bloxkeep's window (Windows
    /// lets an app stack windows under the one in front, never over it: tried on the owner's PC), then that window under
    /// them all and the last one in front (Bloxkeep, in front as it was clicked, may hand that on).
    /// </summary>
    static async Task RaiseAsync(List<IntPtr> windows, IntPtr app)
    {
        IntPtr above = app;
        for (int i = windows.Count - 1; i >= 0; i--)
        {
            SetWindowPos(windows[i], above, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
            await Task.Delay(40); // each in its turn: each moves on its own window's thread
            above = windows[i];
        }
        SetWindowPos(app, windows[0], 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        SetForegroundWindow(windows[^1]);
    }

    [StructLayout(LayoutKind.Sequential)]
    struct RECT { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll")]
    static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")]
    static extern uint GetDpiForWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    static extern int GetSystemMetricsForDpi(int index, uint dpi);

    /// <summary>A window's place on the screen (its whole frame); null once it is gone.</summary>
    public static Rectangle? WindowRect(IntPtr hWnd) =>
        GetWindowRect(hWnd, out var r) ? Rectangle.FromLTRB(r.Left, r.Top, r.Right, r.Bottom) : null;

    /// <summary>Whether a window is a Roblox client's (RobloxPlayerBeta).</summary>
    public static bool IsPlayerWindow(IntPtr hWnd)
    {
        GetWindowThreadProcessId(hWnd, out uint pid);
        try
        {
            using var p = Process.GetProcessById((int)pid);
            return p.ProcessName.Equals(PlayerProcessName, StringComparison.OrdinalIgnoreCase);
        }
        catch (Exception ex) when (ex is ArgumentException or InvalidOperationException)
        {
            return false;
        }
    }

    /// <summary>How far in from a window's frame the mouse grabs an edge to size it (Windows' resize border, at the window's DPI).</summary>
    public static int ResizeBorder(IntPtr hWnd)
    {
        const int SM_CXSIZEFRAME = 32, SM_CXPADDEDBORDER = 92;
        uint dpi = GetDpiForWindow(hWnd) is > 0 and var d ? d : 96;
        return GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) + 2;
    }

    /// <summary>A window put there at that size, as Arrange gives its tiles (without Roblox's least size).</summary>
    public static void PlaceWindow(IntPtr hWnd, Rectangle r) =>
        SetWindowPos(hWnd, IntPtr.Zero, r.X, r.Y, r.Width, r.Height, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSENDCHANGING | SWP_ASYNCWINDOWPOS);

    /// <summary>
    /// The size a window's edge was dragged to, when the window kept a bigger one (Roblox's least size); null when it took
    /// what the mouse asked or the drag moved it. The edges grabbed are those the mouse was on at the start (within
    /// <paramref name="border"/> of the frame); each moves as far as the mouse did, no smaller than <see cref="LeastWindow"/>.
    /// </summary>
    public static Rectangle? AfterDrag(Rectangle start, Rectangle end, Point grab, Point release, int border)
    {
        bool left = grab.X < start.Left + border, right = grab.X >= start.Right - border;
        bool top = grab.Y < start.Top + border, bottom = grab.Y >= start.Bottom - border;
        if (!(left || right || top || bottom))
            return null;
        int dx = release.X - grab.X, dy = release.Y - grab.Y;
        int l = start.Left + (left ? dx : 0), r = start.Right + (right ? dx : 0);
        int t = start.Top + (top ? dy : 0), b = start.Bottom + (bottom ? dy : 0);
        if (r - l < LeastWindow.Width)
        {
            if (left) l = r - LeastWindow.Width;
            else r = l + LeastWindow.Width;
        }
        if (b - t < LeastWindow.Height)
        {
            if (top) t = b - LeastWindow.Height;
            else b = t + LeastWindow.Height;
        }
        var meant = Rectangle.FromLTRB(l, t, r, b);
        return meant.Width < end.Width || meant.Height < end.Height ? meant : null;
    }

    /// <summary>
    /// The size the windows kept when they were given <paramref name="cell"/>, when they refused to get that small (a
    /// window's least size); null when they took it. Read until it holds (each window moves on its own thread): the
    /// smallest of them, as one still moving is larger, never smaller.
    /// </summary>
    static async Task<Size?> KeptSizeAsync(List<IntPtr> windows, Size cell)
    {
        const int Slack = 2;
        Size? last = null;
        for (int tries = 0; tries < 10; tries++)
        {
            await Task.Delay(100);
            var sizes = windows.Select(h => GetWindowRect(h, out var r) ? new Size(r.Right - r.Left, r.Bottom - r.Top) : Size.Empty)
                .Where(s => !s.IsEmpty).ToList();
            if (sizes.Count == 0)
                return null;
            var least = new Size(sizes.Min(s => s.Width), sizes.Min(s => s.Height));
            if (least.Width <= cell.Width + Slack && least.Height <= cell.Height + Slack)
                return null;
            var kept = new Size(Math.Max(least.Width, cell.Width), Math.Max(least.Height, cell.Height));
            if (kept == last)
                return kept;
            last = kept;
        }
        return last;
    }

    /// <summary>
    /// Where <paramref name="count"/> windows of <paramref name="size"/> go when they don't fit side by side: a
    /// near-square grid's columns and rows spread evenly from the top-left to the bottom-right of <paramref name="area"/>
    /// (never further apart than side by side), so every window stays on the screen and overlaps the next ones evenly.
    /// </summary>
    public static List<Point> OverlapSpots(int count, Rectangle area, Size size)
    {
        int cols = Math.Max(1, (int)Math.Ceiling(Math.Sqrt(count)));
        int rows = Math.Max(1, (int)Math.Ceiling(count / (double)cols));
        int w = Math.Min(size.Width, area.Width), h = Math.Min(size.Height, area.Height);
        int stepX = cols > 1 ? Math.Min(w, (area.Width - w) / (cols - 1)) : 0;
        int stepY = rows > 1 ? Math.Min(h, (area.Height - h) / (rows - 1)) : 0;
        return Enumerable.Range(0, count).Select(i => new Point(area.X + i % cols * stepX, area.Y + i / cols * stepY)).ToList();
    }

    /// <summary>
    /// Columns and tile size for <paramref name="count"/> windows. "fill": a near-square grid over the whole area. A grid
    /// ("4x4"): its cells, whatever the count (more windows than cells don't fit, see <see cref="ArrangeAsync"/>).
    /// "custom": <paramref name="custom"/>, held to the screen, as many across as fit. Fixed tiles: 8 / 6 / 4 across (small / medium / large) at a 8:7 shape, rows from the top-left, the rest of the
    /// screen left free — and more, smaller tiles only when the rows wouldn't fit on the screen.
    /// </summary>
    public static (int Cols, int Width, int Height) GridCells(int count, Size area, string layout, Size custom = default)
    {
        if (GridOf(layout) is var (gc, gr))
            return (gc, area.Width / gc, area.Height / gr);
        if (layout == "custom" && !custom.IsEmpty)
        {
            int w = Math.Clamp(custom.Width, LeastWindow.Width, Math.Max(LeastWindow.Width, area.Width));
            int h = Math.Clamp(custom.Height, LeastWindow.Height, Math.Max(LeastWindow.Height, area.Height));
            return (Math.Max(1, area.Width / w), w, h);
        }
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

    [DllImport("kernel32.dll")]
    static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);

    /// <summary>
    /// Whether the process has ended (no process has the id any more, or it has exited while a handle still holds it,
    /// as Bloxkeep's own close keeps one): a change to a client that failed then had nothing left to change, and
    /// Windows refused nothing. Pid lists come from the last scan, so a client may have closed since.
    /// </summary>
    public static bool Ended(int pid)
    {
        const int ERROR_INVALID_PARAMETER = 87;
        IntPtr handle = OpenProcess(SYNCHRONIZE, false, pid);
        if (handle == IntPtr.Zero)
            return Marshal.GetLastWin32Error() == ERROR_INVALID_PARAMETER;
        try
        {
            return WaitForSingleObject(handle, 0) == 0; // WAIT_OBJECT_0: exited
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

    [DllImport("ntdll.dll")]
    static extern int NtQueryInformationProcess(IntPtr process, int infoClass, IntPtr buffer, int length, out int returned);

    const int ProcessCommandLineInformation = 60;
    const uint ProcessQueryLimitedInformation = 0x1000;
    /// <summary>Clients already looked at, by pid and start time (a pid Windows gives to a newer process is asked again).</summary>
    static readonly Dictionary<(int Pid, long Created), bool> _trayCopies = new();

    /// <summary>
    /// The copy Roblox leaves in the notification area when its window closes (0.743, seen 2026-10-16 with the website
    /// client, through Bloxstrap and from the Microsoft Store): the closing client logs "Successfully launched new process
    /// in systray mode" and starts "RobloxPlayerBeta.exe --launch-to-tray", whose log says "AppState/TrayMode". It has
    /// no window and gets none; a Roblox started while it runs hands its launch to it and quits ("NoReload Detector:
    /// WebPlay NoReload with Tray only!"), so no window opens. Told by its command line.
    /// </summary>
    public static bool IsTrayCopy(int pid, long created)
    {
        lock (_trayCopies)
        {
            if (_trayCopies.TryGetValue((pid, created), out bool known))
                return known;
            if (_trayCopies.Count > 200)
                _trayCopies.Clear();
        }
        bool tray = CommandLineOf(pid)?.Contains("--launch-to-tray", StringComparison.OrdinalIgnoreCase) == true;
        lock (_trayCopies)
            _trayCopies[(pid, created)] = tray;
        return tray;
    }

    /// <summary>Roblox's updater, which an outdated client starts and hands its launch to.</summary>
    const string UpdaterProcessName = "RobloxPlayerInstaller";

    /// <summary>
    /// Whether Roblox's updater (RobloxPlayerInstaller.exe) is running, started at or after <paramref name="sinceUtc"/>.
    /// An outdated client starts it and quits, and the updated client comes from it: the client's own lines
    /// ("[FLog::UpdateController] WindowsUpdateController: updaterFullPath: …", "Update thread: start …", the updater
    /// named RobloxPlayerInstaller.exe in it, 0.744). Where Bloxstrap installs Roblox the client finds no updater
    /// ("Updater not found. Skipping update", every log on the owner's PC): Bloxstrap updates before starting it.
    /// </summary>
    public static bool UpdaterRunningSince(DateTime sinceUtc)
    {
        Process[] processes;
        try
        {
            processes = Process.GetProcessesByName(UpdaterProcessName);
        }
        catch (Exception ex) when (ex is InvalidOperationException or Win32Exception)
        {
            return false;
        }
        try
        {
            return processes.Any(p =>
            {
                try
                {
                    return p.StartTime.ToUniversalTime() >= sinceUtc;
                }
                catch (Exception ex) when (ex is InvalidOperationException or Win32Exception)
                {
                    return false; // gone meanwhile, or not ours to read
                }
            });
        }
        finally
        {
            foreach (var p in processes)
                p.Dispose();
        }
    }

    static readonly Dictionary<(int Pid, long Created), string> _trackers = new();
    static readonly System.Text.RegularExpressions.Regex TrackerText = new(@"browsertrackerid[:=](\d+)|(?<=\s)-b\s+(\d+)",
        System.Text.RegularExpressions.RegexOptions.IgnoreCase | System.Text.RegularExpressions.RegexOptions.CultureInvariant);

    /// <summary>
    /// The browser tracker id a client was started with. Each launch link Bloxkeep makes has a new one
    /// ("+browsertrackerid:N"), and the link reaches RobloxPlayerBeta.exe as it is: Bloxstrap's Bootstrapper starts it with
    /// its launch arguments untouched, and Roblox's own link handler and Bloxkeep's other launch options give it the link;
    /// Roblox's launcher of old passed it as "-b N". "" when the command line names none, null when it can't be read
    /// (gone, not ours). Nothing else of the command line is kept: the sign-in ticket is in it.
    /// </summary>
    public static string? LaunchTrackerOf(int pid, long created)
    {
        lock (_trackers)
        {
            if (_trackers.TryGetValue((pid, created), out string? known))
                return known;
        }
        if (CommandLineOf(pid, created) is not { } line)
            return null;
        var found = TrackerText.Match(line);
        string tracker = !found.Success ? "" : found.Groups[1].Success ? found.Groups[1].Value : found.Groups[2].Value;
        lock (_trackers)
        {
            if (_trackers.Count > 200)
                _trackers.Clear();
            _trackers[(pid, created)] = tracker;
        }
        return tracker;
    }

    /// <summary>A process's command line (Windows 8.1 and later), or null when it can't be read (gone, not ours).</summary>
    static string? CommandLineOf(int pid, long expectedCreation = 0)
    {
        IntPtr handle = OpenProcess(ProcessQueryLimitedInformation, false, pid);
        if (handle == IntPtr.Zero)
            return null;
        IntPtr buffer = IntPtr.Zero;
        try
        {
            if (expectedCreation != 0 && (!GetProcessTimes(handle, out long created, out _, out _, out _) || created != expectedCreation))
                return null;
            NtQueryInformationProcess(handle, ProcessCommandLineInformation, IntPtr.Zero, 0, out int size);
            if (size <= 0 || size > 1 << 16)
                return null;
            buffer = Marshal.AllocHGlobal(size);
            if (NtQueryInformationProcess(handle, ProcessCommandLineInformation, buffer, size, out _) != 0)
                return null;
            // A UNICODE_STRING (length in bytes, then a pointer to the text) heads the buffer.
            int length = Marshal.ReadInt16(buffer) & 0xFFFF;
            IntPtr text = Marshal.ReadIntPtr(buffer, IntPtr.Size);
            return length == 0 || text == IntPtr.Zero ? "" : Marshal.PtrToStringUni(text, length / 2);
        }
        finally
        {
            if (buffer != IntPtr.Zero)
                Marshal.FreeHGlobal(buffer);
            CloseHandle(handle);
        }
    }

    /// <summary>
    /// Ends Roblox's notification-area copies (see <see cref="IsTrayCopy"/>) before a launch, which they would
    /// otherwise take over and open no window for. Returns how many were ended.
    /// </summary>
    public static int CloseTrayCopies()
    {
        var players = GetPlayers();
        var windows = VisibleWindows(players.Select(p => p.Pid));
        int ended = 0;
        foreach (var p in players)
        {
            if (windows.TryGetValue(p.Pid, out var list) && list.Count > 0 || !IsTrayCopy(p.Pid, p.Created))
                continue;
            Kill(p.Pid);
            ended++;
        }
        return ended;
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
