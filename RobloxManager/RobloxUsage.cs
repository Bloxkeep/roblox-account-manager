using System.Diagnostics;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// How hard Roblox's windows work the PC: CPU from their processes' time, GPU from Windows' "GPU Engine" counters, as Task
/// Manager shows them. Read on a thread of its own every few seconds, only while Roblox runs and the page shows it; the
/// first read of the counters takes about half a second (measured 2026-11-01). Null when Windows doesn't say.
/// </summary>
public sealed partial class RobloxUsage
{
    static readonly TimeSpan Every = TimeSpan.FromSeconds(3);

    readonly Dictionary<int, TimeSpan> _cpuTimes = [];
    DateTime _readAt;
    InstanceDataCollection? _gpuBefore;
    bool _gpuGone;
    int _reading;

    public int? CpuPct { get; private set; }
    public int? GpuPct { get; private set; }

    public void Refresh(IReadOnlyCollection<int> pids)
    {
        if (pids.Count == 0)
        {
            if (_readAt != default && Interlocked.CompareExchange(ref _reading, 1, 0) == 0)
            {
                (_readAt, _gpuBefore, CpuPct, GpuPct) = (default, null, null, null);
                _cpuTimes.Clear();
                _reading = 0;
            }
            return;
        }
        if (DateTime.UtcNow - _readAt < Every || Interlocked.CompareExchange(ref _reading, 1, 0) != 0)
            return;
        Task.Run(() =>
        {
            try
            {
                Read(pids);
            }
            finally
            {
                _reading = 0;
            }
        });
    }

    void Read(IReadOnlyCollection<int> pids)
    {
        var now = DateTime.UtcNow;
        var seconds = (now - _readAt).TotalSeconds;
        var times = new Dictionary<int, TimeSpan>();
        foreach (int pid in pids)
        {
            try
            {
                using var process = Process.GetProcessById(pid);
                times[pid] = process.TotalProcessorTime;
            }
            catch (Exception ex) when (ex is ArgumentException or InvalidOperationException or System.ComponentModel.Win32Exception)
            {
            }
        }
        // Only what each window used since the last read: one that just opened counts from here.
        double used = times.Where(t => _cpuTimes.ContainsKey(t.Key)).Sum(t => Math.Max(0, (t.Value - _cpuTimes[t.Key]).TotalSeconds));
        CpuPct = _readAt == default ? null : Percent(used / (seconds * Environment.ProcessorCount) * 100);
        _cpuTimes.Clear();
        foreach (var (pid, time) in times)
            _cpuTimes[pid] = time;
        GpuPct = ReadGpu(pids);
        _readAt = now;
    }

    int? ReadGpu(IReadOnlyCollection<int> pids)
    {
        if (_gpuGone)
            return null;
        try
        {
            var after = new PerformanceCounterCategory("GPU Engine").ReadCategory()["Utilization Percentage"];
            var before = _gpuBefore;
            _gpuBefore = after;
            if (before is null || after is null)
                return null;
            var rows = new List<(string, float)>();
            foreach (string instance in after.Keys)
            {
                if (before.Contains(instance))
                    rows.Add((instance, CounterSample.Calculate(before[instance].Sample, after[instance].Sample)));
            }
            return GpuPercent(rows, pids);
        }
        catch (Exception ex) when (ex is InvalidOperationException or UnauthorizedAccessException or System.ComponentModel.Win32Exception or FormatException)
        {
            _gpuGone = true; // Windows without the counters, or with them switched off: not asked again.
            return null;
        }
    }

    /// <summary>Task Manager's figure for a set of processes: their share of each GPU engine added up, then the busiest engine.</summary>
    /// <param name="rows">The counters' instances ("pid_12_luid_0x0_0x14d4d_phys_0_eng_0_engtype_3D") and values.</param>
    public static int GpuPercent(IEnumerable<(string Instance, float Pct)> rows, IReadOnlyCollection<int> pids)
    {
        var engines = new Dictionary<string, double>();
        foreach (var (instance, pct) in rows)
        {
            var m = EngineInstance().Match(instance);
            if (!m.Success || !int.TryParse(m.Groups[1].Value, out int pid) || !pids.Contains(pid))
                continue;
            engines[m.Groups[2].Value] = engines.GetValueOrDefault(m.Groups[2].Value) + pct;
        }
        return Percent(engines.Count == 0 ? 0 : engines.Values.Max());
    }

    static int Percent(double value) => (int)Math.Round(Math.Clamp(value, 0, 100));

    [GeneratedRegex(@"^pid_(\d+)_(luid_\w+?_phys_\d+_eng_\d+)(_|$)", RegexOptions.IgnoreCase)]
    private static partial Regex EngineInstance();
}
