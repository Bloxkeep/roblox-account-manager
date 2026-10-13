using System.Diagnostics;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Vault backups (see <see cref="Backup"/>): a copy after a save that changed data, at most every ten minutes, and
/// always on lock and exit when something changed since the last copy. The settings live in <see cref="Prefs"/> so
/// a restore is possible before unlocking.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan BackupInterval = TimeSpan.FromMinutes(10);
    System.Windows.Forms.Timer? _backupTimer;
    /// <summary>The vault changed since the last backup.</summary>
    bool _backupDirty;
    /// <summary>Judged when the password was typed; the password itself is never kept.</summary>
    bool _weakPassword;
    string? _backupError;

    bool BackupReady => Prefs.BackupEnabled && Prefs.BackupFolder is not null;

    object BackupDto() => new
    {
        enabled = Prefs.BackupEnabled,
        folder = Prefs.BackupFolder,
        lastBackup = Prefs.LastBackup,
        error = _backupError,
        suggested = Backup.Suggestions().Where(s => !SamePath(s.Path, Prefs.BackupFolder)).ToArray(),
        cloud = Prefs.BackupFolder is { } folder && Backup.LooksLikeCloud(folder),
        weakPassword = _vault is not null && _weakPassword,
    };

    static bool SamePath(string a, string? b) =>
        b is not null && string.Equals(Path.TrimEndingDirectorySeparator(Path.GetFullPath(a)), Path.TrimEndingDirectorySeparator(Path.GetFullPath(b)), StringComparison.OrdinalIgnoreCase);

    /// <summary>After each successful save: back up now when the last copy is old enough, otherwise when it is.</summary>
    void OnVaultSaved()
    {
        _backupDirty = true;
        if (!BackupReady)
            return;
        var due = (Prefs.LastBackup ?? DateTime.MinValue) + BackupInterval;
        if (DateTime.UtcNow >= due)
            TryBackup(push: false);
        else
            ScheduleBackup(due - DateTime.UtcNow);
    }

    /// <summary>
    /// A vault saved after the last backup (the app was closed by Task Manager, or the folder was just chosen) is
    /// copied right after unlocking.
    /// </summary>
    void OnVaultUnlocked()
    {
        _backupDirty = Prefs.LastBackup is not { } last || File.GetLastWriteTimeUtc(Vault.VaultPath) > last;
        if (_backupDirty)
            OnVaultSaved();
    }

    /// <summary>Old backups still open only with the old password, so a fresh one goes out at once.</summary>
    void OnPasswordChanged(string password)
    {
        _weakPassword = Backup.WeakPassword(password);
        _backupDirty = true;
        if (BackupReady)
            TryBackup(push: true);
    }

    /// <summary>On lock and exit: whatever changed since the last copy goes out now.</summary>
    void BackupIfDirty()
    {
        if (_backupDirty && BackupReady && File.Exists(Vault.VaultPath))
            TryBackup(push: false);
    }

    void ScheduleBackup(TimeSpan delay)
    {
        if (_disposed)
            return;
        if (_backupTimer is null)
        {
            _backupTimer = new System.Windows.Forms.Timer();
            _backupTimer.Tick += (_, _) =>
            {
                _backupTimer.Stop();
                if (_vault is not null && _backupDirty && BackupReady)
                    TryBackup(push: true);
            };
        }
        _backupTimer.Stop();
        _backupTimer.Interval = (int)Math.Clamp(delay.TotalMilliseconds, 1000, int.MaxValue);
        _backupTimer.Start();
    }

    /// <returns>false when the copy failed; the reason is shown under Settings › Backup and tried again later.</returns>
    bool TryBackup(bool push)
    {
        _backupTimer?.Stop();
        bool ok;
        try
        {
            Backup.Write(Prefs.BackupFolder!);
            Prefs.LastBackup = DateTime.UtcNow;
            Prefs.Save();
            _backupDirty = false;
            _backupError = null;
            ok = true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            _backupError = ex.Message;
            Log.Write(ex);
            ScheduleBackup(BackupInterval);
            ok = false;
        }
        if (push)
            PushState();
        return ok;
    }

    object? BackupNow()
    {
        RequireVault();
        if (Prefs.BackupFolder is null)
            throw new UserError(L.T("Choose a backup folder first."));
        _backupDirty = true;
        if (!TryBackup(push: true))
            throw new UserError(L.F("Couldn't back up: {0}", _backupError));
        return null;
    }

    /// <summary>The switch, or a suggested folder; a new folder gets a copy right away.</summary>
    object? SetBackup(JsonElement a)
    {
        if (OptBool(a, "enabled") is { } enabled)
            Prefs.BackupEnabled = enabled;
        if (OptStr(a, "folder") is { Length: > 0 } folder)
            UseBackupFolder(folder);
        Prefs.Save();
        if (_vault is not null && BackupReady && (_backupDirty || Prefs.LastBackup is null))
            TryBackup(push: false);
        PushState();
        return null;
    }

    void UseBackupFolder(string folder)
    {
        if (!Path.IsPathRooted(folder))
            throw new UserError(L.T("Choose a folder on this PC or a synced drive."));
        // The live vault must not sit in the sync folder (the client locks files mid-save), so never the data folder.
        if (Backup.IsDataFolder(folder))
            throw new UserError(L.T("Pick a folder outside Bloxkeep's own data folder."));
        if (SamePath(folder, Prefs.BackupFolder))
            return;
        Prefs.BackupFolder = Path.GetFullPath(folder);
        Prefs.BackupEnabled = true;
        _backupError = null;
        _backupDirty = true;
    }

    /// <summary>"Choose…": Windows' own folder dialog; cancelling leaves things as they are.</summary>
    object? PickBackupFolder()
    {
        using var dialog = new FolderBrowserDialog
        {
            Description = L.T("Choose a folder for backups"),
            UseDescriptionForTitle = true,
            ShowNewFolderButton = true,
            InitialDirectory = Prefs.BackupFolder is { } current && Directory.Exists(current) ? current
                : Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
        };
        if (dialog.ShowDialog(_owner) == DialogResult.OK)
        {
            UseBackupFolder(dialog.SelectedPath);
            Prefs.Save();
            if (_vault is not null && BackupReady)
                TryBackup(push: false);
        }
        PushState();
        return null;
    }

    static object? OpenBackupFolder()
    {
        if (Prefs.BackupFolder is not { } folder)
            return null;
        Directory.CreateDirectory(folder);
        Process.Start(new ProcessStartInfo("explorer.exe") { ArgumentList = { folder } })?.Dispose();
        return null;
    }

    /// <summary>
    /// Before a vault exists (first-run screen) the backup simply becomes the vault. With one open, it is locked and
    /// saved first, a copy of it kept beside the new one, and the person unlocks the restored vault with its own
    /// master password.
    /// </summary>
    object? RestoreBackup()
    {
        if (_vault is null && Vault.Exists)
            throw new UserError(L.T("Unlock Bloxkeep first, then restore from Settings › Backup."));
        if (_launchCts is not null)
            throw new UserError(L.T("Wait until Bloxkeep finishes opening Roblox."));
        using var dialog = new OpenFileDialog
        {
            Title = L.T("Choose a Bloxkeep backup"),
            Filter = L.T("Bloxkeep backups") + " (*.rmv)|*.rmv",
            InitialDirectory = Prefs.BackupFolder is { } folder && Directory.Exists(folder) ? folder : Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
        };
        if (dialog.ShowDialog(_owner) != DialogResult.OK)
        {
            PushState();
            return null;
        }
        bool replace = _vault is not null;
        if (replace)
            Lock();
        try
        {
            Backup.Restore(dialog.FileName, replace);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            throw new UserError(L.F("Couldn't restore: {0}", ex.Message));
        }
        _backupDirty = false; // the vault now equals a backup
        Toast(L.T("Backup restored. Unlock it with the master password it was saved with."), "ok");
        PushState();
        return null;
    }
}
