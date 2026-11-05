using Microsoft.Win32;

namespace RobloxManager;

/// <summary>
/// Bloxkeep's own entry in Windows' startup apps (HKCU Run, named "Bloxkeep"): started with <see cref="Arg"/>, it opens in
/// the tray, locked, rather than a lock screen over what the person is starting. The registry is the setting itself, so
/// switching it off in Task Manager › Startup apps shows here as off too.
/// </summary>
static class StartWithWindows
{
    public const string Arg = "--startup";
    const string Name = "Bloxkeep";
    const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    const string ApprovedKey = @"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    static string Command => $"\"{Environment.ProcessPath}\" {Arg}";

    /// <summary>On when the entry is there and Task Manager hasn't switched it off.</summary>
    public static bool IsOn
    {
        get
        {
            try
            {
                using var run = Registry.CurrentUser.OpenSubKey(RunKey);
                if (run?.GetValue(Name) is not string)
                    return false;
                using var approved = Registry.CurrentUser.OpenSubKey(ApprovedKey);
                // Task Manager marks a switched-off entry with an odd first byte; none means on.
                return approved?.GetValue(Name) is not byte[] { Length: > 0 } flags || (flags[0] & 1) == 0;
            }
            catch (Exception ex) when (ex is System.Security.SecurityException or UnauthorizedAccessException or IOException)
            {
                Log.Write(ex);
                return false;
            }
        }
    }

    public static void Set(bool on)
    {
        using (var run = Registry.CurrentUser.CreateSubKey(RunKey))
        {
            if (on)
                run.SetValue(Name, Command);
            else
                run.DeleteValue(Name, throwOnMissingValue: false);
        }
        // Switched back on here after Task Manager switched it off: Task Manager's mark goes too.
        using var approved = Registry.CurrentUser.OpenSubKey(ApprovedKey, writable: true);
        approved?.DeleteValue(Name, throwOnMissingValue: false);
    }

    /// <summary>At each start: an entry left pointing at a copy that isn't there any more (a moved portable copy) points
    /// at this one. One pointing at another copy that is there stays as it is.</summary>
    public static void KeepPathCurrent()
    {
        try
        {
            using var run = Registry.CurrentUser.OpenSubKey(RunKey, writable: true);
            if (run?.GetValue(Name) is not string had || had == Command)
                return;
            string at = had.StartsWith('"') && had.IndexOf('"', 1) is > 1 and var end ? had[1..end] : had;
            if (!File.Exists(at))
                run.SetValue(Name, Command);
        }
        catch (Exception ex) when (ex is System.Security.SecurityException or UnauthorizedAccessException or IOException)
        {
            Log.Write(ex);
        }
    }
}
