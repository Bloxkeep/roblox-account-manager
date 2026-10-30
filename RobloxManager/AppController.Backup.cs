using System.ComponentModel;
using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace RobloxManager;

/// <summary>
/// Vault backups (see <see cref="Backup"/>): a copy after a data change, at most every ten minutes, and on lock and exit
/// when something changed (waited for a few seconds at most, else made at the next start). Bookkeeping alone (play
/// time, recent games) is copied at most every twelve hours. Settings live in <see cref="Prefs"/> so a restore works
/// before unlocking.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan BackupInterval = TimeSpan.FromMinutes(10);
    static readonly TimeSpan BookkeepingBackupInterval = TimeSpan.FromHours(12);
    System.Windows.Forms.Timer? _backupTimer;
    bool _backupDirty;
    /// <summary>Saves since unlocking, and how many of them the last backup holds: when they differ, bookkeeping changed.</summary>
    int _vaultSaves, _backedUpSaves;
    /// <summary>The vault on disk and the last backup, hashed without bookkeeping (<see cref="ContentHash"/>).</summary>
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
    /// <summary>Synced folders on this PC; looked for on page load and backup changes only, since it lists the drives.</summary>
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

    /// <summary>What isn't hashed (<see cref="ContentHash"/>), by class. Play time and activity change every few minutes,
    /// so they go with the twelve-hourly bookkeeping copy.</summary>
    static readonly Dictionary<Type, string[]> Bookkeeping = new()
    {
        [typeof(VaultData)] = ["OpenSessions", "EndedSessions", "WaitingRejoins", "PlaySessions", "Activity", "ScheduledRuns"],
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

    /// <summary>The vault's contents minus <see cref="Bookkeeping"/>: what makes a backup due within minutes. Hashed while
    /// serialising, so it costs a fraction of a save.</summary>
    static string ContentHash(VaultData data)
    {
        using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        using (var stream = new HashingStream(hash))
            JsonSerializer.Serialize(stream, data, HashJson);
        return Convert.ToHexString(hash.GetHashAndReset());
    }

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

    /// <summary>Hashed only when a backup needs it (none set up: no hash at all).</summary>
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

    /// <summary>Backs up a moment later once the last copy is old enough: ten minutes for data, twelve hours for bookkeeping.</summary>
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

    /// <summary>A change the last run didn't copy (<see cref="Prefs.BackupPending"/>), or no backup yet, is copied right
    /// after unlocking. Otherwise only bookkeeping can be newer (data is copied on lock and exit): the twelve-hour wait.</summary>
    void OnVaultUnlocked()
    {
        _savedHash = null;
        _backupDirty = Prefs.LastBackup is null || Prefs.BackupPending;
        _backedUpSaves = 0;
        _vaultSaves = Prefs.LastBackup is { } last && File.GetLastWriteTimeUtc(OpenVaultPath) > last ? 1 : 0;
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
        if (_backupDirty && BackupReady && File.Exists(OpenVaultPath))
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

    /// <summary>The open vault's own file (the active one may be another while a vault is being added).</summary>
    string OpenVaultPath => _vault?.FilePath ?? Vault.VaultPath;

    /// <summary>The vault's bytes, read on the UI thread so no save can run meanwhile; null (and noted) when unreadable.</summary>
    byte[]? ReadVaultForBackup()
    {
        try
        {
            return File.ReadAllBytes(OpenVaultPath);
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

    /// <summary>The copy for lock, exit and sleep. The folder may be slow or synced, so it waits <see cref="BackupClosingWait"/>
    /// at most; a slower copy finishes on its own and <see cref="Prefs.BackupPending"/> has the next unlock make it.</summary>
    bool TryBackup(bool push)
    {
        _backupTimer?.Stop();
        var clock = Stopwatch.StartNew();
        TimeSpan Left() => clock.Elapsed < BackupClosingWait ? BackupClosingWait - clock.Elapsed : TimeSpan.Zero;
        // A copy still writing on its worker thread would use the same temp file: it gets what is left of the wait. Only
        // its write is waited for; its bookkeeping needs this thread and runs later, if the app is still there.
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

    /// <summary>The usual copy, written and pruned on a worker thread (the folder is often slow or synced). A request
    /// during one waits for it, then copies what is on disk by then.</summary>
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
        // A change still waiting to be saved (or saving in the background) goes into the file first, so copy and hash match.
        if ((_saveDue is not null || _vault?.SavingInBackground == true) && _vault is not null)
            Persist();
        if (ReadVaultForBackup() is not { } bytes)
            return false;
        string? hash = SavedHash();
        int saves = _vaultSaves;
        string folder = Prefs.BackupFolder!;
        string[] previousIds = _vault?.Data.Settings.PreviousVaultIds.ToArray() ?? []; // a copy: the list belongs to the UI thread
        // Whose copy this is, kept now: a lock that gave up waiting for it may have let another vault open by the time it is
        // written, and its time went on that one's entry (review, 2026-10-30).
        var vault = _vault;
        var entry = Vaults.Current;
        try
        {
            await (_backupWrite = Task.Run(() => Backup.Write(folder, bytes, previousIds)));
            if (_vault == vault)
                BackupDone(hash, saves);
            else if (entry is not null)
            {
                // Only the time: whether it holds the vault's last changes is for that vault's next unlock to find out.
                entry.LastBackup = DateTime.UtcNow;
                Prefs.Save();
            }
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

    /// <summary>Before a vault exists (first-run screen) the backup simply becomes the vault. Otherwise (Settings, or the
    /// lock screen after a forgotten password) the open one is locked and saved first, a copy of it kept beside the new
    /// one; the restored vault unlocks with its own master password.</summary>
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
            // Checked before locking, so a wrong file doesn't cost the session; whether it opens needs its password.
            if (!Vault.LooksLikeVault(dialog.FileName))
                throw new UserError(L.T("This file is not a Bloxkeep backup."));
            if (_vault is not null)
                Lock();
            Backup.Restore(dialog.FileName, replace);
            NoteRestored(dialog.FileName);
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

    /// <summary>A restored backup with an id this PC never had (<see cref="Prefs.KnownVaultIds"/>) was made elsewhere: noted
    /// so its first unlock runs <see cref="DropForeignLinks"/>. A backup of this PC's own vault keeps everything. The caller
    /// saves the prefs.</summary>
    static void NoteRestored(string file) =>
        Prefs.ForeignRestore = Vault.IdOf(file) is { } id && !Prefs.KnownVaultIds.Contains(id) ? id : null;

    /// <summary>At the first unlock of a vault made elsewhere: its program for opening Roblox and its Discord channels were
    /// chosen by whoever made it, so a shared "backup" could run their program at the next Play or send this PC's alerts
    /// about its accounts to their channel. Dropped before anything uses them; the person sets them again if they're theirs.
    /// </summary>
    void DropForeignLinks(Vault vault)
    {
        var data = vault.Data;
        var s = data.Settings;
        bool had = s.CustomLauncher.Length > 0 || data.Discord.WebhookUrl.Length > 0 || s.Presets.Any(p => p.DiscordWebhook.Length > 0);
        s.CustomLauncher = "";
        if (s.LaunchVia == "custom")
            s.LaunchVia = "auto";
        data.Discord.Enabled = false;
        data.Discord.WebhookUrl = "";
        foreach (var preset in s.Presets)
            preset.DiscordWebhook = "";
        PersistSoon();
        if (had)
            Toast(L.T("This backup was made on another PC: the program it opened Roblox with and its Discord links were removed. Set them again in Settings if they're yours."));
    }

    /// <summary>The vault just unlocked (or given a new password) is this PC's from now on, with the ids it had before (its
    /// older backups carry them); not those of a vault from elsewhere, which wrote that list itself.</summary>
    void RememberVault(Vault vault, bool foreign)
    {
        bool changed = Prefs.KnownVaultIds.Add(vault.Id);
        if (foreign)
        {
            Prefs.ForeignRestore = null;
            changed = true;
        }
        else
        {
            foreach (string id in vault.Data.Settings.PreviousVaultIds)
                changed |= Prefs.IsVaultId(id) && Prefs.KnownVaultIds.Add(id);
        }
        if (changed)
            Prefs.Save();
    }
}
