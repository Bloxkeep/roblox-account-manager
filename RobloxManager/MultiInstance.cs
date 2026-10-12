using Microsoft.Win32;

namespace RobloxManager;

public sealed record TrayAutostart(string ValueName, bool Enabled);

/// <summary>
/// Roblox allows one client per Windows session by creating two named kernel objects when it starts.
/// Creating them first, and holding them while this app runs, makes each client's singleton check fail open,
/// so several clients can run side by side. Nothing is injected into, or read from, the Roblox process.
/// </summary>
public static class MultiInstance
{
    const string MutexName = "ROBLOX_singletonMutex";
    const string EventName = "ROBLOX_singletonEvent";
    const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    const string StartupApprovedKey = @"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    static Mutex? _mutex;
    static bool _ownsMutex;
    static EventWaitHandle? _event;

    /// <summary>True only when this app created both objects, i.e. no Roblox client got there first.</summary>
    public static bool Armed { get; private set; }

    /// <remarks>Must be called on the UI thread: the mutex is owned by the thread that creates it.</remarks>
    public static void Enable()
    {
        Disable();
        bool createdMutex = false, createdEvent = false;
        try
        {
            _mutex = new Mutex(initiallyOwned: true, MutexName, out createdMutex);
            _ownsMutex = createdMutex;
        }
        catch (Exception ex) when (ex is UnauthorizedAccessException or WaitHandleCannotBeOpenedException or IOException)
        {
            _mutex = null;
        }
        try
        {
            _event = new EventWaitHandle(false, EventResetMode.ManualReset, EventName, out createdEvent);
        }
        catch (Exception ex) when (ex is UnauthorizedAccessException or WaitHandleCannotBeOpenedException or IOException)
        {
            _event = null;
        }
        Armed = createdMutex && createdEvent;
    }

    public static void Disable()
    {
        if (_mutex is not null)
        {
            if (_ownsMutex)
            {
                try { _mutex.ReleaseMutex(); }
                catch (ApplicationException) { }
            }
            _mutex.Dispose();
            _mutex = null;
            _ownsMutex = false;
        }
        _event?.Dispose();
        _event = null;
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
