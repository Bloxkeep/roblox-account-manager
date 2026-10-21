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
        var (files, made, free, size) = PagingFiles();
        return new MemoryStatus(total, (long)(status.ullAvailPhys / MB), (int)status.dwMemoryLoad, limit, (long)(status.ullAvailPageFile / MB),
            PageFileRoomMb(files, made, total, free, size));
    }

    /// <summary>
    /// How much the page files may still grow, in MB. Windows grows a page file as the commit limit is reached, up to its
    /// largest size, so a limit nearly used up isn't full while that room is left (a 32 GB PC whose page file was set to
    /// 16–32 GB had grown it to 17.5 GB, 15 GB still to go, and was told virtual memory was almost full). Each page file
    /// Windows has counts, from its size now up to its largest: the one set for it, or for one Windows sizes by itself
    /// ("system managed", Windows' default; "?:" for every drive) Microsoft's: 3 × RAM or 4 GB, whichever is larger, limited
    /// to its drive's size ÷ 8 (learn.microsoft.com, "How to determine the appropriate page file size for 64-bit versions
    /// of Windows"; past that only for a crash dump, not counted). Each within its drive's free space less 1 GB.
    /// A page file set but never made counts for nothing: on 2026-10-22 the owner's E: page file (set to 16–32 GB) didn't
    /// exist (BitLocker's PagefileOnOsVolume = 1 has Windows make page files on the system drive only as it starts; one
    /// added while it runs lasts until the next start), the commit limit stayed at RAM + C:'s 4 GB, and counting E:'s 32 GB
    /// as room to grow kept "virtual memory is almost full" quiet while 6 Roblox windows opened into a full commit limit
    /// (Windows' low-virtual-memory event at 99.7 %, two of them crashing on ERROR_COMMITMENT_LIMIT).
    /// </summary>
    /// <param name="pagingFiles">
    /// Windows' PagingFiles setting: "c:\pagefile.sys 1024 4096" (path, initial and largest size in MB); "0 0" or no sizes
    /// for one Windows sizes by itself; "?:\pagefile.sys" for system-managed ones on the drives Windows picks.
    /// </param>
    /// <param name="made">The page files Windows has now, by drive ("C:\"), with their size now in MB (<see cref="PageFilesNow"/>); null when not known: none counts.</param>
    /// <param name="ramMb">The PC's RAM.</param>
    /// <param name="freeMb">A drive's free space.</param>
    /// <param name="sizeMb">A drive's size.</param>
    internal static long PageFileRoomMb(IReadOnlyList<string> pagingFiles, IReadOnlyDictionary<string, long>? made, long ramMb,
        Func<string, long> freeMb, Func<string, long> sizeMb)
    {
        if (made is null)
            return 0;
        // The largest size set for each drive's page file; 0 for one Windows sizes by itself.
        var largest = new Dictionary<string, long>(StringComparer.OrdinalIgnoreCase);
        bool anyDrive = false;
        foreach (var entry in pagingFiles)
        {
            var parts = entry.Split(' ', StringSplitOptions.RemoveEmptyEntries);
            if (parts.Length == 0)
                continue;
            if (parts[0].StartsWith('?'))
                anyDrive = true;
            else if (Path.GetPathRoot(parts[0]) is { Length: > 0 } root)
                largest[root] = parts.Length >= 3 && long.TryParse(parts[2], out long max) && max > 0 ? max : 0;
        }
        long room = 0;
        foreach (var (root, nowMb) in made)
        {
            long most;
            if (largest.TryGetValue(root, out long set) && set > 0)
                most = set;
            else if (largest.ContainsKey(root) || anyDrive)
                most = Math.Min(Math.Max(3 * ramMb, 4096), sizeMb(root) / 8);
            else
                continue; // one the setting doesn't name: how large it may grow isn't known
            room += Math.Max(0, Math.Min(most - nowMb, freeMb(root) - 1024));
        }
        return room;
    }

    static (DateTime At, string[] Files, Dictionary<string, long>? Made, Dictionary<string, (long Free, long Size)> Drives) _paging;

    /// <summary>
    /// Windows' PagingFiles setting, the page files Windows has now and the free space and size of their drives, read
    /// again once a minute.
    /// </summary>
    static (string[] Files, IReadOnlyDictionary<string, long>? Made, Func<string, long> Free, Func<string, long> Size) PagingFiles()
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
            var made = PageFilesNow();
            var drives = new Dictionary<string, (long Free, long Size)>(StringComparer.OrdinalIgnoreCase);
            foreach (string root in made?.Keys ?? Enumerable.Empty<string>())
            {
                try
                {
                    var drive = new DriveInfo(root);
                    drives[root] = (drive.AvailableFreeSpace / MB, drive.TotalSize / MB);
                }
                catch { drives[root] = (0, 0); }
            }
            _paging = p = (DateTime.UtcNow, files, made, drives);
        }
        return (p.Files, p.Made, root => p.Drives.GetValueOrDefault(root).Free, root => p.Drives.GetValueOrDefault(root).Size);
    }

    /// <summary>
    /// The page files Windows has now, by drive ("C:\"), with their size now in MB, from the kernel's own list of them
    /// (what Task Manager counts); null when it can't be read. A page file set in PagingFiles may not be there: Windows
    /// makes them as it starts.
    /// </summary>
    internal static Dictionary<string, long>? PageFilesNow()
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
                var files = new Dictionary<string, long>(StringComparer.OrdinalIgnoreCase);
                // SYSTEM_PAGEFILE_INFORMATION: NextEntryOffset, TotalSize (in pages), TotalInUse, PeakUsage (ULONGs), then
                // the file's name as a UNICODE_STRING (Length, MaximumLength, the buffer pointer aligned after them):
                // "\??\C:\pagefile.sys".
                for (long offset = 0; returned > 0;)
                {
                    IntPtr entry = buffer + (nint)offset;
                    int next = Marshal.ReadInt32(entry);
                    long pages = (uint)Marshal.ReadInt32(entry, 4);
                    int bytes = (ushort)Marshal.ReadInt16(entry, 16);
                    string name = Marshal.PtrToStringUni(Marshal.ReadIntPtr(entry, 16 + IntPtr.Size), bytes / 2);
                    if (name.IndexOf(":\\", StringComparison.Ordinal) is >= 1 and var colon)
                    {
                        string root = name.Substring(colon - 1, 3);
                        files[root] = files.GetValueOrDefault(root) + pages * Environment.SystemPageSize / MB;
                    }
                    if (next <= 0)
                        break;
                    offset += next;
                }
                return files;
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
