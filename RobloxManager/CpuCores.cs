using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// The fast cores for the window being played: a Roblox window in the background runs on the CPU's efficiency cores
/// (on CPUs that have two kinds: Intel 12th gen and later, recent laptop Ryzens) and in Windows' power-saving mode
/// (EcoQoS), and the one in front gets every core back. Only Windows' own switches, as Task Manager's "Set affinity"
/// and its efficiency mode use: nothing in Roblox is touched.
/// </summary>
static class CpuCores
{
    [StructLayout(LayoutKind.Sequential)]
    struct PROCESS_POWER_THROTTLING_STATE
    {
        public uint Version, ControlMask, StateMask;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetSystemCpuSetInformation(IntPtr information, uint length, out uint returned, IntPtr process, uint flags);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetProcessAffinityMask(IntPtr process, out UIntPtr processMask, out UIntPtr systemMask);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool SetProcessAffinityMask(IntPtr process, UIntPtr mask);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool SetProcessInformation(IntPtr process, int infoClass, ref PROCESS_POWER_THROTTLING_STATE info, int size);

    const uint PROCESS_SET_INFORMATION = 0x0200, PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
    const int ProcessPowerThrottling = 4;
    const uint PROCESS_POWER_THROTTLING_CURRENT_VERSION = 1, PROCESS_POWER_THROTTLING_EXECUTION_SPEED = 1;

    /// <summary>The efficiency cores' bits in an affinity mask, 0 when the CPU has one kind of core (or over 64 threads).</summary>
    public static ulong EfficientMask { get; } = ReadEfficientMask();
    /// <summary>Logical processors that are efficiency cores, and all of them (for Settings' words).</summary>
    public static int EfficientCount => System.Numerics.BitOperations.PopCount(EfficientMask);
    public static int LogicalCount => Environment.ProcessorCount;
    public static bool Hybrid => EfficientMask != 0;

    static ulong ReadEfficientMask()
    {
        try
        {
            GetSystemCpuSetInformation(IntPtr.Zero, 0, out uint length, IntPtr.Zero, 0);
            if (length == 0)
                return 0;
            IntPtr buffer = Marshal.AllocHGlobal((int)length);
            try
            {
                if (!GetSystemCpuSetInformation(buffer, length, out length, IntPtr.Zero, 0))
                    return 0;
                // SYSTEM_CPU_SET_INFORMATION: Size, Type, then the CPU set: Id (8), Group (12), LogicalProcessorIndex (14),
                // CoreIndex, LastLevelCacheIndex, NumaNodeIndex, EfficiencyClass (18). A higher class is a faster core.
                var sets = new List<(int Group, int Index, int Class)>();
                for (int offset = 0; offset < length;)
                {
                    int size = Marshal.ReadInt32(buffer, offset);
                    if (size <= 0)
                        break;
                    sets.Add((Marshal.ReadInt16(buffer, offset + 12), Marshal.ReadByte(buffer, offset + 14), Marshal.ReadByte(buffer, offset + 18)));
                    offset += size;
                }
                // One group only: an affinity mask covers 64 logical processors of one group.
                if (sets.Count == 0 || sets.Any(s => s.Group != 0 || s.Index >= 64))
                    return 0;
                int slowest = sets.Min(s => s.Class);
                if (sets.All(s => s.Class == slowest))
                    return 0; // one kind of core
                ulong mask = 0;
                foreach (var s in sets.Where(s => s.Class == slowest))
                    mask |= 1UL << s.Index;
                return mask;
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }
        catch (Exception ex) when (ex is EntryPointNotFoundException or DllNotFoundException)
        {
            return 0; // older Windows
        }
    }

    /// <summary>What a client gets: the efficiency cores and power saving, every core at full speed, or Windows' own choice.</summary>
    public enum Role { Background, Front, Default }

    /// <summary>
    /// A client in the background: on the efficiency cores (when there are) and in power-saving mode. In front: every
    /// core, at full speed. Default (the setting turned off, Bloxkeep closing): every core, Windows deciding the speed,
    /// as before Bloxkeep touched it. False when Windows refused the handle.
    /// </summary>
    public static bool Set(int pid, Role role)
    {
        IntPtr handle = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (handle == IntPtr.Zero)
            return false;
        try
        {
            bool ok = true;
            if (EfficientMask != 0 && GetProcessAffinityMask(handle, out _, out UIntPtr all))
                ok &= SetProcessAffinityMask(handle, new UIntPtr(role == Role.Background ? EfficientMask & all.ToUInt64() : all.ToUInt64()));
            var state = new PROCESS_POWER_THROTTLING_STATE
            {
                Version = PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                ControlMask = role == Role.Default ? 0 : PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
                StateMask = role == Role.Background ? PROCESS_POWER_THROTTLING_EXECUTION_SPEED : 0,
            };
            ok &= SetProcessInformation(handle, ProcessPowerThrottling, ref state, Marshal.SizeOf<PROCESS_POWER_THROTTLING_STATE>());
            return ok;
        }
        finally
        {
            CloseHandle(handle);
        }
    }
}
