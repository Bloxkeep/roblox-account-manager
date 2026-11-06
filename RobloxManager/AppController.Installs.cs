namespace RobloxManager;

/// <summary>What is installed: the Roblox launchers and clients found, which one a launch starts, and the newest Roblox version.</summary>
public sealed partial class AppController
{
    InstallsInfo? _installs;
    /// <summary>The launcher that can update the client the launch option starts (<see cref="RobloxInstalls.UpdaterFor"/>).</summary>
    string? _updaterExe;

    /// <summary>Which Roblox clients and launchers exist, and the version of the client the chosen option starts.</summary>
    /// <param name="Others">Other Bloxstrap-style launchers found here (their option is "other:&lt;name&gt;").</param>
    sealed record InstallsInfo(string? Handler, bool Bloxstrap, bool Fishstrap, List<string> Others, bool Roblox, bool Direct, bool Store, RobloxInstalls.ClientVersion? Client,
        bool Vng = false);

    /// <param name="UpToDate">null until both versions are known.</param>
    /// <param name="Store">Roblox from the Microsoft Store is chosen: the Store installs its updates, not Roblox itself.</param>
    /// <param name="Updater">The launcher that can update this copy of Roblox now (its name), when there is one.</param>
    /// <param name="OldWindows">Windows still on a build older than Roblox's newest.</param>
    /// <param name="Reopens">Of all the windows open, those "Close, update, reopen" would open again.</param>
    sealed record RobloxVersionInfo(string? Installed, string? Latest, bool? UpToDate, bool Direct, bool Store = false, string? Updater = null,
        bool Updating = false, int OldWindows = 0, int Reopens = 0);

    /// <summary>Registry facts shown on the page; BuildState runs on every push, so they're re-read at most every 30 s.</summary>
    (bool Protocol, bool TrayAutostart) RegistryInfo()
    {
        if (_registry is { } cached && DateTime.UtcNow - _registryReadAt < RegistryCacheLifetime)
            return cached;
        _registryReadAt = DateTime.UtcNow;
        var info = (RobloxLauncher.IsProtocolRegistered(), MultiInstance.FindTrayAutostart()?.Enabled == true);
        _registry = info;
        return info;
    }

    InstallsInfo Installs()
    {
        if (_installs is not null && DateTime.UtcNow - _installsReadAt < InstallCacheLifetime)
            return _installs;
        var (via, custom) = (_vault?.Data.Settings.LaunchVia ?? "auto", _vault?.Data.Settings.CustomLauncher ?? "");
        var (installs, updater) = ReadInstalls(via, custom);
        return TakeInstalls(installs, updater, DateTime.UtcNow);
    }

    /// <summary>What is installed, from disk and registry: up to a second on a busy PC (2026-10-29, the owner's: 0.9 s in a
    /// process check), so the process tick reads it on a worker (<see cref="TickRobloxUpdates"/>).</summary>
    static (InstallsInfo Installs, string? Updater) ReadInstalls(string via, string custom) => (new InstallsInfo(
            // What Default opens Roblox with: the links' program, or what stands in for it when they are gone or Roblox
            // VNG's (RobloxLauncher.FallbackLauncher), so Settings names the program really used.
            Handler: (RobloxInstalls.UsableHandlerPath() ?? RobloxLauncher.FallbackLauncher()?.Exe) is { } used
                ? Path.GetFileNameWithoutExtension(used) : null,
            Bloxstrap: RobloxInstalls.FindBootstrapper("Bloxstrap") is not null,
            Fishstrap: RobloxInstalls.FindBootstrapper("Fishstrap") is not null,
            Others: RobloxInstalls.FindOtherBootstrappers(),
            Roblox: RobloxInstalls.FindOfficialClient() is not null,
            Direct: (RobloxInstalls.FindAnyClient() ?? RobloxInstalls.FindVngClient()) is not null,
            Store: RobloxInstalls.StoreEditionInstalled(),
            Client: RobloxInstalls.VersionOf(RobloxInstalls.ClientFor(via, custom)),
            Vng: RobloxInstalls.FindVngClient() is not null),
        RobloxInstalls.UpdaterFor(via, custom));

    /// <summary>A read of what is installed kept, and a new Roblox build since the last one said.</summary>
    InstallsInfo TakeInstalls(InstallsInfo installs, string? updater, DateTime readAt)
    {
        _installsReadAt = readAt;
        string? before = _installs?.Client?.Folder, beforeVersion = _installs?.Client?.Version;
        _installs = installs;
        _updaterExe = updater;
        // A new build since the last look, or an older one (a launcher pinned to a version), which isn't an update. The Store
        // edition has no build folder, so switching to or from it says nothing. Going older while the newer build stays installed
        // is Roblox switching channel (RobloxInstalls.HasBuild): nothing to say.
        if (before is { Length: > 0 } && _installs.Client is { Folder.Length: > 0 } now && now.Folder != before
            && (Version.TryParse(now.Version, out var have) && Version.TryParse(beforeVersion, out var had) && have < had) is var older
            && !(older && RobloxInstalls.HasBuild(now.Exe, before)))
        {
            string updated = older ? L.F("Roblox went back to version {0}.", now.Version) : L.F("Roblox updated to {0}.", now.Version);
            Toast(updated, "ok");
            NoteActivity("robloxUpdated", text: updated, tone: "info");
        }
        NoteRobloxBuild(_installs.Client); // Settings › Roblox updates (AppController.RobloxUpdates.cs)
        return _installs;
    }

    /// <summary>The version the chosen launcher starts, next to the newest Roblox serves. An older client updates itself on
    /// its next start (that window takes longer); Direct only when nothing would update it (<see cref="RobloxInstalls.UpdatesOnOpen"/>).</summary>
    RobloxVersionInfo RobloxVersionDto()
    {
        var installed = Installs().Client;
        // Roblox can ship a new build under the same version number: the build folder ("version-<hash>") tells them
        // apart. A newer number than Roblox serves (another channel) still counts as up to date.
        bool sameBuild = installed is not null && _latestRoblox is not null && installed.Folder == _latestRoblox.Folder;
        bool newer = installed is not null && _latestRoblox is not null
            && Version.TryParse(installed.Version, out var have) && Version.TryParse(_latestRoblox.Version, out var latest) && have > latest;
        bool foldersKnown = installed?.Folder.StartsWith("version-", StringComparison.OrdinalIgnoreCase) == true
            && _latestRoblox?.Folder.StartsWith("version-", StringComparison.OrdinalIgnoreCase) == true;
        bool? upToDate = installed is null || _latestRoblox is null ? null
            : sameBuild || newer || (!foldersKnown && installed.Version == _latestRoblox.Version);
        // The links on another channel's build (Roblox puts some accounts on test builds), with the newest build
        // installed beside it: that one is what an account on the main channel starts.
        string? shown = installed?.Version;
        if (upToDate == false && RobloxInstalls.HasBuild(installed!.Exe, _latestRoblox!.Folder))
            (shown, upToDate) = (_latestRoblox.Version, true);
        return new RobloxVersionInfo(shown, _latestRoblox?.Version, upToDate, _vault?.Data.Settings.LaunchVia == "direct" && !RobloxInstalls.UpdatesOnOpen(installed?.Exe),
            _vault?.Data.Settings.LaunchVia == "store", _updaterExe is { } updater ? RobloxInstalls.IsOwnInstaller(updater) ? "Roblox" : Path.GetFileNameWithoutExtension(updater) : null, _robloxUpdating || _updateReopenSince is not null,
            _running.Count(r => FindAccount(r.Key) is { } a && RunsOldBuild(a, r.Value.Pid)), _running.Count(r => CanReopen(r.Key, r.Value)));
    }

    async Task RefreshLatestRobloxAsync()
    {
        try
        {
            if (await RobloxApi.GetLatestClientVersionAsync() is { } latest && latest != _latestRoblox)
            {
                _latestRoblox = latest;
                PushState();
            }
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            // Shown as "couldn't check"; tried again next time.
        }
    }
}
