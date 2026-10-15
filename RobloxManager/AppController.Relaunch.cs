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
            // A page on screen says what it shows now (one in the background did so as it went there).
            if (PageAttached && !_background)
            {
                Emit("saveView", null);
                await Task.Delay(700);
            }
            // Something may have started meanwhile: the next quiet moment then.
            if (_disposed || _updater.Status != "ready" || !QuietForRelaunch(DateTime.UtcNow) || !SaveBeforeClosing())
                return;
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
            Log.Note($"Installing {_updater.NewVersion} in the background (window {window}).");
            _rejoinCts?.Cancel();
            Ui.ClearSecretClipboard();
            QuitForUpdate?.Invoke();
            // Still here: the window refused to close (changes it couldn't save). The key mustn't stay on disk.
            if (!_disposed)
            {
                Relaunch.Discard();
                _relaunchArgs = null;
            }
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
