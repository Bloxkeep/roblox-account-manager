using Microsoft.Win32;

namespace RobloxManager;

public sealed record TrayAutostart(string ValueName, bool Enabled);

/// <summary>
/// Roblox allows one client per Windows session through a named event, "ROBLOX_singletonEvent": a client opening
/// it tells the one already running to close. Taking that name first with an object of another kind (a mutex) makes
/// Roblox's own create fail, so it never sets up the check and several clients run side by side — Windows lets one
/// object own a name, whatever its kind. Holding the event itself stopped working with Roblox's October 2026 builds
/// (the clients open it and use it to close each other). Nothing is injected into, or read from, the Roblox process.
/// When Roblox renames that object, Bloxkeep's Roblox watch says what to hold instead (<see cref="RobloxWatch"/>), and
/// those names are taken the same way, next to this one.
/// </summary>
public static class MultiInstance
{
    public const string EventName = "ROBLOX_singletonEvent";
    const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    const string StartupApprovedKey = @"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    static readonly List<(Mutex Name, bool Owned)> _held = new();
    static string[] _heldNames = [];
    static string[] _extra = [];

    /// <summary>
    /// True while every name is held as a mutex — by this app, or by another multi-instance tool (then it works too).
    /// False when a Roblox client got there first: the name is its event, until every client has closed.
    /// </summary>
    public static bool Armed { get; private set; }

    /// <summary>The names held besides <see cref="EventName"/> (the Roblox watch's), for the page.</summary>
    public static IReadOnlyList<string> ExtraHeld => Armed ? _heldNames.Skip(1).ToArray() : [];

    /// <summary>
    /// Held, but the names to hold changed since (the Roblox watch found Roblox renamed its lock): taken again once no
    /// Roblox client is running, like <see cref="Enable"/> after a client got there first.
    /// </summary>
    public static bool Stale => Armed && !_heldNames.SequenceEqual(Wanted());

    /// <summary>Names to hold next to <see cref="EventName"/> from the next <see cref="Enable"/> on.</summary>
    public static void SetExtraNames(IEnumerable<string> names) =>
        _extra = names.Where(n => n != EventName).Distinct(StringComparer.Ordinal).ToArray();

    static string[] Wanted() => [EventName, .. _extra];

    /// <remarks>Must be called on the UI thread: a mutex is owned by the thread that creates it.</remarks>
    public static void Enable()
    {
        Disable();
        var names = Wanted();
        foreach (var name in names)
        {
            try
            {
                var mutex = new Mutex(initiallyOwned: true, name, out bool owned);
                _held.Add((mutex, owned));
            }
            // The name is Roblox's event (a client is running, perhaps the one Windows starts in the tray): the
            // process check tries again once no client is running.
            catch (Exception ex) when (ex is UnauthorizedAccessException or WaitHandleCannotBeOpenedException or IOException)
            {
                Disable();
                return;
            }
        }
        _heldNames = names;
        Armed = true;
    }

    public static void Disable()
    {
        foreach (var (name, owned) in _held)
        {
            if (owned)
            {
                try { name.ReleaseMutex(); }
                catch (ApplicationException) { }
            }
            name.Dispose();
        }
        _held.Clear();
        _heldNames = [];
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
