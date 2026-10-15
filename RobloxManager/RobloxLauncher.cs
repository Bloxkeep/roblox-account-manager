using System.Diagnostics;
using Microsoft.Win32;

namespace RobloxManager;

public static class RobloxLauncher
{
    /// <summary>
    /// Values of <see cref="AppSettings.LaunchVia"/>, plus "other:&lt;Name&gt;" for another Bloxstrap-style launcher found on
    /// this PC and "custom" for one the user picked (<see cref="AppSettings.CustomLauncher"/>).
    /// </summary>
    public static readonly string[] LaunchOptions = ["auto", "bloxstrap", "fishstrap", "roblox", "direct", "custom"];

    const string OtherPrefix = "other:";

    public static string Other(string name) => OtherPrefix + name;

    /// <summary>The launcher's name for an "other:&lt;Name&gt;" option, else null.</summary>
    public static string? OtherName(string launchVia) =>
        launchVia.StartsWith(OtherPrefix, StringComparison.Ordinal) && launchVia.Length > OtherPrefix.Length
            ? launchVia[OtherPrefix.Length..]
            : null;

    public static bool IsValidOption(string launchVia) =>
        LaunchOptions.Contains(launchVia) || (OtherName(launchVia) is { } name && name.IndexOfAny(Path.GetInvalidFileNameChars()) < 0);

    public static bool IsProtocolRegistered()
    {
        using var key = Registry.ClassesRoot.OpenSubKey(@"roblox-player\shell\open\command");
        return key is not null;
    }

    static string NewBrowserTrackerId() => Random.Shared.NextInt64(100_000_000_000, 999_999_999_999).ToString();

    static string PlaceLauncherUrl(GameTarget t, string browserTrackerId)
    {
        string common = $"browserTrackerId={browserTrackerId}&placeId={t.PlaceId}&isPlayTogetherGame=false&joinAttemptId={Guid.NewGuid()}";
        const string baseUrl = "https://www.roblox.com/Game/PlaceLauncher.ashx?";
        if (t.LinkCode is not null)
            return $"{baseUrl}request=RequestPrivateGame&{common}&linkCode={Uri.EscapeDataString(t.LinkCode)}&joinAttemptOrigin=privateServerListJoin";
        if (t.JobId is not null)
            return $"{baseUrl}request=RequestGameJob&{common}&gameId={t.JobId}&joinAttemptOrigin=publicServerListJoin";
        return $"{baseUrl}request=RequestGame&{common}&joinAttemptOrigin=PlayButton";
    }

    /// <summary>The same roblox-player: URI the website hands to the client when you press Play.</summary>
    public static string BuildLaunchUri(string ticket, GameTarget? target)
    {
        string trackerId = NewBrowserTrackerId();
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
    public static void Launch(string ticket, GameTarget? target, string launchVia, string customLauncher = "")
    {
        string uri = BuildLaunchUri(ticket, target);
        if (OtherName(launchVia) is { } other)
        {
            Start(RobloxInstalls.FindBootstrapper(other) ?? throw new FileNotFoundException(L.F("{0} is not installed.", other)), uri);
            return;
        }
        switch (launchVia)
        {
            case "custom":
                if (!File.Exists(customLauncher))
                    throw new FileNotFoundException(L.T("The launcher you picked is gone. Choose it again under Settings › Open Roblox with."));
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
                Start(RobloxInstalls.FindAnyClient()
                      ?? throw new FileNotFoundException(L.T("RobloxPlayerBeta.exe was not found. Pick another option under Settings › Open Roblox with, or reinstall Roblox.")), uri);
                break;
            default:
                // A FileNotFoundException, like the other options' "not installed": its sentence is shown as it is.
                if (!IsProtocolRegistered())
                    throw new FileNotFoundException(L.T("Roblox is not installed (the roblox-player protocol is missing)."));
                // Not in Bloxkeep's folder (see Program.Main): the launcher may keep running as long as Roblox does.
                Process.Start(new ProcessStartInfo(uri) { UseShellExecute = true, WorkingDirectory = Path.GetTempPath() })?.Dispose();
                break;
        }
    }

    static void Start(string exe, params string[] args)
    {
        var psi = new ProcessStartInfo(exe) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(exe)! };
        foreach (var arg in args)
            psi.ArgumentList.Add(arg);
        Process.Start(psi)?.Dispose();
    }
}
