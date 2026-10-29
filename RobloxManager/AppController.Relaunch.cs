using System.Diagnostics;
using System.Security.Cryptography;

namespace RobloxManager;

/// <summary>
/// Updates installed in the background (see <see cref="Relaunch"/>): once one is downloaded and nobody is using
/// Bloxkeep, the app quits, Velopack installs and restarts it, and the new run carries on where this one stopped.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Closes the app for real (the window's Quit), for the update's restart.</summary>
    public Action? QuitForUpdate { get; init; }

    /// <summary>The macOS themes' buttons: "close" (as the title bar's ✕, so to the tray when that is on), "minimize", "zoom".</summary>
    public Action<string>? WindowControl { get; init; }

    /// <summary>Window: "hidden" (in the tray), "minimized" or "shown".</summary>
    public Func<(string Window, bool Foreground, bool MiniShown)>? WindowNow { get; init; }

    string[]? _relaunchArgs;
    bool _relaunching;
    /// <summary>Moved back with the clock (see TrackProcesses), like every other start time.</summary>
    DateTime _started = DateTime.UtcNow;

    /// <summary>After an update that didn't take: no new try by itself for a day, so a failing install can't loop every few
    /// minutes. The button still works.</summary>
    DateTime _autoRelaunchPausedUntil;

    Relaunch.State? _resumedFrom;
    string? _updateFailedNotice;

    /// <summary>Not this soon after a start (so a restart can never loop), nor while the PC was used this recently with the app on screen.</summary>
    static readonly TimeSpan RelaunchAfterStart = TimeSpan.FromMinutes(3);
    static readonly TimeSpan RelaunchIdle = TimeSpan.FromMinutes(10);

    /// <summary>The lock screen's answer (see Lock.tsx): true, it gave the update its place (nothing typed); false, it kept the
    /// form (something typed, or its link clicked); null, not answered. An unlock or a failed download clears it.</summary>
    bool? _updateAtLock;

    /// <summary>An update found before the vault is opened installs there and then (the owner's choice, 2026-10-25, as other
    /// apps update when opened); the lock screen offers its place while it downloads. Found after the unlock, it waits
    /// for a quiet moment instead of cutting in.</summary>
    bool UpdateOffersAtLock => _vault is null && !_screenLocked && Prefs.CheckUpdates && _updater.Status is "downloading" or "ready"
        && DateTime.UtcNow >= _autoRelaunchPausedUntil && !_relaunching && !_applyingUpdate;

    object? SetUpdateAtLock(bool go)
    {
        _updateAtLock = UpdateOffersAtLock ? go : null;
        PushState();
        return null;
    }

    /// <summary>Vault closed, and the screen given to the update or the window out of sight: nobody is in the app, so a
    /// downloaded update goes at once.</summary>
    bool UpdateGoesAtLock()
    {
        if (_vault is not null || _screenLocked)
            return false;
        var (window, _, mini) = WindowNow?.Invoke() ?? ("hidden", false, false);
        return _updateAtLock == true || (window != "shown" && !mini);
    }

    void RelaunchIfQuiet(DateTime now)
    {
        bool atLock = UpdateGoesAtLock();
        if (_relaunching || _applyingUpdate || _disposed || !Prefs.CheckUpdates || _updater.Status != "ready"
            || now < _autoRelaunchPausedUntil || !QuietForRelaunch(now, atLock))
            return;
        _ = RelaunchAsync(atLock);
    }

    /// <summary>Nobody is using Bloxkeep and nothing a restart would lose is under way. Running Roblox windows don't count:
    /// they keep running and are followed again after.</summary>
    /// <param name="atLock">See <see cref="UpdateGoesAtLock"/>: no wait after the start, and nobody's use holds the update back.</param>
    bool QuietForRelaunch(DateTime now, bool atLock = false)
    {
        if ((!atLock && now - _started < RelaunchAfterStart) || Settling || HomeSessionsActive)
            return false;
        if (OpeningRoblox || _restarts.Count > 0 || _nudging || now < _sweepBackgroundUntil)
            return false;
        if (_quickLogin is not null || _vaultImportOpening || _vaultImportPath is not null || _backupWrite is { IsCompleted: false }
            || _signInCheckRunning || _discordSending || _saveFailed || PageBusy || Ui.ModalsOpen > 0)
            return false;
        // Another window of the app (the sign-in browser, a captcha) means someone is in the middle of it.
        if (Application.OpenForms.Cast<Form>().Any(f => f != _owner && f is not MiniForm && f.Visible))
            return false;
        if (atLock)
            return true;
        var (window, _, mini) = WindowNow?.Invoke() ?? ("hidden", false, false);
        return (window != "shown" && !mini) || AntiAfk.IdleTime() >= RelaunchIdle;
    }

    /// <summary>A late window counts too: a restart would forget its account. A rejoin due later than <see cref="RejoinSoon"/>
    /// is kept with the vault and picked up after the restart: on 2026-10-21 a banned account's long rejoin wait kept
    /// "Restart to update" refused while nothing was opening.</summary>
    bool OpeningRoblox => _openingHome || _launchCts is not null || _pending.Count > 0 || _late.Count > 0 || _handedOver.Count > 0
        || _rejoinQueue.Any(r => r.Due - DateTime.UtcNow < RejoinSoon) || _rejoining || _closingForRejoin.Count > 0;

    static readonly TimeSpan RejoinSoon = TimeSpan.FromSeconds(30);

    /// <param name="atLock">The "Updating" window shows (when Bloxkeep's is on screen), as for the button.</param>
    async Task RelaunchAsync(bool atLock = false)
    {
        _relaunching = true;
        try
        {
            await SaveViewAsync();
            // Something may have started meanwhile: the next quiet moment then.
            if (_disposed || !Prefs.CheckUpdates || _updater.Status != "ready" || atLock != UpdateGoesAtLock()
                || !QuietForRelaunch(DateTime.UtcNow, atLock) || !SaveBeforeClosing())
                return;
            // Opened to the tray (at sign-in), it updates there without a window coming up.
            QuitIntoUpdate(showWindow: atLock && WindowNow?.Invoke().Window == "shown");
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

    /// <summary>"Restart to update": the background restart at once, with Bloxkeep's own "Updating" window (Velopack's is a
    /// plain Windows dialog). Roblox windows stay open.</summary>
    async Task<object?> ApplyUpdateAsync()
    {
        if (_applyingUpdate || _relaunching)
            return null;
        // The button can be older than the state (pushed before a failed re-check): say why nothing happens.
        if (_updater.Status != "ready")
            throw new UserError(_updater.Status is "checking" or "downloading"
                ? L.T("The update is still downloading. Try again in a moment.")
                : L.T("No update is ready to install. Check for updates again."));
        if (HomeSessionsActive)
            throw new UserError(L.T("Close the separate Roblox sign-in windows before updating Bloxkeep."));
        if (OpeningRoblox)
            throw new UserError(L.T("Wait until Bloxkeep finishes opening Roblox."));
        _applyingUpdate = true;
        try
        {
            await SaveViewAsync();
            // Started during that moment (a scheduled preset, a rejoin): the restart would cut it off.
            if (HomeSessionsActive)
                throw new UserError(L.T("Close the separate Roblox sign-in windows before updating Bloxkeep."));
            if (OpeningRoblox)
                throw new UserError(L.T("Wait until Bloxkeep finishes opening Roblox."));
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

    /// <summary>Writes the hand-over and quits; Velopack then installs and starts the new version with it (see Dispose:
    /// ApplyOnExit). If the window refuses to close, the hand-over is removed: the key mustn't wait on disk.</summary>
    void QuitIntoUpdate(bool showWindow)
    {
        var (window, foreground, _) = WindowNow?.Invoke() ?? ("hidden", false, false);
        var state = new Relaunch.State
        {
            WrittenAt = DateTime.UtcNow, From = _updater.CurrentVersion, To = _updater.NewVersion, Window = window, Foreground = foreground,
            View = Open is not null ? _pageView : null, WeakPassword = _weakPassword, ScreenLocked = _screenLocked,
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
        // In the background, a hand-over that comes too late starts it back in the tray (see Relaunch.AfterUpdate);
        // after the click, the person is waiting for the window.
        if (!showWindow)
            _relaunchArgs = [.. _relaunchArgs, Relaunch.AfterUpdateArg, _updater.NewVersion ?? ""];
        Log.Note($"Installing {_updater.NewVersion} {(showWindow ? "now (Restart to update)" : "in the background")}, window {window}.");
        WatchUpdate(_updater.NewVersion, showWindow);
        _rejoinCts?.Cancel();
        Ui.ClearSecretClipboard();
        QuitForUpdate?.Invoke();
        if (!_disposed)
        {
            Relaunch.Discard();
            _relaunchArgs = null;
        }
    }

    /// <summary>The watch over the restart: the installer's program, run from a %TEMP% copy since the program folder is about
    /// to be replaced. It shows "Updating Bloxkeep to x" (unless quiet); if this process doesn't quit it goes away; if
    /// Bloxkeep isn't up again after a minute or so it removes the hand-over (the key) and starts Bloxkeep itself.</summary>
    void WatchUpdate(string? version, bool showWindow)
    {
        try
        {
            string appDir = Path.TrimEndingDirectorySeparator(AppContext.BaseDirectory);
            string source = Path.Combine(appDir, "BloxkeepUninstall.exe");
            if (!File.Exists(source) || Path.GetDirectoryName(appDir) is not { } root)
                return;
            string dir = Path.Combine(Path.GetTempPath(), "Bloxkeep-Updating-" + Guid.NewGuid().ToString("N")[..8]);
            Directory.CreateDirectory(dir);
            string copy = Path.Combine(dir, "BloxkeepUpdating.exe");
            File.Copy(source, copy);
            var info = new ProcessStartInfo(copy) { UseShellExecute = false, WorkingDirectory = dir };
            List<string> args = ["--updating", version ?? "", "--lang", L.Language, "--wait-pid", Environment.ProcessId.ToString(),
                "--app", Path.Combine(root, "Bloxkeep.exe"), "--data", Vault.DataDir];
            if (!showWindow)
                args.Add("--quiet");
            foreach (string arg in args)
                info.ArgumentList.Add(arg);
            Process.Start(info)?.Dispose();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or System.ComponentModel.Win32Exception)
        {
            Log.Write(ex);
        }
    }

    /// <summary>Unlocks with the handed-over key and keeps the page's view. No key (too late, or started by the watch) or one
    /// that doesn't open the vault: the lock screen asks as usual.</summary>
    public void Resume(Relaunch.State state)
    {
        _resumedFrom = state;
        _vaultPick = false; // the vault that was open comes back, not the picker
        // Still the version it was meant to leave: the install didn't take.
        if (!string.IsNullOrEmpty(state.To) && state.To != _updater.CurrentVersion)
        {
            _autoRelaunchPausedUntil = DateTime.UtcNow.AddDays(1);
            bool busy = UpdateFolderWasBusy();
            Log.Note($"The update to {state.To} didn't install (this is {_updater.CurrentVersion}{(busy ? ", its folder was in use" : "")}); no new try by itself today.");
            // A quiet fix is tried again tomorrow without a word: it has no button to click.
            _updateFailedNotice = WhatsNew.IsQuietUpdate(state.To, _updater.CurrentVersion) ? null : busy
                ? L.F("Bloxkeep couldn't update to {0}: a program it opened earlier (Bloxstrap, or a Roblox window) was still using its folder. Close those, then click Restart to update again.", state.To)
                : L.F("Bloxkeep couldn't update to {0}. Click Restart to update to try again.", state.To);
        }
        if (state.Key is { } key && state.Salt is { } salt)
        {
            // An antivirus can hold the vault a moment right after an install.
            for (int attempt = 1; _vault is null; attempt++)
            {
                try
                {
                    var vault = Vault.OpenWithKey(Convert.FromBase64String(key), Convert.FromBase64String(salt), state.Iterations);
                    _weakPassword = state.WeakPassword;
                    OnUnlocked(vault);
                    // Behind the lock screen it stays, though the key came along for what runs in the background.
                    _screenLocked = state.ScreenLocked;
                }
                catch (IOException ex) when (attempt < 3)
                {
                    Log.Write(ex);
                    Thread.Sleep(400);
                }
                catch (Exception ex)
                {
                    Log.Write(ex);
                    break;
                }
            }
        }
        if (Open is not null && state.View is { } view)
        {
            _pageView = view;
            ResumingPage = true;
        }
        Log.Note($"{(state.From is { } from ? $"Updated from {from}" : "Started again after an update")}; {(_screenLocked ? "behind the lock screen again" : _vault is not null ? "unlocked again" : "locked")}, window {state.Window}.");
    }

    /// <summary>Velopack's log since the newest "Applying package" says the folder was in use: a program Bloxkeep started
    /// before 1.0.6 was still running "in" it.</summary>
    static bool UpdateFolderWasBusy()
    {
        try
        {
            string log = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "velopack", "velopack_BloxkeepApp.log");
            using var file = new FileStream(log, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            file.Seek(Math.Max(0, file.Length - 64_000), SeekOrigin.Begin);
            string tail = new StreamReader(file).ReadToEnd();
            int apply = tail.LastIndexOf("Applying package", StringComparison.Ordinal);
            return apply >= 0 && tail.IndexOf("used by another process", apply, StringComparison.Ordinal) >= 0;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return false;
        }
    }

    void SayUpdateFailed()
    {
        if (_updateFailedNotice is not { } notice || (!PageAttached && !_background))
            return;
        _updateFailedNotice = null;
        Notify(notice, "warn", "updateFailed");
    }

    /// <summary>Bloxkeep came back locked after an update (a scheduled preset can't run until unlocked); else null.</summary>
    public string? LockedAfterUpdate()
    {
        if (_resumedFrom is not { } state || _vault is not null)
            return null;
        return !string.IsNullOrEmpty(state.To) && state.To != _updater.CurrentVersion
            ? WhatsNew.IsQuietUpdate(state.To, _updater.CurrentVersion) ? L.T("Bloxkeep restarted. Unlock it to carry on.")
                : L.F("The update to {0} didn't install, so Bloxkeep started again as it was. Unlock it to carry on.", state.To)
            : WhatsNew.IsQuietUpdate(_updater.CurrentVersion, state.From) ? L.T("Bloxkeep installed a small fix. Unlock it to carry on.")
            : L.F("Bloxkeep was updated to {0}. Unlock it to carry on.", _updater.CurrentVersion ?? "");
    }
}
