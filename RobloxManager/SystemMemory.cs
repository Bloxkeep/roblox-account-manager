using System.Runtime.InteropServices;

namespace RobloxManager;

/// <param name="CommitLimitMb">
/// How much memory programs may hold in all, RAM and page file together (Windows' commit limit); 0 when not known.
/// A program that asks for more past it fails even with RAM free: Roblox crashes as it opens (Hyperion's "Memory dump
/// captured"), as seen on a 32 GB PC whose page file was fixed at 4 GB.
/// </param>
/// <param name="CommitAvailableMb">What of <paramref name="CommitLimitMb"/> programs don't hold yet.</param>
public readonly record struct MemoryStatus(long TotalMb, long AvailableMb, int LoadPercent, long CommitLimitMb = 0, long CommitAvailableMb = 0);

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

    public static MemoryStatus Read()
    {
        var status = new MEMORYSTATUSEX { dwLength = (uint)Marshal.SizeOf<MEMORYSTATUSEX>() };
        if (!GlobalMemoryStatusEx(ref status))
            return default;
        const ulong MB = 1024 * 1024;
        // For a 64-bit process the "page file" fields are the system's commit limit and what is left of it.
        return new MemoryStatus((long)(status.ullTotalPhys / MB), (long)(status.ullAvailPhys / MB), (int)status.dwMemoryLoad,
            (long)(status.ullTotalPageFile / MB), (long)(status.ullAvailPageFile / MB));
    }
}
