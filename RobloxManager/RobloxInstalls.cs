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
        foreach (var name in new[] { "Bloxstrap", "Fishstrap" })
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
    public sealed record ClientVersion(string Version, string Folder);

    /// <summary>The client a launch option would start: bootstrappers keep their own copy of Roblox.</summary>
    public static string? ClientFor(string launchVia)
    {
        string? Bootstrapped(string name) => FindBootstrapper(name) is { } exe ? Newest(ClientsIn(Path.GetDirectoryName(exe))) : null;
        return launchVia switch
        {
            "bloxstrap" => Bootstrapped("Bloxstrap"),
            "fishstrap" => Bootstrapped("Fishstrap"),
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
            return new ClientVersion(version, Path.GetFileName(Path.GetDirectoryName(exe)) ?? "");
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
