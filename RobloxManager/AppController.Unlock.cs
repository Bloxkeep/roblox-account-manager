using System.Text.Json;

namespace RobloxManager;

/// <summary>Creating, unlocking and locking the vault, and changing its password.</summary>
public sealed partial class AppController
{
    async Task<object?> CreateVaultAsync(string password, string name = "")
    {
        if (password.Length < 6)
            throw new UserError(L.T("The master password needs at least 6 characters."));
        // Six spaces pass the length check but are no password at all.
        if (string.IsNullOrWhiteSpace(password))
            throw new UserError(L.T("The master password can't be only spaces."));
        // A vault being added: the one open locks only now, once the new one's password is fine.
        SwitchToNewVault();
        if (Vault.Exists)
            throw new UserError(L.T("Bloxkeep is already set up."));
        var created = await Task.Run(() => Vault.Create(password));
        // Another create or an unlock finished first (two commands at once): that vault stays the open one, and the
        // file on disk is made its own again (this one may have been written over it).
        if (_vault is not null)
        {
            created.Dispose();
            Persist();
            return BuildState();
        }
        _weakPassword = Backup.WeakPassword(password);
        JoinVaultList(name); // before it opens: its backup state is its own from the start
        OnUnlocked(created);
        return BuildState();
    }

    async Task<object?> UnlockAsync(string password)
    {
        if (_vault is { } open)
            return await UnlockScreenAsync(open, password);
        if (password.Length == 0)
            throw new UserError(L.T("Enter your master password."));
        Vault vault;
        try
        {
            vault = await Task.Run(() => Vault.Open(password));
        }
        catch (Exception ex) when (ex is InvalidPasswordException or IOException or UnauthorizedAccessException or InvalidDataException or JsonException)
        {
            // The vault's own refusals are sentences of the app; a file Windows wouldn't read, or broken JSON, aren't.
            if (ex is JsonException)
                Log.Write(ex);
            throw new UserError(ex switch
            {
                InvalidPasswordException or InvalidDataException => ex.Message,
                JsonException => L.T("The data file is not in the right format or is damaged."),
                _ => L.F("Couldn't open the data file: {0}", ErrorText(ex)),
            });
        }
        // Another unlock finished first (two commands at once): that vault stays the open one, this copy is let go.
        if (_vault is not null)
        {
            vault.Dispose();
            return BuildState();
        }
        _weakPassword = Backup.WeakPassword(password);
        // The one the lock screen offers at the next start.
        if (Vaults.Current is { } opened && Prefs.LastVault != opened.Id)
        {
            Prefs.LastVault = opened.Id;
            Prefs.Save();
        }
        OnUnlocked(vault);
        return BuildState();
    }

    void OnUnlocked(Vault vault)
    {
        _vault = vault;
        _updateAtLock = null; // in the app now: an update waits for a quiet moment
        // A backup made somewhere else: its launcher and Discord links go before anything below can use them.
        bool foreign = Prefs.ForeignRestore == vault.Id;
        if (foreign)
            DropForeignLinks(vault);
        RememberVault(vault, foreign);
        _activeAt = DateTime.UtcNow; // the wait for the lock screen starts now
        var s = vault.Data.Settings;
        if (!RobloxLauncher.IsValidOption(s.LaunchVia))
            s.LaunchVia = s.DirectLaunch ? "direct" : "auto";
        _installs = null; // re-read for this vault's launch option
        if (s.MultiInstance && !MultiInstance.Armed)
            MultiInstance.Enable();
        _lastArmed = MultiInstance.Armed;
        KeepMultiAtStart(s.MultiInstance);
        // Read before AdoptOpenSessions: its save writes these lists from memory, still empty then.
        foreach (var e in vault.Data.EndedSessions)
            if (DateTime.UtcNow - e.At < StalePresence)
                _endedHere.TryAdd(e.AccountId, (e.JobId, e.PlaceId, e.At));
        var waiting = vault.Data.WaitingRejoins.ToList();
        AdoptOpenSessions(vault);
        RestoreWaitingRejoins(vault, waiting);
        if (waiting.Count > 0)
            PersistSoon(); // AdoptOpenSessions may have saved the vault without them

        ResumePlaySessions(vault);
        ResumeSchedules(vault);
        // The loading screen's steps: accounts and open clients are ready now; Roblox status and avatars follow.
        bool any = vault.Data.Accounts.Any(a => a.UserId > 0);
        _bootStatusDone = !any;
        _bootAvatarsDone = !any;
        _boot = new BootProgress("status", 0.3);
        AdvanceBoot(push: false);
        // Right away rather than on the next process check (up to 4 s later while idle). A refresh from before the
        // lock may still be running (it drops its answer): the next check then runs one for this vault.
        if (_presenceBusy)
            _nextPresence = DateTime.UtcNow;
        else
            _ = RefreshPresenceAsync();
        _ = LoadAvatarsAsync(vault.Data.Accounts.Select(a => a.UserId));
        _ = LoadRecentGamesAsync();
        RefreshRobuxIfDue(DateTime.UtcNow); // alongside those, not after them
        OnVaultUnlocked();
        AddLockedActivity(vault); // what happened while locked (a window closed) is in the log at once, also if Bloxkeep quits next
    }

    /// <summary>
    /// Remembers the clients that are open, so closing or restarting Bloxkeep (for example to update) doesn't lose
    /// track of them: they keep running, and the next unlock picks them up again, auto-rejoin included. And the ones
    /// that ended here lately (<see cref="_endedHere"/>), for the next unlock too.
    /// </summary>
    /// <returns>true when the lists changed, i.e. the vault needs saving.</returns>
    bool NoteOpenSessions()
    {
        if (_vault is not { } vault)
            return false;
        var ended = _endedHere.Where(e => DateTime.UtcNow - e.Value.At < StalePresence).OrderBy(e => e.Value.At)
            .Select(e => new EndedSession { AccountId = e.Key, JobId = e.Value.JobId, PlaceId = e.Value.PlaceId, At = e.Value.At }).ToList();
        bool endedChanged = !vault.Data.EndedSessions.Select(e => (e.AccountId, e.At)).SequenceEqual(ended.Select(e => (e.AccountId, e.At)));
        if (endedChanged)
            vault.Data.EndedSessions = ended;
        // The rejoins waiting their turn, for after a restart (an update's) or a lock.
        var waiting = _rejoinQueue.Select(r => new WaitingRejoin { AccountId = r.AccountId, Target = r.Target, Due = r.Due, PresetId = r.PresetId, Restart = r.Restart }).ToList();
        if (!vault.Data.WaitingRejoins.Select(w => (w.AccountId, w.Due)).SequenceEqual(waiting.Select(w => (w.AccountId, w.Due))))
        {
            vault.Data.WaitingRejoins = waiting;
            endedChanged = true;
        }
        var sessions = _running
            .Select(r => new OpenSession { AccountId = r.Key, Pid = r.Value.Pid, StartedAt = r.Value.Launch.StartedAt, Target = r.Value.Launch.Target, PresetId = r.Value.Launch.PresetId })
            .ToList();
        var noted = vault.Data.OpenSessions;
        if (noted.Count == sessions.Count && noted.Zip(sessions).All(p => p.First.AccountId == p.Second.AccountId && p.First.Pid == p.Second.Pid))
            return endedChanged;
        vault.Data.OpenSessions = sessions;
        return true;
    }

    /// <summary>
    /// Rejoins that were waiting their turn when Bloxkeep restarted (an update) or locked: on, at their time (one due
    /// meanwhile shortly), unless the account is gone or open again. Not one from long ago: Bloxkeep opened the next
    /// day must not start opening Roblox by itself.
    /// </summary>
    void RestoreWaitingRejoins(Vault vault, List<WaitingRejoin> waiting)
    {
        foreach (var w in waiting)
            if (w.Target is { } target && DateTime.UtcNow - w.Due < WaitingRejoinKept && vault.Data.Accounts.Any(a => a.Id == w.AccountId) && !_running.ContainsKey(w.AccountId)
                && _rejoinQueue.All(r => r.AccountId != w.AccountId))
                _rejoinQueue.Add(new RejoinRequest(w.AccountId, target, w.Due > DateTime.UtcNow ? w.Due : DateTime.UtcNow + RejoinDelay, PresetId: w.PresetId, Restart: w.Restart));
    }

    /// <summary>Leeway between a launch's time and its client's start, for picking saved clients up again.</summary>
    static readonly TimeSpan AdoptSlack = TimeSpan.FromMinutes(2);

    void AdoptOpenSessions(Vault vault)
    {
        var saved = vault.Data.OpenSessions;
        if (saved.Count == 0)
            return;
        var started = RobloxProcesses.GetPlayers().ToDictionary(p => p.Pid, p => p.Created);
        // After a restart of the PC the same process ids belong to other programs (also after a shutdown with Windows'
        // Fast Startup, which doesn't restart the uptime): the client must have started around its launch.
        var booted = DateTime.UtcNow - TimeSpan.FromMilliseconds(Environment.TickCount64);
        bool SameClient(OpenSession session) =>
            started.TryGetValue(session.Pid, out long created) && session.StartedAt >= booted
            && (created == 0 || DateTime.FromFileTimeUtc(created) is var at
                && at >= session.StartedAt - AdoptSlack && at <= session.StartedAt + PendingTimeout + LateTimeout + AdoptSlack);
        foreach (var session in saved)
        {
            if (SameClient(session) && vault.Data.Accounts.Any(a => a.Id == session.AccountId)
                && !_running.ContainsKey(session.AccountId) && !_running.Values.Any(r => r.Pid == session.Pid))
            {
                _running[session.AccountId] = (session.Pid, new PendingLaunch(session.AccountId, [], session.StartedAt, session.Target, PresetId: session.PresetId));
                // A light-AFK window was minimised once already, before Bloxkeep closed: one the person brought back up stays up.
                _afkMinimised.Add(session.Pid);
            }
        }
        saved.Clear();
        Persist();
    }

    object? Lock()
    {
        // Edits that can't be written yet (the file held by an antivirus) would be lost with the vault.
        if (!SaveBeforeClosing())
            throw new UserError(L.T("Couldn't save your changes, so Bloxkeep stays unlocked. Try again in a moment."));
        _launchCts?.Cancel();
        _rejoinCts?.Cancel();
        QuickLoginCancel();
        LoginCancel();
        DropVaultImport(); // another vault's accounts, opened for an import, go with this one
        CloseWebsites(); // signed in as saved accounts
        Ui.ClearSecretClipboard();
        // A second lock (already locked) would start the missed-schedule bookkeeping over and lose what was noted.
        if (_vault is not null)
            RememberSchedulesForLock();
        _ = FinalDiscordBoard(closing: false);
        _vault?.Dispose();
        _vault = null;
        // From the lock screen after a while without use too: closed for real now (not before: a lock refused above
        // leaves that screen up).
        _screenLocked = false;
        // Nothing of the vault stays reachable while locked: Dispose only wipes the key, so the decrypted accounts,
        // cookies and passwords go only once no field holds the vault (or what was made from it) any more.
        _discordVault = null;
        _friendIds.Clear();
        _friendServers.Clear();
        _privateServers.Clear(); // their access codes are the vault's accounts'
        _favoriteGames = _popularGames = null;
        _players.Clear();
        _avatarUrls.Clear();
        _endedHere.Clear();
        _presence.Clear();
        _presenceReliable.Clear();
        _presenceLoaded = false;
        _boot = null;
        _rejoinQueue.Clear();
        _offlineStrikes.Clear();
        ResetDiscordQueue(); // nothing waits to go to Discord while locked
        _lastMetrics = _lastState = _pageView = null;
        // The monitor tab's pictures would show every client as a nameless "Roblox" over the lock screen; the mini
        // window's "Close all" counts clients from a check that no longer runs.
        SetMonitorArea?.Invoke(null);
        _clientRam.Clear();
        PushState();
        return null;
    }

    async Task<object?> ChangePasswordAsync(string current, string next)
    {
        var vault = RequireVault();
        if (next.Length < 6)
            throw new UserError(L.T("The new password needs at least 6 characters."));
        if (string.IsNullOrWhiteSpace(next))
            throw new UserError(L.T("The master password can't be only spaces."));
        if (!await Task.Run(() => vault.VerifyPassword(current)))
            throw new UserError(L.T("The current master password is incorrect."));
        if (next == current)
            throw new UserError(L.T("The new password is the same as the current one."));
        var prepared = await Task.Run(() => Vault.PrepareKey(next));
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));
        // Backups made so far carry the old id (it comes from the salt, which changes with the password): remembered
        // in the vault, so they stay this vault's to prune.
        var previous = vault.Data.Settings.PreviousVaultIds;
        if (!previous.Contains(vault.Id))
            previous.Add(vault.Id);
        // Written here on the UI thread, where the accounts are changed, so nothing changes mid-save.
        try
        {
            vault.ChangePassword(prepared);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            throw new UserError(L.F("Couldn't save data: {0}", ErrorText(ex)));
        }
        OnPasswordChanged(next);
        RememberVault(vault, foreign: false); // its new id: backups from now on carry it
        return true;
    }
}
