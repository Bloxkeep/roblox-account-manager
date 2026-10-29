using Microsoft.Win32;

namespace RobloxManager;

/// <summary>The state the page is drawn from: the app's whole state, each account's and game's part of it, and what is installed.</summary>
public sealed partial class AppController
{
    /// <summary>The page's first request; says once whether it replaces an earlier page of this session.</summary>
    object InitialState()
    {
        bool resume = ResumingPage;
        ResumingPage = false;
        PageAttached = true;
        ForgetSleep(); // a new page is awake and gets everything here
        PageBusy = false; // a fresh page has nothing open yet
        _lastMetrics = _lastState = null; // a new page has nothing yet; send both again
        if (_vault is not null && _boot is null)
            _ = SettledAsync(); // a page made again (after the browser was closed in the background) gets ready too
        string? view = resume && Open is not null ? _pageView : null;
        _pageView = null;
        return new { resume, view, state = BuildState() };
    }

    /// <summary>What the page had on screen (selection, filter, search, scroll), kept in memory while the browser is closed in
    /// the background so a recreated page looks the same. On disk only inside an update's encrypted hand-over (see <see cref="Relaunch"/>).</summary>
    /// <param name="restart">Sent right before the page asks for a browser restart (GPU switch), while the window is in front.</param>
    object? KeepView(string view, bool restart)
    {
        // One that arrives after the window is already back (a quick minimise and restore) is stale.
        if (!_background && !restart)
            return null;
        _pageView = view.Length <= 256 * 1024 ? view : null;
        return null;
    }

    /// <summary>The window came back with its page still open: what the page kept is out of date from here on.</summary>
    public void DropKeptView()
    {
        // Not before any page has come: a view handed over by an update is for the first one.
        if (PageAttached)
            _pageView = null;
    }

    /// <summary>The page has something closing the browser would lose (a form, a sign-in): the window keeps it alive.</summary>
    public bool PageBusy { get; private set; }

    object? SetPageBusy(bool busy)
    {
        PageBusy = busy;
        return null;
    }

    /// <summary>The page is going away (browser released or recreated): nothing it was doing survives, least of all a
    /// sign-in prompt holding a password or a Quick Login code waiting for approval.</summary>
    public void PageGone()
    {
        PageAttached = false; // the next page asks for everything with getState
        ForgetSleep();
        PageBusy = false;
        SetMonitorArea?.Invoke(null); // the next page says again where its monitor tab is, if it shows it
        LoginCancel();
        QuickLoginCancel();
        DropVaultImport();
    }

    object? OnPageShown()
    {
        PageShown?.Invoke();
        return null;
    }

    object BuildState() => new
    {
        language = L.Language,
        theme = Prefs.Theme,
        themeApplied = Themes.Current.Id,
        customTheme = CustomThemeDto(),
        savedThemes = SavedThemesDto(),
        closeToTray = Prefs.CloseToTray,
        hotkeys = new { on = Prefs.GlobalHotkeys, taken = HotkeysTaken },
        hideNames = Prefs.HideNames,
        notifications = Prefs.Notifications,
        notifyOff = Prefs.NotifyOff.Where(NotifyKinds.Contains).Order().ToArray(),
        dailySignInCheck = Prefs.DailySignInCheck,
        skipConfirm = SkippedConfirms(),
        lastSignInCheck = Prefs.LastSignInCheck, // shown next to the switch, like the update check's time
        miniOpen = Prefs.MiniOpen,
        backup = BackupDto(),
        discord = DiscordDto(),
        safeRendering = Prefs.SafeRendering,
        update = _updater.Dto(),
        // An update found before the unlock, which the lock screen gives its place to; go: its answer (null: not yet).
        lockUpdate = UpdateOffersAtLock ? new { go = _updateAtLock } : null,
        countOnline = Prefs.CountOnline,
        remote = RemoteDto(),
        robloxVersion = RobloxVersionDto(),
        robloxUpdates = RobloxUpdatesDto(),
        whatsNew = _updatedFrom is not null && Open is not null
            ? new { from = _updatedFrom, releases = WhatsNew.Between(_updatedFrom, _updater.CurrentVersion) }
            : null,
        vaultExists = Vault.Exists,
        vaults = VaultsDto(),
        multiAtStart = Prefs.MultiRobloxAtStart,
        locked = Open is null,
        // The lock screen after a while without use: Bloxkeep is open behind it, Roblox and the background carry on.
        screenLock = _screenLocked ? new { minutes = _vault?.Data.Settings.AutoLockMinutes ?? 0 } : null,
        dataPath = Vault.DataDir,
        paths = new { vault = _vault?.FilePath, log = LogPath, app = AppFolder },
        // Not only the roblox-player: links: another Roblox's uninstaller takes them with it (Roblox VNG's, 2026-10-28)
        // while a client, a launcher or the Store edition is still here, and Default opens that one (FallbackLauncher).
        robloxInstalled = RegistryInfo().Protocol || Installs() is { Direct: true } or { Store: true } or { Bloxstrap: true } or { Fishstrap: true }
            || Installs().Others.Count > 0,
        launching = _launchCts is not null,
        checking = _checking,
        boot = Open is null ? null : _boot,
        multi = new
        {
            enabled = Open?.Data.Settings.MultiInstance ?? false,
            armed = MultiInstance.Armed,
            windowed = Open?.Data.Settings.MultiInstanceWindowed ?? true,
            tile = Open?.Data.Settings.MultiInstanceTile ?? false,
            trayAutostart = RegistryInfo().TrayAutostart,
        },
        installs = Installs(),
        monitors = Screen.AllScreens.Select((s, i) => new { index = i, s.Primary, width = s.Bounds.Width, height = s.Bounds.Height }).ToArray(),
        settings = Open is null ? null : SettingsDto(Open.Data.Settings),
        accounts = Open?.Data.Accounts.Select(AccountDto).ToArray() ?? [],
    };

    /// <summary>Which Roblox clients and launchers exist, and the version of the client the chosen option starts.</summary>
    /// <param name="Others">Other Bloxstrap-style launchers found here (their option is "other:&lt;name&gt;").</param>
    sealed record InstallsInfo(string? Handler, bool Bloxstrap, bool Fishstrap, List<string> Others, bool Roblox, bool Direct, bool Store, RobloxInstalls.ClientVersion? Client,
        bool Vng = false);

    /// <param name="UpToDate">null until both versions are known.</param>
    /// <param name="Store">Roblox from the Microsoft Store is chosen: the Store installs its updates, not Roblox itself.</param>
    /// <param name="Updater">The launcher that can update this copy of Roblox now (its name), when there is one.</param>
    sealed record RobloxVersionInfo(string? Installed, string? Latest, bool? UpToDate, bool Direct, bool Store = false, string? Updater = null,
        bool Updating = false);

    /// <summary>Registry facts shown on the page; BuildState runs on every push, so they're re-read at most every 30 s.</summary>
    (bool Protocol, bool TrayAutostart) RegistryInfo()
    {
        if (_registry is { } cached && DateTime.UtcNow - _registryReadAt < RegistryCacheLifetime)
            return cached;
        _registryReadAt = DateTime.UtcNow;
        var info = (RobloxLauncher.IsProtocolRegistered(), MultiInstance.FindTrayAutostart()?.Enabled == true);
        _registry = info;
        return info;
    }

    InstallsInfo Installs()
    {
        if (_installs is not null && DateTime.UtcNow - _installsReadAt < InstallCacheLifetime)
            return _installs;
        var (via, custom) = (_vault?.Data.Settings.LaunchVia ?? "auto", _vault?.Data.Settings.CustomLauncher ?? "");
        var (installs, updater) = ReadInstalls(via, custom);
        return TakeInstalls(installs, updater, DateTime.UtcNow);
    }

    /// <summary>What is installed, from disk and registry: up to a second on a busy PC (2026-10-29, the owner's: 0.9 s in a
    /// process check), so the process tick reads it on a worker (<see cref="TickRobloxUpdates"/>).</summary>
    static (InstallsInfo Installs, string? Updater) ReadInstalls(string via, string custom) => (new InstallsInfo(
            // What Default opens Roblox with: the links' program, or what stands in for it when they are gone or Roblox
            // VNG's (RobloxLauncher.FallbackLauncher), so Settings names the program really used.
            Handler: (RobloxInstalls.UsableHandlerPath() ?? RobloxLauncher.FallbackLauncher()?.Exe) is { } used
                ? Path.GetFileNameWithoutExtension(used) : null,
            Bloxstrap: RobloxInstalls.FindBootstrapper("Bloxstrap") is not null,
            Fishstrap: RobloxInstalls.FindBootstrapper("Fishstrap") is not null,
            Others: RobloxInstalls.FindOtherBootstrappers(),
            Roblox: RobloxInstalls.FindOfficialClient() is not null,
            Direct: (RobloxInstalls.FindAnyClient() ?? RobloxInstalls.FindVngClient()) is not null,
            Store: RobloxInstalls.StoreEditionInstalled(),
            Client: RobloxInstalls.VersionOf(RobloxInstalls.ClientFor(via, custom)),
            Vng: RobloxInstalls.FindVngClient() is not null),
        RobloxInstalls.UpdaterFor(via, custom));

    /// <summary>A read of what is installed kept, and a new Roblox build since the last one said.</summary>
    InstallsInfo TakeInstalls(InstallsInfo installs, string? updater, DateTime readAt)
    {
        _installsReadAt = readAt;
        string? before = _installs?.Client?.Folder, beforeVersion = _installs?.Client?.Version;
        _installs = installs;
        _updaterExe = updater;
        // A new build since the last look, or an older one (a launcher pinned to a version), which isn't an update. The Store
        // edition has no build folder, so switching to or from it says nothing. Going older while the newer build stays installed
        // is Roblox switching channel (RobloxInstalls.HasBuild): nothing to say.
        if (before is { Length: > 0 } && _installs.Client is { Folder.Length: > 0 } now && now.Folder != before
            && (Version.TryParse(now.Version, out var have) && Version.TryParse(beforeVersion, out var had) && have < had) is var older
            && !(older && RobloxInstalls.HasBuild(now.Exe, before)))
        {
            string updated = older ? L.F("Roblox went back to version {0}.", now.Version) : L.F("Roblox updated to {0}.", now.Version);
            Toast(updated, "ok");
            NoteActivity("robloxUpdated", text: updated, tone: "info");
        }
        NoteRobloxBuild(_installs.Client); // Settings › Roblox updates (AppController.RobloxUpdates.cs)
        return _installs;
    }

    /// <summary>The version the chosen launcher starts, next to the newest Roblox serves. An older client updates itself on
    /// its next start (that window takes longer), except when started directly.</summary>
    RobloxVersionInfo RobloxVersionDto()
    {
        var installed = Installs().Client;
        // Roblox can ship a new build under the same version number: the build folder ("version-<hash>") tells them
        // apart. A newer number than Roblox serves (another channel) still counts as up to date.
        bool sameBuild = installed is not null && _latestRoblox is not null && installed.Folder == _latestRoblox.Folder;
        bool newer = installed is not null && _latestRoblox is not null
            && Version.TryParse(installed.Version, out var have) && Version.TryParse(_latestRoblox.Version, out var latest) && have > latest;
        bool foldersKnown = installed?.Folder.StartsWith("version-", StringComparison.OrdinalIgnoreCase) == true
            && _latestRoblox?.Folder.StartsWith("version-", StringComparison.OrdinalIgnoreCase) == true;
        bool? upToDate = installed is null || _latestRoblox is null ? null
            : sameBuild || newer || (!foldersKnown && installed.Version == _latestRoblox.Version);
        // The links on another channel's build (Roblox puts some accounts on test builds), with the newest build
        // installed beside it: that one is what an account on the main channel starts.
        string? shown = installed?.Version;
        if (upToDate == false && RobloxInstalls.HasBuild(installed!.Exe, _latestRoblox!.Folder))
            (shown, upToDate) = (_latestRoblox.Version, true);
        return new RobloxVersionInfo(shown, _latestRoblox?.Version, upToDate, _vault?.Data.Settings.LaunchVia == "direct",
            _vault?.Data.Settings.LaunchVia == "store", _updaterExe is { } updater ? Path.GetFileNameWithoutExtension(updater) : null, _robloxUpdating);
    }

    async Task RefreshLatestRobloxAsync()
    {
        try
        {
            if (await RobloxApi.GetLatestClientVersionAsync() is { } latest && latest != _latestRoblox)
            {
                _latestRoblox = latest;
                PushState();
            }
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            // Shown as "couldn't check"; tried again next time.
        }
    }

    object AccountDto(Account a) => new
    {
        id = a.Id,
        a.UserId,
        a.Username,
        a.DisplayName,
        a.Alias,
        a.Label,
        a.Group,
        a.Note,
        hasPassword = a.Password.Length > 0,
        hasAuthenticator = a.Authenticator is not null,
        banned = a.Banned == true,
        a.BannedSince,
        realm = a.Realm,
        a.CookieValid,
        a.LastUsed,
        a.AutoRejoin,
        a.UseRobloxConfig,
        avatarUrl = _avatarUrls.GetValueOrDefault(a.UserId),
        robux = RobuxOf(a),
        robuxWait = RobuxWaiting(a),
        state = _running.ContainsKey(a.Id) ? "running"
            : _pending.Any(p => p.AccountId == a.Id && !_closeWhenClaimed.Contains(p)) ? "launching" : "idle",
        presence = PresenceDto(a),
        session = _running.TryGetValue(a.Id, out var run)
            ? new
            {
                pid = run.Pid,
                since = run.Launch.StartedAt,
                game = SessionGameDto(run.Pid, run.Launch),
                connection = Connection(a, run.Pid, run.Launch),
                phase = Phase(run.Pid),
                queuePosition = Phase(run.Pid) == "full" ? JoinAnswerOf(run.Pid)?.QueuePosition : null,
                reason = _logs.StateOf(run.Pid) is { Status: "disconnected" } kicked ? kicked.Reason : null,
                // The game's words with a kick or ban, and whether it was a ban (both come with Roblox's code 267), or
                // the window is stuck on Roblox's Join Error 600 for a game that banned the account (its log says nothing).
                kickMessage = _logs.StateOf(run.Pid) is { Status: "disconnected" } said ? said.KickMessage
                    : BannedWindow(a, run) is { } stuck ? stuck.Message : null,
                bannedByGame = _logs.StateOf(run.Pid) is { Status: "disconnected", BannedByGame: true } || BannedWindow(a, run) is not null,
                jobId = _logs.StateOf(run.Pid)?.JobId,
                jobPlaceId = _logs.StateOf(run.Pid)?.PlaceId,
                hidden = RobloxProcesses.HasHidden(run.Pid),
                // The preset that opened it may turn auto-rejoin on or off for this client.
                autoRejoin = RejoinOn(a, run.Launch.PresetId),
            }
            : null,
        // A rejoin waiting its turn (auto-rejoin's wait, or a preset's restart): the card counts down and can call it off.
        rejoin = _rejoinQueue.FirstOrDefault(r => r.AccountId == a.Id) is { } queued
            ? new { at = queued.Due, queued.Restart, game = GameDto(queued.Target.PlaceId) }
            : null,
    };

    /// <summary>What a client here is doing: "ingame", "joining" (just started or not known), "menu" (no game, or left it),
    /// "disconnected" (kicked or lost connection; the window stays on Roblox's error screen). Its log says so at once;
    /// without one, Roblox's status for the account is used.</summary>
    string Connection(Account a, int pid, PendingLaunch launch)
    {
        if (_logs.StateOf(pid) is { } log)
            return log.Status switch { "ingame" => "ingame", "joining" => "joining", "left" => "menu", "signedout" => "signedout", _ => "disconnected" };
        bool known = _presence.TryGetValue(a.UserId, out var p);
        if (known && p!.Type == 2)
            return "ingame";
        // Online but not in a game: in Roblox's menu (no log line said otherwise), not cut off.
        if (known && p!.Type == 1 && _presenceReliable.Contains(a.UserId))
            return "menu";
        if (!known || !_presenceReliable.Contains(a.UserId) || DateTime.UtcNow - launch.StartedAt < DisconnectGrace)
            return "joining";
        return launch.Target is null ? "menu" : "disconnected";
    }

    /// <summary>Roblox's answer about a window that hasn't got in (<see cref="CheckJoinAsync"/>), once it came.</summary>
    RobloxApi.JoinAnswer? JoinAnswerOf(int pid) =>
        !_joinChecks.TryGetValue(pid, out var asked) ? null
        : asked.Answer is { IsCompletedSuccessfully: true, Result: { } answer } ? answer
        // Asked again while in a full game's line: the place it had, until the new answer comes.
        : !asked.Answer.IsCompleted ? _inLine.GetValueOrDefault(pid) : null;

    /// <summary>How long a teleport is shown as one: Roblox's took 0.5-3.6 s here (2026-10-21); one that failed leaves the window where it was.</summary>
    static readonly TimeSpan TeleportShown = TimeSpan.FromMinutes(1);

    /// <summary>What a client's log says beyond <see cref="Connection"/>: "teleporting"; "waiting" (no server yet: normally a
    /// second, longer in a full server's line or on a join error, which the log doesn't tell apart); "full" (Roblox's join
    /// answer says so); "joinFailed" (left before any server took it); or null.</summary>
    string? Phase(int pid) => _logs.StateOf(pid) switch
    {
        { Status: "ingame" or "joining", TeleportStarted: { } t } when DateTime.UtcNow - t < TeleportShown => "teleporting",
        { Status: "joining", JobId: null } when _logs.JoinChallengedAt(pid) is not null => "challenged",
        { Status: "joining", JobId: null } when JoinAnswerOf(pid) is { Full: true } => "full",
        { Status: "joining", JobId: null } => "waiting",
        { Status: "left", NeverJoined: true } => "joinFailed",
        _ => null,
    };

    async Task NameLoggedPlacesAsync(List<long> placeIds)
    {
        await LoadGamesAsync(placeIds.Select(id => (id, 0L)).ToList());
        // Nothing came back (offline, rate-limited): the page would show the same thing.
        if (placeIds.Any(id => _games.GetValueOrDefault(id) is not null))
            PushStateIfChanged();
    }

    /// <summary>The game a client here is in: the place its log reports once that place is named, else the game it was opened
    /// with (games often move players to another of their places right after they join).</summary>
    object? SessionGameDto(int pid, PendingLaunch launch)
    {
        long? logged = _logs.StateOf(pid)?.PlaceId, opened = launch.Target?.PlaceId;
        bool loggedNamed = logged is long l && _games.GetValueOrDefault(l)?.Name is { Length: > 0 };
        return GameDto(loggedNamed || opened is null ? logged ?? opened : opened);
    }

    object? GameDto(long? placeId) => placeId is long id
        ? new { placeId = id, name = _games.GetValueOrDefault(id)?.Name ?? "", iconUrl = _games.GetValueOrDefault(id)?.IconUrl }
        : null;

    /// <summary>Clients that ended here, with their game and server. Roblox keeps reporting the account in that game long after:
    /// on 2026-10-13 a clean leave at 00:02:19 UTC was still "in game" at 00:06, flipping with offline. Until Roblox says otherwise,
    /// the same server (or game, when no server is given) is taken as stale, not "playing elsewhere". Kept in the vault
    /// (NoteOpenSessions) so a restart to update right after, or a lock, doesn't forget them.</summary>
    readonly Dictionary<Guid, (string? JobId, long? PlaceId, DateTime At)> _endedHere = new();
    static readonly TimeSpan StalePresence = TimeSpan.FromMinutes(15);

    bool IsStaleInGame(Account a, Presence p)
    {
        if (p.Type != 2 || _running.ContainsKey(a.Id) || !_endedHere.TryGetValue(a.Id, out var ended)
            || DateTime.UtcNow - ended.At >= StalePresence)
            return false;
        if (p.JobId is not null && ended.JobId is not null)
            return string.Equals(p.JobId, ended.JobId, StringComparison.OrdinalIgnoreCase);
        // No server id to compare: the same game counts, and so does no game at all (privacy hides it).
        long? place = p.RootPlaceId ?? p.PlaceId;
        return place is null || ended.PlaceId is null || place == ended.PlaceId || p.PlaceId == ended.PlaceId;
    }

    object? PresenceDto(Account a)
    {
        if (!_presence.TryGetValue(a.UserId, out var p))
            return null;
        if (IsStaleInGame(a, p))
            return new { type = "offline", game = (string?)null, placeId = (long?)null, jobId = (string?)null, joinPlaceId = (long?)null, iconUrl = (string?)null, lastOnline = (DateTime?)_endedHere[a.Id].At };
        long? placeId = p.RootPlaceId ?? p.PlaceId;
        var info = placeId is long id ? _games.GetValueOrDefault(id) : null;
        return new
        {
            type = p.Type switch { 0 => "offline", 1 => "online", 2 => "ingame", 3 => "studio", 4 => "invisible", _ => "unknown" },
            game = info?.Name is { Length: > 0 } name ? name : p.Location,
            placeId,
            jobId = p.JobId,
            // The place that server belongs to (placeId above is the game's start place, for its name and icon).
            joinPlaceId = p.PlaceId,
            iconUrl = info?.IconUrl,
            p.LastOnline,
        };
    }

    object SettingsDto(AppSettings s) => new
    {
        s.AutoLockMinutes,
        s.LastPlaceInput,
        s.LastJobId,
        s.RenameWindows,
        windowPictures = s.WindowPictures ?? s.RenameWindows,
        s.TaskbarGroups,
        s.WindowLayout,
        s.WindowWidth,
        s.WindowHeight,
        s.MaximizeOnOpen,
        s.LaunchVia,
        customLauncher = s.CustomLauncher.Length > 0 ? Path.GetFileNameWithoutExtension(s.CustomLauncher) : null,
        s.Layout,
        s.Sort,
        s.LaunchDelaySeconds,
        s.MemoryWarnings,
        s.TrimBackground,
        s.SplitCores,
        cores = CoresDto(),
        s.AntiAfk,
        robloxConfig = new { s.RobloxConfig.Mode, s.RobloxConfig.Graphics, s.RobloxConfig.Fps, s.RobloxConfig.Volume },
        recentGames = s.RecentGames.Select(SavedGameDto),
        pinnedGames = s.PinnedGames.Select(SavedGameDto),
        savedServers = s.SavedServers.Select(SavedServerDto),
        s.RecentPlayers,
        s.FollowJoinedPlayer,
        presets = s.Presets.Select(PresetDto),
        launchOrder = LaunchOrderDto(),
    };

    object SavedGameDto(RecentGame g) => new
    {
        g.PlaceId,
        g.UniverseId,
        name = g.Name.Length > 0 ? g.Name : $"Place {g.PlaceId}",
        iconUrl = _games.GetValueOrDefault(g.PlaceId)?.IconUrl ?? _iconByUniverse.GetValueOrDefault(g.UniverseId),
    };

    Vault RequireVault() => _vault ?? throw new UserError(L.T("Bloxkeep is locked."));

    Account RequireAccount(Guid id) =>
        RequireVault().Data.Accounts.FirstOrDefault(a => a.Id == id) ?? throw new UserError(L.T("Account not found."));

    Account? FindAccount(Guid id) => _vault?.Data.Accounts.FirstOrDefault(a => a.Id == id);

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
