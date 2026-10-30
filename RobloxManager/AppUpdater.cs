using System.Reflection;
using Velopack;
using Velopack.Locators;
using Velopack.Logging;
using Velopack.Sources;

namespace RobloxManager;

/// <summary>Updates an installed or portable Bloxkeep from GitHub Releases: downloads in the background, offers "Restart to
/// update", and applies on close otherwise. Copies run from a build folder never update.</summary>
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
                : Repo is not null ? new UpdateManager(new ReleaseSource(Repo))
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
            if (DropUnwantedBeta())
                Status = "idle";
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

    /// <summary>Why the last check failed, in the app's language at the time it is asked for; the details are in the log.</summary>
    public string? Error => _offline is null ? null
        : _offline.Value ? L.T("Couldn't reach the update server. Check your internet connection.")
        : L.T("The update couldn't be checked or downloaded. Bloxkeep tries again later.");

    bool? _offline;

    bool _notFoundLogged;

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
            _offline = null;
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
            // Get beta versions turned off during the download: not this one.
            Set(DropUnwantedBeta() ? "idle" : "ready");
        }
        // Offline, GitHub busy, or a download that failed its checksum: report it and try again later.
        catch (Exception ex)
        {
            // A private (or renamed) repository answers 404 every time: that says nothing new after the first one.
            if (ex is not System.Net.Http.HttpRequestException { StatusCode: System.Net.HttpStatusCode.NotFound } || !_notFoundLogged)
                Log.Write(ex);
            _notFoundLogged |= ex is System.Net.Http.HttpRequestException { StatusCode: System.Net.HttpStatusCode.NotFound };
            _offline = ex is System.Net.Http.HttpRequestException { StatusCode: null } or TaskCanceledException;
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
        // When the answer came, so "up to date" says as of when (it may be an hour old).
        if (status is "upToDate" or "ready" or "error")
            CheckedAt = DateTime.UtcNow;
        Changed?.Invoke();
    }

    /// <summary>Get beta versions turned off: a beta downloaded but not installed yet is dropped, and true is returned so the
    /// regular versions are checked again.</summary>
    public bool BetaTurnedOff()
    {
        if (!DropUnwantedBeta())
            return false;
        Set("idle");
        return true;
    }

    /// <summary>A beta downloaded while Get beta versions was on, now off: not installed after all. A quiet restart or the quit
    /// still installed it (2026-10-30 review). Its package goes too, else Velopack would install it at the next start.</summary>
    bool DropUnwantedBeta()
    {
        if (_ready is not { } ready || Prefs.BetaUpdates || !WhatsNew.IsBeta(ready.Version.ToString()))
            return false;
        _ready = null;
        NewVersion = null;
        try
        {
            if (_manager is not null && VelopackLocator.IsCurrentSet && VelopackLocator.Current.PackagesDir is { Length: > 0 } packages)
                File.Delete(Path.Combine(packages, ready.FileName));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException)
        {
            Log.Write(ex);
        }
        return true;
    }

    /// <summary>When the last check got its answer (or failed); null before the first.</summary>
    public DateTime? CheckedAt { get; private set; }

    /// <summary>Installs the download once this process exits; restarts with <paramref name="restartArgs"/> when given
    /// (see <see cref="Relaunch"/>), else it waits for the next start.</summary>
    public void ApplyOnExit(string[]? restartArgs = null)
    {
        if (_manager is null || _ready is null)
            return;
        try
        {
            _manager.WaitExitThenApplyUpdates(_ready, silent: true, restart: restartArgs is not null, restartArgs: restartArgs);
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    public object Dto() => new
    {
        supported = Supported,
        // A quiet fix shows as the version it fixes; the full number stays for anyone looking closer.
        current = WhatsNew.Shown(CurrentVersion),
        build = CurrentVersion,
        status = Status,
        version = NewVersion,
        // A quiet fix (or a beta copy's next beta) on its way: no "Restart to update" while automatic updates install it by themselves.
        quiet = WhatsNew.IsQuietUpdate(NewVersion, CurrentVersion) && Prefs.CheckUpdates,
        progress = Progress,
        error = Error,
        auto = Prefs.CheckUpdates,
        beta = Prefs.BetaUpdates,
        checkedAt = CheckedAt?.ToString("o"),
    };

    /// <summary>GitHub releases, pre-releases only while Get beta versions is on at that check. Turned off on a beta, that
    /// beta stays until a newer regular version comes out.</summary>
    sealed class ReleaseSource(string repo) : IUpdateSource
    {
        readonly GithubSource _regular = new(repo, accessToken: null, prerelease: false);
        readonly GithubSource _beta = new(repo, accessToken: null, prerelease: true);
        GithubSource? _asked;

        public Task<VelopackAssetFeed> GetReleaseFeed(IVelopackLogger logger, string? appId, string channel, Guid? stagingId = null,
            VelopackAsset? latestLocalRelease = null) =>
            (_asked = Prefs.BetaUpdates ? _beta : _regular).GetReleaseFeed(logger, appId, channel, stagingId, latestLocalRelease);

        // From the source that listed it, should the switch have changed in between.
        public Task DownloadReleaseEntry(IVelopackLogger logger, VelopackAsset releaseEntry, string localFile, Action<int> progress,
            CancellationToken cancelToken = default) =>
            (_asked ?? _regular).DownloadReleaseEntry(logger, releaseEntry, localFile, progress, cancelToken);
    }
}
