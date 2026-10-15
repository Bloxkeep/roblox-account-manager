using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Updates installed in the background, without the "Restart to update" click (see <see cref="Relaunch"/>): once one
/// is downloaded and nobody is using Bloxkeep, the app quits, Velopack installs the update and starts it again, and
/// the new run carries on where this one stopped. The button stays for anyone who wants it at once.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Closes the app for real (the window's Quit), for the update's restart.</summary>
    public Action? QuitForUpdate { get; init; }

    /// <summary>The window now: hidden (in the tray), minimized or shown, whether it is the one in front, and whether the mini window is up.</summary>
    public Func<(string Window, bool Foreground, bool MiniShown)>? WindowNow { get; init; }

    /// <summary>What Velopack starts the new version with, once the app quits for an update in the background.</summary>
    string[]? _relaunchArgs;
    bool _relaunching;
    readonly DateTime _started = DateTime.UtcNow;

    /// <summary>Not this soon after a start (a restart can never loop), nor while someone used the PC this recently with a window of the app on screen.</summary>
    static readonly TimeSpan RelaunchAfterStart = TimeSpan.FromMinutes(3);
    static readonly TimeSpan RelaunchIdle = TimeSpan.FromMinutes(10);

    void RelaunchIfQuiet(DateTime now)
    {
        if (_relaunching || _applyingUpdate || _disposed || !Prefs.CheckUpdates || _updater.Status != "ready" || !QuietForRelaunch(now))
            return;
        _ = RelaunchAsync();
    }

    /// <summary>
    /// Nobody is using Bloxkeep and it is in the middle of nothing a restart would lose: no launch, rejoin or planned
    /// restart under way, no sign-in, captcha, message box or half-filled form open, no backup, import or save
    /// pending. Roblox windows that are simply running don't count: they keep running and are followed again after.
    /// </summary>
    bool QuietForRelaunch(DateTime now)
    {
        if (now - _started < RelaunchAfterStart || Settling)
            return false;
        if (_launchCts is not null || _pending.Count > 0 || _rejoinQueue.Count > 0 || _rejoining || _closingForRejoin.Count > 0
            || _restarts.Count > 0 || _handedOver.Count > 0 || _nudging || now < _sweepBackgroundUntil)
            return false;
        if (_quickLogin is not null || _vaultImportOpening || _vaultImportPath is not null || _backupWrite is { IsCompleted: false }
            || _signInCheckRunning || _discordSending || _saveFailed || PageBusy || Ui.ModalsOpen > 0)
            return false;
        // Another window of the app (the sign-in browser, a captcha) means someone is in the middle of it.
        if (Application.OpenForms.Cast<Form>().Any(f => f != _owner && f is not MiniForm && f.Visible))
            return false;
        var (window, _, mini) = WindowNow?.Invoke() ?? ("hidden", false, false);
        return (window != "shown" && !mini) || AntiAfk.IdleTime() >= RelaunchIdle;
    }

    async Task RelaunchAsync()
    {
        _relaunching = true;
        try
        {
            await SaveViewAsync();
            // Something may have started meanwhile: the next quiet moment then.
            if (_disposed || _updater.Status != "ready" || !QuietForRelaunch(DateTime.UtcNow) || !SaveBeforeClosing())
                return;
            QuitIntoUpdate(showWindow: false);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or CryptographicException or ObjectDisposedException)
        {
            Log.Write(ex);
            Relaunch.Discard();
            _relaunchArgs = null;
        }
        finally
        {
            _relaunching = _relaunchArgs is not null;
        }
    }

    /// <summary>
    /// "Restart to update": the same restart as an update in the background, at once, with Bloxkeep's own "Updating"
    /// window over the seconds it takes (Velopack's own is a plain Windows dialog). Roblox windows stay open.
    /// </summary>
    async Task<object?> ApplyUpdateAsync()
    {
        // A second click while the first one is on its way out does nothing.
        if (_applyingUpdate || _relaunching)
            return null;
        // The button can be older than the state (the page pushed before a failed re-check): say why nothing happens.
        if (_updater.Status != "ready")
            throw new UserError(_updater.Status is "checking" or "downloading"
                ? L.T("The update is still downloading. Try again in a moment.")
                : L.T("No update is ready to install. Check for updates again."));
        if (_launchCts is not null)
            throw new UserError(L.T("Wait until Bloxkeep finishes opening Roblox."));
        _applyingUpdate = true;
        try
        {
            await SaveViewAsync();
            if (!SaveBeforeClosing())
                throw new UserError(L.T("Couldn't save your changes, so the update waits. Try again in a moment."));
            QuitIntoUpdate(showWindow: true);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or CryptographicException or ObjectDisposedException)
        {
            Log.Write(ex);
            Relaunch.Discard();
            _relaunchArgs = null;
        }
        finally
        {
            _applyingUpdate = _relaunchArgs is not null;
        }
        if (!_disposed)
            throw new UserError(L.T("Couldn't restart to update. It installs by itself when you quit Bloxkeep."));
        return null;
    }

    /// <summary>A page on screen says what it shows now (one in the background did so as it went there).</summary>
    async Task SaveViewAsync()
    {
        if (!PageAttached || _background)
            return;
        Emit("saveView", null);
        await Task.Delay(700);
    }

    /// <summary>
    /// Writes the hand-over for the next run and quits; once this process is gone Velopack installs the update
    /// silently and starts the new version with it (see Dispose: ApplyOnExit). When the window refuses to close
    /// (changes it couldn't save), nothing of it stays: the key mustn't wait on disk.
    /// </summary>
    void QuitIntoUpdate(bool showWindow)
    {
        var (window, foreground, _) = WindowNow?.Invoke() ?? ("hidden", false, false);
        var state = new Relaunch.State
        {
            WrittenAt = DateTime.UtcNow, From = _updater.CurrentVersion, Window = window, Foreground = foreground,
            View = _vault is not null ? _pageView : null, WeakPassword = _weakPassword,
        };
        if (_vault is { } vault)
        {
            var (key, salt, iterations) = vault.ExportKey();
            state.Key = Convert.ToBase64String(key);
            state.Salt = Convert.ToBase64String(salt);
            state.Iterations = iterations;
            CryptographicOperations.ZeroMemory(key);
        }
        _relaunchArgs = Relaunch.Write(state);
        Log.Note($"Installing {_updater.NewVersion} {(showWindow ? "now (Restart to update)" : "in the background")}, window {window}.");
        if (showWindow)
            ShowUpdatingWindow(_updater.NewVersion);
        _rejoinCts?.Cancel();
        Ui.ClearSecretClipboard();
        QuitForUpdate?.Invoke();
        if (!_disposed)
        {
            Relaunch.Discard();
            _relaunchArgs = null;
        }
    }

    /// <summary>
    /// Bloxkeep's "Updating Bloxkeep to x" window (the installer's window program, shipped beside the app), run from a
    /// copy in %TEMP% since the program folder is about to be replaced; it closes once the new version is up.
    /// </summary>
    void ShowUpdatingWindow(string? version)
    {
        try
        {
            string source = Path.Combine(AppContext.BaseDirectory, "BloxkeepUninstall.exe");
            if (!File.Exists(source))
                return;
            string dir = Path.Combine(Path.GetTempPath(), "Bloxkeep-Updating-" + Guid.NewGuid().ToString("N")[..8]);
            Directory.CreateDirectory(dir);
            string copy = Path.Combine(dir, "BloxkeepUpdating.exe");
            File.Copy(source, copy);
            var info = new ProcessStartInfo(copy) { UseShellExecute = false, WorkingDirectory = dir };
            foreach (string arg in new[] { "--updating", version ?? "", "--lang", L.Language })
                info.ArgumentList.Add(arg);
            Process.Start(info)?.Dispose();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or System.ComponentModel.Win32Exception)
        {
            Log.Write(ex);
        }
    }

    /// <summary>
    /// The run that follows an update in the background: unlocked with the key the previous run handed over, its
    /// page's view kept for the page. When the key doesn't open the vault (the password changed meanwhile), the lock
    /// screen asks as usual.
    /// </summary>
    public void Resume(Relaunch.State state)
    {
        if (state.Key is { } key && state.Salt is { } salt)
        {
            try
            {
                var vault = Vault.OpenWithKey(Convert.FromBase64String(key), Convert.FromBase64String(salt), state.Iterations);
                _weakPassword = state.WeakPassword;
                OnUnlocked(vault);
            }
            catch (Exception ex) when (ex is InvalidPasswordException or IOException or UnauthorizedAccessException or InvalidDataException or JsonException or FormatException)
            {
                Log.Write(ex);
            }
        }
        if (_vault is not null && state.View is { } view)
        {
            _pageView = view;
            ResumingPage = true;
        }
        Log.Note($"Updated from {state.From} in the background; {(_vault is not null ? "unlocked again" : "locked")}, window {state.Window}.");
    }
}
