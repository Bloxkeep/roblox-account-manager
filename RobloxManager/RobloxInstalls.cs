using System.Diagnostics;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>Finds the Roblox clients and launchers (Bloxstrap, Fishstrap, …) installed for this Windows user.</summary>
public static class RobloxInstalls
{
    const string ProtocolCommandKey = @"roblox-player\shell\open\command";
    static readonly string Local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);

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
    static readonly string[] KnownBootstrappers = ["Bloxstrap", "Fishstrap"];

    /// <summary>
    /// Other Bloxstrap-style launchers (its forks keep its installer): an uninstall entry named after the launcher,
    /// whose folder holds "&lt;name&gt;.exe" and a Versions folder of Roblox clients. Only what is really there is listed.
    /// </summary>
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
                    && Directory.Exists(Path.Combine(dir, "Versions")))
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
        string versions = Path.Combine(root, "Versions");
        if (!Directory.Exists(versions))
            return [];
        try
        {
            return Directory.EnumerateDirectories(versions)
                .Select(dir => Path.Combine(dir, "RobloxPlayerBeta.exe"))
                .Where(File.Exists)
                .ToList();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return [];
        }
    }

    static string? Newest(IEnumerable<string> paths) => paths.OrderByDescending(File.GetLastWriteTimeUtc).FirstOrDefault();

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
        if (ProtocolHandlerPath() is { } handler && File.Exists(handler))
        {
            if (Path.GetFileName(handler).Equals("RobloxPlayerBeta.exe", StringComparison.OrdinalIgnoreCase))
                clients.Add(handler);
            else
                clients.AddRange(ClientsIn(Path.GetDirectoryName(handler)));
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
            _ when RobloxLauncher.OtherName(launchVia) is { } other => Bootstrapped(other),
            // A launcher picked by hand: its own copy of Roblox if it keeps one next to it, else the newest anywhere.
            "custom" => (File.Exists(customLauncher) ? Newest(ClientsIn(Path.GetDirectoryName(customLauncher))) : null) ?? FindAnyClient(),
            "roblox" => FindOfficialClient(),
            "direct" => FindAnyClient(),
            // Whatever opens roblox-player: links: the client itself, or a bootstrapper with its own copies.
            _ => ProtocolHandlerPath() is { } handler && File.Exists(handler)
                ? Path.GetFileName(handler).Equals("RobloxPlayerBeta.exe", StringComparison.OrdinalIgnoreCase)
                    ? handler
                    : Newest(ClientsIn(Path.GetDirectoryName(handler))) ?? FindAnyClient()
                : FindAnyClient(),
        };
    }

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
            return new ClientVersion(version, Path.GetFileName(Path.GetDirectoryName(exe)) ?? "", SourceOf(exe), exe);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>
    /// The Microsoft Store edition signs in by itself and cannot be started as a chosen account
    /// or in several windows, so it is only reported, never used.
    /// </summary>
    public static bool StoreEditionInstalled()
    {
        try
        {
            string packages = Path.Combine(Local, "Packages");
            return Directory.Exists(packages) && Directory.EnumerateDirectories(packages, "ROBLOXCORPORATION.ROBLOX_*").Any();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return false;
        }
    }
}
