using Microsoft.Win32;

namespace RobloxManager;

/// <summary>The state the page is drawn from: the app's whole state, each account's and game's part of it (what is installed: Installs.cs).</summary>
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
        // Macros (Beta), AppController.Macros.cs: nothing of them while locked.
        macroRuns = MacroRunsDto(),
        macroRecording = MacroRecordingDto(),
        macroVars = MacroVarsDto(),
        macroSweep = MacroSweepDto(),
        macroCanRun = MacroCanRunDto(),
        panelsShown = PanelsShownDto(),
    };

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
        a.FpsCap,
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
                fpsLimit = run.Launch.FpsLimit,
                robloxVersion = _clientBuilds.GetValueOrDefault(run.Pid)?.Version,
                oldBuild = RunsOldBuild(a, run.Pid),
                // The preset that opened it may turn auto-rejoin on or off for this client.
                autoRejoin = RejoinOn(a, run.Launch.PresetId),
            }
            : null,
        // A rejoin waiting its turn (auto-rejoin's wait, or a preset's restart): the card counts down and can call it off.
        // Also one under way (out of the queue, its window not asked for yet): "Rejoining…", and Cancel and Close still stop it.
        rejoin = (_rejoinQueue.FirstOrDefault(r => r.AccountId == a.Id) ?? (_rejoinInFlight?.AccountId == a.Id ? _rejoinInFlight : null)) is { } queued
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
        s.AutoUpdateRoblox,
        s.FastFlags,
        s.RamLimitMb,
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
        macros = s.Macros,
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
}
