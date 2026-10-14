using Microsoft.Win32;

namespace RobloxManager;

public sealed record TrayAutostart(string ValueName, bool Enabled);

/// <summary>
/// Roblox allows one client per Windows session through a named event, "ROBLOX_singletonEvent": a client opening
/// it tells the one already running to close. Taking that name first with an object of another kind (a mutex) makes
/// Roblox's own create fail, so it never sets up the check and several clients run side by side — Windows lets one
/// object own a name, whatever its kind. Holding the event itself stopped working with Roblox's October 2026 builds
/// (the clients open it and use it to close each other). Nothing is injected into, or read from, the Roblox process.
/// </summary>
public static class MultiInstance
{
    const string EventName = "ROBLOX_singletonEvent";
    const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    const string StartupApprovedKey = @"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    static Mutex? _name;
    static bool _owned;

    /// <summary>
    /// True while the name is held as a mutex — by this app, or by another multi-instance tool (then it works too).
    /// False when a Roblox client got there first: the name is its event, until every client has closed.
    /// </summary>
    public static bool Armed { get; private set; }

    /// <remarks>Must be called on the UI thread: the mutex is owned by the thread that creates it.</remarks>
    public static void Enable()
    {
        Disable();
        try
        {
            _name = new Mutex(initiallyOwned: true, EventName, out _owned);
            Armed = true;
        }
        // The name is Roblox's event (a client is running, perhaps the one Windows starts in the tray): the process
        // check tries again once no client is running.
        catch (Exception ex) when (ex is UnauthorizedAccessException or WaitHandleCannotBeOpenedException or IOException)
        {
            _name = null;
            Armed = false;
        }
    }

    public static void Disable()
    {
        if (_name is not null)
        {
            if (_owned)
            {
                try { _name.ReleaseMutex(); }
                catch (ApplicationException) { }
            }
            _name.Dispose();
            _name = null;
            _owned = false;
        }
        Armed = false;
    }

    /// <summary>
    /// Roblox can register itself to start with Windows in the tray ("--launch-to-tray"); that hidden client
    /// grabs the singleton objects before this app does.
    /// </summary>
    public static TrayAutostart? FindTrayAutostart()
    {
        using var run = Registry.CurrentUser.OpenSubKey(RunKey);
        if (run is null)
            return null;
        foreach (var name in run.GetValueNames())
        {
            if (run.GetValue(name) is not string command
                || !command.Contains("RobloxPlayerBeta", StringComparison.OrdinalIgnoreCase)
                || !command.Contains("--launch-to-tray", StringComparison.OrdinalIgnoreCase))
                continue;
            using var approved = Registry.CurrentUser.OpenSubKey(StartupApprovedKey);
            // Task Manager marks a disabled startup entry with an odd first byte (0x03); missing means enabled.
            bool enabled = approved?.GetValue(name) is not byte[] { Length: > 0 } flags || (flags[0] & 1) == 0;
            return new TrayAutostart(name, enabled);
        }
        return null;
    }

    /// <summary>Same effect as switching the entry off in Task Manager › Startup apps (and reversible there).</summary>
    public static void DisableTrayAutostart(TrayAutostart entry)
    {
        using var key = Registry.CurrentUser.CreateSubKey(StartupApprovedKey);
        var data = new byte[12];
        data[0] = 0x03;
        BitConverter.GetBytes(DateTime.UtcNow.ToFileTimeUtc()).CopyTo(data, 4);
        key.SetValue(entry.ValueName, data, RegistryValueKind.Binary);
    }
}
