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

    [DllImport("ntdll.dll")]
    static extern int NtQuerySystemInformation(int infoClass, IntPtr info, int length, out int returned);

    const int SystemPageFileInformation = 18;
    const int StatusInfoLengthMismatch = unchecked((int)0xC0000004);

    const long MB = 1024 * 1024;

    public static MemoryStatus Read()
    {
        var status = new MEMORYSTATUSEX { dwLength = (uint)Marshal.SizeOf<MEMORYSTATUSEX>() };
        if (!GlobalMemoryStatusEx(ref status))
            return default;
        // For a 64-bit process the "page file" fields are the system's commit limit and what is left of it; the limit is
        // RAM and the page files' size now together.
        long total = (long)(status.ullTotalPhys / MB), limit = (long)(status.ullTotalPageFile / MB);
        var (files, free, made) = PagingFiles();
        return new MemoryStatus(total, (long)(status.ullAvailPhys / MB), (int)status.dwMemoryLoad, limit, (long)(status.ullAvailPageFile / MB),
            PageFileRoomMb(files, Math.Max(0, limit - total), free, made));
    }

    /// <summary>
    /// How much the page files may still grow, in MB. Windows grows a page file as the commit limit is reached, up to the
    /// maximum set for it, so a limit nearly used up isn't full while that room is left (a 32 GB PC whose page file was
    /// set to 16–32 GB had grown it to 17.5 GB, 15 GB still to go, and was told virtual memory was almost full). Only page
    /// files with a maximum set count, within their drives' free space less 1 GB: one Windows sizes by itself ("system
    /// managed", or "?:" for every drive) counts for nothing, its limits not being known. Nor does one set but never made:
    /// on 2026-10-22 the owner's E: page file (set to 16–32 GB) didn't exist (BitLocker's PagefileOnOsVolume = 1 has Windows
    /// make page files on the system drive only as it starts; one added while it runs lasts until the next start), the
    /// commit limit stayed at RAM + C:'s 4 GB, and
    /// counting E:'s 32 GB as room to grow kept "virtual memory is almost full" quiet while 6 Roblox windows opened into
    /// a full commit limit (Windows' low-virtual-memory event at 99.7 %, two of them crashing on ERROR_COMMITMENT_LIMIT).
    /// </summary>
    /// <param name="pagingFiles">Windows' PagingFiles setting: "c:\pagefile.sys 1024 4096" (path, initial and largest size in MB).</param>
    /// <param name="pageFilesNowMb">How large the page files are now, all together.</param>
    /// <param name="freeMb">A drive's free space ("c:\").</param>
    /// <param name="made">Whether Windows made the page file set on a drive ("c:\"): see <see cref="PageFileDrives"/>.</param>
    internal static long PageFileRoomMb(IReadOnlyList<string> pagingFiles, long pageFilesNowMb, Func<string, long> freeMb, Func<string, bool> made)
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
            if (Path.GetPathRoot(parts[0]) is not { Length: > 0 } root || !made(root))
                continue;
            most += max;
            if (drives.Add(root))
                free += Math.Max(0, freeMb(root) - 1024);
        }
        return Math.Max(0, Math.Min(most - pageFilesNowMb, free));
    }

    static (DateTime At, string[] Files, Dictionary<string, long> Free, HashSet<string>? Made) _paging;

    /// <summary>
    /// Windows' PagingFiles setting, the free space of the drives it names, and the drives Windows made a page file on,
    /// read again once a minute.
    /// </summary>
    static (string[] Files, Func<string, long> Free, Func<string, bool> Made) PagingFiles()
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
            _paging = p = (DateTime.UtcNow, files, free, PageFileDrives());
        }
        // Not known which Windows made: none counts, so a warning comes rather than a crash.
        return (p.Files, root => p.Free.GetValueOrDefault(root), root => p.Made?.Contains(root) == true);
    }

    /// <summary>
    /// The drives ("C:\") Windows has a page file on now, from the kernel's own list of them (what Task Manager counts);
    /// null when it can't be read. A page file set in PagingFiles may not be there: Windows makes them as it starts.
    /// </summary>
    internal static HashSet<string>? PageFileDrives()
    {
        for (int length = 4096; length <= 1 << 20; length *= 4)
        {
            IntPtr buffer = Marshal.AllocHGlobal(length);
            try
            {
                int status = NtQuerySystemInformation(SystemPageFileInformation, buffer, length, out int returned);
                if (status == StatusInfoLengthMismatch)
                    continue;
                if (status != 0)
                    return null;
                var drives = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
                // SYSTEM_PAGEFILE_INFORMATION: NextEntryOffset, TotalSize, TotalInUse, PeakUsage (ULONGs), then the file's
                // name as a UNICODE_STRING (Length, MaximumLength, the buffer pointer aligned after them): "\??\C:\pagefile.sys".
                for (long offset = 0; returned > 0;)
                {
                    IntPtr entry = buffer + (nint)offset;
                    int next = Marshal.ReadInt32(entry);
                    int bytes = (ushort)Marshal.ReadInt16(entry, 16);
                    string name = Marshal.PtrToStringUni(Marshal.ReadIntPtr(entry, 16 + IntPtr.Size), bytes / 2);
                    if (name.IndexOf(":\\", StringComparison.Ordinal) is >= 1 and var colon)
                        drives.Add(name.Substring(colon - 1, 3));
                    if (next <= 0)
                        break;
                    offset += next;
                }
                return drives;
            }
            catch (Exception)
            {
                return null;
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }
        return null;
    }
}
