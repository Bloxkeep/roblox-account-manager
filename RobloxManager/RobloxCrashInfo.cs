using System.Diagnostics;
using System.Globalization;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// What a Roblox window that ended on its own left behind, for error.log only: Roblox's crash reports
/// (logs\crashes\reports\*.dmp) written since it opened, and Windows' "Application Error" record for its process.
/// Never opens the process (Roblox's anti-cheat watches handles to it); read after it is gone.
/// </summary>
static class RobloxCrashInfo
{
    /// <summary>The note for error.log, or null when nothing was left behind.</summary>
    public static string? Describe(int pid, DateTime startedUtc, DateTime endedUtc)
    {
        var parts = new List<string>();
        if (DumpsSince(startedUtc) is { Count: > 0 } dumps)
            parts.Add($"{dumps.Count} Roblox crash report(s) since it opened, newest {dumps[0]}");
        if (FaultOf(pid, endedUtc - startedUtc + TimeSpan.FromMinutes(1)) is { } fault)
            parts.Add($"Windows: {fault}");
        return parts.Count == 0 ? null : string.Join("; ", parts);
    }

    /// <summary>Crash report names newer than <paramref name="sinceUtc"/>, newest first.</summary>
    internal static List<string> DumpsSince(DateTime sinceUtc)
    {
        var found = new List<(DateTime At, string Name)>();
        foreach (string logs in RobloxLogWatcher.Folders)
        {
            try
            {
                var reports = new DirectoryInfo(Path.Combine(logs, "crashes", "reports"));
                if (!reports.Exists)
                    continue;
                foreach (var dump in reports.GetFiles("*.dmp", SearchOption.TopDirectoryOnly))
                    if (dump.LastWriteTimeUtc >= sinceUtc)
                        found.Add((dump.LastWriteTimeUtc, dump.Name));
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
            }
        }
        return found.OrderByDescending(d => d.At).Select(d => d.Name).ToList();
    }

    /// <summary>Windows' Application Error record (event 1000) for this process id, written within <paramref name="within"/>.</summary>
    static string? FaultOf(int pid, TimeSpan within)
    {
        long ms = (long)Math.Clamp(within.TotalMilliseconds, 60_000, TimeSpan.FromDays(1).TotalMilliseconds);
        string xml;
        try
        {
            using var run = Process.Start(new ProcessStartInfo(Path.Combine(Environment.SystemDirectory, "wevtutil.exe"))
            {
                ArgumentList = { "qe", "Application", $"/q:*[System[Provider[@Name='Application Error'] and TimeCreated[timediff(@SystemTime) <= {ms}]]]",
                    "/f:xml", "/rd:true", "/c:50" },
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
            });
            if (run is null)
                return null;
            var output = run.StandardOutput.ReadToEndAsync();
            if (!run.WaitForExit(5000))
            {
                try { run.Kill(); } catch (InvalidOperationException) { }
                return null;
            }
            xml = output.Result;
        }
        catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or InvalidOperationException or IOException)
        {
            return null;
        }
        return FaultIn(xml, pid);
    }

    /// <summary>The matching record of wevtutil's XML, said in one line.</summary>
    internal static string? FaultIn(string xml, int pid)
    {
        foreach (string record in xml.Split("<Event ", StringSplitOptions.RemoveEmptyEntries))
        {
            if (Field(record, "ProcessId") is not { } hex || !hex.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
                || !int.TryParse(hex.AsSpan(2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out int id) || id != pid)
                continue;
            return $"{Field(record, "AppName") ?? "?"} {Field(record, "AppVersion") ?? ""} faulted in {Field(record, "ModuleName") ?? "?"}"
                + $" (exception {Field(record, "ExceptionCode") ?? "?"}, offset {Field(record, "FaultingOffset") ?? "?"})";
        }
        return null;
    }

    static string? Field(string record, string name) =>
        Regex.Match(record, $"<Data Name='{name}'>([^<]*)</Data>") is { Success: true } m && m.Groups[1].Value.Trim() is { Length: > 0 } value ? value : null;
}
