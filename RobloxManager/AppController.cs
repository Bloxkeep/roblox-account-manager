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
        /// <summary>The browser tracker id in its launch link, which its client's command line carries (see <see cref="ClaimWindows"/>); null when not known.</summary>
        public string? Tracker { get; init; }
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
    /// <summary>A refused launch ticket the sign-in check didn't confirm as a sign-out: tried again this many times, then said.</summary>
    const int MaxTicketRetries = 3;
    /// <summary>What a refused launch ticket is called when the sign-in check says the account is still signed in.</summary>
    static string RefusedText => L.T("Roblox refused to open the game for this account. Try again in a moment.");
    /// <summary>
    /// Like the Claude app: a new version is found within the hour, then downloaded and installed by itself. One small
    /// request to GitHub's API each time, well within its 60 an hour for anonymous callers (several PCs on one IP too).
    /// </summary>
    static readonly TimeSpan UpdateCheckInterval = TimeSpan.FromHours(1);
    static readonly TimeSpan RobloxVersionInterval = TimeSpan.FromMinutes(30);
    static readonly TimeSpan MemoryWarningInterval = TimeSpan.FromMinutes(5);
    // Scanning launcher folders and reading the client's file version is the slow part; it is also re-read
    // whenever the launch option changes or a vault is opened.
    static readonly TimeSpan InstallCacheLifetime = TimeSpan.FromMinutes(3);
    static readonly TimeSpan RegistryCacheLifetime = TimeSpan.FromSeconds(30);
    static readonly TimeSpan SaveDelay = TimeSpan.FromSeconds(3);
    static readonly TimeSpan BackgroundSweep = TimeSpan.FromSeconds(15);
    const int MemoryWarningLoad = 92;
    const long DefaultClientMb = 900;
    /// <summary>
    /// What a Roblox client holds of Windows' commit limit once in a game, when none runs to go by: 2.1–2.4 GB seen on 0.743,
    /// 3.05–3.16 GB on 0.744 (the three biggest in Windows' low-virtual-memory event of 2026-10-22).
    /// </summary>
    const long DefaultClientCommitMb = 3072;
    /// <summary>Below this much of the commit limit left, programs start failing; Windows itself warns at about this point.</summary>
    const long CommitMarginMb = 1024;
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
    /// <summary>
    /// A client that started its game and got no server in this time isn't getting one: the server it was sent to has
    /// ended or is full (Roblox's join queue), or the join failed. Seen 2026-10-17 (0.743): five clients sent to Steal
    /// An Egg sat on "This experience has ended, or the server became unavailable…" for three minutes without a
    /// "Joining game" line, until Bloxkeep's presence check closed them.
    /// </summary>
    static readonly TimeSpan GetInTimeout = TimeSpan.FromSeconds(60);
    /// <summary>How long past its time a rejoin kept with the vault is still done after an unlock (see OnUnlocked).</summary>
    static readonly TimeSpan WaitingRejoinKept = TimeSpan.FromMinutes(10);
    /// <summary>
    /// When a join not in a server yet is asked about (<see cref="CheckJoinAsync"/>). Most joins name their server
    /// within 1-2 s, but a join waiting for room logs nothing just as a refused one does: on 2026-10-16 four accounts
    /// sent to one server waited 17-38 s, silent, then got in. Asking Roblox in that wait might upset it (not known), so
    /// a first-time refusal is asked about after the longest wait seen; an account the game is known to have banned
    /// (opened anyway) at once, so its ban is told right away (the owner's wish, 2026-10-21).
    /// </summary>
    TimeSpan JoinCheckAfter(Account account, GameTarget target) =>
        GameBanOf(account, target.PlaceId) is not null ? TimeSpan.FromSeconds(5) : TimeSpan.FromSeconds(40);
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
    /// <summary>
    /// Roblox's DisconnectRejoin (276): the client says the same as for <see cref="SameAccountElsewhere"/> ("Same account
    /// launched experience from different device"), so it is taken the same way.
    /// </summary>
    static bool SignedInElsewhere(int? code) => code is SameAccountElsewhere or 276;
    /// <summary>
    /// Roblox's codes whose own error box has no Reconnect button (reconnectDisabledList in its Connection.lua, client
    /// 0.744): a rejoin would only be refused again (no access to a private server any more, Roblox out of date, the same
    /// player twice, security checks, Roblox's join refusals). Bloxkeep doesn't rejoin them either, except a game's kick
    /// (267, also on Roblox's list), which games often ask for ("please rejoin the game") and auto-rejoin is for.
    /// </summary>
    static readonly HashSet<int> NoReconnect =
    [
        5, 6, 7, 8, 9, 257, 258, 259, 263, 264, 265, 268, 270, 272, 280, 281, 282, 284, 286, 294, 295, 304, 305, 306, 307,
        317, 318, 320, 321, 522, 523, 524, 525, 526, 527, 533, 541, 542, 543, 544, 600, 601,
    ];
    static bool RobloxWontReconnect(int? code) => code is int c && NoReconnect.Contains(c);
    /// <summary>Roblox's ServerShutdown: the game's server closed (an update, the developer's shutdown).</summary>
    const int ServerShutdown = 288;
    /// <summary>
    /// Roblox's codes for a connection that failed on the way: the ones its own error screen reconnects by itself
    /// (autoReconnectAllowedList in its Connection.lua: the packet errors 260-262, Timeout, HashTimeout, ConnectionLost,
    /// RaknetErrors, ReplicatorTimeout, NetworkInternal/Send/Timeout). Only these are said as a lost connection; a
    /// disconnect without a code is said as one whose reason isn't known.
    /// </summary>
    static bool LostConnection(int code) => code is 260 or 261 or 262 or 266 or 269 or 277 or 279 or 290 or 296 or 297 or 298;
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

    enum AfkHeld { InUse, FullScreen, Locked, Refused }

    /// <summary>Anti-AFK has seen the windows that were already open when Bloxkeep started (or was unlocked).</summary>
    bool _afkAdopted;

    /// <summary>
    /// Why anti-AFK last held back from a window that was due, since a nudge last reached it: said if Roblox kicks
    /// that window for idling anyway.
    /// </summary>
    readonly Dictionary<int, AfkHeld> _afkHeld = new();
    /// <summary>Roblox windows currently set to a low memory priority (they are in the background).</summary>
    readonly HashSet<int> _lowMemoryPriority = new();
    /// <summary>The Roblox settings file holds values Bloxkeep wrote; the person's own come back when Roblox is closed.</summary>
    bool _robloxSettingsPending = RobloxGameSettings.HasBaseline;
    DateTime _robloxSettingsWrittenAt;
    bool _nudging;
    readonly HashSet<Guid> _userClosed = new();
    readonly List<RejoinRequest> _rejoinQueue = new();
    /// <summary>Rejoins of each account since its client last stayed in a game a while (see <see cref="KeepsDropping"/>).</summary>
    readonly Dictionary<Guid, int> _rejoinsInARow = new();
    readonly Dictionary<Guid, int> _offlineStrikes = new();
    readonly AppUpdater _updater = new();
    /// <summary>
    /// The first check at the first tick (one small request): an update found before the vault is opened installs at
    /// the lock screen (see <see cref="UpdateOffersAtLock"/>), so the sooner it is known, the likelier it comes in time.
    /// </summary>
    DateTime _nextUpdateCheck = DateTime.UtcNow;
    DateTime _nextRobloxVersionCheck;
    RobloxInstalls.ClientVersion? _latestRoblox;
    /// <summary>The version that ran before an update, until the page has shown what's new since then.</summary>
    string? _updatedFrom;
    Vault? _vault;
    CancellationTokenSource? _launchCts, _rejoinCts;
    /// <summary>
    /// While a launch of several accounts waits between two of them: the accounts it has still to open. Rejoins of the
    /// others go meanwhile (see RunDueRejoin); null while it opens one, and when no launch is going.
    /// </summary>
    HashSet<Guid>? _launchWaiting;
    bool _bootStatusDone, _bootAvatarsDone;
    BootProgress? _boot;
    bool _checking, _lastArmed, _presenceBusy, _presenceLoaded, _rejoining, _trimRefusedShown, _disposed;
    DateTime _nextPresence, _lastFullPresence, _lastMemoryWarning, _installsReadAt, _sweepBackgroundUntil, _resumedAt, _lastSessionStart, _lastTick;
    string? _lastMetrics, _lastState, _pageView;
    InstallsInfo? _installs;
    /// <summary>The launcher that can update the client the launch option starts (<see cref="RobloxInstalls.UpdaterFor"/>).</summary>
    string? _updaterExe;
    bool _robloxUpdating;
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
        // The update check before anything else (not at the first tick): one found before the vault is opened takes the
        // lock screen's place, so the sooner it's known, the less likely someone has started typing there.
        if (Prefs.CheckUpdates && _updater.Supported)
        {
            _nextUpdateCheck = DateTime.UtcNow + UpdateCheckInterval;
            _ = _updater.CheckAsync();
        }
        WatchForeground();
        WatchDrags();
        // Multi-Roblox before the master password (see Prefs.MultiRobloxAtStart).
        if (Prefs.MultiRobloxAtStart && !MultiInstance.Armed)
            MultiInstance.Enable();
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

    /// <summary>
    /// The page sleeps while the window is in the background (ShellForm suspends it): no state is built or sent to it
    /// (that would wake it, and it is the costly part of a day in the tray), and events wait, the latest few, with every
    /// reply to a command it sent before. Woken, it gets those, one fresh state and the memory figures.
    /// </summary>
    public bool PageAsleep
    {
        get => _pageAsleep;
        set
        {
            string[] missed;
            lock (_asleepEvents)
            {
                if (_pageAsleep == value)
                    return;
                _pageAsleep = value;
                if (value)
                    return;
                missed = _asleepEvents.ToArray();
                _asleepEvents.Clear();
            }
            foreach (string json in missed)
                Post(json);
            _lastMetrics = null; // sent again at the next look, though unchanged
            PushState();
        }
    }
    volatile bool _pageAsleep;
    readonly List<string> _asleepEvents = new();
    const int MaxAsleepEvents = 30;

    /// <summary>The sleeping page went (the browser closed, or a new page): what waited for it goes too.</summary>
    void ForgetSleep()
    {
        lock (_asleepEvents)
        {
            _pageAsleep = false;
            _asleepEvents.Clear();
        }
    }

    // ───────────────────────────── bridge ─────────────────────────────

    void Post(string json)
    {
        if (_disposed)
            return;
        if (_pageAsleep)
        {
            // States aren't sent to a sleeping page at all (PushState); an event waits for it to wake, the newest kept. A
            // reply always waits: the page's command would otherwise never finish.
            lock (_asleepEvents)
            {
                if (_pageAsleep)
                {
                    _asleepEvents.Add(json);
                    if (_asleepEvents.Count > MaxAsleepEvents && _asleepEvents.FindIndex(j => j.StartsWith("{\"type\":\"event\"", StringComparison.Ordinal)) is >= 0 and var oldest)
                        _asleepEvents.RemoveAt(oldest);
                    return;
                }
            }
        }
        _post(json);
    }

    /// <summary>An event for the page; none while the screen is locked (a toast could name an account on the lock screen).</summary>
    void Emit(string name, object? data)
    {
        if (!_screenLocked || name == "saveView")
            Post(JsonSerializer.Serialize(new { type = "event", name, data }, Json));
    }
    void Toast(string text, string kind = "info") => Emit("toast", new { text, kind });
    void Progress(string? text) => Emit("progress", new { text });

    public void PushState()
    {
        RenameWebsites();
        if (PageAttached && !_pageAsleep)
            PostState(BuildState(), force: true);
    }

    /// <summary>
    /// For timers (presence, process checks): the page re-renders on every state, which is the costly part with
    /// hundreds of accounts, so an identical state isn't sent again. Commands still always push, so a control the
    /// page changed on its own is put back when the app refused the change.
    /// </summary>
    void PushStateIfChanged()
    {
        if (PageAttached && !_pageAsleep)
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

    /// <summary>
    /// The lock screen's own commands, the only ones that go through while the screen is locked (see ScreenLock): the
    /// rest would show or change what is behind it.
    /// </summary>
    static readonly HashSet<string> ScreenLockCommands = ["getState", "keepView", "pageBusy", "pageShown", "monitorArea", "setLanguage", "unlock", "lock"];

    /// <summary>A command the lock screen doesn't send, while it is up: refused, and its answer withheld.</summary>
    bool Withheld(string cmd) => _screenLocked && !ScreenLockCommands.Contains(cmd);

    async Task<object?> DispatchAsync(string cmd, JsonElement a)
    {
        if (Withheld(cmd))
            throw new UserError(L.T("Bloxkeep is locked."));
        object? result = await DispatchCommandAsync(cmd, a);
        // The lock screen came up while it ran (a launch, a password being shown): what it did stands, but its answer
        // (names, a password) doesn't reach the lock screen.
        if (Withheld(cmd))
            throw new UserError(L.T("Bloxkeep is locked."));
        return result;
    }

    Task<object?> DispatchCommandAsync(string cmd, JsonElement a) => cmd switch
    {
        "getState" => Done(InitialState()),
        "keepView" => Done(KeepView(Str(a, "view"), Bool(a, "restart"))),
        "pageBusy" => Done(SetPageBusy(Bool(a, "busy"))),
        "pageShown" => Done(OnPageShown()),
        "checkUpdates" => CheckUpdatesAsync(),
        "applyUpdate" => ApplyUpdateAsync(),
        "updateAtLock" => Done(SetUpdateAtLock(Bool(a, "go"))),
        "setAutoUpdate" => Done(SetAutoUpdate(Bool(a, "enabled"))),
        "setBetaUpdates" => Done(SetBetaUpdates(Bool(a, "enabled"))),
        "setCountOnline" => Done(SetCountOnline(Bool(a, "enabled"))),
        "whatsNew" => Done(WhatsNew.Between(null, _updater.CurrentVersion)),
        "seenWhatsNew" => Done(SeenWhatsNew()),
        "setLanguage" => Done(SetLanguage(Str(a, "language"))),
        "setTheme" => Done(SetTheme(Str(a, "theme"))),
        "setCustomTheme" => Done(SetCustomTheme(Str(a, "background"), Str(a, "accent"),
            Has(a, "fade", JsonValueKind.Number) && a.GetProperty("fade").TryGetInt32(out int fade) ? fade : null,
            Has(a, "blur", JsonValueKind.Number) && a.GetProperty("blur").TryGetInt32(out int blur) ? blur : null)),
        "pickThemePicture" => Done(PickThemePicture()),
        "clearThemePicture" => Done(ClearThemePicture()),
        "saveTheme" => Done(SaveTheme(Str(a, "name"))),
        "useSavedTheme" => Done(UseSavedTheme(Str(a, "id"))),
        "deleteSavedTheme" => Done(DeleteSavedTheme(Str(a, "id"))),
        "exportTheme" => Done(ExportTheme(Str(a, "id"))),
        "importTheme" => Done(ImportTheme()),
        "setCloseToTray" => Done(SetCloseToTray(Bool(a, "enabled"))),
        "setHotkeys" => Done(SetHotkeys(Bool(a, "enabled"))),
        "setHideNames" => Done(SetHideNames(Bool(a, "enabled"))),
        "createVault" => CreateVaultAsync(Str(a, "password"), Str(a, "name")),
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
        "solveBotCheck" => SolveBotCheckAsync(Id(a)),
        "switchServer" => SwitchServerAsync(Ids(a)),
        "hideWindows" => Done(HideWindows(Ids(a))),
        "showWindows" => Done(ShowWindows(Ids(a))),
        "refreshPresence" => RefreshPresenceAsync(),
        "copy" => Done(Copy(Id(a), Str(a, "field"))),
        "resolveGame" => ResolveGameAsync(Str(a, "input")),
        "searchGames" => SearchGamesAsync(Str(a, "query")),
        "gameIcons" => GameIconsAsync(a),
        "gameLists" => GameListsAsync(),
        "servers" => ServersAsync(Long(a, "placeId"), Bool(a, "fewest"), OptStr(a, "cursor")),
        "friendServers" => FriendServersAsync(Long(a, "placeId"), Ids(a)),
        "privateServers" => PrivateServersAsync(Long(a, "placeId"), Ids(a)),
        "pinGame" => Done(TogglePin(Long(a, "placeId"), Long(a, "universeId"), Str(a, "name"))),
        "saveServer" => Done(SaveServer(a)),
        "deleteServer" => Done(DeleteServer(Id(a))),
        "copyServerCode" => Done(CopyServerCode(Id(a))),
        "launch" => LaunchAsync(a),
        "cancelLaunch" => Done(CancelLaunch()),
        "savePreset" => SavePresetAsync(a),
        "deletePreset" => Done(DeletePreset(Id(a))),
        "runPreset" => RunPresetAsync(Id(a), Bool(a, "force")),
        "closePreset" => Done(ClosePresetWindows(Id(a))),
        "presetDiscordTest" => SendPresetDiscordTestAsync(Id(a), OptStr(a, "url")),
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
        "openWebsite" => Done(OpenWebsite(Id(a))),
        "openWebsiteBlank" => Done(OpenBlankWebsite()),
        "openDataFolder" => Done(OpenDataFolder()),
        "showPath" => Done(ShowPath(Str(a, "which"))),
        "monitorArea" => Done(MonitorAreaCommand(a)),
        "setNotifications" => Done(SetNotifications(a)),
        "setDailySignInCheck" => Done(SetDailySignInCheck(Bool(a, "enabled"))),
        "skipConfirm" => Done(SkipConfirm(Str(a, "key"))),
        "resetSkipConfirm" => Done(ResetSkipConfirm()),
        "toggleMini" => Done(ToggleMini()),
        "window" => Done(ControlWindow(Str(a, "action"))),
        "setBackup" => Done(SetBackup(a)),
        "pickBackupFolder" => Done(PickBackupFolder()),
        "openBackupFolder" => Done(OpenBackupFolder()),
        "backupSuggestions" => Done(RefreshBackupSuggestions()),
        "backupNow" => BackupNowAsync(),
        "restoreBackup" => Done(RestoreBackup()),
        "pickVault" => Done(ShowVaultPicker()),
        "closeVaultPicker" => Done(CloseVaultPicker()),
        "selectVault" => Done(SelectVault(Str(a, "id"))),
        "addVault" => Done(AddVault()),
        "cancelAddVault" => Done(CancelAddVault()),
        "renameVault" => Done(RenameVault(Str(a, "id"), Str(a, "name"))),
        "pickVaultPicture" => Done(PickVaultPicture(Str(a, "id"))),
        "clearVaultPicture" => Done(ClearVaultPicture(Str(a, "id"))),
        "removeVault" => Done(RemoveVault(Str(a, "id"))),
        "setAskVault" => Done(SetAskVault(Bool(a, "enabled"))),
        "pickLauncher" => Done(PickLauncher()),
        "playStats" => PlayStatsAsync((int)Long(a, "days")),
        "activity" => Done(ActivityList(a)),
        "exportActivity" => Done(ExportActivity(a)),
        "findPlayer" => FindPlayerAsync(a),
        "joinServer" => JoinServerAsync(a),
        "forgetPlayer" => Done(ForgetPlayer(Str(a, "name"))),
        "setDiscord" => Done(SetDiscord(a)),
        "setDiscordPresence" => Done(SetDiscordPresence(Bool(a, "enabled"))),
        "setDiscordWebhook" => Done(SetDiscordWebhook(Str(a, "url"))),
        "setRemote" => Done(SetRemote(Bool(a, "enabled"))),
        "remoteCode" => Done(RemoteNewCode()),
        "remoteAllow" => Done(RemoteAllow(Bool(a, "allow"))),
        "revealDiscordWebhook" => Done(RevealDiscordWebhook()),
        "discordTest" => SendDiscordNowAsync(summary: false),
        "discordSummaryNow" => SendDiscordNowAsync(summary: true),
        "openReleaseNotes" => Done(OpenReleaseNotes(Str(a, "version"))),
        "hideRobloxClosed" => Done(HideRobloxClosed()),
        "multiOnly" => Done(MultiOnly(Bool(a, "on"))),
        "updateRoblox" => Done(UpdateRoblox()),
        "openRobloxWatch" => Done(OpenUrl(RobloxWatch.DocsUrl)),
        // The user guide on GitHub, in the app's language (docs/guide.md, docs/guide.vi.md).
        "openGuide" => Done(OpenUrl($"https://github.com/Bloxkeep/roblox-account-manager/blob/main/docs/guide{(L.Language == "vi" ? ".vi" : "")}.md")),
        "openGitHub" => Done(OpenUrl("https://github.com/Bloxkeep/roblox-account-manager")),
        // Bloxkeep's Discord bot sends this on to an invite into the community server that works (services/DiscordBot/worker
        // invite()): a new one is made whenever the last stops working, so this address never changes.
        "openDiscord" => Done(OpenUrl("https://bloxkeep-bot.bloxkeep-app.workers.dev/discord")),
        // Bloxkeep's app added to the person's own Discord account, so /pc works in any DM (Control from Discord).
        "openDiscordApp" => Done(OpenUrl("https://discord.com/oauth2/authorize?client_id=1561788269933498408&integration_type=1&scope=applications.commands")),
        "feedbackPreview" => Done(FeedbackPreview(a)),
        "sendFeedback" => SendFeedbackAsync(a),
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
}
