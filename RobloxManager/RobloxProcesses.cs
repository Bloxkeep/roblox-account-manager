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
    const int SW_RESTORE = 9;

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

    public readonly record struct PlayerProcess(int Pid, long WorkingSet);

    /// <summary>
    /// Running clients with their RAM use. Both come from the system-wide process snapshot,
    /// so no handle to the Roblox process is opened.
    /// </summary>
    public static List<PlayerProcess> GetPlayers()
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

    public static bool HasVisibleWindow(int pid) => TopLevelWindows(pid).Count > 0;

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

    /// <summary>Asks the client to close like the window's X button would, and terminates it after 4 s if it is still there.</summary>
    public static void Close(int pid)
    {
        var windows = TopLevelWindows(pid);
        foreach (var hWnd in windows)
            PostMessage(hWnd, WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
        _ = Task.Run(async () =>
        {
            if (windows.Count > 0)
                await Task.Delay(4000);
            if (GetPlayerPids().Contains(pid))
                Kill(pid);
        });
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

    public static int ForegroundPid()
    {
        IntPtr hWnd = GetForegroundWindow();
        if (hWnd == IntPtr.Zero)
            return 0;
        GetWindowThreadProcessId(hWnd, out uint pid);
        return (int)pid;
    }

    static IntPtr MainWindow(int pid)
    {
        var windows = TopLevelWindows(pid);
        return windows.FirstOrDefault(h => GetTitle(h).StartsWith("Roblox", StringComparison.Ordinal)) is var main && main != IntPtr.Zero
            ? main
            : windows.FirstOrDefault();
    }

    /// <summary>Tiles the clients' windows over a screen area, in the given order. Window messages only.</summary>
    public static async Task<int> ArrangeGridAsync(IReadOnlyList<int> pids, Rectangle area)
    {
        var windows = pids.Select(MainWindow).Where(h => h != IntPtr.Zero).ToList();
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

        int cols = (int)Math.Ceiling(Math.Sqrt(windows.Count));
        int rows = (int)Math.Ceiling(windows.Count / (double)cols);
        int width = area.Width / cols, height = area.Height / rows;
        for (int i = 0; i < windows.Count; i++)
        {
            SetWindowPos(windows[i], IntPtr.Zero, area.X + i % cols * width, area.Y + i / cols * height, width, height,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
        }
        return windows.Count;
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
        const uint PROCESS_SET_QUOTA = 0x0100, PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
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

    public static int CloseAll()
    {
        var pids = GetPlayerPids();
        foreach (var pid in pids)
            Close(pid);
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
