using System.Diagnostics;
using System.Runtime.InteropServices;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>Finds the Roblox clients and launchers (Bloxstrap, Fishstrap, Voidstrap, …) installed for this Windows user.</summary>
public static class RobloxInstalls
{
    const string ProtocolCommandKey = @"roblox-player\shell\open\command";
    /// <summary>%LOCALAPPDATA%; a field the checks point at a folder of their own.</summary>
    static string Local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);

    /// <summary>Executable registered for roblox-player: links (the official client or a bootstrapper).</summary>
    public static string? ProtocolHandlerPath()
    {
        using var key = Registry.ClassesRoot.OpenSubKey(ProtocolCommandKey);
        if (key?.GetValue(null) is not string command)
            return null;
        command = command.Trim();
        if (command.StartsWith('"'))
        {
            int end = command.IndexOf('"', 1);
            return end > 1 ? command[1..end] : null;
        }
        int space = command.IndexOf(' ');
        return space > 0 ? command[..space] : command;
    }

    /// <summary>Roblox VNG's client (%LOCALAPPDATA%\RobloxPCVNG). Its installer takes the roblox-player: links (seen
    /// 2026-10-19); Bloxkeep's Default never follows them there, not even for a VNG account.</summary>
    public static bool IsVngClient(string path) => path.Contains(@"\RobloxPCVNG\", StringComparison.OrdinalIgnoreCase);

    /// <summary>Roblox VNG's newest client: the "vng" option, and Default's and "direct"'s last resort when no other Roblox
    /// is installed (never ahead of one; the owner's rule, 2026-10-28). It opens global accounts too (seen 2026-10-19).</summary>
    public static string? FindVngClient() => Newest(ClientsIn(Path.Combine(Local, "RobloxPCVNG")));

    /// <summary>The program roblox-player: links open, when it is still there and isn't Roblox VNG's client.</summary>
    public static string? UsableHandlerPath() =>
        ProtocolHandlerPath() is { } handler && File.Exists(handler) && !IsVngClient(handler) ? handler : null;

    /// <summary>A bootstrapper by name, from its uninstall entry, its default folder, or the protocol handler.</summary>
    public static string? FindBootstrapper(string name)
    {
        string exeName = name + ".exe";
        using (var key = Registry.CurrentUser.OpenSubKey($@"Software\Microsoft\Windows\CurrentVersion\Uninstall\{name}"))
        {
            if (key?.GetValue("InstallLocation") is string dir && File.Exists(Path.Combine(dir, exeName)))
                return Path.Combine(dir, exeName);
        }
        string defaultPath = Path.Combine(Local, name, exeName);
        if (File.Exists(defaultPath))
            return defaultPath;
        return ProtocolHandlerPath() is { } handler && File.Exists(handler)
               && Path.GetFileName(handler).Equals(exeName, StringComparison.OrdinalIgnoreCase)
            ? handler
            : null;
    }

    /// <summary>Launchers with their own option in Settings; others of the same family are found by <see cref="FindOtherBootstrappers"/>.</summary>
    static readonly string[] KnownBootstrappers = ["Bloxstrap", "Fishstrap", "Voidstrap"];

    /// <summary>Where a launcher keeps its copies of Roblox, one "version-…" folder each: Bloxstrap and Fishstrap "Versions",
    /// Voidstrap "RblxVersions" (its Paths.cs, KloBraticc/Voidstrap 1.1.3.7, 2026-11-07).</summary>
    static readonly string[] VersionFolders = ["Versions", "RblxVersions"];

    /// <summary>Other Bloxstrap forks: an uninstall entry named after the launcher, whose folder holds "&lt;name&gt;.exe" and
    /// a Versions folder of Roblox clients.</summary>
    public static List<string> FindOtherBootstrappers()
    {
        var found = new List<string>();
        try
        {
            using var uninstall = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall");
            foreach (string name in uninstall?.GetSubKeyNames() ?? [])
            {
                if (KnownBootstrappers.Contains(name, StringComparer.OrdinalIgnoreCase) || name.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0)
                    continue;
                using var key = uninstall!.OpenSubKey(name);
                if (key?.GetValue("InstallLocation") is string dir && File.Exists(Path.Combine(dir, name + ".exe"))
                    && VersionFolders.Any(v => Directory.Exists(Path.Combine(dir, v))))
                    found.Add(name);
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or System.Security.SecurityException)
        {
        }
        return found.Order(StringComparer.OrdinalIgnoreCase).ToList();
    }

    static IEnumerable<string> ClientsIn(string? root)
    {
        if (root is null)
            return [];
        var found = new List<string>();
        foreach (string folder in VersionFolders)
        {
            string versions = Path.Combine(root, folder);
            if (!Directory.Exists(versions))
                continue;
            try
            {
                found.AddRange(Directory.EnumerateDirectories(versions)
                    .Select(dir => Path.Combine(dir, "RobloxPlayerBeta.exe"))
                    .Where(File.Exists)
                    .Where(exe => !IsBroken(exe)));
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
            }
        }
        return found;
    }

    const string ClientDll = "RobloxPlayerBeta.dll";
    static readonly System.Collections.Concurrent.ConcurrentDictionary<(string, DateTime), bool> NeedsDll = new();

    /// <summary>A client that can't start: the DLL it names isn't beside it (a user's report on a rival, 2026-10-30), and Windows
    /// would only show its own error. Builds from before that DLL don't name it, so they stay usable. The exe (about 137 MB)
    /// is read only when the DLL is gone.</summary>
    public static bool IsBroken(string exe)
    {
        try
        {
            if (Path.GetDirectoryName(exe) is not { } dir || File.Exists(Path.Combine(dir, ClientDll)) || !File.Exists(exe))
                return false;
            return NeedsDll.GetOrAdd((exe, File.GetLastWriteTimeUtc(exe)), _ => FileMentions(exe, ClientDll));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException)
        {
            return false;
        }
    }

    static bool FileMentions(string file, string text)
    {
        byte[] wanted = System.Text.Encoding.ASCII.GetBytes(text);
        byte[] buffer = new byte[1 << 20];
        using var stream = File.OpenRead(file);
        int kept = 0, read;
        while ((read = stream.Read(buffer, kept, buffer.Length - kept)) > 0)
        {
            int have = kept + read;
            if (buffer.AsSpan(0, have).IndexOf(wanted) >= 0)
                return true;
            // The name may lie across two reads.
            kept = Math.Min(wanted.Length - 1, have);
            Array.Copy(buffer, have - kept, buffer, 0, kept);
        }
        return false;
    }

    /// <summary>The links open a client that can't start.</summary>
    public static bool HandlerBroken() =>
        UsableHandlerPath() is { } handler && Path.GetFileName(handler).Equals("RobloxPlayerBeta.exe", StringComparison.OrdinalIgnoreCase) && IsBroken(handler);

    /// <summary>Roblox's own install has a build that can't start: said in place of "not installed" when no other is left.</summary>
    public static bool OwnBuildBroken()
    {
        string programFiles = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86);
        foreach (string root in new[] { Path.Combine(Local, "Roblox"), Path.Combine(programFiles, "Roblox") })
        {
            string versions = Path.Combine(root, "Versions");
            try
            {
                if (Directory.Exists(versions) && Directory.EnumerateDirectories(versions).Any(dir => IsBroken(Path.Combine(dir, "RobloxPlayerBeta.exe"))))
                    return true;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
            }
        }
        return false;
    }

    /// <summary>Whether a client behind Roblox's newest build gets updated when it opens: Roblox's own install runs its updater
    /// (the owner's logs, 2026-10-30), a launcher updates its copy first. Started by itself, a client in a launcher's folder
    /// finds no updater ("Updater not found. Skipping update") and goes on into the game on the old build.</summary>
    public static bool UpdatesOnOpen(string? exe) => exe is null || IsRobloxOwn(exe) || UpdaterFor("direct") is { } updater && !IsOwnInstaller(updater);

    /// <summary>"Directly" with a client behind the newest build, in a launcher's folder: this once through that launcher, which
    /// updates it first. An old build isn't played on (accounts were reported banned for "a disallowed version of Roblox",
    /// 2026-10-31, unconfirmed by Roblox). Anything else, or the newest build not known, is left as chosen.</summary>
    public static string UpdatingOption(string launchVia, bool? upToDate)
    {
        // Roblox's own copy updates itself as it opens: nothing to go through.
        if (launchVia != "direct" || upToDate != false || UpdaterFor(launchVia) is not { } updater || IsOwnInstaller(updater))
            return launchVia;
        string name = Path.GetFileNameWithoutExtension(updater);
        return name.Equals("Bloxstrap", StringComparison.OrdinalIgnoreCase) ? "bloxstrap"
            : name.Equals("Fishstrap", StringComparison.OrdinalIgnoreCase) ? "fishstrap"
            : name.Equals("Voidstrap", StringComparison.OrdinalIgnoreCase) ? "voidstrap"
            : RobloxLauncher.OtherPrefix + name;
    }

    /// <summary>
    /// The client to use of several: the build Roblox's installer registered last (<see cref="RegisteredBuild"/>), then the
    /// highest version, then the newest file. By file time alone, 2026-10-30 picked a test channel's 0.744.478 (its file
    /// from 10-29) over 0.745 (10-28), the build Roblox wanted: each window it opened started Roblox's updater, which
    /// closed it and opened another. The registered build counts only among the builds of Roblox's own install, whose
    /// installer wrote it: between installs (Roblox's and Bloxstrap's, say, for "directly"), each one's pick goes by version,
    /// so Roblox's stale registered build never wins over a launcher's newer one (review, 2026-10-30).
    /// </summary>
    static string? Newest(IEnumerable<string> paths)
    {
        string? registered = RegisteredBuild();
        var version = new Dictionary<string, Version>(StringComparer.OrdinalIgnoreCase);
        Version VersionNumber(string p) => version.TryGetValue(p, out var known) ? known
            : version[p] = Version.TryParse(System.Text.RegularExpressions.Regex.Match(VersionOf(p)?.Version ?? "", @"^\d+(\.\d+){1,3}").Value, out var v)
                ? v : new Version(0, 0);
        bool Registered(string p) => registered is not null && IsRobloxOwn(p)
            && string.Equals(Path.GetFileName(Path.GetDirectoryName(p)), registered, StringComparison.OrdinalIgnoreCase);
        // …\<install>\Versions\version-…\RobloxPlayerBeta.exe: one pick per Versions folder.
        return paths.GroupBy(p => Path.GetDirectoryName(Path.GetDirectoryName(p)) ?? "", StringComparer.OrdinalIgnoreCase)
            .Select(install => install.OrderByDescending(Registered).ThenByDescending(VersionNumber).ThenByDescending(File.GetLastWriteTimeUtc).First())
            .OrderByDescending(VersionNumber)
            .ThenByDescending(File.GetLastWriteTimeUtc)
            .FirstOrDefault();
    }

    /// <summary>In Roblox's own install (its installer's, per user or for all), not a launcher's copy.</summary>
    static bool IsRobloxOwn(string exe) =>
        Under(exe, Path.Combine(Local, "Roblox")) || Under(exe, Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86), "Roblox"));

    /// <summary>The build folder ("version-…") Roblox's installer set the roblox-player link up for, written beside its command.</summary>
    static string? RegisteredBuild()
    {
        if (RegisteredBuildOverride is { } stand)
            return stand();
        try
        {
            using var key = Registry.ClassesRoot.OpenSubKey(ProtocolCommandKey);
            return key?.GetValue("version") as string is { Length: > 0 } build ? build : null;
        }
        catch (Exception ex) when (ex is System.Security.SecurityException or UnauthorizedAccessException or IOException)
        {
            return null;
        }
    }

    /// <summary>Tests: the registered build in place of the registry's.</summary>
    internal static Func<string?>? RegisteredBuildOverride { get; set; }

    /// <summary>Whether the build folder ("version-…") is in this client's Versions folder. Roblox keeps a build per channel
    /// side by side and starts the account's channel's: on 2026-10-29 the links pointed at a test channel's 0.744.478
    /// while a main-channel account started 0.745 from the folder next to it.</summary>
    public static bool HasBuild(string? exe, string? folder)
    {
        if (exe is null || folder is null || !folder.StartsWith("version-", StringComparison.OrdinalIgnoreCase))
            return false;
        try
        {
            return Path.GetDirectoryName(Path.GetDirectoryName(exe)) is { } versions
                && File.Exists(Path.Combine(versions, folder, "RobloxPlayerBeta.exe"))
                && !IsBroken(Path.Combine(versions, folder, "RobloxPlayerBeta.exe"));
        }
        catch (Exception ex) when (ex is ArgumentException or IOException)
        {
            return false;
        }
    }

    /// <summary>Newest client installed by Roblox's own installer.</summary>
    public static string? FindOfficialClient()
    {
        string programFiles = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86);
        return Newest(ClientsIn(Path.Combine(Local, "Roblox")).Concat(ClientsIn(Path.Combine(programFiles, "Roblox"))));
    }

    /// <summary>Newest client from any install, bootstrappers included.</summary>
    public static string? FindAnyClient()
    {
        var clients = ClientsIn(Path.Combine(Local, "Roblox")).ToList();
        foreach (var name in KnownBootstrappers.Concat(FindOtherBootstrappers()))
        {
            if (FindBootstrapper(name) is { } exe)
                clients.AddRange(ClientsIn(Path.GetDirectoryName(exe)));
        }
        if (UsableHandlerPath() is { } handler)
        {
            if (!Path.GetFileName(handler).Equals("RobloxPlayerBeta.exe", StringComparison.OrdinalIgnoreCase))
                clients.AddRange(ClientsIn(Path.GetDirectoryName(handler)));
            else if (!IsBroken(handler))
                clients.Add(handler);
        }
        return Newest(clients.Distinct(StringComparer.OrdinalIgnoreCase));
    }

    /// <param name="Version">e.g. "0.742.0.7421053".</param>
    /// <param name="Folder">The version folder, e.g. "version-cec3ad5889b447cf" (Roblox's id for that build).</param>
    /// <param name="Source">Whose copy it is ("Roblox", "Bloxstrap", …); only for a client found on this PC.</param>
    /// <param name="Exe">The client file itself; never sent to the page.</param>
    public sealed record ClientVersion(string Version, string Folder,
        [property: System.Text.Json.Serialization.JsonIgnore] string? Source = null,
        [property: System.Text.Json.Serialization.JsonIgnore] string? Exe = null);

    static bool Under(string path, string root) =>
        path.StartsWith(root.TrimEnd('\\', '/') + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase);

    /// <summary>Which install a client belongs to: Roblox's own, a known launcher's, or the folder its Versions folder is in.</summary>
    public static string SourceOf(string exe)
    {
        if (IsStoreClient(exe))
            return "Microsoft Store";
        string programFiles = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86);
        if (Under(exe, Path.Combine(Local, "Roblox")) || Under(exe, Path.Combine(programFiles, "Roblox")))
            return "Roblox";
        foreach (var name in KnownBootstrappers.Concat(FindOtherBootstrappers()))
        {
            if (FindBootstrapper(name) is { } launcher && Path.GetDirectoryName(launcher) is { Length: > 0 } root && Under(exe, root))
                return name;
        }
        // …\<install>\Versions\version-…\RobloxPlayerBeta.exe
        return Path.GetFileName(Path.GetDirectoryName(Path.GetDirectoryName(Path.GetDirectoryName(exe)) ?? "") ?? "") ?? "";
    }

    /// <summary>The client a launch option would start: bootstrappers keep their own copy of Roblox.</summary>
    public static string? ClientFor(string launchVia, string customLauncher = "")
    {
        string? Bootstrapped(string name) => FindBootstrapper(name) is { } exe ? Newest(ClientsIn(Path.GetDirectoryName(exe))) : null;
        return launchVia switch
        {
            "bloxstrap" => Bootstrapped("Bloxstrap"),
            "fishstrap" => Bootstrapped("Fishstrap"),
            "voidstrap" => Bootstrapped("Voidstrap"),
            _ when RobloxLauncher.OtherName(launchVia) is { } other => Bootstrapped(other),
            // A launcher picked by hand: its own copy of Roblox if it keeps one next to it, else the newest anywhere.
            "custom" => (RobloxLauncher.CustomLauncherReady(customLauncher) ? Newest(ClientsIn(Path.GetDirectoryName(customLauncher))) : null) ?? FindAnyClient(),
            "roblox" => FindOfficialClient(),
            "direct" => FindAnyClient() ?? FindVngClient(), // VNG's only when nothing else is there
            "store" => FindStoreClient(),
            "vng" => FindVngClient(),
            // Whatever opens roblox-player: links (not VNG's client: see IsVngClient).
            _ => UsableHandlerPath() is { } handler
                ? Path.GetFileName(handler).Equals("RobloxPlayerBeta.exe", StringComparison.OrdinalIgnoreCase)
                    ? handler
                    : Newest(ClientsIn(Path.GetDirectoryName(handler))) ?? FindAnyClient()
                : FindAnyClient() ?? FindVngClient(),
        };
    }

    /// <summary>What updates the client this option starts: the Bloxstrap-style launcher whose copy it is, which with "-player
    /// -nolaunch" installs the newest Roblox without opening it (Bloxstrap 2.11.4 and Fishstrap source, 2026-10-17), or for
    /// Roblox's own copy its installer beside the client (<see cref="OwnInstaller"/>), which installs the newest build and
    /// then opens the Roblox app (the owner's PC, 2026-11-06: no switch it keeps stops that, so the app's window is closed
    /// after). Null for the Store edition, Roblox VNG and a hand-picked launcher.</summary>
    public static string? UpdaterFor(string launchVia, string customLauncher = "")
    {
        if (launchVia is "custom" or "store" or "vng" || ClientFor(launchVia, customLauncher) is not { } client)
            return null;
        foreach (var name in KnownBootstrappers.Concat(FindOtherBootstrappers()))
        {
            if (FindBootstrapper(name) is { } exe && Path.GetDirectoryName(exe) is { } dir && Under(client, dir))
                return exe;
        }
        string own = Path.Combine(Path.GetDirectoryName(client) ?? "", OwnInstaller);
        return IsRobloxOwn(client) && !IsVngClient(client) && File.Exists(own) ? own : null;
    }

    /// <summary>Roblox's installer, in each build folder of its own install: run with no switches it installs the newest build
    /// for this PC (as Roblox serves it) in seconds, then opens the Roblox app (its window: "-isInstallerLaunch").</summary>
    public const string OwnInstaller = "RobloxPlayerInstaller.exe";

    public static bool IsOwnInstaller(string? exe) => exe is not null && Path.GetFileName(exe).Equals(OwnInstaller, StringComparison.OrdinalIgnoreCase);

    /// <summary>A client's build is behind Roblox's newest. Only builds in "version-&lt;hash>" folders can tell (the Store's has
    /// none); a newer number than Roblox serves is another channel's.</summary>
    public static bool IsBehind(ClientVersion? client, ClientVersion? latest) =>
        client is not null && latest is not null && client.Folder != latest.Folder
        && client.Folder.StartsWith("version-", StringComparison.OrdinalIgnoreCase) && latest.Folder.StartsWith("version-", StringComparison.OrdinalIgnoreCase)
        && !(Version.TryParse(client.Version, out var have) && Version.TryParse(latest.Version, out var newest) && have > newest);

    /// <summary>Reads the version stamped in a client's file (the file is only read, never run).</summary>
    public static ClientVersion? VersionOf(string? exe)
    {
        if (exe is null || !File.Exists(exe))
            return null;
        try
        {
            var info = FileVersionInfo.GetVersionInfo(exe);
            // The text form ("0, 742, 0, 7421053"): the numeric parts are 16-bit, and Roblox's build number is larger.
            string version = string.Join('.', (info.FileVersion ?? "").Split(',', '.').Select(part => part.Trim()).Where(part => part.Length > 0));
            if (version.Length == 0)
                version = $"{info.FileMajorPart}.{info.FileMinorPart}.{info.FileBuildPart}.{info.FilePrivatePart}";
            // The Store's exe reads 0.0.0.0: its package folder's name has the version (and no Roblox build id).
            if (IsStoreClient(exe))
                return StoreVersion(Path.GetFileName(Path.GetDirectoryName(exe)) ?? "") is { } store ? new ClientVersion(store, "", SourceOf(exe), exe) : null;
            return new ClientVersion(version, Path.GetFileName(Path.GetDirectoryName(exe)) ?? "", SourceOf(exe), exe);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>Roblox from the Microsoft Store (GDK build since September 2025, replacing ROBLOXCORPORATION.ROBLOX). Its
    /// Start entry runs GameLaunchHelper.exe, which only brings an open Roblox forward, so Bloxkeep starts the package's
    /// RobloxPlayerBeta.exe (RobloxLauncher, "store").</summary>
    public const string StoreFamily = "ROBLOXCorporation.RobloxGDK_55nm5eh3cm0pr";
    public const string StoreAppId = "Game";

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    static extern int GetPackagesByPackageFamily(string packageFamilyName, ref uint count, IntPtr packageFullNames, ref uint bufferLength, IntPtr buffer);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    static extern int GetPackagePathByFullName(string packageFullName, ref uint pathLength, [Out] char[]? path);
    const int ErrorInsufficientBuffer = 122;

    /// <summary>The Microsoft Store edition's RobloxPlayerBeta.exe, or null when it isn't installed for this user.</summary>
    public static string? FindStoreClient()
    {
        IntPtr names = IntPtr.Zero, buffer = IntPtr.Zero;
        try
        {
            uint count = 0, length = 0;
            if (GetPackagesByPackageFamily(StoreFamily, ref count, IntPtr.Zero, ref length, IntPtr.Zero) != ErrorInsufficientBuffer || count == 0)
                return null;
            names = Marshal.AllocHGlobal((int)count * IntPtr.Size);
            buffer = Marshal.AllocHGlobal((int)length * sizeof(char));
            if (GetPackagesByPackageFamily(StoreFamily, ref count, names, ref length, buffer) != 0 || count == 0)
                return null;
            string? fullName = Marshal.PtrToStringUni(Marshal.ReadIntPtr(names));
            if (string.IsNullOrEmpty(fullName))
                return null;
            uint size = 0;
            if (GetPackagePathByFullName(fullName, ref size, null) != ErrorInsufficientBuffer || size == 0)
                return null;
            var path = new char[size];
            if (GetPackagePathByFullName(fullName, ref size, path) != 0)
                return null;
            string exe = Path.Combine(new string(path, 0, (int)size - 1), "RobloxPlayerBeta.exe");
            return File.Exists(exe) ? exe : null;
        }
        catch (Exception ex) when (ex is EntryPointNotFoundException or DllNotFoundException or IOException or UnauthorizedAccessException)
        {
            return null;
        }
        finally
        {
            Marshal.FreeHGlobal(names);
            Marshal.FreeHGlobal(buffer);
        }
    }

    public static bool StoreEditionInstalled() => FindStoreClient() is not null;

    /// <summary>A client in the Store edition's package folder (…\WindowsApps\ROBLOXCorporation.RobloxGDK_…).</summary>
    public static bool IsStoreClient(string exe) =>
        Path.GetFileName(Path.GetDirectoryName(exe) ?? "").StartsWith(StoreFamily.Split('_')[0] + "_", StringComparison.OrdinalIgnoreCase);

    /// <summary>The Store client's version in Roblox's form, from its package folder: "…RobloxGDK_2.743.1134.0_x64__…" gives
    /// 0.743.0.7431134, comparable with what Roblox serves. Null when the name isn't of that form.</summary>
    public static string? StoreVersion(string packageFolder)
    {
        var name = packageFolder.Split('_');
        var parts = name.Length > 1 ? name[1].Split('.') : [];
        return parts.Length == 4 && int.TryParse(parts[1], out int release) && int.TryParse(parts[2], out int build)
            ? $"0.{release}.0.{release}{build:0000}"
            : null;
    }
}
