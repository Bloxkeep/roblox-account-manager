using System.ComponentModel;
using System.Diagnostics;
using System.Net.NetworkInformation;
using System.Text.Json;
using Microsoft.Win32;

namespace RobloxManager;

/// <summary>An error whose message is shown to the user as-is.</summary>
public sealed class UserError(string message) : Exception(message);

/// <summary>
/// Everything behind the web UI: vault, Roblox calls, launching, process tracking, auto-rejoin and the
/// resource guard. The page sends {id, cmd, args}; each command gets a {type:"reply"} and state changes are
/// pushed as events. Cookies and passwords only reach the page when the user explicitly reveals one.
/// </summary>
public sealed partial class AppController : IDisposable
{
    /// <param name="Rejoin">Started by auto-rejoin rather than by the person.</param>
    /// <param name="PresetId">The preset that opened the client (its options apply to it), kept through rejoins and restarts.</param>
    sealed record PendingLaunch(Guid AccountId, HashSet<int> Before, DateTime StartedAt, GameTarget? Target, bool Rejoin = false, Guid? PresetId = null)
    {
        /// <summary>Settable: moved back with the clock (see TrackProcesses) so a set-back clock doesn't make the launch wait longer.</summary>
        public DateTime StartedAt { get; set; } = StartedAt;
    }

    /// <param name="NetworkRetries">Attempts that failed only because there was no connection.</param>
    /// <param name="PresetId">The preset that opened the client being brought back.</param>
    /// <param name="Restart">Its preset's planned restart, not a drop: it goes ahead whatever auto-rejoin says.</param>
    sealed record RejoinRequest(Guid AccountId, GameTarget Target, DateTime Due, int NetworkRetries = 0, Guid? PresetId = null, bool Restart = false);

    static readonly TimeSpan HandOverWindow = TimeSpan.FromSeconds(60);
    static readonly TimeSpan PendingTimeout = TimeSpan.FromSeconds(120);
    static readonly TimeSpan PresenceInterval = TimeSpan.FromSeconds(30);
    /// <summary>How often everyone's status is asked for while the window is in the background.</summary>
    static readonly TimeSpan BackgroundPresenceInterval = TimeSpan.FromMinutes(4);
    static readonly TimeSpan RejoinDelay = TimeSpan.FromSeconds(10);
    static readonly TimeSpan DisconnectGrace = TimeSpan.FromMinutes(2);
    static readonly TimeSpan ResumeGrace = TimeSpan.FromSeconds(60);
    static readonly TimeSpan NetworkRetryDelay = TimeSpan.FromSeconds(30);
    static readonly TimeSpan MaxRetryDelay = TimeSpan.FromMinutes(5);
    const int MaxNetworkRetries = 20;
    static readonly TimeSpan UpdateCheckInterval = TimeSpan.FromHours(6);
    static readonly TimeSpan RobloxVersionInterval = TimeSpan.FromMinutes(30);
    static readonly TimeSpan TrimInterval = TimeSpan.FromMinutes(2);
    const int TrimLoadPercent = 85;
    const long TrimBelowAvailableMb = 1536;
    static readonly TimeSpan MemoryWarningInterval = TimeSpan.FromMinutes(5);
    // Scanning launcher folders and reading the client's file version is the slow part; it is also re-read
    // whenever the launch option changes or a vault is opened.
    static readonly TimeSpan InstallCacheLifetime = TimeSpan.FromMinutes(3);
    static readonly TimeSpan RegistryCacheLifetime = TimeSpan.FromSeconds(30);
    static readonly TimeSpan SaveDelay = TimeSpan.FromSeconds(3);
    static readonly TimeSpan BackgroundSweep = TimeSpan.FromSeconds(15);
    const int MemoryWarningLoad = 92;
    const long DefaultClientMb = 900;
    const long MB = 1024 * 1024;
    static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);

    readonly Form _owner;
    readonly Action<string> _post;
    readonly System.Windows.Forms.Timer _procTimer = new() { Interval = 1500 };
    readonly Dictionary<Guid, (int Pid, PendingLaunch Launch)> _running = new();
    readonly List<PendingLaunch> _pending = new();
    /// <summary>When a client that may belong to each pending launch was last seen running.</summary>
    readonly Dictionary<PendingLaunch, DateTime> _pendingSeenAlive = new(ReferenceEqualityComparer.Instance);
    /// <summary>A launch whose clients have all quit (failed sign-in, closed by the person) stops waiting after this.</summary>
    static readonly TimeSpan PendingGoneGrace = TimeSpan.FromSeconds(5);
    /// <summary>Launches the person closed while still opening: their client is closed as soon as it is claimed.</summary>
    readonly HashSet<PendingLaunch> _closeWhenClaimed = new(ReferenceEqualityComparer.Instance);
    /// <summary>
    /// Clients that were in a game and ended (their X button, a crash): the windowless copy Roblox then starts in
    /// the notification area is their child; it is ended while it appears within this time.
    /// </summary>
    readonly Dictionary<int, DateTime> _traySweep = new();
    static readonly TimeSpan TraySweep = TimeSpan.FromSeconds(15);
    /// <summary>A log stuck in "joining" (a join or teleport that never finished) stops being trusted after this.</summary>
    static readonly TimeSpan JoiningTimeout = TimeSpan.FromMinutes(2);
    readonly Dictionary<long, string> _avatarUrls = new();
    readonly Dictionary<long, GameInfo?> _games = new();
    readonly Dictionary<long, DateTime> _gameFetchedAt = new();
    readonly Dictionary<long, Presence> _presence = new();
    /// <summary>Accounts whose last presence came from their own cookie (or showed them in a game): trustworthy.</summary>
    readonly HashSet<long> _presenceReliable = new();
    readonly RobloxLogWatcher _logs = new();
    /// <summary>Clients already being closed because their log showed a kick (so it happens once).</summary>
    readonly HashSet<int> _closingForRejoin = new();
    /// <summary>Roblox's code for "the same account joined from another device": never fight that with a rejoin.</summary>
    const int SameAccountElsewhere = 273;
    /// <summary>Roblox's code for "kicked for being idle 20 minutes".</summary>
    const int IdleKick = 278;
    /// <summary>Last time each client (by process id) got input: from the person playing it or from anti-AFK.</summary>
    readonly Dictionary<int, DateTime> _lastInput = new();
    readonly HashSet<int> _idleKickReported = new();
    /// <summary>Windows a nudge couldn't reach (focus refused, PC locked): not tried again before this.</summary>
    readonly Dictionary<int, DateTime> _nudgeRetryAt = new();
    static readonly TimeSpan NudgeRetry = TimeSpan.FromMinutes(1), MaxNudgeRetry = TimeSpan.FromMinutes(15);
    /// <summary>Nudges in a row each window missed: the wait before the next one doubles.</summary>
    readonly Dictionary<int, int> _nudgeMisses = new();
    /// <summary>Roblox windows currently set to a low memory priority (they are in the background).</summary>
    readonly HashSet<int> _lowMemoryPriority = new();
    /// <summary>The Roblox settings file holds values Bloxkeep wrote; the person's own come back when Roblox is closed.</summary>
    bool _robloxSettingsPending = RobloxGameSettings.HasBaseline;
    DateTime _robloxSettingsWrittenAt;
    bool _nudging;
    readonly HashSet<Guid> _userClosed = new();
    readonly List<RejoinRequest> _rejoinQueue = new();
    /// <summary>Rejoins of each account since its client last stayed in a game a while (see <see cref="RejoinWait"/>).</summary>
    readonly Dictionary<Guid, int> _rejoinsInARow = new();
    readonly Dictionary<Guid, int> _offlineStrikes = new();
    readonly AppUpdater _updater = new();
    DateTime _nextUpdateCheck = DateTime.UtcNow + TimeSpan.FromSeconds(20);
    DateTime _nextRobloxVersionCheck;
    RobloxInstalls.ClientVersion? _latestRoblox;
    /// <summary>The version that ran before an update, until the page has shown what's new since then.</summary>
    string? _updatedFrom;
    Vault? _vault;
    CancellationTokenSource? _launchCts, _rejoinCts;
    bool _bootStatusDone, _bootAvatarsDone;
    BootProgress? _boot;
    bool _checking, _lastArmed, _presenceBusy, _presenceLoaded, _rejoining, _trimRefusedShown, _disposed;
    DateTime _nextPresence, _lastFullPresence, _lastTrim, _lastMemoryWarning, _installsReadAt, _sweepBackgroundUntil, _resumedAt, _lastSessionStart, _lastTick;
    string? _lastMetrics, _lastState, _pageView;
    InstallsInfo? _installs;
    DateTime? _saveDue;
    bool _saveFailed;
    /// <summary>Saves in a row that failed: the next try waits longer (see Persist).</summary>
    int _saveFailures;
    static readonly TimeSpan MaxSaveRetry = TimeSpan.FromMinutes(5);
    (bool Protocol, bool TrayAutostart)? _registry;
    DateTime _registryReadAt;

    public AppController(Form owner, Action<string> postJson)
    {
        _owner = owner;
        _post = postJson;
        _procTimer.Tick += (_, _) => OnProcTimer();
        _procTimer.Start();
        SystemEvents.PowerModeChanged += OnPowerModeChanged;
        SystemEvents.TimeChanged += OnTimeChanged;
        NetworkChange.NetworkAvailabilityChanged += OnNetworkAvailabilityChanged;
        _updater.Changed += OnUpdaterChanged;
        NoteVersion();
    }

    bool _background;

    /// <summary>Set when the browser was recreated while the app kept running; the next page restores its view.</summary>
    public bool ResumingPage { get; set; }

    /// <summary>The page drew its first screen after loading (the window drops the picture it showed meanwhile).</summary>
    public Action? PageShown { get; init; }

    public bool Unlocked => _vault is not null;

    /// <summary>Recreates the interface drawing on the CPU (true) or the graphics card (false).</summary>
    public Func<bool, Task>? SetSafeRendering { get; init; }

    /// <summary>Runs right before the updater restarts the app (which skips the window's normal shutdown).</summary>
    public Action? BeforeRestart { get; init; }

    /// <summary>Runs when the updater couldn't restart after all: the app keeps running (its tray icon comes back).</summary>
    public Action? RestartFailed { get; init; }

    /// <summary>The window applies a new theme or close-to-tray choice.</summary>
    public Action? AppearanceChanged { get; init; }

    /// <summary>While the window is minimised, poll less often; Roblox status comes back up to date when it shows again.</summary>
    public bool Background
    {
        set
        {
            if (_background && !value)
                RefreshPresenceSoon(TimeSpan.Zero);
            _background = value;
            _procTimer.Interval = value ? 5000 : 1500;
        }
    }

    /// <summary>
    /// A page is there to receive state. False after the window released its browser in the background: building
    /// and serialising the state for nobody is the costly part of a day in the tray. The next page asks for
    /// everything with getState.
    /// </summary>
    public bool PageAttached { get; set; }

    // ───────────────────────────── bridge ─────────────────────────────

    void Post(string json)
    {
        if (!_disposed)
            _post(json);
    }

    void Emit(string name, object? data) => Post(JsonSerializer.Serialize(new { type = "event", name, data }, Json));
    void Toast(string text, string kind = "info") => Emit("toast", new { text, kind });
    void Progress(string? text) => Emit("progress", new { text });

    public void PushState()
    {
        if (PageAttached)
            PostState(BuildState(), force: true);
    }

    /// <summary>
    /// For timers (presence, process checks): the page re-renders on every state, which is the costly part with
    /// hundreds of accounts, so an identical state isn't sent again. Commands still always push, so a control the
    /// page changed on its own is put back when the app refused the change.
    /// </summary>
    void PushStateIfChanged()
    {
        if (PageAttached)
            PostState(BuildState(), force: false);
    }

    void PostState(object state, bool force)
    {
        string json = JsonSerializer.Serialize(state, Json);
        if (!force && json == _lastState)
            return;
        _lastState = json;
        Post($"{{\"type\":\"event\",\"name\":\"state\",\"data\":{json}}}");
    }

    public async Task HandleMessageAsync(string json)
    {
        int id = 0;
        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            id = root.GetProperty("id").GetInt32();
            string cmd = root.GetProperty("cmd").GetString() ?? "";
            var args = root.TryGetProperty("args", out var a) ? a.Clone() : default;
            object? result = await DispatchAsync(cmd, args);
            Post(JsonSerializer.Serialize(new { type = "reply", id, ok = true, result }, Json));
        }
        // This is the boundary to the page: every failure becomes an error reply instead of a crash.
        catch (Exception ex)
        {
            Post(JsonSerializer.Serialize(new { type = "reply", id, ok = false, error = ErrorText(ex) }, Json));
        }
    }

    /// <summary>
    /// What the person is told about a failure: Bloxkeep's and Roblox's own sentences as they are, anything else a
    /// short fixed sentence in the app's language. .NET's own texts are in Windows' language and name internals,
    /// so they go to the log instead.
    /// </summary>
    static string ErrorText(Exception ex)
    {
        if (ex is UserError or RobloxApiException or InvalidPasswordException)
            return ex.Message;
        if (ex is HttpRequestException or TaskCanceledException)
        {
            Log.Note($"{ex.GetType().Name}: {ex.Message}"); // offline, most likely: the reason without a stack
            return L.T("Couldn't reach Roblox. Check your internet connection and try again.");
        }
        Log.Write(ex);
        return ex switch
        {
            UnauthorizedAccessException or System.Security.SecurityException => L.T("Windows denied access to the file."),
            IOException => L.T("Windows couldn't read or write the file (another program may be using it)."),
            JsonException => L.T("Roblox sent an answer Bloxkeep didn't expect. Try again in a moment."),
            _ => L.T("Something went wrong. Try again in a moment."),
        };
    }

    Task<object?> DispatchAsync(string cmd, JsonElement a) => cmd switch
    {
        "getState" => Done(InitialState()),
        "keepView" => Done(KeepView(Str(a, "view"), Bool(a, "restart"))),
        "pageBusy" => Done(SetPageBusy(Bool(a, "busy"))),
        "pageShown" => Done(OnPageShown()),
        "checkUpdates" => CheckUpdatesAsync(),
        "applyUpdate" => Done(ApplyUpdate()),
        "setAutoUpdate" => Done(SetAutoUpdate(Bool(a, "enabled"))),
        "whatsNew" => Done(WhatsNew.Between(null, _updater.CurrentVersion)),
        "seenWhatsNew" => Done(SeenWhatsNew()),
        "setLanguage" => Done(SetLanguage(Str(a, "language"))),
        "setTheme" => Done(SetTheme(Str(a, "theme"))),
        "setCloseToTray" => Done(SetCloseToTray(Bool(a, "enabled"))),
        "setHideNames" => Done(SetHideNames(Bool(a, "enabled"))),
        "createVault" => CreateVaultAsync(Str(a, "password")),
        "unlock" => UnlockAsync(Str(a, "password")),
        "lock" => Done(Lock()),
        "changePassword" => ChangePasswordAsync(Str(a, "current"), Str(a, "next")),
        "addViaBrowser" => AddViaBrowserAsync(),
        "quickLoginStart" => QuickLoginStartAsync(Realm.Normalize(Str(a, "realm"))),
        "quickLoginPoll" => QuickLoginPollAsync(),
        "quickLoginCancel" => Done(QuickLoginCancel()),
        "loginPassword" => LoginStepAsync(() => LoginPasswordAsync(Str(a, "username"), Str(a, "password"), Realm.Normalize(Str(a, "realm"))), newAttempt: true),
        "loginFinish" => LoginStepAsync(LoginFinishAsync),
        "loginTwoStep" => LoginStepAsync(() => LoginTwoStepAsync(Str(a, "method"), Str(a, "code"))),
        "loginTwoStepSend" => LoginStepAsync(() => LoginTwoStepSendAsync(Str(a, "method"))),
        "loginTwoStepCode" => Done(LoginTwoStepCode()),
        "loginCancel" => Done(LoginCancel()),
        "loginCaptcha" => LoginStepAsync(LoginCaptchaAsync),
        "addViaCookie" => AddViaCookieAsync(a),
        "importRam" => ImportRamAsync(),
        "vaultImportPick" => Done(VaultImportPick()),
        "vaultImportOpen" => VaultImportOpenAsync(Str(a, "password")),
        "vaultImportApply" => Done(VaultImportApply(a)),
        "vaultImportCancel" => Done(DropVaultImport()),
        "updateAccount" => UpdateAccountAsync(a),
        "setAutoRejoin" => Done(SetAutoRejoin(Ids(a), Bool(a, "enabled"))),
        "cancelRejoin" => Done(CancelRejoin(Id(a))),
        "setUseRobloxConfig" => Done(SetUseRobloxConfig(Ids(a), Bool(a, "enabled"))),
        "reveal" => Done(Reveal(Id(a), Str(a, "field"))),
        "totpCode" => Done(AuthenticatorCode(Id(a))),
        "totpCheck" => Done(CheckAuthenticator(Str(a, "key"))),
        "deleteAccounts" => Done(DeleteAccounts(Ids(a))),
        "checkAccounts" => CheckAccountsAsync(Ids(a)),
        "refreshPresence" => RefreshPresenceAsync(),
        "copy" => Done(Copy(Id(a), Str(a, "field"))),
        "resolveGame" => ResolveGameAsync(Str(a, "input")),
        "searchGames" => SearchGamesAsync(Str(a, "query")),
        "gameIcons" => GameIconsAsync(a),
        "gameLists" => GameListsAsync(),
        "servers" => ServersAsync(Long(a, "placeId"), Bool(a, "fewest"), OptStr(a, "cursor")),
        "friendServers" => FriendServersAsync(Long(a, "placeId"), Ids(a)),
        "pinGame" => Done(TogglePin(Long(a, "placeId"), Long(a, "universeId"), Str(a, "name"))),
        "launch" => LaunchAsync(a),
        "cancelLaunch" => Done(CancelLaunch()),
        "savePreset" => SavePresetAsync(a),
        "deletePreset" => Done(DeletePreset(Id(a))),
        "runPreset" => RunPresetAsync(Id(a), Bool(a, "force")),
        "duplicatePreset" => Done(DuplicatePreset(Id(a))),
        "setPresetOrder" => Done(SetPresetOrder(Ids(a))),
        "setPresetSchedule" => Done(SetPresetSchedule(Id(a), Bool(a, "enabled"))),
        "presetIcons" => PresetIconsAsync(),
        "setLaunchOrder" => Done(SetLaunchOrder(Ids(a))),
        "closeAccounts" => Done(CloseAccounts(Ids(a), Bool(a, "force"))),
        "closeAll" => Done(CloseAll(Bool(a, "force"))),
        "arrangeWindows" => ArrangeWindowsAsync((int)Long(a, "monitor")),
        "setSettings" => Done(SetSettings(a)),
        "setRendering" => SetRenderingAsync(Bool(a, "gpu")),
        "disableTrayAutostart" => Done(DisableTrayAutostart()),
        "removeRecent" => Done(RemoveRecent(Long(a, "placeId"))),
        "openProfile" => Done(OpenUrl($"https://www.roblox.com/users/{Long(a, "userId")}/profile")),
        "openDataFolder" => Done(OpenDataFolder()),
        "monitorArea" => Done(MonitorAreaCommand(a)),
        "setNotifications" => Done(SetNotifications(a)),
        "setDailySignInCheck" => Done(SetDailySignInCheck(Bool(a, "enabled"))),
        "toggleMini" => Done(ToggleMini()),
        "setBackup" => Done(SetBackup(a)),
        "pickBackupFolder" => Done(PickBackupFolder()),
        "openBackupFolder" => Done(OpenBackupFolder()),
        "backupSuggestions" => Done(RefreshBackupSuggestions()),
        "backupNow" => BackupNowAsync(),
        "restoreBackup" => Done(RestoreBackup()),
        "pickLauncher" => Done(PickLauncher()),
        "playStats" => Done(PlayStats((int)Long(a, "days"))),
        "activity" => Done(ActivityList(a)),
        "exportActivity" => Done(ExportActivity(a)),
        "findPlayer" => FindPlayerAsync(a),
        "joinServer" => JoinServerAsync(a),
        "forgetPlayer" => Done(ForgetPlayer(Str(a, "name"))),
        "setDiscord" => Done(SetDiscord(a)),
        "setDiscordWebhook" => Done(SetDiscordWebhook(Str(a, "url"))),
        "revealDiscordWebhook" => Done(RevealDiscordWebhook()),
        "discordTest" => SendDiscordNowAsync(summary: false),
        "discordSummaryNow" => SendDiscordNowAsync(summary: true),
        "openReleaseNotes" => Done(OpenReleaseNotes(Str(a, "version"))),
        "hideRobloxClosed" => Done(HideRobloxClosed()),
        _ => throw new UserError(L.F("Unknown command: {0}", cmd)),
    };

    static Task<object?> Done(object? value) => Task.FromResult(value);

    static bool Has(JsonElement a, string name, JsonValueKind kind) =>
        a.ValueKind == JsonValueKind.Object && a.TryGetProperty(name, out var v) && v.ValueKind == kind;

    static string Str(JsonElement a, string name) => Has(a, name, JsonValueKind.String) ? a.GetProperty(name).GetString()! : "";
    static string? OptStr(JsonElement a, string name) => Has(a, name, JsonValueKind.String) ? a.GetProperty(name).GetString() : null;
    static bool Bool(JsonElement a, string name) => Has(a, name, JsonValueKind.True);
    static bool? OptBool(JsonElement a, string name) => Has(a, name, JsonValueKind.True) ? true : Has(a, name, JsonValueKind.False) ? false : null;
    static long Long(JsonElement a, string name) => Has(a, name, JsonValueKind.Number) && a.GetProperty(name).TryGetInt64(out long v) ? v : 0;
    static Guid Id(JsonElement a) => Guid.TryParse(Str(a, "id"), out var id) ? id : Guid.Empty;

    static Guid[] Ids(JsonElement a) => Has(a, "ids", JsonValueKind.Array)
        ? a.GetProperty("ids").EnumerateArray()
            .Select(e => e.ValueKind == JsonValueKind.String && Guid.TryParse(e.GetString(), out var g) ? g : Guid.Empty)
            .Where(g => g != Guid.Empty).Distinct().ToArray()
        : [];

    // ───────────────────────────── state ─────────────────────────────

    /// <summary>The page's first request; says once whether it replaces an earlier page of this session.</summary>
    object InitialState()
    {
        bool resume = ResumingPage;
        ResumingPage = false;
        PageAttached = true;
        PageBusy = false; // a fresh page has nothing open yet
        _lastMetrics = _lastState = null; // a new page has nothing yet; send both again
        string? view = resume && _vault is not null ? _pageView : null;
        _pageView = null;
        return new { resume, view, state = BuildState() };
    }

    /// <summary>
    /// What the page had on screen (selection, filter, search, scroll), kept in memory only while the browser is
    /// closed in the background, so a recreated page looks exactly the same. Never written to disk.
    /// </summary>
    /// <param name="restart">Sent right before the page asks for a browser restart (GPU switch), while the window is in front.</param>
    object? KeepView(string view, bool restart)
    {
        // Sent when the page goes hidden; one that arrives after the window is already back (a quick minimise and
        // restore) is stale, and would put an old selection back the next time the browser is recreated.
        if (!_background && !restart)
            return null;
        _pageView = view.Length <= 256 * 1024 ? view : null;
        return null;
    }

    /// <summary>The window came back with its page still open: what the page kept is out of date from here on.</summary>
    public void DropKeptView() => _pageView = null;

    /// <summary>
    /// The page has something open that closing the browser would lose (a form being filled in, a sign-in under way):
    /// the window keeps the browser alive in the background meanwhile.
    /// </summary>
    public bool PageBusy { get; private set; }

    object? SetPageBusy(bool busy)
    {
        PageBusy = busy;
        return null;
    }

    /// <summary>
    /// The page is going away (browser released or recreated): nothing it was in the middle of survives, least of
    /// all a sign-in prompt holding a password or a Quick Login code waiting to be approved.
    /// </summary>
    public void PageGone()
    {
        PageAttached = false; // the next page asks for everything with getState
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
        closeToTray = Prefs.CloseToTray,
        hideNames = Prefs.HideNames,
        notifications = Prefs.Notifications,
        notifyOff = Prefs.NotifyOff.Where(NotifyKinds.Contains).Order().ToArray(),
        dailySignInCheck = Prefs.DailySignInCheck,
        miniOpen = Prefs.MiniOpen,
        backup = BackupDto(),
        discord = DiscordDto(),
        safeRendering = Prefs.SafeRendering,
        update = _updater.Dto(),
        robloxVersion = RobloxVersionDto(),
        robloxUpdates = RobloxUpdatesDto(),
        whatsNew = _updatedFrom is not null && _vault is not null
            ? new { from = _updatedFrom, releases = WhatsNew.Between(_updatedFrom, _updater.CurrentVersion) }
            : null,
        vaultExists = Vault.Exists,
        locked = _vault is null,
        dataPath = Vault.DataDir,
        robloxInstalled = RegistryInfo().Protocol,
        launching = _launchCts is not null,
        checking = _checking,
        boot = _vault is null ? null : _boot,
        multi = new
        {
            enabled = _vault?.Data.Settings.MultiInstance ?? false,
            armed = MultiInstance.Armed,
            windowed = _vault?.Data.Settings.MultiInstanceWindowed ?? true,
            tile = _vault?.Data.Settings.MultiInstanceTile ?? false,
            trayAutostart = RegistryInfo().TrayAutostart,
        },
        installs = Installs(),
        monitors = Screen.AllScreens.Select((s, i) => new { index = i, s.Primary, width = s.Bounds.Width, height = s.Bounds.Height }).ToArray(),
        settings = _vault is null ? null : SettingsDto(_vault.Data.Settings),
        accounts = _vault?.Data.Accounts.Select(AccountDto).ToArray() ?? [],
    };

    /// <summary>Which Roblox clients and launchers exist, and the version of the client the chosen option starts.</summary>
    /// <param name="Others">Other Bloxstrap-style launchers found here (their option is "other:&lt;name&gt;").</param>
    sealed record InstallsInfo(string? Handler, bool Bloxstrap, bool Fishstrap, List<string> Others, bool Roblox, bool Direct, bool Store, RobloxInstalls.ClientVersion? Client);

    /// <param name="UpToDate">null until both versions are known.</param>
    sealed record RobloxVersionInfo(string? Installed, string? Latest, bool? UpToDate, bool Direct);

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
        _installsReadAt = DateTime.UtcNow;
        string? before = _installs?.Client?.Folder;
        _installs = new InstallsInfo(
            Handler: RobloxInstalls.ProtocolHandlerPath() is { } h ? Path.GetFileNameWithoutExtension(h) : null,
            Bloxstrap: RobloxInstalls.FindBootstrapper("Bloxstrap") is not null,
            Fishstrap: RobloxInstalls.FindBootstrapper("Fishstrap") is not null,
            Others: RobloxInstalls.FindOtherBootstrappers(),
            Roblox: RobloxInstalls.FindOfficialClient() is not null,
            Direct: RobloxInstalls.FindAnyClient() is not null,
            Store: RobloxInstalls.StoreEditionInstalled(),
            Client: RobloxInstalls.VersionOf(RobloxInstalls.ClientFor(_vault?.Data.Settings.LaunchVia ?? "auto", _vault?.Data.Settings.CustomLauncher ?? "")));
        // Roblox (or its launcher) put a new build in place since the last look.
        if (before is not null && _installs.Client is { } now && now.Folder != before)
        {
            string updated = L.F("Roblox updated to {0}.", now.Version);
            Toast(updated, "ok");
            NoteActivity("robloxUpdated", text: updated, tone: "info");
        }
        NoteRobloxBuild(_installs.Client); // Settings › Roblox updates (AppController.RobloxUpdates.cs)
        return _installs;
    }

    /// <summary>
    /// The Roblox version the chosen launcher starts, next to the newest one Roblox serves. An older client updates
    /// itself on its next start (that first window takes longer), except when started directly.
    /// </summary>
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
        return new RobloxVersionInfo(installed?.Version, _latestRoblox?.Version, upToDate, _vault?.Data.Settings.LaunchVia == "direct");
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
                reason = _logs.StateOf(run.Pid) is { Status: "disconnected" } kicked ? kicked.Reason : null,
                jobId = _logs.StateOf(run.Pid)?.JobId,
                jobPlaceId = _logs.StateOf(run.Pid)?.PlaceId,
                // The preset that opened it may turn auto-rejoin on or off for this client.
                autoRejoin = RejoinOn(a, run.Launch.PresetId),
            }
            : null,
        // A rejoin waiting its turn (auto-rejoin's wait, or a preset's restart): the card counts down and can call it off.
        rejoin = _rejoinQueue.FirstOrDefault(r => r.AccountId == a.Id) is { } queued
            ? new { at = queued.Due, queued.Restart, game = GameDto(queued.Target.PlaceId) }
            : null,
    };

    /// <summary>
    /// What a client open on this PC is doing: "ingame"; "joining" (just started, or not known yet); "menu" (opened
    /// without a game, or left it); "disconnected" (kicked or lost connection — the window stays open on Roblox's error
    /// screen). The client's own log says so at once; without one, Roblox's status for the account is used.
    /// </summary>
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

    async Task NameLoggedPlacesAsync(List<long> placeIds)
    {
        await LoadGamesAsync(placeIds.Select(id => (id, 0L)).ToList());
        // Nothing came back (offline, rate-limited): the page would show the same thing.
        if (placeIds.Any(id => _games.GetValueOrDefault(id) is not null))
            PushStateIfChanged();
    }

    /// <summary>
    /// The game a client here is in: the place its log reports, named once that place is looked up; until then the
    /// game it was opened with (a game often moves players to another of its places right after they join).
    /// </summary>
    object? SessionGameDto(int pid, PendingLaunch launch)
    {
        long? logged = _logs.StateOf(pid)?.PlaceId, opened = launch.Target?.PlaceId;
        bool loggedNamed = logged is long l && _games.GetValueOrDefault(l)?.Name is { Length: > 0 };
        return GameDto(loggedNamed || opened is null ? logged ?? opened : opened);
    }

    object? GameDto(long? placeId) => placeId is long id
        ? new { placeId = id, name = _games.GetValueOrDefault(id)?.Name ?? "", iconUrl = _games.GetValueOrDefault(id)?.IconUrl }
        : null;

    /// <summary>
    /// Clients that ended on this PC, with the game and server they were in. Roblox keeps reporting the account in
    /// that game long after the client left: on 2026-10-13 a clean leave logged at 00:02:19 UTC was still "in game"
    /// in every presence answer at 00:06, and answers flipped between in game and offline. Until Roblox reports
    /// something else, the same server (or the same game, when Roblox doesn't say which server) is taken as that
    /// stale report instead of "playing elsewhere".
    /// </summary>
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
        s.LastPlaceInput,
        s.LastJobId,
        s.RenameWindows,
        s.WindowLayout,
        s.LaunchVia,
        customLauncher = s.CustomLauncher.Length > 0 ? Path.GetFileNameWithoutExtension(s.CustomLauncher) : null,
        s.Layout,
        s.Sort,
        s.LaunchDelaySeconds,
        s.MemoryWarnings,
        s.TrimBackground,
        s.AntiAfk,
        robloxConfig = new { s.RobloxConfig.Mode, s.RobloxConfig.Graphics, s.RobloxConfig.Fps, s.RobloxConfig.Volume },
        recentGames = s.RecentGames.Select(SavedGameDto),
        pinnedGames = s.PinnedGames.Select(SavedGameDto),
        s.RecentPlayers,
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

    /// <summary>
    /// Saves a few seconds later, so a burst of small changes (typing a game link) writes the encrypted vault once.
    /// Anything still waiting is saved on lock and on exit.
    /// </summary>
    void PersistSoon() => _saveDue ??= DateTime.UtcNow + SaveDelay;

    void PersistIfDue(DateTime now)
    {
        if (_saveDue is { } due && now >= due)
            Persist();
    }

    /// <returns>false when the vault couldn't be written (it is tried again shortly).</returns>
    bool Persist()
    {
        _saveDue = null;
        if (_vault is null)
            return true;
        // Every save holds the clients open now, not only the one on lock or exit: after a crash (or the app being
        // ended) the next unlock still finds them, with their auto-rejoin and scheduled stop.
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
            // Try again shortly (and on lock or exit); an antivirus usually lets go within seconds. Say so once. A file
            // that stays locked (a sync client, a read-only folder) is tried less and less often, up to every 5 minutes.
            _saveDue = DateTime.UtcNow + TimeSpan.FromTicks(Math.Min(SaveDelay.Ticks << Math.Min(_saveFailures, 7), MaxSaveRetry.Ticks));
            _saveFailures++;
            if (!_saveFailed)
                Toast(L.F("Couldn't save data: {0}", ErrorText(ex)), "error");
            _saveFailed = true;
            return false;
        }
    }

    /// <summary>
    /// Before the vault goes away (lock, exit, an update's restart) or the PC sleeps: what is still waiting, and
    /// which clients are open, is written now. Tried a few times, since an antivirus usually lets go of the file
    /// within a second; false when it still couldn't be written, so the caller can refuse rather than lose changes.
    /// </summary>
    public bool SaveBeforeClosing()
    {
        bool due = NoteOpenSessions() | NotePlaySessions() | NoteScheduledRuns() | _saveDue is not null;
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

    object? SetCloseToTray(bool enabled)
    {
        Prefs.CloseToTray = enabled;
        Prefs.Save();
        AppearanceChanged?.Invoke();
        PushState();
        return null;
    }

    // ───────────────────────────── vault ─────────────────────────────

    async Task<object?> CreateVaultAsync(string password)
    {
        if (Vault.Exists)
            throw new UserError(L.T("Bloxkeep is already set up."));
        if (password.Length < 6)
            throw new UserError(L.T("The master password needs at least 6 characters."));
        // Six spaces pass the length check but are no password at all.
        if (string.IsNullOrWhiteSpace(password))
            throw new UserError(L.T("The master password can't be only spaces."));
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
        OnUnlocked(created);
        return BuildState();
    }

    async Task<object?> UnlockAsync(string password)
    {
        if (_vault is not null)
            return BuildState();
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
        OnUnlocked(vault);
        return BuildState();
    }

    void OnUnlocked(Vault vault)
    {
        _vault = vault;
        var s = vault.Data.Settings;
        if (!RobloxLauncher.IsValidOption(s.LaunchVia))
            s.LaunchVia = s.DirectLaunch ? "direct" : "auto";
        _installs = null; // re-read for this vault's launch option
        if (s.MultiInstance && !MultiInstance.Armed)
            MultiInstance.Enable();
        _lastArmed = MultiInstance.Armed;
        AdoptOpenSessions(vault);
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
        OnVaultUnlocked();
        AddLockedActivity(vault); // what happened while locked (a window closed) is in the log at once, also if Bloxkeep quits next
    }

    /// <summary>
    /// Remembers the clients that are open, so closing or restarting Bloxkeep (for example to update) doesn't lose
    /// track of them: they keep running, and the next unlock picks them up again, auto-rejoin included.
    /// </summary>
    /// <returns>true when the list changed, i.e. the vault needs saving.</returns>
    bool NoteOpenSessions()
    {
        if (_vault is not { } vault)
            return false;
        var sessions = _running
            .Select(r => new OpenSession { AccountId = r.Key, Pid = r.Value.Pid, StartedAt = r.Value.Launch.StartedAt, Target = r.Value.Launch.Target, PresetId = r.Value.Launch.PresetId })
            .ToList();
        var noted = vault.Data.OpenSessions;
        if (noted.Count == sessions.Count && noted.Zip(sessions).All(p => p.First.AccountId == p.Second.AccountId && p.First.Pid == p.Second.Pid))
            return false;
        vault.Data.OpenSessions = sessions;
        return true;
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
        Ui.ClearSecretClipboard();
        // A second lock (already locked) would start the missed-schedule bookkeeping over and lose what was noted.
        if (_vault is not null)
            RememberSchedulesForLock();
        _ = FinalDiscordBoard(closing: false);
        _vault?.Dispose();
        _vault = null;
        // Nothing of the vault stays reachable while locked: Dispose only wipes the key, so the decrypted accounts,
        // cookies and passwords go only once no field holds the vault (or what was made from it) any more.
        _discordVault = null;
        _friendIds.Clear();
        _friendServers.Clear();
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
        return true;
    }

    // ───────────────────────────── accounts ─────────────────────────────

    /// <summary>Opens the sign-in window; every account signed in there is saved right away (several in a row).</summary>
    async Task<object?> AddViaBrowserAsync()
    {
        RequireVault();
        var added = new List<AddResult>();
        using (var form = new BrowserLoginForm(async cookie =>
               {
                   var user = await RobloxApi.GetAuthenticatedUserAsync(cookie)
                       ?? throw new UserError(L.T("Roblox didn't accept that sign-in. Please try again."));
                   var result = AddOrMerge(new Account
                   {
                       Cookie = cookie,
                       UserId = user.Id,
                       Username = user.Name,
                       DisplayName = user.DisplayName,
                       CookieValid = true,
                       LastChecked = DateTime.Now,
                   });
                   added.Add(result);
                   return L.F(result.Merged ? "{0} was already here — its sign-in was updated." : "{0} saved.", result.Label);
               }))
        {
            form.ShowDialog(_owner);
        }
        return new { added };
    }

    /// <summary>The code on screen in the Quick Login window; one at a time.</summary>
    QuickLoginCode? _quickLogin;
    bool _quickLoginFinishing;
    int _quickLoginStarts, _quickLoginFinishTries;
    /// <summary>The session a confirmed code was traded for, while its account lookup is being retried.</summary>
    (QuickLoginCode Login, string Cookie)? _quickLoginCookie;

    async Task<object?> QuickLoginStartAsync(string realm)
    {
        RequireVault();
        QuickLoginCancel();
        int start = ++_quickLoginStarts;
        var login = await RobloxApi.CreateQuickLoginAsync(realm);
        // Another code was asked for meanwhile (tab switched, "new code" pressed twice): that one is on screen.
        if (start != _quickLoginStarts || _vault is null)
        {
            _ = RobloxApi.CancelQuickLoginAsync(login);
            return null;
        }
        _quickLogin = login;
        string? qr = await RobloxApi.GetQuickLoginQrAsync(login);
        int seconds = (int)Math.Max(0, (login.ExpiresUtc - DateTime.UtcNow).TotalSeconds);
        return new { code = login.Code, qr, seconds };
    }

    /// <summary>
    /// Called every couple of seconds while the window is open. Once the user confirms on their device the code is
    /// traded for a session and the account is saved, just like signing in through the browser.
    /// </summary>
    async Task<object?> QuickLoginPollAsync()
    {
        if (_quickLogin is not { } login || _quickLoginFinishing)
            return new { status = _quickLoginFinishing ? "adding" : "none" };
        // A session already in hand only needs its account looked up again: the spent code's status no longer matters.
        var status = _quickLoginCookie is { } pending && ReferenceEquals(pending.Login, login)
            ? new QuickLoginStatus("Validated", null, null)
            : await RobloxApi.GetQuickLoginStatusAsync(login);
        if (!ReferenceEquals(login, _quickLogin))
            return new { status = "none" }; // the window asked for a new code meanwhile
        string? picture = Uri.TryCreate(status.PictureUrl, UriKind.Absolute, out var p) && p.Host.EndsWith(".rbxcdn.com") ? status.PictureUrl : null;
        switch (status.Status)
        {
            case "Validated":
                _quickLoginFinishing = true;
                bool retry = false;
                try
                {
                    // The code is spent once traded: a retry after the account lookup failed reuses the session it gave.
                    if (_quickLoginCookie is not { } held || !ReferenceEquals(held.Login, login))
                        _quickLoginCookie = held = (login, await RobloxApi.FinishQuickLoginAsync(login));
                    string cookie = held.Cookie;
                    var user = await RobloxApi.GetAuthenticatedUserAsync(cookie, login.Realm)
                        ?? throw new UserError(L.T("Roblox didn't accept that sign-in. Please try again."));
                    var added = AddOrMerge(new Account
                    {
                        Cookie = cookie,
                        Realm = login.Realm,
                        UserId = user.Id,
                        Username = user.Name,
                        DisplayName = user.DisplayName,
                        CookieValid = true,
                        LastChecked = DateTime.Now,
                    });
                    return new { status = "added", added };
                }
                // A network hiccup or Roblox down right after the person approved on their phone: the next poll tries
                // again a couple of times rather than making them start over. A refusal (403, 429) is final: the code
                // may be spent. Once the session is in hand, only the account lookup is repeated.
                catch (Exception ex) when ((IsNetworkError(ex) || _quickLoginCookie is { } got && ReferenceEquals(got.Login, login)
                                               && RobloxApi.IsExpectedError(ex)) && ++_quickLoginFinishTries < 3)
                {
                    retry = true;
                    return new { status = "adding" };
                }
                finally
                {
                    _quickLoginFinishing = false;
                    if (!retry)
                    {
                        _quickLoginFinishTries = 0;
                        _quickLoginCookie = null;
                        if (ReferenceEquals(login, _quickLogin))
                            _quickLogin = null;
                    }
                }
            case "UserLinked":
                return new { status = "linked", name = status.AccountName, picture };
            case "Cancelled":
            case "Expired":
                if (ReferenceEquals(login, _quickLogin))
                    _quickLogin = null;
                return new { status = status.Status.ToLowerInvariant() };
            default:
                return new { status = "waiting", seconds = (int)Math.Max(0, (login.ExpiresUtc - DateTime.UtcNow).TotalSeconds) };
        }
    }

    object? QuickLoginCancel()
    {
        if (_quickLogin is { } login && !_quickLoginFinishing)
            _ = RobloxApi.CancelQuickLoginAsync(login);
        if (!_quickLoginFinishing)
        {
            _quickLogin = null;
            _quickLoginFinishTries = 0;
            _quickLoginCookie = null;
        }
        ++_quickLoginStarts; // a code still being asked for is cancelled when it arrives
        return null;
    }

    // A two-step prompt or captcha waiting for the user; held only while the sign-in is in progress, never persisted.
    /// <summary>
    /// The password sign-in window closed: drop the half-done sign-in, which still holds the password. A reply still
    /// on its way belongs to the sign-in before this (see <see cref="_loginGeneration"/>) and is dropped too.
    /// </summary>
    object? LoginCancel()
    {
        _loginGeneration++;
        // A check being answered on its own (proof-of-work) stops where it is: its answer would send the password again.
        _loginCts.Cancel();
        _loginCts = new CancellationTokenSource();
        DropLoginPrompts();
        return null;
    }

    /// <summary>Forgets what the sign-in in progress holds: its prompts (with the password) and a session not saved yet.</summary>
    void DropLoginPrompts()
    {
        _twoStep = null;
        _captcha = null;
        _humanCaptcha = null;
        _captchaUsed = false;
        _freshPuzzles = 0;
        _loginCookie = null;
        _twoStepWaitUntil = default;
    }

    RobloxApi.TwoStepPrompt? _twoStep;
    RobloxApi.CaptchaPrompt? _captcha;
    RobloxApi.HumanCaptchaPrompt? _humanCaptcha;
    /// <summary>The puzzle of this sign-in attempt was already shown and closed: it can't be opened again (it comes up blank).</summary>
    bool _captchaUsed;
    /// <summary>Puzzles asked for again in this attempt after their window was closed; each one sends the password again.</summary>
    int _freshPuzzles;
    const int MaxFreshPuzzles = 3;
    /// <summary>
    /// A session Roblox handed out whose account couldn't be looked up yet (a network hiccup): "Try again" only looks it
    /// up again, instead of signing in again with the password or a code that is spent.
    /// </summary>
    (string Cookie, string Realm)? _loginCookie;
    /// <summary>Roblox said "too many tries" to a 2-step code: Verify and Send wait until then.</summary>
    DateTime _twoStepWaitUntil;
    static readonly TimeSpan TwoStepWait = TimeSpan.FromSeconds(60);
    /// <summary>Bumped by a cancel or a lock: a sign-in step that started before it must not keep its result.</summary>
    int _loginGeneration;
    /// <summary>Cancelled (and replaced) with each bump of <see cref="_loginGeneration"/>, for the steps that go on by themselves.</summary>
    CancellationTokenSource _loginCts = new();

    bool HasLoginPrompt => _twoStep is not null || _captcha is not null || _humanCaptcha is not null;

    /// <summary>
    /// One step of a password sign-in. Roblox ending the attempt (a wrong password, a locked account, too many tries)
    /// isn't an error of this step: the prompt, which holds the password, is dropped, so nothing can send it again,
    /// and the page goes back to the username and password with the reason ({ status: "login", error }).
    /// </summary>
    /// <param name="newAttempt">A new password sign-in: whatever attempt was going on is dropped, and a late reply to it too.</param>
    async Task<object?> LoginStepAsync(Func<Task<object?>> step, bool newAttempt = false)
    {
        if (newAttempt)
            LoginCancel();
        int generation = _loginGeneration;
        bool hadPrompt = HasLoginPrompt;
        try
        {
            return await step();
        }
        // Called off (cancel, lock, the page gone) while a check was being answered: nothing is kept.
        catch (OperationCanceledException) when (generation != _loginGeneration)
        {
            return null;
        }
        catch (RobloxApiException ex) when (ex.AttemptOver)
        {
            if (generation != _loginGeneration)
                return null;
            DropLoginPrompts();
            return new { status = "login", error = ex.Message };
        }
        // Too many codes tried: the code screen stays (the prompt is still good), with Verify held back for a minute.
        catch (RobloxApiException ex) when (ex.RateLimited && _twoStep is not null)
        {
            if (generation != _loginGeneration)
                return null;
            _twoStepWaitUntil = DateTime.UtcNow + TwoStepWait;
            return new { status = "wait", error = ex.Message, seconds = (int)TwoStepWait.TotalSeconds };
        }
        // The step spent its prompt (the puzzle or the code was answered) and then failed: nothing is left to answer on
        // this screen, so the page goes back to the username and password with the reason. A session already handed
        // out is kept, and only looked up again.
        catch (Exception ex) when (ex is UserError || RobloxApi.IsExpectedError(ex))
        {
            if (generation != _loginGeneration)
                return null;
            if (_loginCookie is not null)
                return new { status = "finish", error = ErrorText(ex) };
            if (hadPrompt && !HasLoginPrompt)
                return new { status = "login", error = ErrorText(ex) };
            throw;
        }
    }

    async Task<object?> LoginPasswordAsync(string username, string password, string realm)
    {
        RequireVault();
        if (username.Trim().Length == 0 || password.Length == 0)
            throw new UserError(L.T("Enter the username and password."));
        int generation = _loginGeneration;
        return await HandleLoginAsync(await RobloxApi.LoginWithPasswordAsync(username.Trim(), password, realm), realm, generation);
    }

    /// <returns>null when the sign-in was cancelled (or the vault locked) while Roblox was answering: nothing is kept.</returns>
    async Task<object?> HandleLoginAsync(RobloxApi.LoginResult result, string realm, int generation)
    {
        // Proof-of-work needs no input: solve it and carry on. Once: answering it repeats the password sign-in, which
        // happens at most once per click — and not at all once the sign-in was cancelled or Bloxkeep locked, also
        // while the puzzle is being solved (the token stops it before anything more is sent).
        if (result.ProofOfWork is { } pow)
        {
            if (generation != _loginGeneration)
                return null;
            result = await RobloxApi.FinishProofOfWorkLoginAsync(pow, _loginCts.Token);
        }
        // Which 2-step methods the account has decides the tabs, or that only the phone or browser can finish it.
        if (result.TwoStep is { } asked)
            result = await RobloxApi.WithTwoStepMethodsAsync(asked);
        if (generation != _loginGeneration)
            return null;
        if (result.ProofOfWork is not null)
            throw new UserError(L.T("Roblox didn't accept the sign-in check. Please try again."));
        // A check only Roblox's site or app can show ends this attempt, with the reason, like a refusal does.
        if (result.Blocked is { } blocked)
            throw new RobloxApiException(blocked, attemptOver: true);
        if (result.Captcha is { } captcha)
        {
            _captcha = captcha;
            _captchaUsed = false;
            return new { status = "captcha" };
        }
        if (result.HumanCaptcha is { } human)
        {
            _humanCaptcha = human;
            _captchaUsed = false;
            return new { status = "captcha" };
        }
        if (result.TwoStep is { } prompt)
        {
            _twoStep = prompt;
            // The first method is the one the code was sent by, or Roblox's preferred one.
            // autofill: the account's authenticator key is saved here, so the page fills its code in (Verify stays the person's).
            return new { status = "twostep", media = prompt.Methods[0], methods = prompt.Methods, autofill = AuthenticatorAccount(prompt) is not null };
        }
        return await SaveLoginAsync(result.Cookie!, realm, generation);
    }

    /// <summary>Shows the captcha for the user to solve; on success the sign-in carries on (it may still ask for 2-step).</summary>
    async Task<object?> LoginCaptchaAsync()
    {
        RequireVault();
        int generation = _loginGeneration;
        // A puzzle belongs to one sign-in attempt; once its window was closed, ask Roblox for a fresh one.
        if (_captchaUsed && (_captcha?.Username ?? _humanCaptcha?.Username) is { } username)
        {
            // Each fresh puzzle sends the password again: a few per attempt, then the attempt ends.
            if (_freshPuzzles >= MaxFreshPuzzles)
                throw new RobloxApiException(L.T("Roblox keeps asking for a new puzzle. Sign in with your phone instead, or try again later."), attemptOver: true);
            _freshPuzzles++;
            string password = _captcha?.Password ?? _humanCaptcha!.Password;
            string realm = _captcha?.Realm ?? _humanCaptcha!.Realm;
            _captcha = null;
            _humanCaptcha = null;
            _captchaUsed = false;
            var again = await HandleLoginAsync(await RobloxApi.LoginWithPasswordAsync(username, password, realm), realm, generation);
            // Roblox may let it through without a puzzle this time, or ask for 2-step instead.
            if (generation != _loginGeneration || (_captcha is null && _humanCaptcha is null))
                return again;
        }
        if (_humanCaptcha is { } human)
        {
            string? redemption = ShowCaptcha(CaptchaForm.Human(human.Realm, human.CaptchaChallengeId));
            // Cancelled, locked or the page gone while the puzzle was open: nothing more goes to Roblox.
            if (generation != _loginGeneration || _vault is null || !ReferenceEquals(_humanCaptcha, human))
                return null;
            if (redemption is null)
            {
                _captchaUsed = true;
                return new { status = "captcha", closed = true };
            }
            _humanCaptcha = null;
            return await HandleLoginAsync(await RobloxApi.FinishHumanCaptchaLoginAsync(human, redemption), human.Realm, generation);
        }
        if (_captcha is not { } prompt)
            throw new UserError(L.T("Start the sign-in again."));
        string key = await RobloxApi.GetLoginCaptchaKeyAsync(prompt.Realm);
        if (generation != _loginGeneration)
            return null;
        string? token = ShowCaptcha(CaptchaForm.Arkose(prompt.Realm, key, prompt.Blob));
        if (generation != _loginGeneration || _vault is null || !ReferenceEquals(_captcha, prompt))
            return null;
        if (token is null)
        {
            _captchaUsed = true;
            return new { status = "captcha", closed = true };
        }
        _captcha = null;
        return await HandleLoginAsync(await RobloxApi.FinishCaptchaLoginAsync(prompt, token), prompt.Realm, generation);
    }

    /// <returns>What the solved puzzle handed back, or null when the window was closed first.</returns>
    string? ShowCaptcha(CaptchaForm form)
    {
        using (form)
        {
            form.ShowDialog(_owner);
            return form.Token;
        }
    }

    async Task<object?> LoginTwoStepAsync(string method, string code)
    {
        RequireVault();
        if (_twoStep is not { } prompt)
            throw new UserError(L.T("Start the sign-in again."));
        if (code.Trim().Length == 0 && prompt.VerificationToken is null)
            throw new UserError(L.T("Enter the verification code."));
        WaitForTwoStep();
        int generation = _loginGeneration;
        RobloxApi.LoginResult result;
        try
        {
            // A wrong code throws and keeps the prompt, so it can be typed again; anything else moves the sign-in on.
            // Once the code was accepted, a retry after a network hiccup skips it (it is spent) and only finishes.
            result = await RobloxApi.VerifyTwoStepAsync(prompt, TwoStepMethod(prompt, method), code.Trim());
        }
        catch (Exception ex) when (prompt.VerificationToken is not null && !IsNetworkError(ex))
        {
            // Accepted, then refused on the way: nothing is left to answer on this screen.
            if (generation == _loginGeneration && ReferenceEquals(_twoStep, prompt))
                _twoStep = null;
            throw;
        }
        if (generation != _loginGeneration)
            return null;
        _twoStep = null;
        return await HandleLoginAsync(result, prompt.Realm, generation);
    }

    /// <summary>Has Roblox send the code (again) by email or SMS, when the user asks for it.</summary>
    async Task<object?> LoginTwoStepSendAsync(string method)
    {
        RequireVault();
        if (_twoStep is not { } prompt)
            throw new UserError(L.T("Start the sign-in again."));
        string chosen = TwoStepMethod(prompt, method);
        if (chosen == "authenticator")
            throw new UserError(L.T("Enter the code from the authenticator app."));
        WaitForTwoStep();
        await RobloxApi.SendTwoStepCodeAsync(prompt, chosen);
        return new { status = "sent" };
    }

    /// <summary>The method the page picked, if it is one this account was offered; otherwise the first offered.</summary>
    static string TwoStepMethod(RobloxApi.TwoStepPrompt prompt, string method) =>
        prompt.Methods.Contains(method) ? method : prompt.Methods[0];

    /// <summary>Roblox said "too many tries" a moment ago: nothing goes to it until the minute is over.</summary>
    void WaitForTwoStep()
    {
        var left = _twoStepWaitUntil - DateTime.UtcNow;
        if (left > TimeSpan.Zero)
            throw new UserError(L.F("Too many tries. Wait {0} s, then try again.", (int)Math.Ceiling(left.TotalSeconds)));
    }

    /// <summary>Offline, a timeout or Roblox down: the request may never have been answered, so it's worth repeating.</summary>
    static bool IsNetworkError(Exception ex) =>
        ex is HttpRequestException or TaskCanceledException || ex is RobloxApiException { ServerError: true };

    /// <summary>"Try again" after the account couldn't be looked up: the session Roblox handed out is used, no new sign-in.</summary>
    async Task<object?> LoginFinishAsync()
    {
        RequireVault();
        if (_loginCookie is not { } held)
            throw new UserError(L.T("Start the sign-in again."));
        return await SaveLoginAsync(held.Cookie, held.Realm, _loginGeneration);
    }

    async Task<object?> SaveLoginAsync(string cookie, string realm, int generation)
    {
        // Held until the account is saved: a failed lookup is tried again without signing in again.
        _loginCookie = (cookie, realm);
        var user = await RobloxApi.GetAuthenticatedUserAsync(cookie, realm);
        // Locked or cancelled meanwhile: the account isn't saved (RequireVault would say so anyway when locked).
        if (generation != _loginGeneration)
            return null;
        if (user is null)
        {
            _loginCookie = null; // Roblox's answer, not a hiccup: the session is no good
            throw new UserError(L.T("Roblox didn't accept that sign-in. Please try again."));
        }
        var added = AddOrMerge(new Account
        {
            Cookie = cookie,
            Realm = realm,
            UserId = user.Id,
            Username = user.Name,
            DisplayName = user.DisplayName,
            CookieValid = true,
            LastChecked = DateTime.Now,
        });
        _loginCookie = null;
        return new { status = "added", added };
    }

    async Task<object?> AddViaCookieAsync(JsonElement a)
    {
        RequireVault();
        string cookie = RobloxApi.NormalizeCookie(Str(a, "cookie"));
        if (cookie.Length == 0)
            throw new UserError(L.T("Paste the .ROBLOSECURITY cookie."));
        var user = await RobloxApi.GetAuthenticatedUserAsync(cookie)
            ?? throw new UserError(L.T("The cookie is invalid or has expired."));
        return AddOrMerge(new Account
        {
            Cookie = cookie,
            UserId = user.Id,
            Username = user.Name,
            DisplayName = user.DisplayName,
            Alias = Str(a, "alias").Trim(),
            Group = Str(a, "group").Trim(),
            Password = Str(a, "password"),
            Note = Str(a, "note").Trim(),
            CookieValid = true,
            LastChecked = DateTime.Now,
        });
    }

    sealed record AddResult(Guid Id, string Label, bool Merged);

    /// <summary>
    /// Adding an account that is already in the list just refreshes its cookie. Roblox and Roblox VNG count user ids
    /// apart, so the same id in the other realm is another account.
    /// </summary>
    AddResult AddOrMerge(Account incoming)
    {
        var vault = RequireVault();
        string realm = Realm.Normalize(incoming.Realm);
        var existing = vault.Data.Accounts.FirstOrDefault(a => a.UserId != 0 && a.UserId == incoming.UserId && Realm.Normalize(a.Realm) == realm);
        if (existing is not null)
        {
            existing.Cookie = incoming.Cookie;
            existing.Realm = incoming.Realm;
            existing.Username = incoming.Username;
            existing.DisplayName = incoming.DisplayName;
            existing.CookieValid = incoming.CookieValid;
            existing.LastChecked = incoming.LastChecked;
            if (incoming.Password.Length > 0) existing.Password = incoming.Password;
            if (incoming.Alias.Length > 0) existing.Alias = incoming.Alias;
            if (incoming.Group.Length > 0) existing.Group = incoming.Group;
            if (incoming.Note.Length > 0) existing.Note = incoming.Note;
        }
        else
        {
            vault.Data.Accounts.Add(incoming);
        }
        var account = existing ?? incoming;
        Persist();
        PushState();
        _ = LoadAvatarsAsync([account.UserId]);
        RefreshPresenceSoon(TimeSpan.FromSeconds(2));
        return new AddResult(account.Id, NameText(account), existing is not null);
    }

    /// <summary>Imports a decrypted AccountData.json from Roblox Account Manager, then checks the cookies in the background.</summary>
    async Task<object?> ImportRamAsync()
    {
        var vault = RequireVault();
        string path;
        using (var dialog = new OpenFileDialog
               {
                   Title = L.T("Choose the decrypted AccountData.json from Roblox Account Manager"),
                   Filter = "AccountData (*.json)|*.json|*.*|*.*",
                   CheckFileExists = true,
               })
        {
            if (dialog.ShowDialog(_owner) != DialogResult.OK)
                return null;
            path = dialog.FileName;
        }

        var imported = await Task.Run(() => RamImport.Read(path));
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));

        int added = 0, updated = 0, skipped = 0;
        var touched = new List<Account>();
        foreach (var item in imported)
        {
            string cookie = RobloxApi.NormalizeCookie(item.Cookie);
            if (cookie.Length == 0)
            {
                skipped++;
                continue;
            }
            var existing = vault.Data.Accounts.FirstOrDefault(a =>
                (item.UserId != 0 && a.UserId == item.UserId && Realm.Normalize(a.Realm) == Realm.Global) || (item.UserId == 0 && a.Cookie == cookie));
            if (existing is not null)
            {
                // A sign-in that works here is newer than RAM's copy; only a dead or unchecked one is replaced.
                if (existing.Cookie != cookie && existing.CookieValid != true)
                {
                    existing.Cookie = cookie;
                    existing.CookieValid = null;
                }
                if (existing.Alias.Length == 0) existing.Alias = item.Alias;
                if (existing.Note.Length == 0) existing.Note = item.Note;
                if (existing.Password.Length == 0) existing.Password = item.Password;
                if (existing.Group.Length == 0) existing.Group = item.Group;
                touched.Add(existing);
                updated++;
                continue;
            }
            var account = new Account
            {
                Cookie = cookie,
                UserId = item.UserId,
                Username = item.Username,
                Alias = item.Alias,
                Note = item.Note,
                Password = item.Password,
                Group = item.Group,
                LastUsed = item.LastUse,
            };
            vault.Data.Accounts.Add(account);
            touched.Add(account);
            added++;
        }
        Persist();
        PushState();
        if (touched.Count > 0)
        {
            _ = LoadAvatarsAsync(touched.Select(a => a.UserId));
            _ = CheckImportedAsync(touched.Select(a => a.Id).ToArray());
        }
        return new { added, updated, skipped, total = imported.Count };
    }

    /// <summary>Imported cookies may be old, so verify them right away and report the result.</summary>
    async Task CheckImportedAsync(Guid[] ids)
    {
        try
        {
            while (_checking)
                await Task.Delay(1000);
            var vault = _vault;
            if (vault is null)
                return;
            await CheckAccountsAsync(ids);
            var checkedAccounts = vault.Data.Accounts.Where(a => ids.Contains(a.Id)).ToList();
            Toast(L.F("Imported accounts checked: {0} valid, {1} expired.",
                checkedAccounts.Count(a => a.CookieValid == true), checkedAccounts.Count(a => a.CookieValid == false)), "ok");
        }
        catch (UserError)
        {
            // Vault locked or another check started meanwhile; the user can check again from the menu.
        }
    }

    async Task<object?> UpdateAccountAsync(JsonElement a)
    {
        var vault = RequireVault();
        var account = RequireAccount(Id(a));
        // Checked before anything changes: a key with a typo is refused with what's wrong, and nothing is saved.
        // "" removes the key; no field at all keeps it.
        string? typedKey = OptStr(a, "authenticator")?.Trim();
        TotpKey? authenticator = typedKey is { Length: > 0 } ? ParseAuthenticator(typedKey) : null;
        RobloxUser? user = null;
        string? cookie = OptStr(a, "cookie") is { } raw ? RobloxApi.NormalizeCookie(raw) : null;
        if (cookie is { Length: > 0 } && cookie != account.Cookie)
        {
            user = await RobloxApi.GetAuthenticatedUserAsync(cookie, account.Realm)
                ?? throw new UserError(L.T("The new cookie is invalid or has expired."));
            // Locked (and maybe reopened) meanwhile: this account object belongs to the old vault.
            if (_vault != vault)
                throw new UserError(L.T("Bloxkeep was locked."));
        }

        // No alias sent while names are hidden: the form can't show it then, so it stays as it was.
        if (OptStr(a, "alias") is { } alias)
            account.Alias = alias.Trim();
        account.Group = Str(a, "group").Trim();
        account.Note = Str(a, "note").Trim();
        if (OptStr(a, "password") is { } password)
            account.Password = password;
        if (typedKey is not null)
            account.Authenticator = authenticator;
        if (user is not null)
        {
            account.Cookie = cookie!;
            account.UserId = user.Id;
            account.Username = user.Name;
            account.DisplayName = user.DisplayName;
            account.CookieValid = true;
            account.LastChecked = DateTime.Now;
            _ = LoadAvatarsAsync([user.Id]);
        }
        Persist();
        PushState();
        return true;
    }

    object SetUseRobloxConfig(Guid[] ids, bool enabled)
    {
        int changed = 0;
        foreach (var account in RequireVault().Data.Accounts.Where(a => ids.Contains(a.Id)))
        {
            account.UseRobloxConfig = enabled;
            changed++;
        }
        Persist();
        PushState();
        return changed;
    }

    object SetAutoRejoin(Guid[] ids, bool enabled)
    {
        int changed = 0;
        foreach (var account in RequireVault().Data.Accounts.Where(a => ids.Contains(a.Id)))
        {
            account.AutoRejoin = enabled;
            if (enabled)
                _rejoinsInARow.Remove(account.Id);
            else // a client whose preset turns auto-rejoin on, and a planned restart, still go ahead
                _rejoinQueue.RemoveAll(r => r.AccountId == account.Id && !r.Restart && !RejoinOn(account, r.PresetId));
            changed++;
        }
        Persist();
        PushState();
        return changed;
    }

    object Reveal(Guid id, string field)
    {
        var account = RequireAccount(id);
        return field switch
        {
            "password" => account.Password,
            _ => throw new UserError(L.T("Invalid field.")),
        };
    }

    object DeleteAccounts(Guid[] ids)
    {
        var set = ids.ToHashSet();
        int removed = RequireVault().Data.Accounts.RemoveAll(a => set.Contains(a.Id));
        foreach (var id in set)
        {
            _running.Remove(id);
            _rejoinsInARow.Remove(id);
            _restarts.Remove(id);
            _offlineStrikes.Remove(id);
            _userClosed.Remove(id);
            EndPlay(id, null, DateTime.UtcNow); // its stay ends now, not at the next lock
            ForgetScheduledOpen(id);
        }
        _pending.RemoveAll(p => set.Contains(p.AccountId));
        _late.RemoveWhere(l => set.Contains(l.AccountId));
        foreach (var launch in _handedOver.Keys.Where(l => set.Contains(l.AccountId)).ToList())
            _handedOver.Remove(launch);
        _closeWhenClaimed.RemoveWhere(l => set.Contains(l.AccountId));
        _rejoinQueue.RemoveAll(r => set.Contains(r.AccountId));
        Persist();
        PushState();
        return removed;
    }

    const int CheckParallelism = 6;

    /// <summary>
    /// Asks Roblox who each saved sign-in belongs to: the same read-only call the website makes on every page.
    /// A rejected sign-in means Roblox ended that session (password change, "log out everywhere", security).
    /// Six accounts are checked at a time; when Roblox asks to slow down, all checks pause and try again.
    /// </summary>
    async Task<object?> CheckAccountsAsync(Guid[] ids)
    {
        var vault = RequireVault();
        if (_checking)
            throw new UserError(L.T("Already checking — please wait."));
        var accounts = ids.Length == 0
            ? vault.Data.Accounts.ToList()
            : vault.Data.Accounts.Where(a => ids.Contains(a.Id)).ToList();

        _checking = true;
        PushState();
        int ok = 0, bad = 0, failed = 0, done = 0;
        var queue = new Queue<Account>(accounts);
        var resumeAt = DateTime.UtcNow;
        var newlyBanned = new List<Account>();

        // Every continuation runs on the UI thread, so the queue and counters need no locking.
        async Task WorkAsync()
        {
            while (_vault == vault && queue.TryDequeue(out var account))
            {
                for (int attempt = 1; ; attempt++)
                {
                    if (resumeAt > DateTime.UtcNow)
                        await Task.Delay(resumeAt - DateTime.UtcNow);
                    string cookie = account.Cookie;
                    try
                    {
                        var user = await RobloxApi.GetAuthenticatedUserAsync(cookie, account.Realm);
                        if (account.Cookie != cookie)
                            break; // signed in again meanwhile: the old cookie's answer says nothing about the new one
                        if (user is null)
                        {
                            account.CookieValid = false;
                            bad++;
                        }
                        else
                        {
                            account.CookieValid = true;
                            account.UserId = user.Id;
                            account.Username = user.Name;
                            account.DisplayName = user.DisplayName;
                            ok++;
                        }
                        account.LastChecked = DateTime.Now;
                    }
                    catch (RobloxApiException ex) when (ex.RateLimited && attempt < 4)
                    {
                        var wait = DateTime.UtcNow + TimeSpan.FromSeconds(4 * attempt);
                        if (wait > resumeAt)
                            resumeAt = wait;
                        continue;
                    }
                    catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                    {
                        failed++;
                    }
                    break;
                }
                // A ban, from the account's public profile (no cookie). No clear answer leaves it as it was. Roblox asking
                // to slow down pauses every check, like the sign-in call above.
                for (int attempt = 1; _vault == vault; attempt++)
                {
                    if (resumeAt > DateTime.UtcNow)
                        await Task.Delay(resumeAt - DateTime.UtcNow);
                    try
                    {
                        if (_vault == vault && await CheckBanAsync(account) is true)
                            newlyBanned.Add(account);
                    }
                    catch (RobloxApiException ex) when (ex.RateLimited)
                    {
                        var wait = DateTime.UtcNow + TimeSpan.FromSeconds(4 * attempt);
                        if (wait > resumeAt)
                            resumeAt = wait;
                        if (attempt < 3)
                            continue;
                    }
                    catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                    {
                    }
                    break;
                }
                Progress(L.F("Checking sign-ins… {0}/{1}", ++done, accounts.Count));
            }
        }

        try
        {
            Progress(L.F("Checking sign-ins… {0}/{1}", 0, accounts.Count));
            await Task.WhenAll(Enumerable.Range(0, Math.Min(CheckParallelism, accounts.Count)).Select(_ => WorkAsync()));
        }
        finally
        {
            _checking = false;
            Progress(null);
            Persist();
            PushState();
        }
        _ = LoadAvatarsAsync(accounts.Select(a => a.UserId));
        if (newlyBanned.Count > 0 && _vault == vault)
            NotifyBanned(newlyBanned);
        return new { ok, bad, failed, banned = accounts.Count(a => a.Banned == true) };
    }

    object Copy(Guid id, string field)
    {
        var account = RequireAccount(id);
        var now = DateTimeOffset.UtcNow;
        (string value, bool secret, string missing) = field switch
        {
            "username" => (account.Username, false, "This account has no saved username."),
            "password" => (account.Password, true, "This account has no saved password."),
            // The authenticator code of the moment (never the key); off the clipboard again like a password.
            "totp" => (account.Authenticator is { } key ? Totp.Code(key, now) : "", true, "This account has no authenticator key."),
            // The client's process id, for other tools that pick a window by it.
            "pid" => (_running.TryGetValue(id, out var run) ? run.Pid.ToString() : "", false, "This account has no Roblox window open."),
            _ => throw new UserError(L.T("Invalid field.")),
        };
        if (value.Length == 0)
            throw new UserError(L.T(missing));
        if (!Ui.CopyToClipboard(value, secret))
            throw new UserError(L.T("Couldn't access the clipboard; try again."));
        // A code says how long it still works.
        return new { secret, seconds = field == "totp" ? Totp.SecondsLeft(account.Authenticator!, now) : (int?)null };
    }

    async Task LoadAvatarsAsync(IEnumerable<long> userIds)
    {
        var missing = userIds.Where(id => id > 0 && !_avatarUrls.ContainsKey(id)).Distinct().ToList();
        try
        {
            if (missing.Count == 0)
                return;
            foreach (var (id, url) in await RobloxApi.GetHeadshotUrlsAsync(missing))
                _avatarUrls[id] = url;
            PushState();
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
        }
        finally
        {
            if (!_bootAvatarsDone)
            {
                _bootAvatarsDone = true;
                AdvanceBoot();
            }
        }
    }

    /// <summary>A step of the loading screen and how far along it is (0–1); the page shows it until it is null.</summary>
    sealed record BootProgress(string Stage, double Percent);

    /// <summary>
    /// Moves the loading screen to the first step still running: Roblox status (30 % → 80 %), then avatars
    /// (80 % → 100 %), then done.
    /// </summary>
    void AdvanceBoot(bool push = true)
    {
        if (_boot is null)
            return;
        _boot = !_bootStatusDone ? new BootProgress("status", 0.3)
            : !_bootAvatarsDone ? new BootProgress("avatars", 0.8)
            : null;
        if (push)
            PushState();
    }

    // ───────────────────────────── games ─────────────────────────────

    static readonly TimeSpan GameRetry = TimeSpan.FromMinutes(2);
    /// <summary>The longest a place Roblox keeps answering without (unpublished, private) waits before it is asked again.</summary>
    static readonly TimeSpan MaxGameRetry = TimeSpan.FromHours(1);
    /// <summary>Places looked up so far; the ones looked up longest ago go first beyond this.</summary>
    const int MaxCachedGames = 1000;
    /// <summary>Lookups in a row that Roblox answered without the place: each one doubles the wait before the next.</summary>
    readonly Dictionary<long, int> _gameMisses = new();

    /// <summary>Whether the cache can answer for a place; <paramref name="fresh"/> wants a live player count.</summary>
    bool HasGame(long placeId, bool fresh) =>
        _games.TryGetValue(placeId, out var cached)
        && ((cached is not null && !fresh) || DateTime.UtcNow - _gameFetchedAt.GetValueOrDefault(placeId) < GameRetryFor(placeId, cached, fresh));

    /// <summary>
    /// How long a lookup holds: two minutes, and for a place Roblox answered without (asked again by every status
    /// check and process tick while a client is in it) 2, 4, 8 … up to 60 minutes. A lookup the person asked for
    /// (fresh) waits two minutes at most.
    /// </summary>
    TimeSpan GameRetryFor(long placeId, GameInfo? cached, bool fresh = false) =>
        !fresh && cached is null && _gameMisses.TryGetValue(placeId, out int misses)
            ? TimeSpan.FromTicks(Math.Min(MaxGameRetry.Ticks, GameRetry.Ticks << Math.Min(misses - 1, 6)))
            : GameRetry;

    /// <summary>
    /// Fills the game cache for many places with one batch of requests. Presence already says which universe a
    /// place belongs to; other places are looked up first. Failed lookups are retried after two minutes.
    /// </summary>
    async Task LoadGamesAsync(IEnumerable<(long PlaceId, long UniverseId)> places, bool fresh = false)
    {
        var wanted = places.Where(p => p.PlaceId > 0 && !HasGame(p.PlaceId, fresh)).DistinctBy(p => p.PlaceId).ToList();
        if (wanted.Count == 0)
            return;
        var now = DateTime.UtcNow;
        if (_games.Count > MaxCachedGames)
        {
            foreach (long old in _games.Keys.OrderBy(id => _gameFetchedAt.GetValueOrDefault(id)).Take(_games.Count - MaxCachedGames / 2).ToList())
            {
                _games.Remove(old);
                _gameFetchedAt.Remove(old);
                _gameMisses.Remove(old);
            }
        }
        foreach (var (placeId, _) in wanted)
            _gameFetchedAt[placeId] = now;
        try
        {
            // Roblox has no batch lookup from place to universe, so the unknown ones are asked a few at a time.
            var universeOf = new Dictionary<long, long>();
            using (var gate = new SemaphoreSlim(4))
            {
                var lookups = await Task.WhenAll(wanted.Select(async w =>
                {
                    long known = w.UniverseId > 0 ? w.UniverseId : _games.GetValueOrDefault(w.PlaceId)?.UniverseId ?? 0;
                    if (known > 0)
                        return (w.PlaceId, known);
                    await gate.WaitAsync();
                    try
                    {
                        return (w.PlaceId, await RobloxApi.GetUniverseIdAsync(w.PlaceId));
                    }
                    finally
                    {
                        gate.Release();
                    }
                }));
                foreach (var (placeId, universeId) in lookups)
                    universeOf[placeId] = universeId;
            }
            var games = await RobloxApi.GetGamesAsync(universeOf.Values, AnyGlobalCookie());
            foreach (var (placeId, universeId) in universeOf)
            {
                // A missing answer (a request failed) keeps what was known before instead of forgetting the name.
                _games[placeId] = games.TryGetValue(universeId, out var info) ? info with { PlaceId = placeId } : _games.GetValueOrDefault(placeId);
                if (_games[placeId] is null)
                    _gameMisses[placeId] = _gameMisses.GetValueOrDefault(placeId) + 1;
                else
                    _gameMisses.Remove(placeId);
            }
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            foreach (var (placeId, _) in wanted)
                _games.TryAdd(placeId, null);
        }
    }

    /// <summary>A sign-in for roblox.com's own services (a Roblox VNG one only works there), for calls any account can make.</summary>
    string? AnyGlobalCookie() =>
        _vault?.Data.Accounts.FirstOrDefault(a => a.CookieValid != false && a.Cookie.Length > 0 && a.Realm == Realm.Global)?.Cookie;

    async Task<GameInfo?> GetGameInfoAsync(long placeId, bool fresh = false)
    {
        await LoadGamesAsync([(placeId, 0)], fresh);
        return _games.GetValueOrDefault(placeId);
    }

    static readonly TimeSpan ServerListLifetime = TimeSpan.FromSeconds(30);
    readonly Dictionary<(long, bool, string), (object Result, DateTime At)> _serverPages = new();

    /// <summary>
    /// A page of a game's public servers. Pages are kept for 30 seconds (switching the order back and forth doesn't ask
    /// Roblox again), and when Roblox asks to slow down the page says so (busy) so the window can retry by itself.
    /// </summary>
    async Task<object?> ServersAsync(long placeId, bool fewestFirst, string? cursor)
    {
        if (placeId <= 0)
            throw new UserError(L.T("Invalid Place ID."));
        var key = (placeId, fewestFirst, cursor ?? "");
        if (_serverPages.TryGetValue(key, out var kept) && DateTime.UtcNow - kept.At < ServerListLifetime)
            return kept.Result;
        try
        {
            var (servers, next) = await RobloxApi.GetServersAsync(placeId, fewestFirst, cursor);
            object result = new { servers, next, busy = false };
            if (_serverPages.Count > 40)
                _serverPages.Clear();
            _serverPages[key] = (result, DateTime.UtcNow);
            return result;
        }
        catch (RobloxApiException ex) when (ex.RateLimited)
        {
            return new { servers = Array.Empty<ServerInfo>(), next = (string?)null, busy = true };
        }
    }

    static readonly TimeSpan FriendServerLifetime = TimeSpan.FromSeconds(45);
    /// <summary>A friend found in a game: who, where, and which of your accounts saw them (named when shown, so hiding names applies).</summary>
    sealed record FriendFound(long UserId, string Name, string DisplayName, string? AvatarUrl, string JobId, long PlaceId, Guid Via);
    /// <summary>Kept for a moment per game and per accounts asked (another selection asks other accounts).</summary>
    readonly Dictionary<(long PlaceId, string Viewers), (List<FriendFound> Friends, int Asked, DateTime At)> _friendServers = new();
    /// <summary>Who each account's friends are changes rarely; where they are is asked afresh.</summary>
    static readonly TimeSpan FriendIdsLifetime = TimeSpan.FromMinutes(10);
    readonly Dictionary<long, (List<long> Ids, DateTime At)> _friendIds = new();

    async Task<List<long>> FriendIdsAsync(long userId)
    {
        if (_friendIds.TryGetValue(userId, out var kept) && DateTime.UtcNow - kept.At < FriendIdsLifetime)
            return kept.Ids;
        var ids = await RobloxApi.GetFriendIdsAsync(userId);
        _friendIds[userId] = (ids, DateTime.UtcNow);
        return ids;
    }

    /// <summary>
    /// Friends of your accounts who are playing this game, with the server they are in. Each account's friends are
    /// asked about with that account's own cookie: Roblox only tells a friend the server when that person's privacy
    /// settings let them join. At most 3 accounts are used (the selected ones first) to keep the requests few.
    /// </summary>
    async Task<object?> FriendServersAsync(long placeId, Guid[] preferred)
    {
        var vault = RequireVault();
        if (placeId <= 0)
            throw new UserError(L.T("Invalid Place ID."));
        var mine = vault.Data.Accounts.Select(a => a.UserId).ToHashSet();
        var viewers = vault.Data.Accounts
            .Where(a => a.UserId > 0 && a.CookieValid != false && a.Cookie.Length > 0 && a.Realm == Realm.Global)
            .OrderBy(a => preferred.Contains(a.Id) ? 0 : 1)
            .Take(3)
            .ToList();
        var key = (placeId, string.Join(",", viewers.Select(v => v.Id)));
        if (_friendServers.TryGetValue(key, out var kept) && DateTime.UtcNow - kept.At < FriendServerLifetime)
            return FriendServersDto(kept.Friends, kept.Asked);
        var found = new Dictionary<long, (string JobId, long PlaceId, Guid Via)>();
        await Task.WhenAll(viewers.Select(async viewer =>
        {
            try
            {
                var friends = (await FriendIdsAsync(viewer.UserId)).Where(id => !mine.Contains(id)).ToList();
                if (friends.Count == 0)
                    return;
                foreach (var (id, p) in await RobloxApi.GetPresenceAsync(viewer.Cookie, friends))
                {
                    if (p.Type == 2 && p.JobId is { Length: > 0 } job && (p.PlaceId == placeId || p.RootPlaceId == placeId))
                    {
                        lock (found)
                            found.TryAdd(id, (job, p.PlaceId ?? placeId, viewer.Id));
                    }
                }
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                // That account's friends are just left out.
            }
        }));
        var ids = found.Keys.ToList();
        // Names and pictures are extras: without them the friends still show (by id) and can be joined.
        Dictionary<long, RobloxUser> users = [];
        Dictionary<long, string> faces = [];
        try
        {
            if (ids.Count > 0)
            {
                var named = RobloxApi.GetUsersAsync(ids);
                var pictured = RobloxApi.GetHeadshotUrlsAsync(ids);
                users = await named;
                faces = await pictured;
            }
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
        }
        var friends = found
            // The server belongs to this place, which can be another place of the game than the one asked.
            .Select(f => new FriendFound(f.Key, users.GetValueOrDefault(f.Key)?.Name ?? "", users.GetValueOrDefault(f.Key)?.DisplayName ?? "",
                faces.GetValueOrDefault(f.Key), f.Value.JobId, f.Value.PlaceId, f.Value.Via))
            .OrderBy(f => f.DisplayName, StringComparer.CurrentCultureIgnoreCase)
            .ToList();
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));
        if (_friendServers.Count > 20)
            _friendServers.Clear();
        _friendServers[key] = (friends, viewers.Count, DateTime.UtcNow);
        return FriendServersDto(friends, viewers.Count);
    }

    /// <summary>The friends as the page gets them, with the account that saw each named as it reads now.</summary>
    object FriendServersDto(List<FriendFound> friends, int asked) => new
    {
        asked,
        friends = friends.Select(f => new
        {
            userId = f.UserId,
            name = f.Name,
            displayName = f.DisplayName,
            avatarUrl = f.AvatarUrl,
            jobId = f.JobId,
            placeId = f.PlaceId,
            via = FindAccount(f.Via) is { } account ? NameText(account) : "",
        }).ToList(),
    };

    async Task<object?> ResolveGameAsync(string input)
    {
        if (!GameTarget.TryParse(input, "", out var target, out string error))
            return new { ok = false, error };
        if (target!.ShareCode is not null)
            return new { ok = true, kind = "share" };
        var info = await GetGameInfoAsync(target.PlaceId, fresh: true);
        string kind = target.LinkCode is not null ? "private" : target.JobId is not null ? "server" : "game";
        return new
        {
            ok = true,
            kind,
            placeId = target.PlaceId,
            name = info?.Name,
            creator = info?.Creator,
            playing = info?.Playing,
            iconUrl = info?.IconUrl,
        };
    }

    void RememberGame(long placeId)
    {
        var recent = RequireVault().Data.Settings.RecentGames;
        recent.RemoveAll(g => g.PlaceId == placeId);
        var info = _games.GetValueOrDefault(placeId);
        recent.Insert(0, new RecentGame { PlaceId = placeId, UniverseId = info?.UniverseId ?? 0, Name = info?.Name ?? "" });
        if (recent.Count > 8)
            recent.RemoveRange(8, recent.Count - 8);
    }

    async Task LoadRecentGamesAsync()
    {
        var vault = _vault;
        if (vault is null)
            return;
        bool changed = false;
        await LoadGamesAsync(vault.Data.Settings.RecentGames.Select(g => (g.PlaceId, g.UniverseId)).ToList());
        foreach (var game in vault.Data.Settings.RecentGames)
        {
            if (_games.GetValueOrDefault(game.PlaceId) is not { Name.Length: > 0 } info)
                continue;
            if (info.Name != game.Name || game.UniverseId != info.UniverseId)
            {
                game.Name = info.Name;
                game.UniverseId = info.UniverseId;
                changed = true;
            }
        }
        if (_vault != vault)
            return;
        if (changed)
            Persist();
        PushState();
    }

    // ───────────────────────────── game picker ─────────────────────────────

    static readonly TimeSpan GameListLifetime = TimeSpan.FromMinutes(10);
    readonly Dictionary<long, string> _iconByUniverse = new();
    (List<GameSummary> Games, DateTime At)? _favoriteGames, _popularGames;

    /// <summary>Icons not known yet, in one request per 100 games.</summary>
    async Task FetchIconsAsync(IEnumerable<GameSummary> games)
    {
        if (_iconByUniverse.Count > MaxCachedGames)
            _iconByUniverse.Clear(); // a link per game ever shown; the ones on screen are asked for again
        var missing = games.Select(g => g.UniverseId).Where(id => id > 0 && !_iconByUniverse.ContainsKey(id)).Distinct().ToList();
        if (missing.Count == 0)
            return;
        try
        {
            foreach (var (id, url) in await RobloxApi.GetGameIconsAsync(missing))
                _iconByUniverse[id] = url;
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            // Rows still work without icons.
        }
    }

    /// <param name="fetchIcons">False for search results: they show at once, and the page asks for the icons after.</param>
    async Task<List<object>> GameRowsAsync(IEnumerable<GameSummary> games, bool fetchIcons = true)
    {
        var list = games.GroupBy(g => g.PlaceId).Select(g => g.First()).ToList();
        if (fetchIcons)
            await FetchIconsAsync(list);
        var pinned = _vault?.Data.Settings.PinnedGames.Select(p => p.PlaceId).ToHashSet() ?? [];
        return list.Select(g => (object)new
        {
            g.PlaceId,
            g.UniverseId,
            g.Name,
            g.Playing,
            iconUrl = _iconByUniverse.GetValueOrDefault(g.UniverseId),
            pinned = pinned.Contains(g.PlaceId),
        }).ToList();
    }

    async Task<object?> SearchGamesAsync(string query)
    {
        query = query.Trim();
        if (query.Length < 2)
            return Array.Empty<object>();
        return await GameRowsAsync((await RobloxApi.SearchGamesAsync(query)).Take(12), fetchIcons: false);
    }

    /// <summary>Icons for games already on screen (search results), keyed by universe id.</summary>
    async Task<object?> GameIconsAsync(JsonElement a)
    {
        var ids = a.TryGetProperty("universeIds", out var arr) && arr.ValueKind == JsonValueKind.Array
            ? arr.EnumerateArray().Select(x => x.TryGetInt64(out long v) ? v : 0).Where(v => v > 0).Distinct().Take(50).ToList()
            : [];
        if (_iconByUniverse.Count > MaxCachedGames)
            _iconByUniverse.Clear(); // a link per game ever shown; the ones on screen are asked for again
        var missing = ids.Where(id => !_iconByUniverse.ContainsKey(id)).ToList();
        if (missing.Count > 0)
        {
            try
            {
                foreach (var (id, url) in await RobloxApi.GetGameIconsAsync(missing))
                    _iconByUniverse[id] = url;
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                // The rows keep their placeholder.
            }
        }
        return ids.Where(_iconByUniverse.ContainsKey).ToDictionary(id => id.ToString(), id => _iconByUniverse[id]);
    }

    /// <summary>Recent and pinned games plus, cached for a while, the accounts' Roblox favourites and the popular chart.</summary>
    async Task<object?> GameListsAsync()
    {
        var vault = RequireVault();
        // Favourites (first 5 accounts) and the popular list are fetched together, not one after another.
        async Task<List<GameSummary>> Favorites()
        {
            var lists = await Task.WhenAll(vault.Data.Accounts.Where(a => a.UserId > 0).Take(5).Select(async account =>
            {
                try
                {
                    return await RobloxApi.GetFavoriteGamesAsync(account.UserId);
                }
                catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                {
                    return [];
                }
            }));
            return lists.SelectMany(l => l).GroupBy(g => g.PlaceId).Select(g => g.First()).Take(30).ToList();
        }
        async Task<List<GameSummary>> Popular()
        {
            try
            {
                return (await RobloxApi.GetPopularGamesAsync(AnyGlobalCookie())).Take(30).ToList();
            }
            catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
            {
                return [];
            }
        }
        var now = DateTime.UtcNow;
        var favTask = _favoriteGames is { } fav && now - fav.At <= GameListLifetime ? Task.FromResult(fav.Games) : Favorites();
        var popTask = _popularGames is { } pop && now - pop.At <= GameListLifetime ? Task.FromResult(pop.Games) : Popular();
        await Task.WhenAll(favTask, popTask);
        _favoriteGames = (favTask.Result, _favoriteGames is { } f && f.Games == favTask.Result ? f.At : now);
        _popularGames = (popTask.Result, _popularGames is { } p && p.Games == popTask.Result ? p.At : now);

        var s = vault.Data.Settings;
        var recent = s.RecentGames.Select(g => new GameSummary(g.PlaceId, g.UniverseId, g.Name.Length > 0 ? g.Name : $"Place {g.PlaceId}", 0)).ToList();
        var pinned = s.PinnedGames.Select(g => new GameSummary(g.PlaceId, g.UniverseId, g.Name, 0)).ToList();
        // Every list's missing icons in one go; the rows below then find them all cached.
        await FetchIconsAsync([.. recent, .. pinned, .. favTask.Result, .. popTask.Result]);
        return new
        {
            recent = await GameRowsAsync(recent),
            pinned = await GameRowsAsync(pinned),
            favorites = await GameRowsAsync(favTask.Result),
            popular = await GameRowsAsync(popTask.Result),
        };
    }

    object TogglePin(long placeId, long universeId, string name)
    {
        var pinned = RequireVault().Data.Settings.PinnedGames;
        bool nowPinned = pinned.RemoveAll(g => g.PlaceId == placeId) == 0;
        if (nowPinned && placeId > 0)
        {
            pinned.Insert(0, new RecentGame { PlaceId = placeId, UniverseId = universeId, Name = name.Trim() });
            if (pinned.Count > 30)
                pinned.RemoveRange(30, pinned.Count - 30);
        }
        Persist();
        PushState();
        return nowPinned;
    }

    object? RemoveRecent(long placeId)
    {
        RequireVault().Data.Settings.RecentGames.RemoveAll(g => g.PlaceId == placeId);
        Persist();
        PushState();
        return null;
    }

    // ───────────────────────────── launching ─────────────────────────────

    static string MultiNotReadyText()
    {
        string text = L.T("Multi-Roblox isn't working yet: Roblox was already open before it was turned on.");
        if (MultiInstance.FindTrayAutostart() is { Enabled: true })
            text += " " + L.T("Roblox also starts by itself in the background when Windows starts.");
        return text + " " + L.T("A new Roblox window may close the old one.");
    }

    static string FormatMb(long mb) => mb >= 1024 ? $"{mb / 1024.0:0.#} GB" : $"{mb} MB";

    async Task<object?> LaunchAsync(JsonElement a)
    {
        var vault = RequireVault();
        var accounts = InLaunchOrder(vault, Ids(a)); // several selected: the launch order from the ⋯ menu
        CheckCanLaunch(accounts);
        bool appOnly = Bool(a, "appOnly");

        var s = vault.Data.Settings;
        s.LastPlaceInput = Str(a, "placeInput").Trim();
        s.LastJobId = Str(a, "jobId").Trim();
        GameTarget? target = null;
        if (!appOnly && !GameTarget.TryParse(s.LastPlaceInput, s.LastJobId, out target, out string error))
            throw new UserError(error);
        return await LaunchAccountsAsync(vault, accounts, target, Bool(a, "force"), s.LaunchDelaySeconds);
    }

    void CheckCanLaunch(List<Account> accounts)
    {
        if (_launchCts is not null)
            throw new UserError(L.T("Already launching — wait a moment or press Stop."));
        if (accounts.Count == 0)
            throw new UserError(L.T("Select at least one account."));
        // VNG accounts need the Roblox VNG client, whose launch hasn't been tried yet. Mixed with others they are left
        // out with a line in the result (LaunchAccountsAsync), so one of them doesn't stop the whole launch.
        if (accounts.All(acc => acc.Realm == Realm.Vng))
            throw new UserError(string.Join("\n", accounts.Select(VngNotOpened)));
    }

    string VngNotOpened(Account account) => L.F("{0} is a Roblox VNG account. Opening Roblox VNG isn't supported yet.", Name(account));

    string BannedNotOpened(Account account) =>
        L.F("{0} is banned on Roblox, so it wasn't opened. If the ban is over, Check sign-in clears this.", Name(account));

    /// <summary>
    /// Opens the accounts one after another (the dock's Play and presets); callers run <see cref="CheckCanLaunch"/>
    /// first. Asks for a confirmation (without <paramref name="force"/>) when Multi-Roblox or free RAM look wrong.
    /// </summary>
    /// <param name="pickServer">Chooses the server once the launch has started, so Stop works meanwhile; gets how many accounts will open.</param>
    /// <param name="presetId">The preset being run: the clients it opens follow its options.</param>
    async Task<LaunchResult> LaunchAccountsAsync(Vault vault, List<Account> accounts, GameTarget? target, bool force, int delaySeconds,
        Func<GameTarget, int, CancellationToken, Task<GameTarget>>? pickServer = null, Guid? presetId = null)
    {
        // Accounts Roblox reports banned (the account checks read it) aren't opened: their window would only show the
        // ban. Nor are Roblox VNG accounts (see CheckCanLaunch). Said with the result, like an account that failed to
        // open; with nothing else to open, said at once.
        var leftOut = accounts.Where(acc => acc.Realm == Realm.Vng || acc.Banned == true).ToList();
        var leftOutText = leftOut.Select(acc => acc.Realm == Realm.Vng ? VngNotOpened(acc) : BannedNotOpened(acc)).ToList();
        if (leftOut.Count > 0)
        {
            if (leftOut.Count == accounts.Count)
                throw new UserError(string.Join("\n", leftOutText));
            accounts = accounts.Except(leftOut).ToList();
        }
        // The slot is taken before the first await: a second Play (or preset) meanwhile would pass CheckCanLaunch,
        // and its own "switching" list could close the client this one had just opened.
        var cts = _launchCts = new CancellationTokenSource();
        var ct = cts.Token;
        void Release()
        {
            if (_launchCts == cts)
                _launchCts = null;
            cts.Dispose();
        }

        var s = vault.Data.Settings;
        var warnings = new List<string>();
        var switching = new List<Account>();
        var errors = leftOutText;
        // Accounts left as they were (already open where asked): not failures.
        var skipped = new List<string>();
        // Clients open here that a new client closes when Multi-Roblox isn't ready: not drops to rejoin from.
        var replaced = new List<Guid>();
        int launched = 0;
        // Past the checks and warnings: from then on an error is reported with the result rather than thrown.
        bool started = false;
        // One try from the moment the slot is taken: whatever happens, the slot is freed and the progress text cleared.
        try
        {
            // A share link only names the private server once Roblox has resolved it; clients already in that
            // server are compared by its link code, so it is resolved before deciding who switches.
            if (target?.ShareCode is { } shareCode)
            {
                Progress(L.T("Reading the private server link…"));
                var (placeId, linkCode) = await ResolveShareAsync(accounts, shareCode, ct);
                target = target with { PlaceId = placeId, LinkCode = linkCode, ShareCode = null };
                Progress(null);
            }
            // Things worth a second look before opening more windows.
            var players = RobloxProcesses.GetPlayers();
            if (s.MultiInstance && !MultiInstance.Armed && players.Count == 0)
                MultiInstance.Enable();
            // Accounts already playing here: left alone in the same game, switched over when another game (or server)
            // is chosen. Roblox kicks a second client of the same account, so the old one is closed first.
            if (target is null)
            {
                // "Open Roblox" (home screen): clients in a game go back to the home screen; ones already there stay.
                switching = accounts.Where(acc => _running.TryGetValue(acc.Id, out var run) && InGameHere(run)).ToList();
                if (switching.Count > 0)
                    warnings.Add(L.F("{0} will leave the game and go to the Roblox home screen.", Names(switching)));
            }
            else if (accounts.Any(acc => _running.ContainsKey(acc.Id)))
            {
                await LoadGamesAsync(accounts.Where(acc => _running.ContainsKey(acc.Id)).Select(acc => (CurrentPlace(acc.Id), 0L))
                    .Append((target.PlaceId, 0L)).Where(p => p.Item1 > 0).ToList());
                switching = accounts.Where(acc => _running.TryGetValue(acc.Id, out var run) && NeedsSwitch(run, target)).ToList();
                if (switching.Count > 0)
                    warnings.Add(L.F("{0} will leave what they're playing now and open this game instead.",
                        Names(switching)));
            }
            bool robloxOpen = RobloxProcesses.VisibleWindows(players.Select(p => p.Pid)).Values.Any(list => list.Count > 0);
            if ((accounts.Count > 1 || robloxOpen) && !MultiInstance.Armed)
                warnings.Add(s.MultiInstance ? MultiNotReadyText() : L.T("Multi-Roblox is off, so each new Roblox window closes the previous one."));
            int toOpen = accounts.Count(acc => !_running.ContainsKey(acc.Id));
            var memory = SystemMemory.Read();
            long perClientMb = players.Count > 0 ? Math.Max(400, (long)players.Average(p => p.WorkingSet) / MB) : DefaultClientMb;
            long neededMb = toOpen * perClientMb;
            if (s.MemoryWarnings && memory.TotalMb > 0 && memory.AvailableMb - neededMb < memory.TotalMb * 8 / 100)
                warnings.Add(L.P(toOpen, "Only {0} of RAM is free, and {1} more Roblox window needs about {2}. The PC may slow down or Roblox may crash.",
                    "Only {0} of RAM is free, and {1} more Roblox windows need about {2}. The PC may slow down or Roblox may crash.",
                    FormatMb(memory.AvailableMb), toOpen, FormatMb(neededMb)));
            if (warnings.Count > 0 && !force)
                return new LaunchResult(NeedsConfirm: true, Message: string.Join("\n\n", warnings) + "\n\n" + L.T("Open anyway?"));

            started = true;
            PushState();
            await WaitForSweepAsync(ct);
            if (pickServer is not null && target is not null)
                target = await pickServer(target, accounts.Count(acc => (!_running.ContainsKey(acc.Id) || switching.Contains(acc))
                                                                         && !StillOpening(acc.Id)), ct);

            for (int i = 0; i < accounts.Count && _vault == vault; i++)
            {
                ct.ThrowIfCancellationRequested();
                var account = accounts[i];
                await WaitForPendingAsync(ct);
                if (FindAccount(account.Id) is null)
                    continue; // deleted while the earlier ones opened
                if (switching.Contains(account) && _running.TryGetValue(account.Id, out var old))
                {
                    Progress(L.F("Closing {0}'s Roblox to switch games…", Name(account)));
                    _userClosed.Add(account.Id); // not a disconnect: no auto-rejoin into the old game
                    RobloxProcesses.Close(old.Pid, InGameNow(old.Pid));
                    try
                    {
                        // The process check ends the session once the client is gone; only then can the account open again.
                        for (int waited = 0; _running.ContainsKey(account.Id) && waited < 15_000; waited += 300)
                            await Task.Delay(300, ct);
                    }
                    finally
                    {
                        // Still open (it wouldn't close, or Stop was pressed): it stays this account's client, and a
                        // real drop later must still rejoin.
                        if (_running.ContainsKey(account.Id))
                            _userClosed.Remove(account.Id);
                    }
                }
                if (_running.ContainsKey(account.Id) || StillOpening(account.Id))
                {
                    bool stayed = _running.ContainsKey(account.Id) && !switching.Contains(account);
                    skipped.Add(stayed && target is null ? L.F("{0}: already on the home screen — skipped.", Name(account))
                        : stayed ? L.F("{0}: already in this game — skipped.", Name(account))
                        : L.F("{0}: already running — skipped.", Name(account)));
                    continue;
                }

                Progress(L.F("Launching {0} ({1}/{2})…", Name(account), i + 1, accounts.Count));
                var startedAt = DateTime.UtcNow;
                if (!MultiInstance.Armed)
                {
                    foreach (var id in _running.Keys)
                    {
                        if (id != account.Id && _userClosed.Add(id))
                            replaced.Add(id);
                    }
                }
                try
                {
                    await StartSessionAsync(account, target, s.LaunchVia, ct, presetId: presetId);
                    launched++;
                }
                catch (Exception ex) when (!ct.IsCancellationRequested
                                           && (RobloxApi.IsExpectedError(ex) || ex is Win32Exception or FileNotFoundException or UserError))
                {
                    errors.Add($"{NameText(account)}: {(ex is Win32Exception or FileNotFoundException ? ex.Message : ErrorText(ex))}");
                    if (ex is RobloxApiException { CookieInvalid: true })
                        account.CookieValid = false;
                }
                PushState();

                if (i < accounts.Count - 1)
                    await WaitBeforeNextLaunchAsync(account, startedAt, delaySeconds, ct);
            }
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested)
        {
            // Stop: a cancelled result, also while the link was still being read.
        }
        catch (Exception ex) when (started && RobloxApi.IsExpectedError(ex))
        {
            errors.Add(ErrorText(ex));
        }
        finally
        {
            Release();
            if (started)
            {
                _ = ForgetReplacedAsync(replaced);
                if (launched > 0 && target is not null && _vault == vault)
                {
                    RememberGame(target.PlaceId);
                    _ = LoadRecentGamesAsync();
                }
                if (launched > 0)
                    RefreshPresenceSoon(TimeSpan.FromSeconds(20));
                if (launched > 0 && s.MultiInstance && s.MultiInstanceTile)
                    _ = TileWhenOpenAsync();
                Persist();
            }
            Progress(null);
            PushState();
        }
        // The page shows the details; while the window is in the background a short notification says so.
        if (errors.Count > 0 && !ct.IsCancellationRequested)
            Notify(L.P(errors.Count, "{0} account didn't open.", "{0} accounts didn't open.", errors.Count), "warn", "launch", toast: false);
        return new LaunchResult(Launched: launched, Errors: errors, Cancelled: ct.IsCancellationRequested, Skipped: skipped);
    }

    /// <summary>A launch of this account that is still waiting for its window (not one the person closed meanwhile).</summary>
    bool StillOpening(Guid accountId) => _pending.Any(p => p.AccountId == accountId && !_closeWhenClaimed.Contains(p));

    /// <summary>How long after its window a new client may still be closing the one it replaces.</summary>
    static readonly TimeSpan ReplacedGrace = TimeSpan.FromSeconds(15);

    /// <summary>
    /// Without Multi-Roblox the new client closes the ones open here — but only once it is up itself, after the launch
    /// has returned. Those stay "closed on purpose" (no auto-rejoin) until the new client has had its window a while;
    /// one still open by then wasn't replaced after all, so a real drop later must rejoin it again.
    /// </summary>
    async Task ForgetReplacedAsync(List<Guid> replaced)
    {
        if (replaced.Count == 0)
            return;
        var deadline = DateTime.UtcNow + PendingTimeout + ReplacedGrace;
        DateTime? settled = null;
        while (!_disposed && replaced.Any(_running.ContainsKey) && DateTime.UtcNow < deadline)
        {
            settled = _pending.Count == 0 ? settled ?? DateTime.UtcNow : null;
            if (settled is { } since && DateTime.UtcNow - since >= ReplacedGrace)
                break;
            await Task.Delay(1000);
        }
        foreach (var id in replaced.Where(_running.ContainsKey))
            _userClosed.Remove(id);
    }

    /// <summary>
    /// Resolves a share link with the first of these accounts whose sign-in Roblox accepts; one it rejects is marked
    /// as signed out (once a second call agrees) and the next one tried, so a dead first account doesn't stop the
    /// whole launch.
    /// </summary>
    async Task<(long PlaceId, string LinkCode)> ResolveShareAsync(List<Account> accounts, string shareCode, CancellationToken ct)
    {
        var candidates = accounts.Where(acc => acc.CookieValid != false).Concat(accounts).Distinct().ToList();
        for (int i = 0; ; i++)
        {
            try
            {
                return await RobloxApi.ResolvePrivateServerShareAsync(candidates[i].Cookie, shareCode, ct);
            }
            catch (RobloxApiException ex) when (ex.CookieInvalid && i < candidates.Count - 1)
            {
                await ConfirmSignedOutAsync(candidates[i], ct);
            }
        }
    }

    /// <summary>
    /// Waits until the client just started has its window, then for the rest of the launch delay. When Roblox has
    /// an update, its launcher downloads it before the window appears; starting more launchers meanwhile would make
    /// them all download at once and fight over the install.
    /// </summary>
    async Task WaitBeforeNextLaunchAsync(Account account, DateTime startedAt, int delaySeconds, CancellationToken ct)
    {
        while (_pending.Any(p => p.AccountId == account.Id) && DateTime.UtcNow - startedAt < PendingTimeout)
        {
            if (DateTime.UtcNow - startedAt > TimeSpan.FromSeconds(Math.Max(10, delaySeconds)))
            {
                var version = RobloxVersionDto();
                Progress(version.UpToDate == false
                    ? L.F("Waiting for Roblox to open {0}… (Roblox is updating to {1})", Name(account), version.Latest)
                    : L.F("Waiting for Roblox to open {0}… (Roblox may be updating)", Name(account)));
            }
            await Task.Delay(500, ct);
        }
        var rest = TimeSpan.FromSeconds(delaySeconds) - (DateTime.UtcNow - startedAt);
        if (rest > TimeSpan.Zero)
        {
            Progress(L.F("Waiting {0}s before the next account…", (int)Math.Ceiling(rest.TotalSeconds)));
            await Task.Delay(rest, ct);
        }
    }

    /// <summary>The place a client here is in (its log), else the one it was opened with; 0 when unknown.</summary>
    long CurrentPlace(Guid accountId) =>
        _running.TryGetValue(accountId, out var run) ? _logs.StateOf(run.Pid)?.PlaceId ?? run.Launch.Target?.PlaceId ?? 0 : 0;

    /// <summary>
    /// A client in a game answers the window's close button with "leave the game?" instead of closing (seen
    /// 2026-10-13), so closing it from Bloxkeep ends it straight away; one on the home screen is closed normally.
    /// </summary>
    bool InGameNow(int pid) => _logs.StateOf(pid)?.Status is "ingame" or "joining";

    /// <summary>In a game (or joining one), as opposed to the home screen; without a log, by what it was opened with.</summary>
    bool InGameHere((int Pid, PendingLaunch Launch) run) =>
        _logs.StateOf(run.Pid) is { } log ? log.Status is "ingame" or "joining" : run.Launch.Target is not null;

    /// <summary>Two places of the same game (a game moves players between its places) count as the same.</summary>
    bool SameGame(long a, long b) =>
        a == b || (_games.GetValueOrDefault(a)?.UniverseId is long ua && ua > 0 && ua == _games.GetValueOrDefault(b)?.UniverseId);

    /// <summary>
    /// Whether opening <paramref name="target"/> means leaving what this client is doing: another game, a chosen
    /// server it isn't in, a private server it wasn't opened into, or it is only in the menu / on an error screen.
    /// </summary>
    bool NeedsSwitch((int Pid, PendingLaunch Launch) run, GameTarget target)
    {
        var log = _logs.StateOf(run.Pid);
        long current = log?.PlaceId ?? run.Launch.Target?.PlaceId ?? 0;
        if (current == 0 || log is { Status: "left" or "disconnected" or "signedout" })
            return true;
        if (target.JobId is { } job)
            return !string.Equals(job, log?.JobId, StringComparison.OrdinalIgnoreCase);
        if (target.LinkCode is not null || target.ShareCode is not null)
            return target.LinkCode is null || run.Launch.Target?.LinkCode != target.LinkCode;
        return !SameGame(current, target.PlaceId);
    }

    /// <summary>
    /// Windows are matched to launches newest first, so a client still opening (the previous account, a rejoin, a
    /// hand-over — also one the person closed meanwhile, whose window is still on its way) must have its window
    /// before the next one starts, or the two could get each other's window. A rejoin that is still getting its
    /// ticket is waited for too, so one account isn't opened twice.
    /// </summary>
    async Task WaitForPendingAsync(CancellationToken ct)
    {
        bool Opening() => _rejoining || _pending.Count > 0;
        if (!Opening())
            return;
        Progress(L.T("Waiting for the previous Roblox window to open…"));
        // Bounded: a launch stops pending after PendingTimeout at the latest.
        while (Opening())
            await Task.Delay(500, ct);
    }

    /// <summary>Gets a sign-in ticket, starts the client and waits for its window to show up (see TrackProcesses).</summary>
    /// <param name="presetId">The preset this client is opened for (also on its rejoins and restarts): its options apply.</param>
    /// <param name="restart">Opened again by its preset's planned restart (a rejoin too).</param>
    async Task StartSessionAsync(Account account, GameTarget? target, string launchVia, CancellationToken ct = default, bool rejoin = false,
        Guid? presetId = null, bool restart = false)
    {
        string ticket = await RobloxApi.GetAuthTicketAsync(account.Cookie, ct);
        // Stop (or Close all) and a deletion while the ticket was fetched: no client for it any more.
        ct.ThrowIfCancellationRequested();
        if (FindAccount(account.Id) is null)
            throw new UserError(L.T("Account not found."));
        var before = RobloxProcesses.GetPlayerPids().ToHashSet();
        var settings = RequireVault().Data.Settings;
        var config = settings.RobloxConfig;
        bool windowed = settings.MultiInstance && settings.MultiInstanceWindowed;
        // A light-AFK preset gives this client its own values, whatever the shared setup says; the next client without
        // it gets the person's own values back (PrepareFor writes them from the copy it keeps).
        bool afk = PresetFor(presetId)?.Profile == AfkProfile;
        if (afk || config.Mode != "off" || windowed || RobloxGameSettings.HasBaseline)
        {
            bool use = afk || config.Mode == "all" || (config.Mode == "marked" && account.UseRobloxConfig);
            if (RobloxGameSettings.PrepareFor(afk ? RobloxGameSettings.LightAfk : config, use, windowed))
            {
                _robloxSettingsPending = true;
                _robloxSettingsWrittenAt = DateTime.UtcNow;
            }
        }
        RobloxLauncher.Launch(ticket, target, launchVia, _vault?.Data.Settings.CustomLauncher ?? "");
        _lastSessionStart = DateTime.UtcNow;
        _userClosed.Remove(account.Id);
        _late.RemoveWhere(l => l.AccountId == account.Id); // opened again: a window of the earlier try is not this one's
        _pending.Add(new PendingLaunch(account.Id, before, DateTime.UtcNow, target, rejoin, presetId));
        NoteActivity(restart ? "reopened" : rejoin ? "rejoined" : "opened", account.Id, target?.PlaceId); // Statistics › Activity
        account.LastUsed = DateTime.Now;
        account.CookieValid = true;
    }

    object? CancelLaunch()
    {
        _launchCts?.Cancel();
        return null;
    }

    /// <param name="force">End the process at once (a frozen client) instead of asking its windows to close first.</param>
    object CloseAccounts(Guid[] ids, bool force)
    {
        int closed = 0;
        foreach (var id in ids)
        {
            _rejoinQueue.RemoveAll(r => r.AccountId == id);
            _rejoinsInARow.Remove(id); // closed by the person: a client opened later counts its drops from one
            // Still opening (no window yet): its client is closed as soon as it shows up, instead of staying open
            // untracked.
            foreach (var launch in _pending.Where(p => p.AccountId == id))
            {
                if (_closeWhenClaimed.Add(launch))
                {
                    closed++;
                    PushState();
                }
            }
            if (!_running.TryGetValue(id, out var info))
                continue;
            _userClosed.Add(id);
            RobloxProcesses.Close(info.Pid, force || InGameNow(info.Pid));
            closed++;
        }
        return closed;
    }

    object CloseAll(bool force)
    {
        // Also what is still opening: the launch in progress and a rejoin getting its ticket.
        _launchCts?.Cancel();
        _rejoinCts?.Cancel();
        foreach (var id in _running.Keys)
            _userClosed.Add(id);
        _rejoinQueue.Clear();
        _rejoinsInARow.Clear();
        _handedOver.Clear();
        // Clients still opening (a launcher downloading an update) would show up later, untracked: they are closed
        // as soon as they appear instead, like a single account closed while opening.
        if (_pending.Count > 0)
        {
            _closeWhenClaimed.UnionWith(_pending);
            PushState();
        }
        _closeWhenClaimed.UnionWith(_late);
        // Roblox starts a windowless background client when its last window closes; sweep those for a moment
        // so "close all" really leaves nothing running and multi-instance can re-arm.
        _sweepBackgroundUntil = DateTime.UtcNow + BackgroundSweep;
        return RobloxProcesses.CloseAll(force, InGameNow);
    }

    /// <summary>Launching while the sweep runs would get the new client killed, so wait it out.</summary>
    async Task WaitForSweepAsync(CancellationToken ct = default)
    {
        var remaining = _sweepBackgroundUntil - DateTime.UtcNow;
        if (remaining <= TimeSpan.Zero)
            return;
        Progress(L.T("Waiting for Roblox to finish closing…"));
        await Task.Delay(remaining, ct);
    }

    /// <summary>Tiles every Roblox window on one monitor: tracked accounts first (by name), then any other clients.</summary>
    /// <param name="auto">Tiling after a launch: minimised windows (a light-AFK preset's) stay minimised and get no tile.</param>
    async Task<object?> ArrangeWindowsAsync(int monitor, bool auto = false)
    {
        var screens = Screen.AllScreens;
        var screen = monitor >= 0 && monitor < screens.Length ? screens[monitor] : Screen.PrimaryScreen ?? screens[0];
        var tracked = _running
            .OrderBy(r => FindAccount(r.Key)?.Label ?? "", StringComparer.CurrentCultureIgnoreCase)
            .Select(r => r.Value.Pid)
            .ToList();
        var others = RobloxProcesses.GetPlayerPids().Where(p => !tracked.Contains(p)).OrderBy(p => p);
        // After a launch, a light-AFK preset's windows get no tile: they are minimised once in their game.
        var afk = auto ? AfkClients() : [];
        return await RobloxProcesses.ArrangeGridAsync(tracked.Concat(others).Where(p => !afk.Contains(p)).ToList(), screen.WorkingArea,
            _vault?.Data.Settings.WindowLayout ?? "fill", skipMinimized: auto);
    }

    /// <summary>The clients a light-AFK preset opened (by process id).</summary>
    HashSet<int> AfkClients() =>
        _running.Values.Where(r => PresetFor(r.Launch.PresetId)?.Profile == AfkProfile).Select(r => r.Pid).ToHashSet();

    /// <summary>Multi-Roblox's "tile" option: once the launch's last window is up, two or more windows are tiled on the main screen.</summary>
    async Task TileWhenOpenAsync()
    {
        var deadline = DateTime.UtcNow + PendingTimeout;
        while (!_disposed && (_pending.Count > 0 || _launchCts is not null) && DateTime.UtcNow < deadline)
            await Task.Delay(500);
        // Roblox sizes its window again as it finishes starting up.
        await Task.Delay(3000);
        if (_disposed || _vault is not { Data.Settings: { MultiInstance: true, MultiInstanceTile: true } } || _launchCts is not null)
            return; // locked, switched off, or another launch started (it tiles when it is done)
        // Two or more windows that get a tile: minimised ones and a light-AFK preset's don't (see ArrangeWindowsAsync),
        // so one window isn't stretched over the whole screen as a grid of one.
        var afk = AfkClients();
        if (RobloxProcesses.VisibleWindows(RobloxProcesses.GetPlayerPids())
                .Count(w => !afk.Contains(w.Key) && w.Value.Count > 0 && !RobloxProcesses.IsMinimised(RobloxProcesses.MainWindowOf(w.Value))) >= 2)
            await ArrangeWindowsAsync(-1, auto: true);
    }

    // ───────────────────────────── background work ─────────────────────────────

    /// <summary>A time moved back by a clock jump; one never set (the default) stays as it is.</summary>
    static DateTime MovedBack(DateTime at, TimeSpan back) => at > DateTime.MinValue + back ? at - back : at;

    /// <summary>How often Windows' process list is read while nothing of Roblox's runs and the window is in the tray.</summary>
    static readonly TimeSpan IdleScanInterval = TimeSpan.FromSeconds(15);
    DateTime _lastScan;
    bool _lastScanFoundNone;
    /// <summary>The clients the last process check found, and when.</summary>
    (List<RobloxProcesses.PlayerProcess> Players, DateTime At)? _lastPlayers;

    /// <summary>
    /// Whether this tick reads the process list: always, unless the window is in the tray, the last read found no
    /// Roblox client, nothing is opening, closing or waiting to rejoin, and the last read is under 15 s old. A Roblox
    /// started by hand meanwhile is noticed at the next read; one Bloxkeep starts makes every tick read it again.
    /// </summary>
    bool ScanDue(DateTime now) =>
        !(_background && _lastScanFoundNone && _running.Count == 0 && _pending.Count == 0 && _late.Count == 0 && _rejoinQueue.Count == 0
          && !_rejoining && _launchCts is null && now >= _sweepBackgroundUntil && _traySweep.Count == 0 && _closingForRejoin.Count == 0
          && now >= _lastScan && now - _lastScan < IdleScanInterval);

    void OnProcTimer()
    {
        if (_disposed)
            return;
        try
        {
            TrackProcesses();
        }
        // A timer callback must never take the app down; log (once for a failure that repeats) and try again on the next tick.
        catch (Exception ex)
        {
            LogTickError(ex);
        }
    }

    /// <summary>
    /// Matches launches to the processes they start, keeps window titles in sync, publishes RAM use, runs
    /// auto-rejoin and the resource guard, re-arms multi-instance, and schedules presence refreshes.
    /// </summary>
    void TrackProcesses()
    {
        var now = DateTime.UtcNow;
        // The clock was set back: everything timed by it would wait that much longer, so periodic work is due again
        // now, and deadlines and start times are moved back by the same amount.
        if (now < _lastTick)
        {
            var back = _lastTick - now;
            if (_saveDue is not null)
                _saveDue = now;
            _nextPresence = _nextRobloxVersionCheck = _nextUpdateCheck = now;
            _sweepBackgroundUntil = MovedBack(_sweepBackgroundUntil, back);
            _resumedAt = MovedBack(_resumedAt, back);
            _lastSessionStart = MovedBack(_lastSessionStart, back);
            _robloxSettingsWrittenAt = MovedBack(_robloxSettingsWrittenAt, back);
            foreach (var launch in _pending.Concat(_late).Concat(_running.Values.Select(r => r.Launch)))
                launch.StartedAt = MovedBack(launch.StartedAt, back);
            foreach (var parent in _traySweep.Keys.ToList())
                _traySweep[parent] = MovedBack(_traySweep[parent], back);
            foreach (var launch in _pendingSeenAlive.Keys.ToList())
                _pendingSeenAlive[launch] = MovedBack(_pendingSeenAlive[launch], back);
            for (int i = 0; i < _rejoinQueue.Count; i++)
                _rejoinQueue[i] = _rejoinQueue[i] with { Due = MovedBack(_rejoinQueue[i].Due, back) };
            _signInCheckRetryAt = MovedBack(_signInCheckRetryAt, back);
            MoveDiscordBack(back);
            if (Prefs.LastSignInCheck is { } checkedAt)
                Prefs.LastSignInCheck = MovedBack(checkedAt, back); // written with the next prefs change; a restart clamps it anyway
        }
        NoteCheckGap(now, _lastTick);
        _lastTick = now;
        // Windows' process list is the costly part of a tick (a few ms, whatever runs): in the tray with no Roblox
        // running and nothing on its way, it is read every 15 s rather than every 5.
        bool scan = ScanDue(now);
        var players = scan ? RobloxProcesses.GetPlayers() : [];
        IReadOnlyList<RobloxProcesses.PlayerProcess> scanned = players.ToArray();
        if (scan)
            _lastScan = now;
        var pids = players.Select(p => p.Pid).ToHashSet();
        // One pass over the desktop's windows serves every window check of this tick.
        var windows = RobloxProcesses.VisibleWindows(pids);
        bool HasWindow(int pid) => windows.TryGetValue(pid, out var list) && list.Count > 0;
        // The same snapshot answers every parent → children question (it has each client's parent).
        Dictionary<int, List<int>>? tree = null;
        List<int> ChildrenOf(int parent) => (tree ??= RobloxProcesses.PlayerChildren(scanned)).GetValueOrDefault(parent) ?? [];
        int ParentOf(int child) => (tree ??= RobloxProcesses.PlayerChildren(scanned)).FirstOrDefault(kv => kv.Value.Contains(child)).Key;
        if (now < _sweepBackgroundUntil)
        {
            foreach (int pid in pids.Where(p => !HasWindow(p)).ToList())
            {
                RobloxProcesses.Kill(pid);
                pids.Remove(pid);
            }
            players.RemoveAll(p => !pids.Contains(p.Pid));
        }
        NoteClientStarts(pids, now, ParentOf);
        bool changed = false;

        foreach (var (id, info) in _running.ToList())
        {
            if (pids.Contains(info.Pid))
                continue;
            _running.Remove(id);
            PersistSoon(); // the open clients on disk (see Persist)
            // Read now, with the log's last lines (a leave by choice logged right before the client quit), before the
            // log watcher drops the pid below.
            var log = _logs.LastStateOf(info.Pid);
            // Roblox closed it because another client opened: not a hand-over, and no auto-rejoin.
            bool closedByRoblox = CheckClosedByRoblox(id, info.Pid, info.Launch.StartedAt, now, ChildrenOf);
            _endedHere[id] = (log?.JobId ?? info.Launch.Target?.JobId, log?.PlaceId ?? info.Launch.Target?.PlaceId, now);
            EndPlay(id, log, now);
            RefreshPresenceSoon(TimeSpan.FromSeconds(8));
            // The first process may only bootstrap and hand over to a child: wait for that only when a new,
            // unclaimed client is actually running. A window the person closed ends the session right away.
            // A client that got as far as joining a game (its log says so) was the real one, not a bootstrap: a
            // windowless child of it is the copy Roblox starts in the notification area, not a hand-over.
            var claimedNow = _running.Values.Select(r => r.Pid).ToHashSet();
            List<int> trayCopies = log is null ? [] : ChildrenOf(info.Pid).Where(p => !HasWindow(p)).ToList();
            if (log is not null)
                _traySweep[info.Pid] = now + TraySweep;
            bool handOver = now - info.Launch.StartedAt < HandOverWindow && !_userClosed.Contains(id) && !closedByRoblox
                && pids.Any(p => !info.Launch.Before.Contains(p) && !claimedNow.Contains(p) && !trayCopies.Contains(p));
            if (handOver)
            {
                _pending.Add(info.Launch);
                _pendingSeenAlive[info.Launch] = now;
                _handedOver[info.Launch] = (info.Pid, log);
            }
            else
                OnSessionEnded(id, info.Launch, log);
            changed = true;
        }

        foreach (var expired in _pending.Where(p => now - p.StartedAt > PendingTimeout).ToList())
        {
            // No window yet (Roblox updating itself on a slow link, its launcher stuck): one that comes later is still
            // this launch's client, rather than nobody's, or the next account's (see _late).
            bool waitLonger = !_closeWhenClaimed.Contains(expired) && !_handedOver.ContainsKey(expired);
            DropPending(expired, late: waitLonger);
            if (waitLonger)
            {
                _late.RemoveWhere(l => l.AccountId == expired.AccountId);
                _late.Add(expired);
            }
            changed = true;
        }
        var claimed = _running.Values.Select(r => r.Pid).ToHashSet();
        if (_pending.Count > 0)
        {
            // Newest launch first: a new client can only belong to a launch made before it appeared (its Before
            // list lacks it), and a session waiting for a hand-over must not take the next account's window.
            foreach (var launch in _pending.OrderByDescending(p => p.StartedAt).ToList())
            {
                // A session waiting for a hand-over takes a child of its ended process before any other client.
                List<int> children = _handedOver.TryGetValue(launch, out var handed) ? ChildrenOf(handed.Pid) : [];
                int pid = pids.Where(p => !launch.Before.Contains(p) && !claimed.Contains(p))
                    .Where(HasWindow)
                    .OrderByDescending(children.Contains)
                    .ThenBy(p => p)
                    .FirstOrDefault();
                if (pid == 0)
                    continue;
                claimed.Add(pid);
                _pending.Remove(launch);
                _handedOver.Remove(launch);
                // The person closed this account while it was still opening.
                if (_closeWhenClaimed.Remove(launch))
                    RobloxProcesses.Close(pid);
                else
                {
                    _running[launch.AccountId] = (pid, launch);
                    PersistSoon(); // the open clients on disk (see Persist)
                }
                changed = true;
            }
            // A client appeared for this launch and then every new client quit without a game window: stop waiting.
            foreach (var launch in _pending.ToList())
            {
                if (pids.Any(p => !launch.Before.Contains(p) && !claimed.Contains(p)))
                    _pendingSeenAlive[launch] = now;
                else if (_pendingSeenAlive.TryGetValue(launch, out var seen) && now - seen > PendingGoneGrace)
                {
                    DropPending(launch);
                    changed = true;
                }
            }
        }
        // Launches past their wait, oldest first, take only a client no launch still waiting could be taking (one
        // that appeared before all of them started): a late window never takes the window of the launch under way.
        foreach (var launch in _late.OrderBy(l => l.StartedAt).ToList())
        {
            if (now - launch.StartedAt > PendingTimeout + LateTimeout || _running.ContainsKey(launch.AccountId))
            {
                _late.Remove(launch);
                _closeWhenClaimed.Remove(launch);
                if (!_running.ContainsKey(launch.AccountId) && !_pending.Any(p => p.AccountId == launch.AccountId))
                    ForgetScheduledOpen(launch.AccountId); // no client came of it: a scheduled stop has nothing of this account's to close
                continue;
            }
            int pid = pids.Where(p => !launch.Before.Contains(p) && !claimed.Contains(p) && HasWindow(p) && _pending.All(l => l.Before.Contains(p)))
                .OrderBy(p => p)
                .FirstOrDefault();
            if (pid == 0)
                continue;
            claimed.Add(pid);
            _late.Remove(launch);
            if (_closeWhenClaimed.Remove(launch))
                RobloxProcesses.Close(pid);
            else
            {
                _running[launch.AccountId] = (pid, launch);
                PersistSoon();
            }
            changed = true;
        }
        foreach (var gone in _pendingSeenAlive.Keys.Where(l => !_pending.Contains(l)).ToList())
            _pendingSeenAlive.Remove(gone);

        // The copy Roblox starts in the notification area when a client closes (by its X button, a crash): ended
        // while it has no window — never one a launch could be waiting for, or one that isn't in this tick's list.
        foreach (var (parent, until) in _traySweep.ToList())
        {
            if (now > until)
            {
                _traySweep.Remove(parent);
                continue;
            }
            var tracked = _running.Values.Select(r => r.Pid).ToHashSet();
            foreach (int child in ChildrenOf(parent))
            {
                if (!pids.Contains(child) || HasWindow(child) || tracked.Contains(child) || _pending.Any(l => !l.Before.Contains(child)))
                    continue;
                RobloxProcesses.Kill(child);
                pids.Remove(child);
                ForgetClientStart(child);
            }
        }
        players.RemoveAll(p => !pids.Contains(p.Pid));
        if (scan)
        {
            _lastScanFoundNone = pids.Count == 0;
            _lastPlayers = (players.ToList(), now); // the monitor tab's tiles use it (MonitorClients)
        }

        _closingForRejoin.RemoveWhere(pid => !pids.Contains(pid));
        // Once a client has started (its window is up), the file goes back to the person's own settings.
        if (_robloxSettingsPending && _launchCts is null && _pending.Count == 0 && !_rejoining
            && now - _robloxSettingsWrittenAt > TimeSpan.FromSeconds(10))
        {
            bool final = pids.Count == 0;
            RobloxGameSettings.Restore(final);
            if (final)
                _robloxSettingsPending = false;
        }
        // Every Roblox window's log: the ones Bloxkeep opened for their status, all of them for anti-AFK.
        changed |= _logs.Poll(pids.ToList()) && _running.Count > 0;
        // A client moved to a place not looked up yet: name it now rather than at the next status check.
        var unnamed = _running.Values.Select(r => _logs.StateOf(r.Pid)?.PlaceId ?? 0)
            .Where(id => id > 0 && !HasGame(id, false) && !(_gameFetchedAt.TryGetValue(id, out var asked) && now - asked < GameRetry))
            .Distinct().ToList();
        if (unnamed.Count > 0)
            _ = NameLoggedPlacesAsync(unnamed);
        RejoinFromLogs();
        ForgetSettledRejoins(now);
        TrackPlay(now);
        RunAntiAfk(now, windows);
        MinimiseAfkWindows(windows);

        if (_vault is { Data.Settings.RenameWindows: true })
        {
            foreach (var (id, info) in _running)
            {
                if (FindAccount(id) is { } account && windows.TryGetValue(info.Pid, out var list))
                    RobloxProcesses.SetWindowTitle(list, $"Roblox - {NameText(account)}");
            }
        }

        if (_vault is { Data.Settings.MultiInstance: true } && !MultiInstance.Armed && scan && pids.Count == 0)
            MultiInstance.Enable();
        if (MultiInstance.Armed != _lastArmed)
        {
            _lastArmed = MultiInstance.Armed;
            changed = true;
        }
        if (changed)
            PushStateIfChanged();

        // Poll fast only while something is happening; an idle app should cost next to nothing.
        bool active = pids.Count > 0 || _pending.Count > 0 || _rejoinQueue.Count > 0 || now < _sweepBackgroundUntil;
        int interval = _background ? 5000 : active ? 1500 : 4000;
        if (_procTimer.Interval != interval)
            _procTimer.Interval = interval;

        if (now >= _nextRobloxVersionCheck && !_background)
        {
            _nextRobloxVersionCheck = now + RobloxVersionInterval;
            _ = RefreshLatestRobloxAsync();
        }
        // The periodic jobs from here on don't depend on each other: one that fails (logged) doesn't stop the rest,
        // least of all the vault's save and auto-rejoin.
        Step(() => TickRobloxUpdates(now, pids.Count(pid => _logs.StateOf(pid)?.Status == "ingame")));

        if (Prefs.CheckUpdates && _updater.Supported && now >= _nextUpdateCheck)
        {
            _nextUpdateCheck = now + UpdateCheckInterval;
            _ = _updater.CheckAsync();
        }

        Step(CheckSchedules); // also while locked: a run due then is reported as missed
        if (_vault is null)
            return;
        Step(() => PersistIfDue(now));
        Step(() =>
        {
            var memory = SystemMemory.Read();
            NoteClientMemory(players);
            EmitMetrics(players, windows, memory);
            GuardResources(players, memory, now);
        });
        Step(() => RunDueRejoin(now));
        Step(() => RestartDueClients(now));
        Step(() => RunSignInCheckIfDue(now));
        Step(() => TickDiscord(now, players));
        if (!_presenceBusy && now >= _nextPresence)
            _ = RefreshPresenceAsync();
    }

    /// <summary>One job of the process check, on its own: a failure is logged (see <see cref="LogTickError"/>) and the next job runs.</summary>
    void Step(Action job)
    {
        try
        {
            job();
        }
        catch (Exception ex)
        {
            LogTickError(ex);
        }
    }

    /// <summary>The last failure of the process check and how often it came again since it was written.</summary>
    (string Key, DateTime Logged, int Repeats)? _tickError;
    /// <summary>A failure that keeps coming back is written again at most this often (with how many times it came).</summary>
    static readonly TimeSpan TickErrorRepeat = TimeSpan.FromHours(1);

    /// <summary>
    /// A failure in the process check goes to the log once, not on every tick (every 1.5 s, which would also delete
    /// and rewrite error.log every few minutes): the same failure again is counted, and written once an hour.
    /// </summary>
    void LogTickError(Exception ex)
    {
        string where = ex.StackTrace is { } trace ? trace.TrimStart().Split('\n', 2)[0] : "";
        string key = $"{ex.GetType().FullName}|{ex.Message}|{where}";
        var now = DateTime.UtcNow;
        if (_tickError is { } last && last.Key == key && now >= last.Logged && now - last.Logged < TickErrorRepeat)
        {
            _tickError = last with { Repeats = last.Repeats + 1 };
            return;
        }
        if (_tickError is { Repeats: > 0 } previous)
            Log.Note($"The process check's last logged error came {previous.Repeats} more time(s) after it was written.");
        _tickError = (key, now, 0);
        Log.Write(ex);
    }

    /// <summary>Live RAM per client and for the PC, sent apart from the state so the page can patch numbers in place.</summary>
    void EmitMetrics(List<RobloxProcesses.PlayerProcess> players, Dictionary<int, List<IntPtr>> windows, MemoryStatus memory)
    {
        if (!PageAttached)
            return;
        var workingSets = players.ToDictionary(p => p.Pid, p => p.WorkingSet);
        string json = JsonSerializer.Serialize(new
        {
            accounts = _running.Select(r => new { id = r.Key, ramMb = workingSets.GetValueOrDefault(r.Value.Pid) / MB }),
            robloxCount = players.Count,
            // The monitor tab's tiles: clients with a visible window (see MonitorClients), from this tick's window pass.
            monitorCount = players.Count(p => windows.TryGetValue(p.Pid, out var list) && list.Count > 0),
            totalMb = players.Sum(p => p.WorkingSet) / MB,
            systemMb = memory.TotalMb,
            availableMb = memory.AvailableMb / 128 * 128, // coarse, so idle jitter doesn't repaint the page
            usedPct = memory.LoadPercent,
        }, Json);
        if (json == _lastMetrics)
            return;
        _lastMetrics = json;
        Post($"{{\"type\":\"event\",\"name\":\"metrics\",\"data\":{json}}}");
    }

    /// <summary>
    /// Warns when the PC runs low on RAM. With "save RAM on background windows", Roblox windows you aren't playing get a
    /// low memory priority (Windows takes RAM from them first, only when it runs short — nothing changes while there is
    /// plenty), and when the PC is really short of RAM they are asked to give some back, at most every two minutes.
    /// A window gets its normal priority back as soon as it is in front, or when the option is turned off.
    /// </summary>
    void GuardResources(List<RobloxProcesses.PlayerProcess> players, MemoryStatus memory, DateTime now)
    {
        var s = _vault!.Data.Settings;
        if (s.MemoryWarnings && players.Count > 0 && memory.LoadPercent >= MemoryWarningLoad
            && now - _lastMemoryWarning > MemoryWarningInterval)
        {
            _lastMemoryWarning = now;
            Notify(L.F("Your PC is using {0}% of its RAM. Close some Roblox windows if things get slow.", memory.LoadPercent), "warn", "ram");
        }

        foreach (int gone in _lowMemoryPriority.Where(pid => players.All(p => p.Pid != pid)).ToList())
            _lowMemoryPriority.Remove(gone);
        if (!s.TrimBackground)
        {
            foreach (int pid in _lowMemoryPriority)
                RobloxProcesses.SetMemoryPriority(pid, background: false);
            _lowMemoryPriority.Clear();
            return;
        }
        if (players.Count == 0)
            return;

        // Only touch a window when its role changes (it went to the background, or came to the front).
        int foreground = RobloxProcesses.ForegroundPid();
        bool refused = false;
        foreach (var p in players)
        {
            bool background = p.Pid != foreground;
            if (background == _lowMemoryPriority.Contains(p.Pid))
                continue;
            if (!RobloxProcesses.SetMemoryPriority(p.Pid, background))
            {
                refused = true;
                continue;
            }
            if (background)
                _lowMemoryPriority.Add(p.Pid);
            else
                _lowMemoryPriority.Remove(p.Pid);
        }

        bool shortOfRam = memory.TotalMb > 0 && (memory.LoadPercent >= TrimLoadPercent || memory.AvailableMb < TrimBelowAvailableMb);
        if (shortOfRam && now - _lastTrim >= TrimInterval)
        {
            _lastTrim = now;
            var background = players.Where(p => p.Pid != foreground).ToList();
            if (background.Count > 0 && background.Count(p => RobloxProcesses.TrimMemory(p.Pid)) == 0)
                refused = true;
        }
        if (refused && !_trimRefusedShown)
        {
            _trimRefusedShown = true;
            Toast(L.T("Roblox refused memory trimming, so it was skipped."), "info");
        }
    }

    /// <param name="lastLog">What the client's log said last, when it was still being followed.</param>
    void OnSessionEnded(Guid accountId, PendingLaunch launch, RobloxLogWatcher.ClientState? lastLog = null)
    {
        NoteSessionEnd(accountId, launch, lastLog); // before auto-rejoin reads (and forgets) whether Roblox closed it
        _offlineStrikes.Remove(accountId);
        // Closed for good (not a drop auto-rejoin undoes): a scheduled stop no longer owns the account, so a client
        // the person opens for it by hand later stays open.
        if (!QueueRejoin(accountId, launch, lastLog))
            ForgetScheduledOpen(accountId);
    }

    /// <returns>true when auto-rejoin (or the preset's planned restart) will bring the client back.</returns>
    bool QueueRejoin(Guid accountId, PendingLaunch launch, RobloxLogWatcher.ClientState? lastLog)
    {
        bool restart = _restarts.Remove(accountId, out var restarting);
        if (!ShouldRejoin(accountId, launch, lastLog, restart, out var account))
        {
            _rejoinsInARow.Remove(accountId); // closed for good: a client opened later counts from one again
            return false;
        }
        _rejoinQueue.RemoveAll(r => r.AccountId == accountId);
        if (restart)
        {
            // Not a drop: it doesn't count towards the waits below, and goes ahead whatever auto-rejoin says.
            _rejoinQueue.Add(new RejoinRequest(accountId, restarting.Target, DateTime.UtcNow + RejoinDelay, PresetId: launch.PresetId, Restart: true));
            return true;
        }
        int inARow = _rejoinsInARow.GetValueOrDefault(accountId);
        _rejoinsInARow[accountId] = inARow + 1;
        var wait = RejoinWait(inARow);
        // Rejoin the same game or private server rather than the exact server, which may be gone.
        _rejoinQueue.Add(new RejoinRequest(accountId, launch.Target! with { JobId = null }, DateTime.UtcNow + wait, PresetId: launch.PresetId));
        if (inARow < QuickRejoins)
            Notify(L.F("{0} left the game — rejoining in {1}s…", Name(account), (int)wait.TotalSeconds), "info", "drop", account.Id);
        else if (inARow == QuickRejoins)
            // Something keeps failing (a ban, a broken game, a bad connection): said once, then it waits between tries.
            Notify(L.F("{0} keeps getting disconnected — trying again every few minutes.", Name(account)), "warn", "rejoinSlow", account.Id);
        else
            Toast(L.F("{0} left the game again — rejoining in {1} min.", Name(account), (int)wait.TotalMinutes));
        return true;
    }

    bool ShouldRejoin(Guid accountId, PendingLaunch launch, RobloxLogWatcher.ClientState? lastLog, bool restart,
        [System.Diagnostics.CodeAnalysis.NotNullWhen(true)] out Account? account)
    {
        account = null;
        if (_userClosed.Remove(accountId))
            return false;
        // Roblox closed it when another window opened: a rejoin would only close the next one (AppController.RobloxUpdates.cs).
        if (_closedByRoblox.Remove(accountId))
            return false;
        if (FindAccount(accountId) is not { } found || launch.Target is null)
            return false;
        account = found;
        // Banned (a sign-in check said so): a new window would only show the ban again, every few minutes for good.
        if (found.Banned == true)
            return false;
        if (restart)
            return true;
        if (!RejoinOn(found, launch.PresetId))
            return false;
        // The person left the game themselves, or the same account joined from another device: not a drop.
        return lastLog is not ({ Status: "left" or "signedout" } or { Status: "disconnected", Reason: SameAccountElsewhere });
    }

    /// <summary>Rejoins in a row that go at once (after <see cref="RejoinDelay"/>); the ones after wait <see cref="RejoinBackoff"/>.</summary>
    const int QuickRejoins = 2;
    /// <summary>The waits after the quick rejoins; the last one repeats for as long as auto-rejoin stays on.</summary>
    static readonly TimeSpan[] RejoinBackoff = [TimeSpan.FromMinutes(2), TimeSpan.FromMinutes(5), TimeSpan.FromMinutes(10), TimeSpan.FromMinutes(15)];
    /// <summary>A client in its game this long is fine again: its next drop counts from one.</summary>
    static readonly TimeSpan StayResetsRejoins = TimeSpan.FromMinutes(10);

    /// <summary>How long a rejoin waits, given how many came right before it without a real stay in between.</summary>
    static TimeSpan RejoinWait(int inARow) =>
        inARow < QuickRejoins ? RejoinDelay : RejoinBackoff[Math.Min(inARow - QuickRejoins, RejoinBackoff.Length - 1)];

    /// <summary>Clients that have stayed in their game <see cref="StayResetsRejoins"/>: their earlier drops no longer count.</summary>
    void ForgetSettledRejoins(DateTime now)
    {
        if (_rejoinsInARow.Count == 0)
            return;
        foreach (var (id, info) in _running)
        {
            if (!_rejoinsInARow.ContainsKey(id))
                continue;
            bool settled = _logs.StateOf(info.Pid) is { } log
                ? log.Status == "ingame" && now - log.At >= StayResetsRejoins
                : now - info.Launch.StartedAt >= StayResetsRejoins && FindAccount(id) is { } account
                  && _presence.TryGetValue(account.UserId, out var presence) && presence.Type == 2;
            if (settled)
                _rejoinsInARow.Remove(id);
        }
    }

    /// <summary>The account card's Cancel on a waiting rejoin: the client stays closed.</summary>
    object? CancelRejoin(Guid id)
    {
        _rejoinsInARow.Remove(id);
        if (_rejoinQueue.RemoveAll(r => r.AccountId == id) > 0)
            ForgetScheduledOpen(id); // closed for good now: a scheduled stop has nothing of this account's to close
        PushState();
        return null;
    }

    /// <summary>
    /// Acts on a kick or lost connection as soon as the client logs it, instead of after two presence checks. A
    /// voluntary leave, or the same account joining from another device, is left alone.
    /// </summary>
    void RejoinFromLogs()
    {
        if (_vault is null || _running.Count == 0 || Settling)
            return;
        foreach (var (id, info) in _running.ToList())
        {
            if (_logs.StateOf(info.Pid) is not { Status: "disconnected" } state || state.Reason == SameAccountElsewhere
                || _closingForRejoin.Contains(info.Pid) || DateTime.UtcNow - state.At < TimeSpan.FromSeconds(3)
                || FindAccount(id) is not { } account || !RejoinOn(account, info.Launch.PresetId) || info.Launch.Target is null
                || account.Banned == true) // no rejoin follows (see ShouldRejoin): the window stays, with Roblox's reason
                continue;
            _closingForRejoin.Add(info.Pid);
            Notify(L.F("{0} seems disconnected — restarting Roblox…", Name(account)), "info", "drop", account.Id);
            RobloxProcesses.Close(info.Pid);
        }
    }

    /// <summary>
    /// Every <see cref="AntiAfk.Interval"/> per Roblox window (counted from joining a server or its last input), and
    /// only while nobody is using the PC, nudges every Roblox window on the PC that is in a server — also ones opened
    /// outside Bloxkeep. A window the person is playing in gets their input anyway. Settings' switch covers them all;
    /// a preset's own anti-AFK option decides for the windows it opened (<see cref="AntiAfkOn"/>).
    /// </summary>
    void RunAntiAfk(DateTime now, Dictionary<int, List<IntPtr>> windows)
    {
        foreach (int gone in _lastInput.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _lastInput.Remove(gone);
        foreach (int gone in _nudgeRetryAt.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _nudgeRetryAt.Remove(gone);
        foreach (int gone in _nudgeMisses.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _nudgeMisses.Remove(gone);
        _idleKickReported.RemoveWhere(pid => !windows.ContainsKey(pid));
        if (_vault is null || windows.Count == 0)
            return;
        var on = windows.Keys.Where(AntiAfkOn).ToHashSet();
        if (on.Count == 0)
            return;
        ReportIdleKicks(on);
        // A window seen for the first time has been idle at least since it joined its server (or since now).
        foreach (int pid in on)
            _lastInput.TryAdd(pid, _logs.StateOf(pid) is { Status: "ingame" } joined ? joined.At : now);
        if (_nudging)
            return;

        bool personHere = AntiAfk.PersonActive();
        int foreground = RobloxProcesses.ForegroundPid();
        if (personHere && windows.ContainsKey(foreground))
            _lastInput[foreground] = now;
        if (personHere)
        {
            // Someone is at the PC (it isn't locked): the next idle spell tries every window again from the start.
            _nudgeMisses.Clear();
            _nudgeRetryAt.Clear();
            return;
        }

        var due = windows
            .Where(w => on.Contains(w.Key) && w.Value.Count > 0 && _logs.StateOf(w.Key)?.Status is null or "ingame" && now - LastInput(w.Key) >= AntiAfk.Interval
                        && !(_nudgeRetryAt.TryGetValue(w.Key, out var retry) && now < retry))
            .Select(w => (Pid: w.Key, Window: w.Value[0]))
            .ToList();
        if (due.Count > 0)
            _ = NudgeAsync(due);
    }

    DateTime LastInput(int pid)
    {
        var last = _lastInput.GetValueOrDefault(pid, DateTime.UtcNow);
        // Roblox counts idle time from joining a server (a server change starts it over).
        return _logs.StateOf(pid) is { Status: "ingame" } log && log.At > last ? log.At : last;
    }

    async Task NudgeAsync(List<(int Pid, IntPtr Window)> clients)
    {
        _nudging = true;
        try
        {
            // Turning anti-AFK off (or locking) stops it between two windows.
            var reached = await AntiAfk.NudgeAsync(clients.Select(c => c.Window).ToList(), () => clients.Any(c => AntiAfkOn(c.Pid)));
            foreach (var (pid, window) in clients)
            {
                if (reached.Contains(window))
                {
                    _lastInput[pid] = DateTime.UtcNow;
                    _nudgeRetryAt.Remove(pid);
                    _nudgeMisses.Remove(pid);
                }
                else
                {
                    // Focus refused or the PC is locked: not every tick, and less often while it lasts (a locked PC
                    // all night would otherwise restore and minimise a minimised window every minute), 15 min at most.
                    int misses = _nudgeMisses[pid] = _nudgeMisses.GetValueOrDefault(pid) + 1;
                    _nudgeRetryAt[pid] = DateTime.UtcNow + TimeSpan.FromTicks(Math.Min(NudgeRetry.Ticks << Math.Min(misses - 1, 4), MaxNudgeRetry.Ticks));
                }
            }
        }
        finally
        {
            _nudging = false;
        }
    }

    /// <summary>Says so when an account is kicked for idling although anti-AFK is on for its window (<paramref name="on"/>).</summary>
    void ReportIdleKicks(HashSet<int> on)
    {
        foreach (var (id, info) in _running)
        {
            if (on.Contains(info.Pid) && _logs.StateOf(info.Pid) is { Status: "disconnected", Reason: IdleKick } && _idleKickReported.Add(info.Pid)
                && FindAccount(id) is { } account)
                Notify(L.F("{0} was kicked for being AFK. Anti-AFK doesn't work while the PC is locked.", Name(account)), "warn", "afk", account.Id);
        }
    }

    void RunDueRejoin(DateTime now)
    {
        // One client at a time: the previous one must have its window (Roblox may be updating), then a short gap.
        var spacing = TimeSpan.FromSeconds(Math.Max(3, _vault?.Data.Settings.LaunchDelaySeconds ?? 0));
        if (_rejoinQueue.Count == 0 || _rejoining || _launchCts is not null || now < _sweepBackgroundUntil || _pending.Count > 0
            || now - _lastSessionStart < spacing || Settling)
            return;
        var due = _rejoinQueue.FirstOrDefault(r => r.Due <= now);
        if (due is null)
            return;
        _rejoinQueue.Remove(due);
        _ = RejoinAsync(due);
    }

    async Task RejoinAsync(RejoinRequest request)
    {
        var vault = _vault;
        var account = FindAccount(request.AccountId);
        if (vault is null || account is null || !request.Restart && !RejoinOn(account, request.PresetId) || account.Banned == true
            || _running.ContainsKey(account.Id) || _pending.Any(p => p.AccountId == account.Id))
            return;
        _rejoining = true;
        var cts = _rejoinCts = new CancellationTokenSource();
        try
        {
            var players = RobloxProcesses.GetPlayerPids();
            if (vault.Data.Settings.MultiInstance && !MultiInstance.Armed && players.Length == 0)
                MultiInstance.Enable();
            // Without Multi-Roblox the new client closes the open one — which would then rejoin and close this one,
            // and so on.
            if (!MultiInstance.Armed && RobloxProcesses.VisibleWindows(players).Values.Any(w => w.Count > 0))
            {
                Notify(L.F("Didn't rejoin {0}: Multi-Roblox isn't ready, so a new Roblox window would close the one that's open.", Name(account)), "warn", "rejoinFail", account.Id);
                return;
            }
            await StartSessionAsync(account, request.Target, vault.Data.Settings.LaunchVia, cts.Token, rejoin: true, presetId: request.PresetId,
                restart: request.Restart);
            Persist();
            PushState();
        }
        catch (OperationCanceledException) when (cts.IsCancellationRequested)
        {
            // Close all.
        }
        catch (UserError)
        {
            // The account was deleted while the ticket was fetched.
        }
        catch (Exception ex) when ((ex is HttpRequestException or TaskCanceledException or RobloxApiException { RateLimited: true } or RobloxApiException { ServerError: true })
                                   && request.NetworkRetries < MaxNetworkRetries)
        {
            // No connection (often right after sleep), or Roblox busy or down (rate limit, a server error): try again
            // later, each time a little later. This is not counted as a rejoin. A refusal (a ban, a challenge) is
            // reported below instead: twenty silent retries wouldn't change it.
            var delay = TimeSpan.FromTicks(Math.Min(NetworkRetryDelay.Ticks * (request.NetworkRetries + 1), MaxRetryDelay.Ticks));
            _rejoinQueue.Add(request with { Due = DateTime.UtcNow + delay, NetworkRetries = request.NetworkRetries + 1 });
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex) || ex is Win32Exception or FileNotFoundException)
        {
            if (ex is RobloxApiException { CookieInvalid: true })
            {
                account.CookieValid = false;
                account.AutoRejoin = false;
                Persist();
                PushState();
            }
            Notify(L.F("Couldn't rejoin {0}: {1}", Name(account),ex is Win32Exception or FileNotFoundException ? ex.Message : ErrorText(ex)), "error", "rejoinFail", account.Id);
        }
        finally
        {
            _rejoining = false;
            if (_rejoinCts == cts)
                _rejoinCts = null;
            cts.Dispose();
        }
    }

    /// <summary>
    /// The client's own log is the better source while it says something current; one stuck in "joining" (a join
    /// or teleport that never finished) is left to Roblox's status again.
    /// </summary>
    bool LogFollowed(int pid) =>
        _logs.StateOf(pid) is { } log && !(log.Status == "joining" && DateTime.UtcNow - log.At > JoiningTimeout);

    /// <summary>
    /// A client that is still open while its account has not been in a game for two presence checks is
    /// usually sitting on a kick or disconnect screen; close it so auto-rejoin starts a fresh one.
    /// </summary>
    /// <param name="reliable">Accounts whose status came from their own cookie or shows them in a game.</param>
    void DetectDisconnectedSessions(HashSet<long> reliable)
    {
        // Right after sleep everyone looks offline until the network is back; that says nothing about the clients.
        if (Settling)
        {
            _offlineStrikes.Clear();
            return;
        }
        foreach (var (id, info) in _running.ToList())
        {
            var account = FindAccount(id);
            // A status hidden by privacy settings or a failed request proves nothing; wait for a clear answer.
            // When the client's own log is followed, it is the better source (RejoinFromLogs).
            if ((account is not null && !reliable.Contains(account.UserId)) || LogFollowed(info.Pid))
                continue;
            if (account is null || !RejoinOn(account, info.Launch.PresetId) || account.Banned == true || info.Launch.Target is null || DateTime.UtcNow - info.Launch.StartedAt < DisconnectGrace
                || !_presence.TryGetValue(account.UserId, out var presence) || presence.Type is 2 or 4)
            {
                _offlineStrikes.Remove(id);
                continue;
            }
            int strikes = _offlineStrikes[id] = _offlineStrikes.GetValueOrDefault(id) + 1;
            if (strikes < 2)
                continue;
            _offlineStrikes.Remove(id);
            _closingForRejoin.Add(info.Pid); // Bloxkeep's own close: never taken for Roblox closing it
            Notify(L.F("{0} seems disconnected — restarting Roblox…", Name(account)), "info", "drop", account.Id);
            RobloxProcesses.Close(info.Pid);
        }
    }

    // ───────────────────────────── updates ─────────────────────────────

    void OnUpdaterChanged()
    {
        if (_disposed || !_owner.IsHandleCreated)
            return;
        try
        {
            _owner.BeginInvoke(new Action(() =>
            {
                NoteUpdateReady(); // Statistics › Activity
                PushState();
            }));
        }
        catch (InvalidOperationException)
        {
            // The window is closing.
        }
    }

    async Task<object?> CheckUpdatesAsync()
    {
        _nextUpdateCheck = DateTime.UtcNow + UpdateCheckInterval;
        await _updater.CheckAsync();
        return _updater.Dto();
    }

    /// <summary>Closes Bloxkeep, installs the downloaded version and opens it again. Roblox windows stay open.</summary>
    object? ApplyUpdate()
    {
        // A second click while the first one is on its way out does nothing.
        if (_applyingUpdate)
            return null;
        // The button can be older than the state (the page pushed before a failed re-check): say why nothing happens.
        if (_updater.Status != "ready")
            throw new UserError(_updater.Status is "checking" or "downloading"
                ? L.T("The update is still downloading. Try again in a moment.")
                : L.T("No update is ready to install. Check for updates again."));
        if (_launchCts is not null)
            throw new UserError(L.T("Wait until Bloxkeep finishes opening Roblox."));
        if (!SaveBeforeClosing())
            throw new UserError(L.T("Couldn't save your changes, so the update waits. Try again in a moment."));
        // The updater ends the process at once; the window's own cleanup (the tray icon, the clipboard, a rejoin
        // getting its ticket) doesn't run.
        _applyingUpdate = true;
        _rejoinCts?.Cancel();
        Ui.ClearSecretClipboard();
        BeforeRestart?.Invoke();
        try
        {
            _updater.RestartToUpdate();
        }
        catch (Exception ex)
        {
            Log.Write(ex);
            _applyingUpdate = false;
            RestartFailed?.Invoke();
            throw new UserError(L.T("Couldn't restart to update. It installs by itself when you quit Bloxkeep."));
        }
        return null;
    }

    bool _applyingUpdate;

    /// <summary>
    /// Launches whose client quit early and are waiting for the client it may have handed over to, with the pid
    /// that quit (its children are the first choice) and what its log said last.
    /// </summary>
    readonly Dictionary<PendingLaunch, (int Pid, RobloxLogWatcher.ClientState? Log)> _handedOver = new(ReferenceEqualityComparer.Instance);

    /// <summary>
    /// Launches whose window didn't come within <see cref="PendingTimeout"/>: a window that appears for one within
    /// <see cref="LateTimeout"/> more is still claimed for it (see TrackProcesses), so its client isn't left untracked.
    /// </summary>
    readonly HashSet<PendingLaunch> _late = new(ReferenceEqualityComparer.Instance);
    static readonly TimeSpan LateTimeout = TimeSpan.FromMinutes(5);

    /// <summary>
    /// Stops waiting for a launch; one that was already running ends its session (auto-rejoin, toast). A rejoin whose
    /// client never got a window is said so, since its account is otherwise left with no word and no client.
    /// </summary>
    /// <param name="late">Its window may still come (see <see cref="_late"/>): a scheduled stop keeps the account until then.</param>
    void DropPending(PendingLaunch launch, bool late = false)
    {
        _pending.Remove(launch);
        bool closed = _closeWhenClaimed.Remove(launch);
        if (_handedOver.Remove(launch, out var handed) && !closed)
        {
            OnSessionEnded(launch.AccountId, launch, handed.Log);
            return;
        }
        if (launch.Rejoin && !closed && FindAccount(launch.AccountId) is { } account)
            Notify(L.F("{0} didn't rejoin: no Roblox window appeared.", Name(account)), "warn", "rejoinFail", account.Id);
        if (!late && !_running.ContainsKey(launch.AccountId))
            ForgetScheduledOpen(launch.AccountId); // no client came of it: a scheduled stop has nothing of this account's to close
    }

    /// <summary>
    /// Remembers which version last ran. When that is older than this one, the app was updated and the page shows
    /// the release notes in between, once; a fresh install has nothing remembered and shows nothing.
    /// </summary>
    void NoteVersion()
    {
        string current = _updater.CurrentVersion;
        // The updater swapped the program files and rewrote the shortcuts; Explorer may keep showing a blank icon.
        if (Prefs.LastVersion is { } previous && previous != current && _updater.Supported)
            Ui.RefreshShellIcons();
        if (Prefs.LastVersion is { } last && WhatsNew.Parse(last) < WhatsNew.Parse(current) && WhatsNew.Between(last, current).Length > 0)
        {
            _updatedFrom = last;
            return;
        }
        // Only ever raised: an older build run afterwards (a development copy) must not make "what's new" show again.
        if (Prefs.LastVersion is not { } seen || WhatsNew.Parse(seen) < WhatsNew.Parse(current))
        {
            Prefs.LastVersion = current;
            Prefs.Save();
        }
    }

    object? SeenWhatsNew()
    {
        _updatedFrom = null;
        Prefs.LastVersion = _updater.CurrentVersion;
        Prefs.Save();
        return null;
    }

    object? SetAutoUpdate(bool enabled)
    {
        Prefs.CheckUpdates = enabled;
        Prefs.Save();
        if (enabled)
            _nextUpdateCheck = DateTime.UtcNow;
        PushState();
        return null;
    }

    /// <summary>The PC woke up less than a minute ago, or has no network: auto-rejoin holds off.</summary>
    bool Settling => DateTime.UtcNow - _resumedAt < ResumeGrace || !NetworkUp;

    /// <summary>
    /// Kept up to date by Windows' notification: asking directly lists every network adapter, too slow for a check
    /// that runs on each process tick.
    /// </summary>
    volatile bool _networkUp = NetworkInterface.GetIsNetworkAvailable();

    void OnNetworkAvailabilityChanged(object? sender, NetworkAvailabilityEventArgs e) => _networkUp = e.IsAvailable;

    DateTime _networkAskedAt;
    static readonly TimeSpan NetworkAskInterval = TimeSpan.FromSeconds(30);

    /// <summary>
    /// Windows' word on the network, asked again every 30 s while it says "down": an "available" notification that
    /// never came (a wake, a Wi-Fi flap, two notifications in the wrong order) would otherwise hold auto-rejoin and
    /// the sign-in check back until the next change.
    /// </summary>
    bool NetworkUp
    {
        get
        {
            var now = DateTime.UtcNow;
            if (!_networkUp && (now - _networkAskedAt >= NetworkAskInterval || now < _networkAskedAt))
            {
                _networkAskedAt = now;
                _networkUp = NetworkInterface.GetIsNetworkAvailable();
            }
            return _networkUp;
        }
    }

    /// <summary>The time zone was changed: local times (the schedules' minutes) must come from the new one, not a cached one.</summary>
    static void OnTimeChanged(object? sender, EventArgs e) => TimeZoneInfo.ClearCachedData();

    /// <summary>
    /// After sleep every client has lost its connection and the network takes a moment to come back. Disconnect
    /// detection and auto-rejoin wait for it, so the clients aren't all restarted into a dead network at once.
    /// </summary>
    void OnPowerModeChanged(object? sender, PowerModeChangedEventArgs e)
    {
        if (_disposed)
            return;
        try
        {
            // Going to sleep: a pending save and backup are written first (the PC may never wake up the same way);
            // waited for, since Windows suspends right after this. Waking: on the window's thread, when it gets there.
            if (e.Mode == PowerModes.Suspend)
                _owner.Invoke(new Action(OnSuspending));
            else if (e.Mode == PowerModes.Resume)
                _owner.BeginInvoke(new Action(OnResumed));
        }
        catch (InvalidOperationException)
        {
            // The window is closing.
        }
    }

    void OnSuspending()
    {
        _asleepTick = Environment.TickCount64;
        if (_vault is null)
        {
            // Stays left open by the lock end at this sleep, once unlocked (see ResumePlaySessions).
            _sleptWhileLocked ??= DateTime.UtcNow;
            return;
        }
        // Open stays end here: time asleep isn't play time, and the drop every client logs at wake isn't a kick.
        if (SuspendPlay(DateTime.UtcNow) | NoteOpenSessions() | NoteScheduledRuns() | _saveDue is not null)
            Persist();
        BackupIfDirty();
    }

    void OnResumed()
    {
        _asleepTick = 0;
        _resumedAt = DateTime.UtcNow;
        _offlineStrikes.Clear();
        _nextPresence = _resumedAt + ResumeGrace;
        if (_nextUpdateCheck > _resumedAt + TimeSpan.FromMinutes(2))
            _nextUpdateCheck = _resumedAt + TimeSpan.FromMinutes(2);
        Resumed?.Invoke();
        if (_vault is not null && (_rejoinQueue.Count > 0 || _running.Any(r => FindAccount(r.Key) is { } account && RejoinOn(account, r.Value.Launch.PresetId))))
            Toast(L.T("The PC woke up — auto-rejoin waits until the connection is back."));
    }

    /// <summary>Raised on the UI thread when the PC wakes from sleep.</summary>
    public Action? Resumed { get; set; }

    void RefreshPresenceSoon(TimeSpan delay)
    {
        var at = DateTime.UtcNow + delay;
        if (at < _nextPresence)
            _nextPresence = at;
    }

    /// <summary>
    /// Asks Roblox which accounts are online and what they are playing. One request covers everyone;
    /// accounts whose game is hidden by privacy settings are asked again with their own cookie.
    /// </summary>
    async Task<object?> RefreshPresenceAsync()
    {
        var vault = _vault;
        if (vault is null)
            return null;
        if (_presenceBusy)
            return new { skipped = true }; // one is under way; its answer is on its way to the page
        // In the background (nobody is looking) only the clients open here whose log isn't followed are asked —
        // auto-rejoin relies on their status — and everyone else waits for the full round every few minutes.
        bool light = _background && _presenceLoaded && DateTime.UtcNow - _lastFullPresence < BackgroundPresenceInterval;
        var accounts = vault.Data.Accounts
            .Where(a => a.UserId > 0 && (!light || _running.TryGetValue(a.Id, out var run) && !LogFollowed(run.Pid)))
            .ToList();
        _nextPresence = DateTime.UtcNow + PresenceInterval;
        if (accounts.Count == 0)
            return null;

        _presenceBusy = true;
        var viewer = accounts.FirstOrDefault(a => a.CookieValid != false && a.Cookie.Length > 0 && a.Realm == Realm.Global);
        try
        {
            var result = await RobloxApi.GetPresenceAsync(viewer?.Cookie, accounts.Select(a => a.UserId).ToList());
            // First answer since unlocking: show it now instead of "status unknown" until every follow-up request
            // (accounts hidden by privacy, game names) is back; those fill in below.
            if (!_presenceLoaded && _vault == vault)
            {
                foreach (var (userId, presence) in result)
                    _presence[userId] = presence;
                _presenceLoaded = true;
                _bootStatusDone = true;
                AdvanceBoot();
                PushStateIfChanged();
            }

            // Privacy settings can hide an account from the others. Accounts with a client open here are asked with
            // their own cookie (auto-rejoin relies on it) unless that client's log is followed, which already says
            // what it is doing; others only when their game is hidden.
            var askOwn = accounts.Where(a => a != viewer && a.CookieValid != false && a.Cookie.Length > 0
                                             && !(_running.TryGetValue(a.Id, out var run) && LogFollowed(run.Pid))
                                             && (a.Realm == Realm.Vng || result.TryGetValue(a.UserId, out var p)
                                                 && (_running.ContainsKey(a.Id) ? p.Type != 2 || p.PlaceId is null : p is { Type: 2, PlaceId: null })))
                .ToList();
            askOwn = askOwn.Where(a => _running.ContainsKey(a.Id)).Concat(askOwn.Where(a => !_running.ContainsKey(a.Id)).Take(10)).ToList();
            var reliable = result.Where(r => r.Value.Type == 2).Select(r => r.Key).ToHashSet();
            if (viewer is not null)
                reliable.Add(viewer.UserId);
            // Accounts whose own request was rejected: at most one is marked per round, after a second call agrees.
            var rejected = new List<Account>();
            using (var gate = new SemaphoreSlim(4))
            {
                await Task.WhenAll(askOwn.Select(async account =>
                {
                    await gate.WaitAsync();
                    try
                    {
                        var own = await RobloxApi.GetPresenceAsync(account.Cookie, [account.UserId], account.Realm);
                        if (own.TryGetValue(account.UserId, out var p))
                        {
                            result[account.UserId] = p;
                            reliable.Add(account.UserId);
                        }
                    }
                    catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
                    {
                        // Keep what the other account saw; auto-rejoin won't act on it.
                        if (ex is RobloxApiException { CookieInvalid: true })
                            rejected.Add(account);
                    }
                    finally
                    {
                        gate.Release();
                    }
                }));
            }
            if (rejected.Count > 0 && _vault == vault)
                await ConfirmSignedOutAsync(rejected[0]); // the rest are asked again next round

            await LoadGamesAsync(result.Values
                .Where(p => (p.RootPlaceId ?? p.PlaceId) > 0)
                .Select(p => ((p.RootPlaceId ?? p.PlaceId)!.Value, p.UniverseId ?? 0))
                .Concat(_running.Values.Select(r => r.Launch).Concat(_pending)
                    .Where(l => l.Target is { PlaceId: > 0 })
                    .Select(l => (l.Target!.PlaceId, 0L)))
                // Many games move players to another place of the same game after joining; the log has that one.
                .Concat(_running.Values.Select(r => _logs.StateOf(r.Pid)?.PlaceId ?? 0).Where(id => id > 0).Select(id => (id, 0L)))
                .ToList());

            if (_vault != vault)
                return null;
            // A light round only updates the accounts it asked about.
            if (!light)
            {
                _presence.Clear();
                _presenceReliable.Clear();
                _lastFullPresence = DateTime.UtcNow;
            }
            foreach (var (userId, presence) in result)
                _presence[userId] = presence;
            _presenceReliable.UnionWith(reliable);
            _presenceLoaded = true;
            DetectDisconnectedSessions(reliable);
            PushStateIfChanged();
        }
        // The account asking was signed out meanwhile: it is marked so (the next check asks with another one) — once
        // a second call agrees, so an outage of Roblox's sign-in service doesn't sign everyone out here one by one.
        catch (RobloxApiException ex) when (ex.CookieInvalid && viewer is not null && _vault == vault)
        {
            await ConfirmSignedOutAsync(viewer);
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            _nextPresence = DateTime.UtcNow + PresenceInterval * 2;
        }
        finally
        {
            _presenceBusy = false;
            // A failed first try leaves the accounts as "status unknown" instead of loading forever.
            if (!_presenceLoaded)
            {
                _bootStatusDone = true;
                AdvanceBoot();
                PushStateIfChanged();
            }
        }
        return null;
    }

    /// <summary>
    /// One rejected request isn't proof that an account was signed out (an outage of Roblox's sign-in service answers
    /// 401 for everyone): it is marked so only when the cheap "who am I" call, the one the sign-in check uses, rejects
    /// the cookie too. Without a clear answer it is left as it was.
    /// </summary>
    async Task<bool> ConfirmSignedOutAsync(Account account, CancellationToken ct = default)
    {
        string cookie = account.Cookie;
        try
        {
            if (await RobloxApi.GetAuthenticatedUserAsync(cookie, account.Realm, ct) is not null)
                return false;
        }
        catch (Exception ex) when (!ct.IsCancellationRequested && RobloxApi.IsExpectedError(ex))
        {
            return false;
        }
        // Signed in again meanwhile: the old cookie's answer says nothing about the new one.
        if (account.Cookie != cookie)
            return false;
        account.CookieValid = false;
        PersistSoon();
        return true;
    }

    // ───────────────────────────── settings ─────────────────────────────

    object? SetSettings(JsonElement a)
    {
        var s = RequireVault().Data.Settings;
        if (OptStr(a, "lastPlaceInput") is { } place) s.LastPlaceInput = place.Trim();
        if (OptStr(a, "lastJobId") is { } job) s.LastJobId = job.Trim();
        if (OptStr(a, "layout") is "cards" or "list") s.Layout = OptStr(a, "layout")!;
        if (OptStr(a, "sort") is "name" or "status" or "recent" or "order") s.Sort = OptStr(a, "sort")!;
        // Sent while typing in the game box, or when the list's look changes: the page already shows it, so just
        // remember it.
        if (a.ValueKind == JsonValueKind.Object && a.EnumerateObject().Any()
            && a.EnumerateObject().All(p => p.Name is "lastPlaceInput" or "lastJobId" or "layout" or "sort"))
        {
            PersistSoon();
            return null;
        }
        if (OptBool(a, "renameWindows") is { } rename) s.RenameWindows = rename;
        if (OptStr(a, "windowLayout") is "fill" or "small" or "medium" or "large") s.WindowLayout = OptStr(a, "windowLayout")!;
        // "custom" needs a launcher picked with PickLauncher; the page never supplies a path itself.
        // "other:<name>" only for a launcher actually found here, so the page can't name an arbitrary program.
        if (OptStr(a, "launchVia") is { } via && RobloxLauncher.IsValidOption(via) && (via != "custom" || File.Exists(s.CustomLauncher))
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
        if (Has(a, "launchDelaySeconds", JsonValueKind.Number)) s.LaunchDelaySeconds = (int)Math.Clamp(Long(a, "launchDelaySeconds"), 0, 120);
        if (OptBool(a, "multiInstance") is { } multi && multi != s.MultiInstance)
        {
            s.MultiInstance = multi;
            if (multi)
                MultiInstance.Enable();
            else
                MultiInstance.Disable();
            _lastArmed = MultiInstance.Armed;
        }
        // Both take effect at the next launch; the person's own fullscreen setting is put back by the process check.
        if (OptBool(a, "multiWindowed") is { } windowed)
            s.MultiInstanceWindowed = windowed;
        if (OptBool(a, "multiTile") is { } tile)
            s.MultiInstanceTile = tile;
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

    /// <summary>
    /// "Choose a launcher…": any program the user picks in Windows' own file dialog. It is then started with the
    /// roblox-player: link as its first argument, like Windows does for the program that handles those links.
    /// </summary>
    object? PickLauncher()
    {
        var s = RequireVault().Data.Settings;
        using var dialog = new OpenFileDialog
        {
            Title = L.T("Choose the program that opens Roblox"),
            Filter = L.T("Programs") + " (*.exe)|*.exe",
            InitialDirectory = File.Exists(s.CustomLauncher) ? Path.GetDirectoryName(s.CustomLauncher)
                : Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        };
        if (dialog.ShowDialog(_owner) == DialogResult.OK)
        {
            if (string.Equals(Path.GetFullPath(dialog.FileName), Path.GetFullPath(Application.ExecutablePath), StringComparison.OrdinalIgnoreCase))
                throw new UserError(L.T("Pick the program that opens Roblox, not Bloxkeep itself."));
            s.CustomLauncher = dialog.FileName;
            s.LaunchVia = "custom";
            _installs = null;
            Persist();
        }
        // Also when cancelled: the page put its list back on the previous choice and waits for this.
        PushState();
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
        SystemEvents.PowerModeChanged -= OnPowerModeChanged;
        SystemEvents.TimeChanged -= OnTimeChanged;
        NetworkChange.NetworkAvailabilityChanged -= OnNetworkAvailabilityChanged;
        _updater.Changed -= OnUpdaterChanged;
        _procTimer.Dispose();
        SaveBeforeClosing(); // normally done already, before the window agreed to close
        _backupTimer?.Dispose();
        DropVaultImport();
        // A downloaded update is installed once the app has closed; the next start is the new version.
        _updater.ApplyOnExit();
        Ui.ClearSecretClipboard();
        _launchCts?.Cancel();
        _rejoinCts?.Cancel();
        // The Discord status message says Bloxkeep closed, if Discord answers within a moment.
        try
        {
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
