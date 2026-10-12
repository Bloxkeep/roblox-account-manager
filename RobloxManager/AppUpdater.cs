using System.Reflection;
using Velopack;
using Velopack.Sources;

namespace RobloxManager;

/// <summary>
/// Keeps an installed (or portable) Bloxkeep up to date from GitHub Releases: checks quietly, downloads the new
/// version in the background, then offers "Restart to update". A downloaded update is also applied when the app
/// closes. Copies run from a build folder never update themselves.
/// </summary>
public sealed class AppUpdater
{
    /// <summary>Where releases come from, set at build time: -p:UpdateRepo=https://github.com/owner/name.</summary>
    static readonly string? Repo = typeof(AppUpdater).Assembly.GetCustomAttributes<AssemblyMetadataAttribute>()
        .FirstOrDefault(a => a.Key == "UpdateRepo")?.Value is { Length: > 0 } repo ? repo : null;

    readonly UpdateManager? _manager;
    VelopackAsset? _ready;
    Task? _run;
    int _reportedProgress = -1;

    public AppUpdater()
    {
        // For trying an update end to end before publishing: a folder (or URL) holding vpk's release files.
        string? source = Environment.GetEnvironmentVariable("BLOXKEEP_UPDATE_SOURCE") is { Length: > 0 } s ? s : null;
        try
        {
            var manager = source is not null ? new UpdateManager(source)
                : Repo is not null ? new UpdateManager(new GithubSource(Repo, accessToken: null, prerelease: false))
                : null;
            if (manager is { IsInstalled: true })
                _manager = manager;
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
        CurrentVersion = _manager?.CurrentVersion?.ToString() ?? typeof(AppUpdater).Assembly.GetName().Version?.ToString(3) ?? "";
        // Downloaded earlier but not applied yet (normally applied on exit already).
        if (_manager?.UpdatePendingRestart is { } pending)
        {
            _ready = pending;
            NewVersion = pending.Version.ToString();
            Status = "ready";
        }
    }

    /// <summary>Raised, possibly from a background thread, when the status or download progress changes.</summary>
    public event Action? Changed;

    public bool Supported => _manager is not null;
    public string CurrentVersion { get; }

    /// <summary>idle, checking, downloading, ready, upToDate or error.</summary>
    public string Status { get; private set; } = "idle";

    public string? NewVersion { get; private set; }
    public int Progress { get; private set; }
    public string? Error { get; private set; }

    /// <summary>Checks for a newer version and downloads it. Calls made while one is running share it.</summary>
    public Task CheckAsync()
    {
        if (_manager is null || _ready is not null)
            return Task.CompletedTask;
        return _run is { IsCompleted: false } ? _run : _run = RunAsync(_manager);
    }

    async Task RunAsync(UpdateManager manager)
    {
        try
        {
            Error = null;
            Set("checking");
            var update = await manager.CheckForUpdatesAsync();
            if (update is null)
            {
                Set("upToDate");
                return;
            }
            NewVersion = update.TargetFullRelease.Version.ToString();
            Progress = 0;
            _reportedProgress = -1;
            Set("downloading");
            await manager.DownloadUpdatesAsync(update, OnProgress);
            _ready = update.TargetFullRelease;
            Set("ready");
        }
        // Offline, GitHub busy, or a download that failed its checksum: report it and try again later.
        catch (Exception ex)
        {
            Log.Write(ex);
            Error = ex.Message;
            Set("error");
        }
    }

    void OnProgress(int percent)
    {
        Progress = percent;
        if (percent / 5 == _reportedProgress / 5)
            return;
        _reportedProgress = percent;
        Changed?.Invoke();
    }

    void Set(string status)
    {
        Status = status;
        Changed?.Invoke();
    }

    /// <summary>Ends this process, installs the downloaded version and starts it.</summary>
    public void RestartToUpdate()
    {
        if (_manager is not null && _ready is not null)
            _manager.ApplyUpdatesAndRestart(_ready);
    }

    /// <summary>Has the downloaded version installed once this process has exited, without starting it.</summary>
    public void ApplyOnExit()
    {
        if (_manager is null || _ready is null)
            return;
        try
        {
            _manager.WaitExitThenApplyUpdates(_ready, silent: true, restart: false);
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    public object Dto() => new
    {
        supported = Supported,
        current = CurrentVersion,
        status = Status,
        version = NewVersion,
        progress = Progress,
        error = Error,
        auto = Prefs.CheckUpdates,
    };
}
