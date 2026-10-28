using System.Diagnostics;
using Microsoft.Win32;

namespace RobloxManager;

public static class RobloxLauncher
{
    /// <summary>
    /// Values of <see cref="AppSettings.LaunchVia"/>, plus "other:&lt;Name&gt;" for another Bloxstrap-style launcher found on
    /// this PC and "custom" for one the user picked (<see cref="AppSettings.CustomLauncher"/>).
    /// </summary>
    public static readonly string[] LaunchOptions = ["auto", "bloxstrap", "fishstrap", "roblox", "direct", "store", "custom", "vng"];

    const string OtherPrefix = "other:";

    public static string Other(string name) => OtherPrefix + name;

    /// <summary>The launcher's name for an "other:&lt;Name&gt;" option, else null.</summary>
    public static string? OtherName(string launchVia) =>
        launchVia.StartsWith(OtherPrefix, StringComparison.Ordinal) && launchVia.Length > OtherPrefix.Length
            ? launchVia[OtherPrefix.Length..]
            : null;

    public static bool IsValidOption(string launchVia) =>
        LaunchOptions.Contains(launchVia) || (OtherName(launchVia) is { } name && name.IndexOfAny(Path.GetInvalidFileNameChars()) < 0);

    /// <summary>
    /// A launcher picked by hand that Bloxkeep may look at and start: a program on this PC, not on the network
    /// (\\server\share, \\?\UNC\…, \\host@SSL\… or a mapped network drive). Windows would reach a network path the moment
    /// Bloxkeep checked it was there, handing the PC's Windows sign-in to that server, and then run what it serves.
    /// </summary>
    public static bool IsLocalProgram(string path)
    {
        if (path.Length == 0 || !Path.IsPathFullyQualified(path) || path.StartsWith(@"\\", StringComparison.Ordinal) || path.StartsWith("//", StringComparison.Ordinal))
            return false;
        try
        {
            return new DriveInfo(Path.GetPathRoot(path)!).DriveType is not (DriveType.Network or DriveType.NoRootDirectory or DriveType.Unknown);
        }
        catch (ArgumentException)
        {
            return false;
        }
    }

    /// <summary>The launcher picked by hand is a program on this PC (<see cref="IsLocalProgram"/>) and still there.</summary>
    public static bool CustomLauncherReady(string path) => IsLocalProgram(path) && File.Exists(path);

    public static bool IsProtocolRegistered()
    {
        using var key = Registry.ClassesRoot.OpenSubKey(@"roblox-player\shell\open\command");
        return key is not null;
    }

    /// <summary>
    /// Whether Default may hand the link to Windows: registered, the program it names is still there, and it isn't
    /// Roblox VNG's client (whose installer takes the links; Bloxkeep never opens Roblox with it, see IsVngClient).
    /// </summary>
    static bool LinksOpen() =>
        IsProtocolRegistered() && (RobloxInstalls.ProtocolHandlerPath() is not { } handler || (File.Exists(handler) && !RobloxInstalls.IsVngClient(handler)));

    /// <summary>
    /// What opens Roblox when the links don't: Bloxstrap or Fishstrap (as their own options start them), another
    /// Bloxstrap-style launcher, else the newest RobloxPlayerBeta.exe on the PC, and Roblox VNG's client only when
    /// nothing else is there (the owner's rule, 2026-10-28: VNG is always the last choice). Null when there is none.
    /// </summary>
    internal static (string Exe, string[] Args)? FallbackLauncher()
    {
        foreach (string name in new[] { "Bloxstrap", "Fishstrap" })
        {
            if (RobloxInstalls.FindBootstrapper(name) is { } exe)
                return (exe, ["-player"]);
        }
        foreach (string name in RobloxInstalls.FindOtherBootstrappers())
        {
            if (RobloxInstalls.FindBootstrapper(name) is { } exe)
                return (exe, []);
        }
        return (RobloxInstalls.FindAnyClient() ?? RobloxInstalls.FindVngClient()) is { } client ? (client, []) : null;
    }

    static string NewBrowserTrackerId() => Random.Shared.NextInt64(100_000_000_000, 999_999_999_999).ToString();

    static string PlaceLauncherUrl(GameTarget t, string browserTrackerId)
    {
        string common = $"browserTrackerId={browserTrackerId}&placeId={t.PlaceId}&isPlayTogetherGame=false&joinAttemptId={Guid.NewGuid()}";
        const string baseUrl = "https://www.roblox.com/Game/PlaceLauncher.ashx?";
        // One of the account's own private servers: the access code from its list, as the website's Join button sends it
        // (Roblox's PlaceLauncher.js RequestPrivateGame(placeId, accessCode); Roblox Account Manager joins the same way).
        if (t.AccessCode is not null)
            return $"{baseUrl}request=RequestPrivateGame&{common}&accessCode={Uri.EscapeDataString(t.AccessCode)}"
                + (t.LinkCode is null ? "" : $"&linkCode={Uri.EscapeDataString(t.LinkCode)}") + "&joinAttemptOrigin=privateServerListJoin";
        if (t.LinkCode is not null)
            return $"{baseUrl}request=RequestPrivateGame&{common}&linkCode={Uri.EscapeDataString(t.LinkCode)}&joinAttemptOrigin=privateServerListJoin";
        if (t.JobId is not null)
            return $"{baseUrl}request=RequestGameJob&{common}&gameId={t.JobId}&joinAttemptOrigin=publicServerListJoin";
        return $"{baseUrl}request=RequestGame&{common}&joinAttemptOrigin=PlayButton";
    }

    /// <summary>The same roblox-player: URI the website hands to the client when you press Play.</summary>
    /// <param name="trackerId">Its browser tracker id (a new one when not given), which the client's command line carries.</param>
    public static string BuildLaunchUri(string ticket, GameTarget? target, string? trackerId = null)
    {
        trackerId ??= NewBrowserTrackerId();
        long now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        string tail = $"+browsertrackerid:{trackerId}+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp";
        if (target is null)
            return $"roblox-player:1+launchmode:app+gameinfo:{ticket}+launchtime:{now}{tail}";
        string placeLauncher = Uri.EscapeDataString(PlaceLauncherUrl(target, trackerId));
        return $"roblox-player:1+launchmode:play+gameinfo:{ticket}+launchtime:{now}+placelauncherurl:{placeLauncher}{tail}";
    }

    /// <param name="launchVia">
    /// "auto" = whatever Windows opens roblox-player: links with; "bloxstrap"/"fishstrap" = that launcher, keeping its
    /// mods; "roblox" = the official client; "direct" = the newest RobloxPlayerBeta.exe, skipping any launcher;
    /// "other:&lt;Name&gt;" and "custom" get the link as their first argument, the way Windows hands it to whatever
    /// handles roblox-player: links.
    /// </param>
    /// <returns>
    /// The link's browser tracker id, new for each launch: the client's command line carries it, so its window is told
    /// to be this launch's (<see cref="RobloxProcesses.LaunchTrackerOf"/>).
    /// </returns>
    public static string Launch(string ticket, GameTarget? target, string launchVia, string customLauncher = "")
    {
        string tracker = NewBrowserTrackerId();
        string uri = BuildLaunchUri(ticket, target, tracker);
        if (OtherName(launchVia) is { } other)
        {
            Start(RobloxInstalls.FindBootstrapper(other) ?? throw new FileNotFoundException(L.F("{0} is not installed.", other)), uri);
            return tracker;
        }
        switch (launchVia)
        {
            case "custom":
                // One on the network (a vault from elsewhere, say) is treated as gone: never looked at, never started.
                if (!CustomLauncherReady(customLauncher))
                    throw new FileNotFoundException(L.T("The launcher you picked is gone. Choose it again under Settings › Launching › Open Roblox with."));
                Start(customLauncher, uri);
                break;
            case "bloxstrap":
            case "fishstrap":
                string name = launchVia == "bloxstrap" ? "Bloxstrap" : "Fishstrap";
                Start(RobloxInstalls.FindBootstrapper(name) ?? throw new FileNotFoundException(L.F("{0} is not installed.", name)), "-player", uri);
                break;
            case "roblox":
                Start(RobloxInstalls.FindOfficialClient() ?? throw new FileNotFoundException(L.T("The official Roblox client is not installed.")), uri);
                break;
            case "direct":
                // Roblox VNG's client only when no other is on the PC: always the last choice.
                Start(RobloxInstalls.FindAnyClient() ?? RobloxInstalls.FindVngClient()
                      ?? throw new FileNotFoundException(L.T("RobloxPlayerBeta.exe was not found. Pick another option under Settings › Launching › Open Roblox with, or reinstall Roblox.")), uri);
                break;
            case "vng":
                Start(RobloxInstalls.FindVngClient()
                      ?? throw new FileNotFoundException(L.T("Roblox VNG isn't installed. Pick another option under Settings › Launching › Open Roblox with.")), uri);
                break;
            case "store":
                StartInStorePackage(RobloxInstalls.FindStoreClient()
                                    ?? throw new FileNotFoundException(L.T("Roblox from the Microsoft Store isn't installed. Pick another option under Settings › Launching › Open Roblox with.")), uri);
                break;
            default:
                if (!LinksOpen())
                {
                    // Nothing opens roblox-player: links any more, yet a launcher or client may still be here: another
                    // Roblox's uninstaller takes the links with it (seen 2026-10-19: Roblox VNG installed, took them from
                    // Bloxstrap, then removed). Whatever is left opens it, as Windows would have.
                    if (FallbackLauncher() is { } fallback)
                    {
                        Start(fallback.Exe, [.. fallback.Args, uri]);
                        break;
                    }
                    // A FileNotFoundException, like the other options' "not installed": its sentence is shown as it is.
                    if (RobloxInstalls.ProtocolHandlerPath() is { } vng && RobloxInstalls.IsVngClient(vng))
                        throw new FileNotFoundException(L.T("Only Roblox VNG is installed. Choose Roblox VNG under Settings › Launching › Open Roblox with, or install Roblox from roblox.com/download."));
                    throw new FileNotFoundException(RobloxInstalls.StoreEditionInstalled()
                        ? L.T("Only Roblox from the Microsoft Store is installed. Choose it under Settings › Launching › Open Roblox with, or install Roblox from roblox.com/download.")
                        : L.T("Roblox isn't installed. Install it from roblox.com/download, then try again."));
                }
                // Not in Bloxkeep's folder (see Program.Main): the launcher may keep running as long as Roblox does.
                Process.Start(new ProcessStartInfo(uri) { UseShellExecute = true, WorkingDirectory = Path.GetTempPath() })?.Dispose();
                break;
        }
        return tracker;
    }

    static void Start(string exe, params string[] args)
    {
        var psi = new ProcessStartInfo(exe) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(exe)! };
        foreach (var arg in args)
            psi.ArgumentList.Add(arg);
        Process.Start(psi)?.Dispose();
    }

    /// <summary>
    /// Roblox from the Microsoft Store, given the link like the other clients. Started from its Start menu entry (or its
    /// roblox: links), Microsoft's GameLaunchHelper only brings an open Roblox forward; started inside its package with
    /// Windows' Invoke-CommandInDesktopPackage, the client keeps the package's sign-in and files and opens a window of
    /// its own (seen 2026-10-16, Store Roblox 2.743).
    /// </summary>
    /// <summary>The ticket in a launch link, and any long token-like run (a wrapped line may cut "gameinfo:" off).</summary>
    static readonly System.Text.RegularExpressions.Regex TicketText = new(@"gameinfo:[^+\s'""]*|[A-Za-z0-9_\-]{40,}", System.Text.RegularExpressions.RegexOptions.CultureInvariant);

    static void StartInStorePackage(string exe, string uri)
    {
        static string Quote(string s) => "'" + s.Replace("'", "''") + "'";
        var psi = new ProcessStartInfo(Path.Combine(Environment.SystemDirectory, @"WindowsPowerShell\v1.0\powershell.exe"))
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardError = true,
            WorkingDirectory = Path.GetTempPath(),
        };
        foreach (var arg in new[] { "-NoProfile", "-NonInteractive", "-Command",
                     $"Invoke-CommandInDesktopPackage -PackageFamilyName {Quote(RobloxInstalls.StoreFamily)} -AppId {Quote(RobloxInstalls.StoreAppId)} -Command {Quote(exe)} -Args {Quote(uri)}" })
            psi.ArgumentList.Add(arg);
        var shell = Process.Start(psi) ?? throw new FileNotFoundException(L.T("Couldn't start Roblox from the Microsoft Store."));
        // Not waited for (a second or two): a failure goes to the log, and the window that doesn't come is reported as usual.
        var error = new System.Text.StringBuilder();
        shell.ErrorDataReceived += (_, e) => { if (e.Data is { Length: > 0 } line && error.Length < 2000) error.AppendLine(line); };
        shell.EnableRaisingEvents = true;
        shell.Exited += (_, _) =>
        {
            // PowerShell's error repeats the command, sign-in ticket included: that part never reaches the log.
            if (shell.ExitCode != 0 || error.Length > 0)
                Log.Note($"Store Roblox start: exit {shell.ExitCode} {TicketText.Replace(error.ToString().Trim(), "<hidden>")}");
            shell.Dispose();
        };
        shell.BeginErrorReadLine();
    }
}
