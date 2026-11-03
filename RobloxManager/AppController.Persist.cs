namespace RobloxManager;

/// <summary>Saving the vault: soon after a change, in the background, again after a failed save, and before closing.</summary>
public sealed partial class AppController
{
    DateTime? _saveDue;
    bool _saveFailed;
    /// <summary>Saves in a row that failed: the next try waits longer (see Persist).</summary>
    int _saveFailures;
    static readonly TimeSpan MaxSaveRetry = TimeSpan.FromMinutes(5);

    /// <summary>Saves a few seconds later, so a burst of changes writes the encrypted vault once; on lock and exit too.</summary>
    void PersistSoon() => _saveDue ??= DateTime.UtcNow + SaveDelay;

    void PersistIfDue(DateTime now)
    {
        if (_saveDue is { } due && now >= due)
            _ = PersistInBackgroundAsync();
    }

    /// <summary>The usual save: encrypted, written and flushed on a worker (<see cref="Vault.SaveInBackground"/>), which took the
    /// window 20-70 ms with a long play history. A save that must be on disk before going on (lock, exit, a deleted
    /// account) is <see cref="Persist"/>.</summary>
    async Task PersistInBackgroundAsync()
    {
        _saveDue = null;
        if (_vault is not { } vault)
            return;
        NoteOpenSessions(); // as Persist does: after a crash the next unlock still finds the clients open now
        try
        {
            await vault.SaveInBackground();
            if (_vault != vault)
                return; // locked meanwhile: closing finished the write
            _saveFailed = false;
            _saveFailures = 0;
            OnVaultSaved();
        }
        catch (Exception ex) when (_vault == vault)
        {
            if (ex is not (IOException or UnauthorizedAccessException))
                Log.Write(ex);
            SaveFailed(ex);
        }
        catch (Exception ex) when (ex is not (IOException or UnauthorizedAccessException or ObjectDisposedException))
        {
            Log.Write(ex);
        }
    }

    /// <returns>false when the vault couldn't be written (it is tried again shortly).</returns>
    bool Persist()
    {
        _saveDue = null;
        if (_vault is null)
            return true;
        // Every save holds the clients open now, so after a crash the next unlock still finds them (auto-rejoin, scheduled stop).
        NoteOpenSessions();
        try
        {
            _vault.Save();
            _saveFailed = false;
            _saveFailures = 0;
            OnVaultSaved();
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            SaveFailed(ex);
            return false;
        }
    }

    /// <summary>Retried shortly, then less and less often up to every 5 minutes (an antivirus usually lets go within seconds; a
    /// sync client or read-only folder may not). Said once.</summary>
    void SaveFailed(Exception ex)
    {
        _saveDue = DateTime.UtcNow + TimeSpan.FromTicks(Math.Min(SaveDelay.Ticks << Math.Min(_saveFailures, 7), MaxSaveRetry.Ticks));
        _saveFailures++;
        if (!_saveFailed)
            Toast(L.F("Couldn't save data: {0}", ErrorText(ex)), "error");
        _saveFailed = true;
    }

    /// <summary>Before the vault goes away (lock, exit, an update's restart) or the PC sleeps: writes what waits and which clients
    /// are open. Tried a few times, since an antivirus usually lets go within a second; false when it still couldn't be written,
    /// so the caller can refuse rather than lose changes.</summary>
    public bool SaveBeforeClosing()
    {
        // A save still being written in the background is written again now, so a failure is known here.
        bool due = NoteOpenSessions() | NotePlaySessions() | NoteScheduledRuns() | _saveDue is not null | _vault?.SavingInBackground == true;
        bool saved = !due;
        for (int attempt = 1; !saved && attempt <= 3; attempt++)
        {
            if (attempt > 1)
                Thread.Sleep(500);
            saved = Persist();
        }
        BackupIfDirty();
        return saved;
    }
}
