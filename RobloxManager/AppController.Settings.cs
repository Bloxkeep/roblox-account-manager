using System.Diagnostics;
using System.Net.NetworkInformation;
using System.Text.Json;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>Settings the page changes, one command each.</summary>
public sealed partial class AppController
{
    object? SetSettings(JsonElement a)
    {
        var s = RequireVault().Data.Settings;
        if (OptStr(a, "lastPlaceInput") is { } place) s.LastPlaceInput = place.Trim();
        if (OptStr(a, "lastJobId") is { } job) s.LastJobId = job.Trim();
        if (OptStr(a, "layout") is "cards" or "list") s.Layout = OptStr(a, "layout")!;
        if (OptStr(a, "sort") is "name" or "status" or "recent" or "order") s.Sort = OptStr(a, "sort")!;
        // Sent while typing in the game box or on a list-look change: the page already shows it, so just remember it.
        if (a.ValueKind == JsonValueKind.Object && a.EnumerateObject().Any()
            && a.EnumerateObject().All(p => p.Name is "lastPlaceInput" or "lastJobId" or "layout" or "sort"))
        {
            PersistSoon();
            return null;
        }
        if (OptBool(a, "renameWindows") is { } rename) s.RenameWindows = rename;
        if (OptBool(a, "windowPictures") is { } pictures) s.WindowPictures = pictures;
        if (OptStr(a, "taskbarGroups") is { } groups && groups is "roblox" or "account" or "preset") s.TaskbarGroups = groups;
        if (OptStr(a, "windowLayout") is { } windowLayout && RobloxProcesses.IsLayout(windowLayout)) s.WindowLayout = windowLayout;
        if (a.TryGetProperty("windowWidth", out var ww) && ww.TryGetInt32(out int width))
            s.WindowWidth = Math.Clamp(width, RobloxProcesses.LeastWindow.Width, RobloxProcesses.MostWindow.Width);
        if (a.TryGetProperty("windowHeight", out var wh) && wh.TryGetInt32(out int height))
            s.WindowHeight = Math.Clamp(height, RobloxProcesses.LeastWindow.Height, RobloxProcesses.MostWindow.Height);
        // "custom" needs a launcher picked with PickLauncher; the page never supplies a path itself.
        // "other:<name>" only for a launcher actually found here, so the page can't name an arbitrary program.
        if (OptStr(a, "launchVia") is { } via && RobloxLauncher.IsValidOption(via) && (via != "custom" || RobloxLauncher.CustomLauncherReady(s.CustomLauncher))
            && (RobloxLauncher.OtherName(via) is not { } other || RobloxInstalls.FindOtherBootstrappers().Contains(other)))
        {
            s.LaunchVia = via;
            _installs = null; // the client (and its version) depends on the option
        }
        if (OptBool(a, "memoryWarnings") is { } warnings) s.MemoryWarnings = warnings;
        if (OptBool(a, "antiAfk") is { } antiAfk) s.AntiAfk = antiAfk;
        if (Has(a, "robloxConfig", JsonValueKind.Object))
        {
            var rc = a.GetProperty("robloxConfig");
            var c = s.RobloxConfig;
            if (OptStr(rc, "mode") is "off" or "all" or "marked") c.Mode = OptStr(rc, "mode")!;
            if (Has(rc, "graphics", JsonValueKind.Number)) c.Graphics = (int)Math.Clamp(Long(rc, "graphics"), 0, 10);
            if (Has(rc, "fps", JsonValueKind.Number)) c.Fps = RobloxGameSettings.FpsChoices.Contains((int)Long(rc, "fps")) ? (int)Long(rc, "fps") : 0;
            if (Has(rc, "volume", JsonValueKind.Number)) c.Volume = (int)Math.Clamp(Long(rc, "volume"), -1, 100);
        }
        if (OptBool(a, "trimBackground") is { } trim)
        {
            s.TrimBackground = trim;
            _trimRefusedShown = false;
        }
        if (OptBool(a, "autoUpdateRoblox") is { } autoUpdate)
            s.AutoUpdateRoblox = autoUpdate;
        if (OptBool(a, "fastFlags") is { } fastFlags)
            s.FastFlags = fastFlags;
        if (Has(a, "hopAbovePlayers", JsonValueKind.Number) && HopAboveChoices.Contains((int)Long(a, "hopAbovePlayers")))
            s.HopAbovePlayers = (int)Long(a, "hopAbovePlayers");
        if (Has(a, "ramLimitMb", JsonValueKind.Number) && RamLimitChoices.Contains((int)Long(a, "ramLimitMb")))
            s.RamLimitMb = (int)Long(a, "ramLimitMb");
        if (OptBool(a, "splitCores") is { } split)
        {
            s.SplitCores = split;
            _coresRefusedShown = false;
            SplitCores(_corePids); // at once, not at the next scan
        }
        if (Has(a, "launchDelaySeconds", JsonValueKind.Number)) s.LaunchDelaySeconds = (int)Math.Clamp(Long(a, "launchDelaySeconds"), 0, 120);
        // Only the choices Settings offers; choosing counts as using the PC (the wait starts from now).
        if (Has(a, "autoLockMinutes", JsonValueKind.Number) && AutoLockChoices.Contains((int)Long(a, "autoLockMinutes")))
        {
            s.AutoLockMinutes = (int)Long(a, "autoLockMinutes");
            _activeAt = DateTime.UtcNow;
        }
        if (OptBool(a, "multiInstance") is { } multi && multi != s.MultiInstance)
        {
            s.MultiInstance = multi;
            if (multi)
                MultiInstance.Enable();
            else
                MultiInstance.Disable();
            _lastArmed = MultiInstance.Armed;
            KeepMultiAtStart(multi);
        }
        // Both take effect at the next launch; the person's own fullscreen setting is put back by the process check.
        if (OptBool(a, "multiWindowed") is { } windowed)
            s.MultiInstanceWindowed = windowed;
        if (OptBool(a, "multiTile") is { } tile)
        {
            s.MultiInstanceTile = tile;
            if (tile)
                s.MaximizeOnOpen = false;
        }
        if (OptBool(a, "maximizeOnOpen") is { } maximize)
        {
            s.MaximizeOnOpen = maximize;
            if (maximize)
                s.MultiInstanceTile = false;
        }
        Persist();
        PushState();
        return null;
    }

    /// <summary>The page is replaced while this runs, so its reply may never arrive; the new page reads the state.</summary>
    async Task<object?> SetRenderingAsync(bool gpu)
    {
        if (SetSafeRendering is not null && gpu == Prefs.SafeRendering)
            await SetSafeRendering(!gpu);
        return null;
    }

    object? DisableTrayAutostart()
    {
        _registry = null;
        if (MultiInstance.FindTrayAutostart() is { Enabled: true } tray)
        {
            try
            {
                MultiInstance.DisableTrayAutostart(tray);
            }
            catch (Exception ex) when (ex is UnauthorizedAccessException or IOException or System.Security.SecurityException)
            {
                throw new UserError(L.F("Couldn't turn it off: {0}", ErrorText(ex)));
            }
        }
        PushState();
        return null;
    }

    static object? OpenUrl(string url)
    {
        Process.Start(new ProcessStartInfo(url) { UseShellExecute = true })?.Dispose();
        return null;
    }

    /// <summary>"Choose a launcher…": started with the roblox-player: link as its first argument, as Windows does for the link's handler.</summary>
    object? PickLauncher()
    {
        var s = RequireVault().Data.Settings;
        using var dialog = new OpenFileDialog
        {
            Title = L.T("Choose the program that opens Roblox"),
            Filter = L.T("Programs") + " (*.exe)|*.exe",
            InitialDirectory = RobloxLauncher.CustomLauncherReady(s.CustomLauncher) ? Path.GetDirectoryName(s.CustomLauncher)
                : Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        };
        if (Ui.Modal(() => dialog.ShowDialog(_owner)) == DialogResult.OK)
        {
            if (string.Equals(Path.GetFullPath(dialog.FileName), Path.GetFullPath(Application.ExecutablePath), StringComparison.OrdinalIgnoreCase))
                throw new UserError(L.T("Pick the program that opens Roblox, not Bloxkeep itself."));
            if (!RobloxLauncher.IsLocalProgram(dialog.FileName))
                throw new UserError(L.T("Pick a launcher on this PC: Bloxkeep doesn't open programs from a network folder."));
            s.CustomLauncher = dialog.FileName;
            s.LaunchVia = "custom";
            _installs = null;
            Persist();
        }
        // Also when cancelled: the page put its list back on the previous choice and waits for this.
        PushState();
        return null;
    }

    static string LogPath => Path.Combine(Vault.DataDir, "error.log");
    /// <summary>The folder Bloxkeep.exe runs from (an installed copy's current version, or a portable copy's own folder).</summary>
    static string AppFolder => Path.TrimEndingDirectorySeparator(AppContext.BaseDirectory);

    /// <summary>Settings' Show buttons: the vault file or error log selected in Explorer (its folder if not there yet), or the
    /// program's folder. The page never names a path to open.</summary>
    object? ShowPath(string which)
    {
        string? file = which switch { "vault" => _vault?.FilePath, "log" => LogPath, _ => null };
        var start = new ProcessStartInfo("explorer.exe");
        if (file is not null && File.Exists(file))
            start.Arguments = $"/select,\"{file}\"";
        else if (which is "vault" or "log")
            start.ArgumentList.Add(Vault.DataDir);
        else if (which == "app")
            start.ArgumentList.Add(AppFolder);
        else
            return null;
        Process.Start(start)?.Dispose();
        return null;
    }

    static object? OpenDataFolder()
    {
        Directory.CreateDirectory(Vault.DataDir);
        Process.Start(new ProcessStartInfo("explorer.exe") { ArgumentList = { Vault.DataDir } })?.Dispose();
        return null;
    }

    public void Dispose()
    {
        if (_disposed)
            return;
        _disposed = true;
        // Each step on its own: one that throws is logged and the rest still run, so the save, the update's install and
        // the vault's closing never depend on the others (review 2026-11-03). The save comes first for the same reason.
        static void Safe(Action step)
        {
            try
            {
                step();
            }
            catch (Exception ex)
            {
                Log.Write(ex);
            }
        }
        Safe(() => SaveBeforeClosing()); // normally done already, before the window agreed to close
        Safe(GiveCoresBack);
        Safe(LiftRamLimits);
        Safe(GiveMemoryPriorityBack); // no Roblox window stays at low RAM priority without Bloxkeep to raise it again
        // A launch cut short: the next Roblox opened by hand gets the person's own file. Also one that couldn't be put
        // back earlier (held, read-only): still noted, tried once more.
        if (_robloxFlagsPending || RobloxFlags.Pending)
            Safe(RobloxFlags.Restore);
        // Quit within seconds of a launch (or during one): the process check never put the person's own in-game settings
        // back, so their next Roblox would start windowed, muted, at Light AFK's FPS (2026-10-30 review). With Roblox
        // still open the copy is kept, for the next start to finish the job. Also one that couldn't be written earlier.
        if (_robloxSettingsPending || RobloxGameSettings.HasBaseline)
            Safe(() => RobloxGameSettings.Restore(final: RobloxProcesses.GetPlayerPids().Length == 0));
        Safe(StopWatchingDrags);
        Safe(GiveIconsBack);
        // A macro's held keys and buttons let go now, before the windows go back: its own endings never run once this
        // thread closes, and W stayed down until the person pressed it (B5).
        Safe(() => _macroPlayer?.ReleaseAll("closing"));
        // Quitting during a nudge or a /pc screenshot left that window see-through and click-through until Bloxkeep ran
        // again (review 2026-10-30). Not waited for: both go on on this thread, which is closing; one still running
        // stops at its next window (_disposed) and puts back only what it changed.
        Safe(() => AntiAfk.Recover(RobloxProcesses.VisibleWindows(RobloxProcesses.GetPlayerPids()).Values.SelectMany(w => w)));
        Safe(() =>
        {
            SystemEvents.PowerModeChanged -= OnPowerModeChanged;
            SystemEvents.TimeChanged -= OnTimeChanged;
            NetworkChange.NetworkAvailabilityChanged -= OnNetworkAvailabilityChanged;
            _updater.Changed -= OnUpdaterChanged;
        });
        Safe(() => _discordPresence.Dispose()); // the profile stops showing Bloxkeep
        Safe(() => _robloxInput?.Dispose());
        Safe(SayOfflineOnExit); // no longer counted as open (not for an update in the background, back within seconds)
        Safe(StopRemote); // Discord says the PC isn't connected
        Safe(() => _procTimer.Dispose());
        Safe(() => _homeSession?.Dispose());
        _homeSession = null;
        Safe(() => SaveBeforeClosing()); // anything the steps above changed
        Safe(() => _backupTimer?.Dispose());
        Safe(() => DropVaultImport());
        Safe(() => CloseWebsites());
        // A downloaded update installs once the app has closed (started again at once when this quit is the background update).
        Safe(() => _updater.ApplyOnExit(_relaunchArgs));
        Safe(Ui.ClearSecretClipboard);
        Safe(() => _launchCts?.Cancel());
        Safe(() => _rejoinCts?.Cancel());
        // The Discord status message says Bloxkeep closed (if Discord answers quickly), unless this quit is a background update.
        Safe(() =>
        {
            try
            {
                if (_relaunchArgs is null)
                    FinalDiscordBoard(closing: true)?.Wait(TimeSpan.FromSeconds(2));
            }
            catch (AggregateException)
            {
                // Best effort.
            }
        });
        Safe(() => _vault?.Dispose());
        _vault = null;
    }

    object? SetLanguage(string language)
    {
        L.Set(language);
        PushState();
        return null;
    }

    object? SetTheme(string theme)
    {
        Prefs.Theme = Themes.Normalize(theme);
        Prefs.Save();
        AppearanceChanged?.Invoke();
        PushState();
        return null;
    }

    object? ControlWindow(string action)
    {
        if (action is not ("close" or "minimize" or "zoom"))
            throw new UserError(L.T("Unknown window action."));
        WindowControl?.Invoke(action);
        return null;
    }

    object? SetHotkeys(bool enabled)
    {
        Prefs.GlobalHotkeys = enabled;
        Prefs.Save();
        HotkeysChanged?.Invoke();
        PushState();
        return null;
    }

    object? SetCloseToTray(bool enabled)
    {
        Prefs.CloseToTray = enabled;
        Prefs.Save();
        AppearanceChanged?.Invoke();
        PushState();
        return null;
    }
}
