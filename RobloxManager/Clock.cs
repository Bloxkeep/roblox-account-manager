namespace RobloxManager;

/// <summary>The time the schedules read: the wall clock and uptime, swapped by tests to walk a clock forward, back or across
/// a DST change (each test suite runs in a process of its own, so a swap never leaks into another suite).</summary>
static class Clock
{
    public static Func<DateTime> Now = () => DateTime.Now;
    public static Func<DateTime> UtcNow = () => DateTime.UtcNow;
    /// <summary><see cref="Environment.TickCount64"/>: milliseconds since Windows started, never set back.</summary>
    public static Func<long> Tick = () => Environment.TickCount64;
}
