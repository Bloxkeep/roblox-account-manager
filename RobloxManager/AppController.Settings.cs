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
            if (Has(rc, "fps", JsonValueKind.Number)) c.Fps = Long(rc, "fps") is 30 or 60 or 144 or 240 ? (int)Long(rc, "fps") : 0;
            if (Has(rc, "volume", JsonValueKind.Number)) c.Volume = (int)Math.Clamp(Long(rc, "volume"), -1, 100);
        }
        if (OptBool(a, "trimBackground") is { } trim)
        {
            s.TrimBackground = trim;
            _trimRefusedShown = false;
        }
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
        GiveCoresBack();
        LiftRamLimits();
        if (_robloxFlagsPending)
            RobloxFlags.Restore(); // a launch cut short: the next Roblox opened by hand gets the person's own file
        StopWatchingDrags();
        GiveIconsBack();
        SystemEvents.PowerModeChanged -= OnPowerModeChanged;
        SystemEvents.TimeChanged -= OnTimeChanged;
        NetworkChange.NetworkAvailabilityChanged -= OnNetworkAvailabilityChanged;
        _updater.Changed -= OnUpdaterChanged;
        _discordPresence.Dispose(); // the profile stops showing Bloxkeep
        _robloxInput?.Dispose();
        SayOfflineOnExit(); // no longer counted as open (not for an update in the background, back within seconds)
        StopRemote(); // Discord says the PC isn't connected
        _procTimer.Dispose();
        _homeSession?.Dispose();
        _homeSession = null;
        SaveBeforeClosing(); // normally done already, before the window agreed to close
        _backupTimer?.Dispose();
        DropVaultImport();
        CloseWebsites();
        // A downloaded update installs once the app has closed (started again at once when this quit is the background update).
        _updater.ApplyOnExit(_relaunchArgs);
        Ui.ClearSecretClipboard();
        _launchCts?.Cancel();
        _rejoinCts?.Cancel();
        // The Discord status message says Bloxkeep closed (if Discord answers quickly), unless this quit is a background update.
        try
        {
            if (_relaunchArgs is null)
                FinalDiscordBoard(closing: true)?.Wait(TimeSpan.FromSeconds(2));
        }
        catch (AggregateException)
        {
            // Best effort.
        }
        _vault?.Dispose();
        _vault = null;
    }
}
