using System.Runtime.InteropServices;

namespace RobloxManager;

public readonly record struct MemoryStatus(long TotalMb, long AvailableMb, int LoadPercent);

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
        return new MemoryStatus((long)(status.ullTotalPhys / MB), (long)(status.ullAvailPhys / MB), (int)status.dwMemoryLoad);
    }
}
