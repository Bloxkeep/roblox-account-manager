using System.ComponentModel;
using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace RobloxManager;

/// <summary>
/// Vault backups (see <see cref="Backup"/>): a copy after a save that changed data, at most every ten minutes, and
/// on lock and exit when something changed since the last copy (waited for a few seconds at most, else made at the
/// next start). Bookkeeping alone (play time, recent games) is copied too, at most every twelve hours. The settings
/// live in <see cref="Prefs"/> so a restore is possible before unlocking.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan BackupInterval = TimeSpan.FromMinutes(10);
    static readonly TimeSpan BookkeepingBackupInterval = TimeSpan.FromHours(12);
    System.Windows.Forms.Timer? _backupTimer;
    /// <summary>The vault's data changed since the last backup.</summary>
    bool _backupDirty;
    /// <summary>Saves since unlocking, and how many of them the last backup holds: when they differ, bookkeeping changed.</summary>
    int _vaultSaves, _backedUpSaves;
    /// <summary>
    /// What the vault on disk and the last backup hold, apart from bookkeeping (see <see cref="ContentHash"/>): a
    /// save that only noted an open client or a last-used time doesn't make a new backup due.
    /// </summary>
    string? _savedHash, _backupHash;
    /// <summary>The copy in progress on a worker thread, so two never run at once.</summary>
    Task<bool>? _backupTask;
    /// <summary>Its write alone (no bookkeeping, which needs the UI thread): what the copy on lock and exit waits for.</summary>
    Task? _backupWrite;
    /// <summary>The most the copy on lock, exit or sleep holds the window up; a slower one is made at the next start.</summary>
    static readonly TimeSpan BackupClosingWait = TimeSpan.FromSeconds(5);
    /// <summary>Judged when the password was typed; the password itself is never kept.</summary>
    bool _weakPassword;
    string? _backupError;
    /// <summary>
    /// Synced folders found on this PC, looked for when the page loads and when backup settings change rather than
    /// on every state (that lists the drives each time).
    /// </summary>
    List<Backup.Folder>? _suggestedFolders;

    bool BackupReady => Prefs.BackupEnabled && Prefs.BackupFolder is not null;

    object BackupDto() => new
    {
        enabled = Prefs.BackupEnabled,
        folder = Prefs.BackupFolder,
        lastBackup = Prefs.LastBackup,
        error = _backupError,
        suggested = (_suggestedFolders ??= Backup.Suggestions()).Where(s => !SamePath(s.Path, Prefs.BackupFolder)).ToArray(),
        cloud = Prefs.BackupFolder is { } folder && Backup.LooksLikeCloud(folder),
        weakPassword = _vault is not null && _weakPassword,
    };

    /// <summary>The page opened Settings: look for synced folders again (a drive may have been connected since).</summary>
    object? RefreshBackupSuggestions()
    {
        _suggestedFolders = Backup.Suggestions();
        PushStateIfChanged();
        return null;
    }

    static bool SamePath(string a, string? b) =>
        b is not null && string.Equals(Path.TrimEndingDirectorySeparator(Path.GetFullPath(a)), Path.TrimEndingDirectorySeparator(Path.GetFullPath(b)), StringComparison.OrdinalIgnoreCase);

    /// <summary>
    /// What isn't hashed (<see cref="ContentHash"/>), by the class it belongs to: open clients; play time and the
    /// activity log (a stay begins or ends every few minutes: copied with the twelve-hourly bookkeeping); last-used and
    /// last-checked times; recent games and players; when presets and summaries last ran; what the dock showed; the
    /// list's look.
    /// </summary>
    static readonly Dictionary<Type, string[]> Bookkeeping = new()
    {
        [typeof(VaultData)] = ["OpenSessions", "PlaySessions", "Activity", "ScheduledRuns"],
        [typeof(DiscordSettings)] = ["LastDaily", "LastWeekly", "BoardMessageId"],
        [typeof(AppSettings)] = ["LastPlaceInput", "LastJobId", "Layout", "Sort", "RecentGames", "RecentPlayers"],
        [typeof(LaunchPreset)] = ["LastRunAt"],
        [typeof(PresetSchedule)] = ["LastRun"],
        [typeof(Account)] = ["LastUsed", "LastChecked"],
    };

    /// <summary>The vault's own JSON, with the bookkeeping left out as it is written (no copy of the whole vault in memory).</summary>
    static readonly JsonSerializerOptions HashJson = new()
    {
        TypeInfoResolver = new System.Text.Json.Serialization.Metadata.DefaultJsonTypeInfoResolver
        {
            Modifiers =
            {
                info =>
                {
                    if (Bookkeeping.TryGetValue(info.Type, out var skip))
                    {
                        foreach (var property in info.Properties)
                        {
                            if (skip.Contains(property.Name))
                                property.ShouldSerialize = static (_, _) => false;
                        }
                    }
                },
            },
        },
    };

    /// <summary>
    /// The vault's contents without bookkeeping (see <see cref="Bookkeeping"/>): the parts whose change makes a backup
    /// due within minutes. Hashed as it is serialised, so it costs a fraction of a save, whatever the play history.
    /// </summary>
    static string ContentHash(VaultData data)
    {
        using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        using (var stream = new HashingStream(hash))
            JsonSerializer.Serialize(stream, data, HashJson);
        return Convert.ToHexString(hash.GetHashAndReset());
    }

    /// <summary>A stream that only feeds what is written to it into a hash.</summary>
    sealed class HashingStream(IncrementalHash hash) : Stream
    {
        public override bool CanRead => false;
        public override bool CanSeek => false;
        public override bool CanWrite => true;
        public override long Length => throw new NotSupportedException();
        public override long Position { get => throw new NotSupportedException(); set => throw new NotSupportedException(); }
        public override void Write(byte[] buffer, int offset, int count) => hash.AppendData(buffer, offset, count);
        public override void Write(ReadOnlySpan<byte> buffer) => hash.AppendData(buffer);
        public override void Flush() { }
        public override int Read(byte[] buffer, int offset, int count) => throw new NotSupportedException();
        public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
        public override void SetLength(long value) => throw new NotSupportedException();
    }

    /// <summary>
    /// What the vault holds now apart from bookkeeping, hashed only when a backup needs it (none set up: no hash at all).
    /// </summary>
    string? SavedHash() => _savedHash ??= _vault is { } vault ? ContentHash(vault.Data) : null;

    /// <summary>After each successful save: a backup is due when something other than bookkeeping changed, else later.</summary>
    void OnVaultSaved()
    {
        _vaultSaves++;
        _savedHash = null;
        // Without backups nothing reads the hash: a save doesn't pay for it. Turning them on copies at once anyway.
        if (!BackupReady)
        {
            _backupDirty = true;
            return;
        }
        _backupDirty = SavedHash() != _backupHash;
        // Noted outside the vault, so a change the app couldn't copy before it closed (killed, or the copy on exit took
        // too long) is copied at the next unlock.
        if (_backupDirty && BackupReady && !Prefs.BackupPending)
        {
            Prefs.BackupPending = true;
            Prefs.Save();
        }
        BackupWhenDue();
    }

    /// <summary>
    /// Backs up when the last copy is old enough, otherwise when it is; never on the spot, a moment later. A change
    /// of data waits for the ten-minute interval, bookkeeping alone for the twelve-hour one.
    /// </summary>
    void BackupWhenDue()
    {
        if (!BackupReady)
            return;
        var interval = _backupDirty ? BackupInterval : _vaultSaves != _backedUpSaves ? BookkeepingBackupInterval : (TimeSpan?)null;
        if (interval is not { } every)
            return;
        var due = (Prefs.LastBackup ?? DateTime.MinValue) + every;
        // Never further off than the interval: a last-backup time in the future (the clock was set back) must not
        // hold the next copy off for days.
        ScheduleBackup(TimeSpan.FromTicks(Math.Clamp((due - DateTime.UtcNow).Ticks, 0, every.Ticks)));
    }

    /// <summary>
    /// A change the last run of the app didn't get to copy (see <see cref="Prefs.BackupPending"/>), or no backup yet,
    /// is copied right after unlocking. A vault merely saved after the last backup holds bookkeeping only (data
    /// changes are copied on lock and exit), so it waits for the twelve-hour interval.
    /// </summary>
    void OnVaultUnlocked()
    {
        _savedHash = null;
        _backupDirty = Prefs.LastBackup is null || Prefs.BackupPending;
        _backedUpSaves = 0;
        _vaultSaves = Prefs.LastBackup is { } last && File.GetLastWriteTimeUtc(Vault.VaultPath) > last ? 1 : 0;
        if (!_backupDirty && BackupReady)
            _backupHash = SavedHash();
        BackupWhenDue();
    }

    /// <summary>Old backups still open only with the old password, so a fresh one goes out at once.</summary>
    void OnPasswordChanged(string password)
    {
        _weakPassword = Backup.WeakPassword(password);
        _backupHash = null;
        _backupDirty = true;
        if (BackupReady)
            _ = TryBackupAsync(push: true);
    }

    /// <summary>On lock and exit: whatever changed since the last copy goes out now, before the vault goes away.</summary>
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
                if (_vault is not null && (_backupDirty || _vaultSaves != _backedUpSaves) && BackupReady)
                    _ = TryBackupAsync(push: true);
            };
        }
        _backupTimer.Stop();
        _backupTimer.Interval = (int)Math.Clamp(delay.TotalMilliseconds, 1000, int.MaxValue);
        _backupTimer.Start();
    }

    /// <summary>The vault's bytes, read on the UI thread so no save can run meanwhile; null (and noted) when unreadable.</summary>
    byte[]? ReadVaultForBackup()
    {
        try
        {
            return File.ReadAllBytes(Vault.VaultPath);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            BackupFailed(ex);
            return null;
        }
    }

    /// <param name="hash">What the vault held when its bytes were read.</param>
    /// <param name="saves">How many saves since unlocking those bytes held.</param>
    void BackupDone(string? hash, int saves)
    {
        Prefs.LastBackup = DateTime.UtcNow;
        _backupHash = hash;
        _backedUpSaves = saves;
        _backupDirty = SavedHash() != hash;
        Prefs.BackupPending = _backupDirty;
        Prefs.Save();
        _backupError = null;
    }

    /// <summary>The reason is shown under Settings › Backup and the copy tried again later.</summary>
    void BackupFailed(Exception ex)
    {
        _backupError = ErrorText(ex); // the details go to the log
        ScheduleBackup(BackupInterval);
    }

    /// <summary>
    /// The copy made for lock, exit and sleep, where it should be done before going on. The folder may be on a slow or
    /// synced drive, so the window waits for it a few seconds at most (<see cref="BackupClosingWait"/>); a copy that
    /// takes longer is left to finish on its own, and <see cref="Prefs.BackupPending"/> has the next unlock make it.
    /// </summary>
    bool TryBackup(bool push)
    {
        _backupTimer?.Stop();
        var clock = Stopwatch.StartNew();
        TimeSpan Left() => clock.Elapsed < BackupClosingWait ? BackupClosingWait - clock.Elapsed : TimeSpan.Zero;
        // A copy still being written on its worker thread would use the same temp file: it is given what is left of
        // the wait. Only its write is waited for; the bookkeeping needs this thread and runs later, if the app is
        // still there.
        if (_backupWrite is { IsCompleted: false } writing)
        {
            try
            {
                if (!writing.Wait(Left()))
                    return false;
            }
            catch (AggregateException)
            {
                // Its own continuation reports the failure.
            }
        }
        bool ok = false;
        if (ReadVaultForBackup() is { } bytes)
        {
            string? hash = SavedHash();
            int saves = _vaultSaves;
            string folder = Prefs.BackupFolder!;
            string[] previousIds = _vault?.Data.Settings.PreviousVaultIds.ToArray() ?? [];
            var write = Task.Run(() => Backup.Write(folder, bytes, previousIds));
            _backupWrite = write;
            try
            {
                if (write.Wait(Left()))
                {
                    BackupDone(hash, saves);
                    ok = true;
                }
                else
                {
                    Log.Note("Backup on closing took too long; it is made at the next unlock.");
                }
            }
            catch (AggregateException ex) when (ex.InnerException is not null)
            {
                BackupFailed(ex.InnerException);
            }
        }
        if (push)
            PushState();
        return ok;
    }

    /// <summary>
    /// The usual copy: written and pruned on a worker thread, since the folder is often on a synced or slow drive.
    /// A request that arrives while one runs waits for it, then copies what is on disk by then.
    /// </summary>
    /// <returns>false when the copy failed.</returns>
    async Task<bool> TryBackupAsync(bool push)
    {
        _backupTimer?.Stop();
        while (_backupTask is { IsCompleted: false } running)
            await running;
        bool ok = await (_backupTask = BackupOnWorkerAsync());
        if (ok)
            BackupWhenDue(); // the vault changed again while the copy was being written (a failure retries later by itself)
        if (push)
            PushState();
        return ok;
    }

    async Task<bool> BackupOnWorkerAsync()
    {
        // A copy the lock or exit stopped waiting for may still be writing the same temp file.
        if (_backupWrite is { IsCompleted: false } lingering)
        {
            try
            {
                await lingering;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                // That copy failed; this one is made anyway.
            }
        }
        // A change still waiting to be saved (or being written in the background) goes into the file first, so the
        // copy and its hash hold the same.
        if ((_saveDue is not null || _vault?.SavingInBackground == true) && _vault is not null)
            Persist();
        if (ReadVaultForBackup() is not { } bytes)
            return false;
        string? hash = SavedHash();
        int saves = _vaultSaves;
        string folder = Prefs.BackupFolder!;
        string[] previousIds = _vault?.Data.Settings.PreviousVaultIds.ToArray() ?? []; // a copy: the list belongs to the UI thread
        try
        {
            await (_backupWrite = Task.Run(() => Backup.Write(folder, bytes, previousIds)));
            BackupDone(hash, saves);
            return true;
        }
        // Any reason, not only a file Windows refused: a folder setting that isn't a usable path would otherwise stop
        // the copies without a word (the reason shows in Settings › Backup and goes to the log).
        catch (Exception ex)
        {
            BackupFailed(ex);
            return false;
        }
    }

    async Task<object?> BackupNowAsync()
    {
        RequireVault();
        if (Prefs.BackupFolder is null)
            throw new UserError(L.T("Choose a backup folder first."));
        if (!await TryBackupAsync(push: true))
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
            _ = TryBackupAsync(push: true);
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
        _backupHash = null; // a new folder has no copy yet
        _backupDirty = true;
        _suggestedFolders = null;
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
        if (Ui.Modal(() => dialog.ShowDialog(_owner)) == DialogResult.OK)
        {
            UseBackupFolder(dialog.SelectedPath);
            Prefs.Save();
            if (_vault is not null && BackupReady)
                _ = TryBackupAsync(push: true);
        }
        PushState();
        return null;
    }

    static object? OpenBackupFolder()
    {
        if (Prefs.BackupFolder is not { } folder)
            return null;
        // The folder may sit on a drive that isn't there right now (Google Drive not running).
        try
        {
            Directory.CreateDirectory(folder);
            Process.Start(new ProcessStartInfo("explorer.exe") { ArgumentList = { folder } })?.Dispose();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or Win32Exception)
        {
            throw new UserError(L.F("Couldn't open the backup folder: {0}", ex is Win32Exception ? L.T("Windows couldn't open File Explorer.") : ErrorText(ex)));
        }
        return null;
    }

    /// <summary>
    /// Before a vault exists (first-run screen) the backup simply becomes the vault. Otherwise (Settings, or the lock
    /// screen for someone who forgot the master password) an open one is locked and saved first, a copy of the vault
    /// kept beside the new one, and the person unlocks the restored vault with its own master password.
    /// </summary>
    object? RestoreBackup()
    {
        if (_launchCts is not null)
            throw new UserError(L.T("Wait until Bloxkeep finishes opening Roblox."));
        using var dialog = new OpenFileDialog
        {
            Title = L.T("Choose a Bloxkeep backup"),
            Filter = L.T("Bloxkeep backups") + " (*.rmv)|*.rmv",
            InitialDirectory = Prefs.BackupFolder is { } folder && Directory.Exists(folder) ? folder : Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
        };
        if (Ui.Modal(() => dialog.ShowDialog(_owner)) != DialogResult.OK)
        {
            PushState();
            return null;
        }
        bool replace = _vault is not null || Vault.Exists;
        try
        {
            // Checked before locking, so picking a wrong file doesn't cost the person their session. Whether it
            // opens is only known with its password, once they unlock it.
            if (!Vault.LooksLikeVault(dialog.FileName))
                throw new UserError(L.T("This file is not a Bloxkeep backup."));
            if (_vault is not null)
                Lock();
            Backup.Restore(dialog.FileName, replace);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            throw new UserError(L.F("Couldn't restore: {0}", ErrorText(ex)));
        }
        _backupDirty = false; // the vault now equals a backup
        Prefs.BackupPending = false;
        Prefs.Save();
        Toast(L.T("Backup restored. Unlock it with the master password it was saved with."), "ok");
        PushState();
        return null;
    }
}
