using System.Runtime.InteropServices;
using Microsoft.Win32;

namespace RobloxManager;

/// <param name="CommitLimitMb">
/// How much memory programs may hold in all, RAM and page file together (Windows' commit limit); 0 when not known.
/// A program that asks for more past it fails even with RAM free: Roblox crashes as it opens (Hyperion's "Memory dump
/// captured"), as seen on a 32 GB PC whose page file was fixed at 4 GB.
/// </param>
/// <param name="CommitAvailableMb">What of <paramref name="CommitLimitMb"/> programs don't hold yet.</param>
/// <param name="PageFileRoomMb">How much the commit limit may still grow with the page files (<see cref="SystemMemory.PageFileRoomMb"/>).</param>
public readonly record struct MemoryStatus(long TotalMb, long AvailableMb, int LoadPercent, long CommitLimitMb = 0, long CommitAvailableMb = 0, long PageFileRoomMb = 0)
{
    /// <summary>What programs may still take before one fails: what is left of the commit limit, and the room it may still grow.</summary>
    public long CommitRoomMb => CommitAvailableMb + PageFileRoomMb;
}

public static class SystemMemory
{
    [StructLayout(LayoutKind.Sequential)]
    struct MEMORYSTATUSEX
    {
        public uint dwLength;
        public uint dwMemoryLoad;
        public ulong ullTotalPhys;
        public ulong ullAvailPhys;
        public ulong ullTotalPageFile;
        public ulong ullAvailPageFile;
        public ulong ullTotalVirtual;
        public ulong ullAvailVirtual;
        public ulong ullAvailExtendedVirtual;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GlobalMemoryStatusEx(ref MEMORYSTATUSEX buffer);

    const long MB = 1024 * 1024;

    public static MemoryStatus Read()
    {
        var status = new MEMORYSTATUSEX { dwLength = (uint)Marshal.SizeOf<MEMORYSTATUSEX>() };
        if (!GlobalMemoryStatusEx(ref status))
            return default;
        // For a 64-bit process the "page file" fields are the system's commit limit and what is left of it; the limit is
        // RAM and the page files' size now together.
        long total = (long)(status.ullTotalPhys / MB), limit = (long)(status.ullTotalPageFile / MB);
        var (files, free) = PagingFiles();
        return new MemoryStatus(total, (long)(status.ullAvailPhys / MB), (int)status.dwMemoryLoad, limit, (long)(status.ullAvailPageFile / MB),
            PageFileRoomMb(files, Math.Max(0, limit - total), free));
    }

    /// <summary>
    /// How much the page files may still grow, in MB. Windows grows a page file as the commit limit is reached, up to the
    /// maximum set for it, so a limit nearly used up isn't full while that room is left (a 32 GB PC whose page file was
    /// set to 16–32 GB had grown it to 17.5 GB, 15 GB still to go, and was told virtual memory was almost full). Only page
    /// files with a maximum set count, within their drives' free space less 1 GB: one Windows sizes by itself ("system
    /// managed", or "?:" for every drive) counts for nothing, its limits not being known.
    /// </summary>
    /// <param name="pagingFiles">Windows' PagingFiles setting: "c:\pagefile.sys 1024 4096" (path, initial and largest size in MB).</param>
    /// <param name="pageFilesNowMb">How large the page files are now, all together.</param>
    /// <param name="freeMb">A drive's free space ("c:\").</param>
    internal static long PageFileRoomMb(IReadOnlyList<string> pagingFiles, long pageFilesNowMb, Func<string, long> freeMb)
    {
        long most = 0, free = 0;
        var drives = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var entry in pagingFiles)
        {
            var parts = entry.Split(' ', StringSplitOptions.RemoveEmptyEntries);
            if (parts.Length == 0)
                continue;
            if (parts[0].StartsWith('?') || parts.Length < 3 || !long.TryParse(parts[2], out long max) || max <= 0)
                return 0;
            most += max;
            if (Path.GetPathRoot(parts[0]) is { Length: > 0 } root && drives.Add(root))
                free += Math.Max(0, freeMb(root) - 1024);
        }
        return Math.Max(0, Math.Min(most - pageFilesNowMb, free));
    }

    static (DateTime At, string[] Files, Dictionary<string, long> Free) _paging;

    /// <summary>Windows' PagingFiles setting and the free space of the drives it names, read again once a minute.</summary>
    static (string[] Files, Func<string, long> Free) PagingFiles()
    {
        var p = _paging;
        if (p.Files == null || DateTime.UtcNow - p.At > TimeSpan.FromMinutes(1))
        {
            string[] files = [];
            try
            {
                using var key = Registry.LocalMachine.OpenSubKey(@"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management");
                files = key?.GetValue("PagingFiles") as string[] ?? [];
            }
            catch { }
            var free = new Dictionary<string, long>(StringComparer.OrdinalIgnoreCase);
            foreach (var f in files)
                if (Path.GetPathRoot(f.Split(' ')[0]) is { Length: > 0 } root && !root.StartsWith('?') && !free.ContainsKey(root))
                {
                    try { free[root] = new DriveInfo(root).AvailableFreeSpace / MB; }
                    catch { free[root] = 0; }
                }
            _paging = p = (DateTime.UtcNow, files, free);
        }
        return (p.Files, root => p.Free.GetValueOrDefault(root));
    }
}
