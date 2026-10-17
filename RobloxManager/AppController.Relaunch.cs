using System.Diagnostics;
using System.Security.Cryptography;

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

    /// <summary>
    /// The macOS themes' red, yellow and green buttons: "close" (as the title bar's ✕, so to the tray when that is
    /// on), "minimize", "zoom" (maximise, or back from it).
    /// </summary>
    public Action<string>? WindowControl { get; init; }

    /// <summary>The window now: hidden (in the tray), minimized or shown, whether it is the one in front, and whether the mini window is up.</summary>
    public Func<(string Window, bool Foreground, bool MiniShown)>? WindowNow { get; init; }

    /// <summary>What Velopack starts the new version with, once the app quits for an update in the background.</summary>
    string[]? _relaunchArgs;
    bool _relaunching;
    /// <summary>Moved back with the clock (see TrackProcesses), like every other start time.</summary>
    DateTime _started = DateTime.UtcNow;

    /// <summary>
    /// An update in the background didn't take (the run that came back is still the old version): no new try by
    /// itself for a day, so a failing install can't go round every few minutes. The button still works.
    /// </summary>
    DateTime _autoRelaunchPausedUntil;

    /// <summary>This run was started again after an update (see <see cref="Resume"/>).</summary>
    Relaunch.State? _resumedFrom;
    /// <summary>Said once a page (or Windows' notification) can show it: the update this run was meant to be didn't install.</summary>
    string? _updateFailedNotice;

    /// <summary>Not this soon after a start (a restart can never loop), nor while someone used the PC this recently with a window of the app on screen.</summary>
    static readonly TimeSpan RelaunchAfterStart = TimeSpan.FromMinutes(3);
    static readonly TimeSpan RelaunchIdle = TimeSpan.FromMinutes(10);

    void RelaunchIfQuiet(DateTime now)
    {
        if (_relaunching || _applyingUpdate || _disposed || !Prefs.CheckUpdates || _updater.Status != "ready"
            || now < _autoRelaunchPausedUntil || !QuietForRelaunch(now))
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
        if (now - _started < RelaunchAfterStart || Settling || HomeSessionsActive)
            return false;
        if (OpeningRoblox || _restarts.Count > 0 || _nudging || now < _sweepBackgroundUntil)
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

    /// <summary>
    /// Roblox being opened or rejoined: a launch, a window still awaited (a late one too, whose account a restart
    /// would forget), a rejoin queued or under way.
    /// </summary>
    bool OpeningRoblox => _openingHome || _launchCts is not null || _pending.Count > 0 || _late.Count > 0 || _handedOver.Count > 0
        || _rejoinQueue.Count > 0 || _rejoining || _closingForRejoin.Count > 0;

    async Task RelaunchAsync()
    {
        _relaunching = true;
        try
        {
            await SaveViewAsync();
            // Something may have started meanwhile (or updates were turned off): the next quiet moment then.
            if (_disposed || !Prefs.CheckUpdates || _updater.Status != "ready" || !QuietForRelaunch(DateTime.UtcNow) || !SaveBeforeClosing())
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
        if (HomeSessionsActive)
            throw new UserError(L.T("Close all Roblox windows before updating while separate sign-ins are open."));
        if (OpeningRoblox)
            throw new UserError(L.T("Wait until Bloxkeep finishes opening Roblox."));
        _applyingUpdate = true;
        try
        {
            await SaveViewAsync();
            // Started during that moment (a scheduled preset, a rejoin): the restart would cut it off.
            if (HomeSessionsActive)
                throw new UserError(L.T("Close all Roblox windows before updating while separate sign-ins are open."));
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

    /// <summary>
    /// The watch over the restart, by the installer's window program (shipped beside the app, run from a copy in
    /// %TEMP% since the program folder is about to be replaced). For "Restart to update" it shows "Updating Bloxkeep
    /// to x"; in the background it shows nothing. When this process doesn't quit (the close was refused) it goes
    /// away; when Bloxkeep isn't up again after a minute or so (the install failed, the new version crashed) it
    /// removes the hand-over, since the key mustn't wait on disk, and starts Bloxkeep itself.
    /// </summary>
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

    /// <summary>
    /// The run that follows an update: unlocked with the key the previous run handed over, its page's view kept for
    /// the page. When there is no key (it came too late, or Bloxkeep was started by the watch over the update) or
    /// it doesn't open the vault (the password changed meanwhile), the lock screen asks as usual.
    /// </summary>
    public void Resume(Relaunch.State state)
    {
        _resumedFrom = state;
        // Still the version it was meant to leave: the install didn't take. No new try by itself for a while.
        if (!string.IsNullOrEmpty(state.To) && state.To != _updater.CurrentVersion)
        {
            _autoRelaunchPausedUntil = DateTime.UtcNow.AddDays(1);
            bool busy = UpdateFolderWasBusy();
            Log.Note($"The update to {state.To} didn't install (this is {_updater.CurrentVersion}{(busy ? ", its folder was in use" : "")}); no new try by itself today.");
            _updateFailedNotice = busy
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
                    // Behind the lock screen it was: so it stays, though the key came along for what runs in the background.
                    _screenLocked = state.ScreenLocked;
                }
                catch (IOException ex) when (attempt < 3)
                {
                    Log.Write(ex);
                    Thread.Sleep(400);
                }
                // Anything else (the password changed meanwhile, a damaged file): the lock screen asks as usual.
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

    /// <summary>
    /// Whether Velopack's last try to install said Bloxkeep's folder was in use (its log, the newest "Applying package"
    /// onwards): a program Bloxkeep started before 1.0.6 was still running "in" that folder.
    /// </summary>
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

    /// <summary>The failed update, said once the page is up (a toast) or, in the tray, as a Windows notification.</summary>
    void SayUpdateFailed()
    {
        if (_updateFailedNotice is not { } notice || (!PageAttached && !_background))
            return;
        _updateFailedNotice = null;
        Notify(notice, "warn", "updateFailed");
    }

    /// <summary>
    /// What to tell someone whose Bloxkeep came back locked after an update (a scheduled preset can't run until it is
    /// unlocked); null when it came back unlocked, or after a normal start.
    /// </summary>
    public string? LockedAfterUpdate()
    {
        if (_resumedFrom is not { } state || _vault is not null)
            return null;
        return !string.IsNullOrEmpty(state.To) && state.To != _updater.CurrentVersion
            ? L.F("The update to {0} didn't install, so Bloxkeep started again as it was. Unlock it to carry on.", state.To)
            : L.F("Bloxkeep was updated to {0}. Unlock it to carry on.", _updater.CurrentVersion ?? "");
    }
}
