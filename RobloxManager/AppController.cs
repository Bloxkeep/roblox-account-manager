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
public sealed class AppController : IDisposable
{
    sealed record PendingLaunch(Guid AccountId, HashSet<int> Before, DateTime StartedAt, GameTarget? Target);

    /// <param name="NetworkRetries">Attempts that failed only because there was no connection.</param>
    sealed record RejoinRequest(Guid AccountId, GameTarget Target, DateTime Due, int NetworkRetries = 0);

    static readonly TimeSpan HandOverWindow = TimeSpan.FromSeconds(60);
    static readonly TimeSpan PendingTimeout = TimeSpan.FromSeconds(120);
    static readonly TimeSpan PresenceInterval = TimeSpan.FromSeconds(30);
    static readonly TimeSpan RejoinDelay = TimeSpan.FromSeconds(10);
    static readonly TimeSpan RejoinWindow = TimeSpan.FromMinutes(15);
    static readonly TimeSpan DisconnectGrace = TimeSpan.FromMinutes(2);
    static readonly TimeSpan ResumeGrace = TimeSpan.FromSeconds(60);
    static readonly TimeSpan NetworkRetryDelay = TimeSpan.FromSeconds(30);
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
    const int MaxRejoins = 5;
    const int MemoryWarningLoad = 92;
    const long DefaultClientMb = 900;
    const long MB = 1024 * 1024;
    static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);

    readonly Form _owner;
    readonly Action<string> _post;
    readonly System.Windows.Forms.Timer _procTimer = new() { Interval = 1500 };
    readonly Dictionary<Guid, (int Pid, PendingLaunch Launch)> _running = new();
    readonly List<PendingLaunch> _pending = new();
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
    /// <summary>Roblox windows currently set to a low memory priority (they are in the background).</summary>
    readonly HashSet<int> _lowMemoryPriority = new();
    /// <summary>The Roblox settings file holds values Bloxkeep wrote; the person's own come back when Roblox is closed.</summary>
    bool _robloxSettingsPending = RobloxGameSettings.HasBaseline;
    DateTime _robloxSettingsWrittenAt;
    bool _nudging;
    readonly HashSet<Guid> _userClosed = new();
    readonly List<RejoinRequest> _rejoinQueue = new();
    readonly Dictionary<Guid, List<DateTime>> _rejoinHistory = new();
    readonly Dictionary<Guid, int> _offlineStrikes = new();
    readonly AppUpdater _updater = new();
    DateTime _nextUpdateCheck = DateTime.UtcNow + TimeSpan.FromSeconds(20);
    DateTime _nextRobloxVersionCheck;
    RobloxInstalls.ClientVersion? _latestRoblox;
    /// <summary>The version that ran before an update, until the page has shown what's new since then.</summary>
    string? _updatedFrom;
    Vault? _vault;
    CancellationTokenSource? _launchCts;
    bool _checking, _lastArmed, _presenceBusy, _rejoining, _trimRefusedShown, _disposed;
    DateTime _nextPresence, _lastTrim, _lastMemoryWarning, _installsReadAt, _sweepBackgroundUntil, _resumedAt, _lastSessionStart;
    string? _lastMetrics, _lastState, _pageView;
    InstallsInfo? _installs;
    DateTime? _saveDue;
    (bool Protocol, bool TrayAutostart)? _registry;
    DateTime _registryReadAt;

    public AppController(Form owner, Action<string> postJson)
    {
        _owner = owner;
        _post = postJson;
        _procTimer.Tick += (_, _) => OnProcTimer();
        _procTimer.Start();
        SystemEvents.PowerModeChanged += OnPowerModeChanged;
        NetworkChange.NetworkAvailabilityChanged += OnNetworkAvailabilityChanged;
        _updater.Changed += OnUpdaterChanged;
        NoteVersion();
    }

    bool _background;

    /// <summary>Set when the browser was recreated while the app kept running; the next page restores its view.</summary>
    public bool ResumingPage { get; set; }

    /// <summary>Recreates the interface drawing on the CPU (true) or the graphics card (false).</summary>
    public Func<bool, Task>? SetSafeRendering { get; init; }

    /// <summary>The window applies a new theme or close-to-tray choice.</summary>
    public Action? AppearanceChanged { get; init; }

    /// <summary>While the window is minimised, poll less often.</summary>
    public bool Background
    {
        set
        {
            _background = value;
            _procTimer.Interval = value ? 5000 : 1500;
        }
    }

    // ───────────────────────────── bridge ─────────────────────────────

    void Post(string json)
    {
        if (!_disposed)
            _post(json);
    }

    void Emit(string name, object? data) => Post(JsonSerializer.Serialize(new { type = "event", name, data }, Json));
    void Toast(string text, string kind = "info") => Emit("toast", new { text, kind });
    void Progress(string? text) => Emit("progress", new { text });
    public void PushState() => PostState(BuildState(), force: true);

    /// <summary>
    /// For timers (presence, process checks): the page re-renders on every state, which is the costly part with
    /// hundreds of accounts, so an identical state isn't sent again. Commands still always push, so a control the
    /// page changed on its own is put back when the app refused the change.
    /// </summary>
    void PushStateIfChanged() => PostState(BuildState(), force: false);

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
            string error = ex is UserError or RobloxApiException ? ex.Message
                : RobloxApi.IsExpectedError(ex) ? L.F("Couldn't reach Roblox: {0}", ex.Message)
                : L.F("Error: {0}", ex.Message);
            if (ex is not (UserError or RobloxApiException) && !RobloxApi.IsExpectedError(ex))
                Log.Write(ex);
            Post(JsonSerializer.Serialize(new { type = "reply", id, ok = false, error }, Json));
        }
    }

    Task<object?> DispatchAsync(string cmd, JsonElement a) => cmd switch
    {
        "getState" => Done(InitialState()),
        "keepView" => Done(KeepView(Str(a, "view"))),
        "checkUpdates" => CheckUpdatesAsync(),
        "applyUpdate" => Done(ApplyUpdate()),
        "setAutoUpdate" => Done(SetAutoUpdate(Bool(a, "enabled"))),
        "whatsNew" => Done(WhatsNew.Between(null, _updater.CurrentVersion)),
        "seenWhatsNew" => Done(SeenWhatsNew()),
        "setLanguage" => Done(SetLanguage(Str(a, "language"))),
        "setTheme" => Done(SetTheme(Str(a, "theme"))),
        "setCloseToTray" => Done(SetCloseToTray(Bool(a, "enabled"))),
        "createVault" => CreateVaultAsync(Str(a, "password")),
        "unlock" => UnlockAsync(Str(a, "password")),
        "lock" => Done(Lock()),
        "changePassword" => ChangePasswordAsync(Str(a, "current"), Str(a, "next")),
        "addViaBrowser" => AddViaBrowserAsync(),
        "quickLoginStart" => QuickLoginStartAsync(Realm.Normalize(Str(a, "realm"))),
        "quickLoginPoll" => QuickLoginPollAsync(),
        "quickLoginCancel" => Done(QuickLoginCancel()),
        "loginPassword" => LoginPasswordAsync(Str(a, "username"), Str(a, "password"), Realm.Normalize(Str(a, "realm"))),
        "loginTwoStep" => LoginTwoStepAsync(Str(a, "method"), Str(a, "code")),
        "addViaCookie" => AddViaCookieAsync(a),
        "importRam" => ImportRamAsync(),
        "updateAccount" => UpdateAccountAsync(a),
        "setAutoRejoin" => Done(SetAutoRejoin(Ids(a), Bool(a, "enabled"))),
        "setUseRobloxConfig" => Done(SetUseRobloxConfig(Ids(a), Bool(a, "enabled"))),
        "reveal" => Done(Reveal(Id(a), Str(a, "field"))),
        "deleteAccounts" => Done(DeleteAccounts(Ids(a))),
        "checkAccounts" => CheckAccountsAsync(Ids(a)),
        "refreshPresence" => RefreshPresenceAsync(),
        "copy" => Done(Copy(Id(a), Str(a, "field"))),
        "resolveGame" => ResolveGameAsync(Str(a, "input")),
        "searchGames" => SearchGamesAsync(Str(a, "query")),
        "gameLists" => GameListsAsync(),
        "servers" => ServersAsync(Long(a, "placeId"), Bool(a, "fewest"), OptStr(a, "cursor")),
        "pinGame" => Done(TogglePin(Long(a, "placeId"), Long(a, "universeId"), Str(a, "name"))),
        "launch" => LaunchAsync(a),
        "switchTo" => SwitchToAsync(Id(a), Bool(a, "force")),
        "cancelLaunch" => Done(CancelLaunch()),
        "closeAccounts" => Done(CloseAccounts(Ids(a))),
        "closeAll" => Done(CloseAll()),
        "arrangeWindows" => ArrangeWindowsAsync((int)Long(a, "monitor")),
        "setSettings" => Done(SetSettings(a)),
        "setRendering" => SetRenderingAsync(Bool(a, "gpu")),
        "disableTrayAutostart" => Done(DisableTrayAutostart()),
        "removeRecent" => Done(RemoveRecent(Long(a, "placeId"))),
        "openProfile" => Done(OpenUrl($"https://www.roblox.com/users/{Long(a, "userId")}/profile")),
        "openDataFolder" => Done(OpenDataFolder()),
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
        _lastMetrics = _lastState = null; // a new page has nothing yet; send both again
        string? view = resume && _vault is not null ? _pageView : null;
        _pageView = null;
        return new { resume, view, state = BuildState() };
    }

    /// <summary>
    /// What the page had on screen (selection, filter, search, scroll), kept in memory only while the browser is
    /// closed in the background, so a recreated page looks exactly the same. Never written to disk.
    /// </summary>
    object? KeepView(string view)
    {
        _pageView = view.Length <= 256 * 1024 ? view : null;
        return null;
    }

    object BuildState() => new
    {
        language = L.Language,
        theme = Prefs.Theme,
        themeApplied = Themes.Current.Id,
        closeToTray = Prefs.CloseToTray,
        safeRendering = Prefs.SafeRendering,
        update = _updater.Dto(),
        robloxVersion = RobloxVersionDto(),
        whatsNew = _updatedFrom is not null && _vault is not null
            ? new { from = _updatedFrom, releases = WhatsNew.Between(_updatedFrom, _updater.CurrentVersion) }
            : null,
        vaultExists = Vault.Exists,
        locked = _vault is null,
        dataPath = Vault.DataDir,
        robloxInstalled = RegistryInfo().Protocol,
        launching = _launchCts is not null,
        checking = _checking,
        multi = new
        {
            enabled = _vault?.Data.Settings.MultiInstance ?? false,
            armed = MultiInstance.Armed,
            trayAutostart = RegistryInfo().TrayAutostart,
        },
        installs = Installs(),
        monitors = Screen.AllScreens.Select((s, i) => new { index = i, s.Primary, width = s.Bounds.Width, height = s.Bounds.Height }).ToArray(),
        settings = _vault is null ? null : SettingsDto(_vault.Data.Settings),
        accounts = _vault?.Data.Accounts.Select(AccountDto).ToArray() ?? [],
    };

    /// <summary>Which Roblox clients and launchers exist, and the version of the client the chosen option starts.</summary>
    sealed record InstallsInfo(string? Handler, bool Bloxstrap, bool Fishstrap, bool Roblox, bool Direct, bool Store, RobloxInstalls.ClientVersion? Client);

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
        return _installs = new InstallsInfo(
            Handler: RobloxInstalls.ProtocolHandlerPath() is { } h ? Path.GetFileNameWithoutExtension(h) : null,
            Bloxstrap: RobloxInstalls.FindBootstrapper("Bloxstrap") is not null,
            Fishstrap: RobloxInstalls.FindBootstrapper("Fishstrap") is not null,
            Roblox: RobloxInstalls.FindOfficialClient() is not null,
            Direct: RobloxInstalls.FindAnyClient() is not null,
            Store: RobloxInstalls.StoreEditionInstalled(),
            Client: RobloxInstalls.VersionOf(RobloxInstalls.ClientFor(_vault?.Data.Settings.LaunchVia ?? "auto")));
    }

    /// <summary>
    /// The Roblox version the chosen launcher starts, next to the newest one Roblox serves. An older client updates
    /// itself on its next start (that first window takes longer), except when started directly.
    /// </summary>
    RobloxVersionInfo RobloxVersionDto()
    {
        var installed = Installs().Client;
        bool? upToDate = installed is null || _latestRoblox is null ? null
            : installed.Folder == _latestRoblox.Folder
              || (Version.TryParse(installed.Version, out var have) && Version.TryParse(_latestRoblox.Version, out var latest) && have >= latest);
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
        realm = a.Realm,
        a.CookieValid,
        a.LastUsed,
        a.AutoRejoin,
        a.UseRobloxConfig,
        avatarUrl = _avatarUrls.GetValueOrDefault(a.UserId),
        state = _running.ContainsKey(a.Id) ? "running" : _pending.Any(p => p.AccountId == a.Id) ? "launching" : "idle",
        presence = PresenceDto(a),
        session = _running.TryGetValue(a.Id, out var run)
            ? new
            {
                pid = run.Pid,
                since = run.Launch.StartedAt,
                game = GameDto(_logs.StateOf(run.Pid)?.PlaceId ?? run.Launch.Target?.PlaceId),
                connection = Connection(a, run.Pid, run.Launch),
                reason = _logs.StateOf(run.Pid) is { Status: "disconnected" } kicked ? kicked.Reason : null,
                jobId = _logs.StateOf(run.Pid)?.JobId,
            }
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
            return log.Status switch { "ingame" => "ingame", "joining" => "joining", "left" => "menu", _ => "disconnected" };
        bool known = _presence.TryGetValue(a.UserId, out var p);
        if (known && p!.Type == 2)
            return "ingame";
        if (!known || !_presenceReliable.Contains(a.UserId) || DateTime.UtcNow - launch.StartedAt < DisconnectGrace)
            return "joining";
        return launch.Target is null ? "menu" : "disconnected";
    }

    object? GameDto(long? placeId) => placeId is long id
        ? new { placeId = id, name = _games.GetValueOrDefault(id)?.Name ?? "", iconUrl = _games.GetValueOrDefault(id)?.IconUrl }
        : null;

    object? PresenceDto(Account a)
    {
        if (!_presence.TryGetValue(a.UserId, out var p))
            return null;
        long? placeId = p.RootPlaceId ?? p.PlaceId;
        var info = placeId is long id ? _games.GetValueOrDefault(id) : null;
        return new
        {
            type = p.Type switch { 0 => "offline", 1 => "online", 2 => "ingame", 3 => "studio", 4 => "invisible", _ => "unknown" },
            game = info?.Name is { Length: > 0 } name ? name : p.Location,
            placeId,
            jobId = p.JobId,
            iconUrl = info?.IconUrl,
            p.LastOnline,
        };
    }

    object SettingsDto(AppSettings s) => new
    {
        s.LastPlaceInput,
        s.LastJobId,
        s.RenameWindows,
        s.LaunchVia,
        s.LaunchDelaySeconds,
        s.MemoryWarnings,
        s.TrimBackground,
        s.AntiAfk,
        s.RobloxConfig,
        recentGames = s.RecentGames.Select(SavedGameDto),
        pinnedGames = s.PinnedGames.Select(SavedGameDto),
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

    void Persist()
    {
        _saveDue = null;
        if (_vault is null)
            return;
        try
        {
            _vault.Save();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Toast(L.F("Couldn't save data: {0}", ex.Message), "error");
        }
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
        OnUnlocked(await Task.Run(() => Vault.Create(password)));
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
            throw new UserError(ex.Message);
        }
        OnUnlocked(vault);
        return BuildState();
    }

    void OnUnlocked(Vault vault)
    {
        _vault = vault;
        var s = vault.Data.Settings;
        if (!RobloxLauncher.LaunchOptions.Contains(s.LaunchVia))
            s.LaunchVia = s.DirectLaunch ? "direct" : "auto";
        _installs = null; // re-read for this vault's launch option
        if (s.MultiInstance && !MultiInstance.Armed)
            MultiInstance.Enable();
        _lastArmed = MultiInstance.Armed;
        _nextPresence = DateTime.UtcNow;
        AdoptOpenSessions(vault);
        _ = LoadAvatarsAsync(vault.Data.Accounts.Select(a => a.UserId));
        _ = LoadRecentGamesAsync();
    }

    /// <summary>
    /// Remembers the clients that are open, so closing or restarting Bloxkeep (for example to update) doesn't lose
    /// track of them: they keep running, and the next unlock picks them up again, auto-rejoin included.
    /// </summary>
    void SaveOpenSessions()
    {
        if (_vault is not { } vault || (_running.Count == 0 && vault.Data.OpenSessions.Count == 0))
            return;
        vault.Data.OpenSessions = _running
            .Select(r => new OpenSession { AccountId = r.Key, Pid = r.Value.Pid, StartedAt = r.Value.Launch.StartedAt, Target = r.Value.Launch.Target })
            .ToList();
        Persist();
    }

    void AdoptOpenSessions(Vault vault)
    {
        var saved = vault.Data.OpenSessions;
        if (saved.Count == 0)
            return;
        var pids = RobloxProcesses.GetPlayerPids().ToHashSet();
        foreach (var session in saved)
        {
            if (pids.Contains(session.Pid) && vault.Data.Accounts.Any(a => a.Id == session.AccountId)
                && !_running.ContainsKey(session.AccountId) && !_running.Values.Any(r => r.Pid == session.Pid))
                _running[session.AccountId] = (session.Pid, new PendingLaunch(session.AccountId, [], session.StartedAt, session.Target));
        }
        saved.Clear();
        Persist();
    }

    object? Lock()
    {
        if (_saveDue is not null)
            Persist();
        SaveOpenSessions();
        _launchCts?.Cancel();
        QuickLoginCancel();
        _twoStep = null;
        _vault?.Dispose();
        _vault = null;
        _presence.Clear();
        _presenceReliable.Clear();
        _rejoinQueue.Clear();
        _offlineStrikes.Clear();
        _lastMetrics = _pageView = null;
        PushState();
        return null;
    }

    async Task<object?> ChangePasswordAsync(string current, string next)
    {
        var vault = RequireVault();
        if (next.Length < 6)
            throw new UserError(L.T("The new password needs at least 6 characters."));
        if (!await Task.Run(() => vault.VerifyPassword(current)))
            throw new UserError(L.T("The current master password is incorrect."));
        await Task.Run(() => vault.ChangePassword(next));
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
                   return L.F(result.Merged ? "{0} was already here — its sign-in was updated." : "{0} saved.", user.Name);
               }))
        {
            form.ShowDialog(_owner);
        }
        return new { added };
    }

    /// <summary>The code on screen in the Quick Login window; one at a time.</summary>
    QuickLoginCode? _quickLogin;
    bool _quickLoginFinishing;

    async Task<object?> QuickLoginStartAsync(string realm)
    {
        RequireVault();
        QuickLoginCancel();
        var login = await RobloxApi.CreateQuickLoginAsync(realm);
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
        var status = await RobloxApi.GetQuickLoginStatusAsync(login);
        if (!ReferenceEquals(login, _quickLogin))
            return new { status = "none" }; // the window asked for a new code meanwhile
        string? picture = Uri.TryCreate(status.PictureUrl, UriKind.Absolute, out var p) && p.Host.EndsWith(".rbxcdn.com") ? status.PictureUrl : null;
        switch (status.Status)
        {
            case "Validated":
                _quickLoginFinishing = true;
                try
                {
                    string cookie = await RobloxApi.FinishQuickLoginAsync(login);
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
                finally
                {
                    _quickLoginFinishing = false;
                    if (ReferenceEquals(login, _quickLogin))
                        _quickLogin = null;
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
            _quickLogin = null;
        return null;
    }

    // A two-step prompt waiting for the code; held only between loginPassword and loginTwoStep, never persisted.
    RobloxApi.TwoStepPrompt? _twoStep;

    async Task<object?> LoginPasswordAsync(string username, string password, string realm)
    {
        RequireVault();
        _twoStep = null;
        if (username.Trim().Length == 0 || password.Length == 0)
            throw new UserError(L.T("Enter the username and password."));
        var result = await RobloxApi.LoginWithPasswordAsync(username.Trim(), password, realm);
        if (result.Blocked == "captcha")
            return new { status = "captcha" };
        if (result.TwoStep is { } prompt)
        {
            _twoStep = prompt;
            return new { status = "twostep" };
        }
        return await SaveLoginAsync(result.Cookie!, realm);
    }

    async Task<object?> LoginTwoStepAsync(string method, string code)
    {
        RequireVault();
        if (_twoStep is not { } prompt)
            throw new UserError(L.T("Start the sign-in again."));
        if (code.Trim().Length == 0)
            throw new UserError(L.T("Enter the verification code."));
        string cookie = await RobloxApi.VerifyTwoStepAsync(prompt, method, code.Trim());
        _twoStep = null;
        return await SaveLoginAsync(cookie, prompt.Realm);
    }

    async Task<object> SaveLoginAsync(string cookie, string realm)
    {
        var user = await RobloxApi.GetAuthenticatedUserAsync(cookie, realm)
            ?? throw new UserError(L.T("Roblox didn't accept that sign-in. Please try again."));
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

    /// <summary>Adding an account that is already in the list just refreshes its cookie.</summary>
    AddResult AddOrMerge(Account incoming)
    {
        var vault = RequireVault();
        var existing = vault.Data.Accounts.FirstOrDefault(a => a.UserId != 0 && a.UserId == incoming.UserId);
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
        return new AddResult(account.Id, account.Label, existing is not null);
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
                (item.UserId != 0 && a.UserId == item.UserId) || (item.UserId == 0 && a.Cookie == cookie));
            if (existing is not null)
            {
                if (existing.Cookie != cookie)
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
        var account = RequireAccount(Id(a));
        RobloxUser? user = null;
        string? cookie = OptStr(a, "cookie") is { } raw ? RobloxApi.NormalizeCookie(raw) : null;
        if (cookie is { Length: > 0 } && cookie != account.Cookie)
        {
            user = await RobloxApi.GetAuthenticatedUserAsync(cookie, account.Realm)
                ?? throw new UserError(L.T("The new cookie is invalid or has expired."));
            if (_vault is null)
                throw new UserError(L.T("Bloxkeep was locked."));
        }

        account.Alias = Str(a, "alias").Trim();
        account.Group = Str(a, "group").Trim();
        account.Note = Str(a, "note").Trim();
        if (OptStr(a, "password") is { } password)
            account.Password = password;
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
                _rejoinHistory.Remove(account.Id);
            else
                _rejoinQueue.RemoveAll(r => r.AccountId == account.Id);
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
            _rejoinHistory.Remove(id);
            _offlineStrikes.Remove(id);
        }
        _pending.RemoveAll(p => set.Contains(p.AccountId));
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

        // Every continuation runs on the UI thread, so the queue and counters need no locking.
        async Task WorkAsync()
        {
            while (_vault == vault && queue.TryDequeue(out var account))
            {
                for (int attempt = 1; ; attempt++)
                {
                    if (resumeAt > DateTime.UtcNow)
                        await Task.Delay(resumeAt - DateTime.UtcNow);
                    try
                    {
                        var user = await RobloxApi.GetAuthenticatedUserAsync(account.Cookie, account.Realm);
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
        return new { ok, bad, failed };
    }

    object Copy(Guid id, string field)
    {
        var account = RequireAccount(id);
        (string value, bool secret, string missing) = field switch
        {
            "username" => (account.Username, false, "This account has no saved username."),
            "password" => (account.Password, true, "This account has no saved password."),
            _ => throw new UserError(L.T("Invalid field.")),
        };
        if (value.Length == 0)
            throw new UserError(L.T(missing));
        if (!Ui.CopyToClipboard(value, secret))
            throw new UserError(L.T("Couldn't access the clipboard; try again."));
        return new { secret };
    }

    async Task LoadAvatarsAsync(IEnumerable<long> userIds)
    {
        var missing = userIds.Where(id => id > 0 && !_avatarUrls.ContainsKey(id)).Distinct().ToList();
        if (missing.Count == 0)
            return;
        try
        {
            foreach (var (id, url) in await RobloxApi.GetHeadshotUrlsAsync(missing))
                _avatarUrls[id] = url;
            PushState();
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
        }
    }

    // ───────────────────────────── games ─────────────────────────────

    static readonly TimeSpan GameRetry = TimeSpan.FromMinutes(2);

    /// <summary>Whether the cache can answer for a place; <paramref name="fresh"/> wants a live player count.</summary>
    bool HasGame(long placeId, bool fresh) =>
        _games.TryGetValue(placeId, out var cached)
        && ((cached is not null && !fresh) || DateTime.UtcNow - _gameFetchedAt.GetValueOrDefault(placeId) < GameRetry);

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
            string? cookie = _vault?.Data.Accounts.FirstOrDefault(a => a.CookieValid != false && a.Cookie.Length > 0)?.Cookie;
            var games = await RobloxApi.GetGamesAsync(universeOf.Values, cookie);
            foreach (var (placeId, universeId) in universeOf)
                _games[placeId] = games.TryGetValue(universeId, out var info) ? info with { PlaceId = placeId } : null;
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            foreach (var (placeId, _) in wanted)
                _games.TryAdd(placeId, null);
        }
    }

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

    async Task<List<object>> GameRowsAsync(IEnumerable<GameSummary> games)
    {
        var list = games.GroupBy(g => g.PlaceId).Select(g => g.First()).ToList();
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
        return await GameRowsAsync((await RobloxApi.SearchGamesAsync(query)).Take(12));
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
                string? cookie = vault.Data.Accounts.FirstOrDefault(a => a.CookieValid != false && a.Cookie.Length > 0)?.Cookie;
                return (await RobloxApi.GetPopularGamesAsync(cookie)).Take(30).ToList();
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
        if (_launchCts is not null)
            throw new UserError(L.T("Already launching — wait a moment or press Stop."));
        var accounts = Ids(a).Select(id => vault.Data.Accounts.FirstOrDefault(x => x.Id == id)).OfType<Account>().ToList();
        if (accounts.Count == 0)
            throw new UserError(L.T("Select at least one account."));
        // VNG accounts need the Roblox VNG client, whose launch hasn't been tried yet.
        if (accounts.FirstOrDefault(acc => acc.Realm == Realm.Vng) is { } vng)
            throw new UserError(L.F("{0} is a Roblox VNG account. Opening Roblox VNG isn't supported yet.", vng.Label));
        bool appOnly = Bool(a, "appOnly");

        var s = vault.Data.Settings;
        s.LastPlaceInput = Str(a, "placeInput").Trim();
        s.LastJobId = Str(a, "jobId").Trim();
        GameTarget? target = null;
        if (!appOnly && !GameTarget.TryParse(s.LastPlaceInput, s.LastJobId, out target, out string error))
            throw new UserError(error);

        // Things worth a second look before opening more windows.
        var players = RobloxProcesses.GetPlayers();
        if (s.MultiInstance && !MultiInstance.Armed && players.Count == 0)
            MultiInstance.Enable();
        var warnings = new List<string>();
        bool robloxOpen = RobloxProcesses.VisibleWindows(players.Select(p => p.Pid)).Values.Any(list => list.Count > 0);
        if ((accounts.Count > 1 || robloxOpen) && !MultiInstance.Armed)
            warnings.Add(s.MultiInstance ? MultiNotReadyText() : L.T("Multi-Roblox is off, so each new Roblox window closes the previous one."));
        int toOpen = accounts.Count(acc => !_running.ContainsKey(acc.Id));
        var memory = SystemMemory.Read();
        long perClientMb = players.Count > 0 ? Math.Max(400, (long)players.Average(p => p.WorkingSet) / MB) : DefaultClientMb;
        long neededMb = toOpen * perClientMb;
        if (s.MemoryWarnings && memory.TotalMb > 0 && memory.AvailableMb - neededMb < memory.TotalMb * 8 / 100)
            warnings.Add(L.F("Only {0} of RAM is free, and {1} more Roblox window(s) need about {2}. The PC may slow down or Roblox may crash.",
                FormatMb(memory.AvailableMb), toOpen, FormatMb(neededMb)));
        if (warnings.Count > 0 && !Bool(a, "force"))
            return new { needsConfirm = true, message = string.Join("\n\n", warnings) + "\n\n" + L.T("Open anyway?") };

        var cts = _launchCts = new CancellationTokenSource();
        var ct = cts.Token;
        var errors = new List<string>();
        int launched = 0;
        PushState();
        try
        {
            await WaitForSweepAsync(ct);
            if (target?.ShareCode is { } shareCode)
            {
                Progress(L.T("Reading the private server link…"));
                var (placeId, linkCode) = await RobloxApi.ResolvePrivateServerShareAsync(accounts[0].Cookie, shareCode, ct);
                target = target with { PlaceId = placeId, LinkCode = linkCode, ShareCode = null };
            }

            for (int i = 0; i < accounts.Count && _vault == vault; i++)
            {
                ct.ThrowIfCancellationRequested();
                var account = accounts[i];
                if (_running.ContainsKey(account.Id) || _pending.Any(p => p.AccountId == account.Id))
                {
                    errors.Add(L.F("{0}: already running — skipped.", account.Label));
                    continue;
                }

                Progress(L.F("Launching {0} ({1}/{2})…", account.Label, i + 1, accounts.Count));
                var startedAt = DateTime.UtcNow;
                try
                {
                    await StartSessionAsync(account, target, s.LaunchVia, ct);
                    launched++;
                }
                catch (Exception ex) when (!ct.IsCancellationRequested
                                           && (RobloxApi.IsExpectedError(ex) || ex is Win32Exception or FileNotFoundException))
                {
                    errors.Add($"{account.Label}: {ex.Message}");
                    if (ex is RobloxApiException { CookieInvalid: true })
                        account.CookieValid = false;
                }
                PushState();

                if (i < accounts.Count - 1)
                    await WaitBeforeNextLaunchAsync(account, startedAt, s.LaunchDelaySeconds, ct);
            }
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested)
        {
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            errors.Add(ex.Message);
        }
        finally
        {
            _launchCts = null;
            cts.Dispose();
            if (launched > 0 && target is not null && _vault == vault)
            {
                RememberGame(target.PlaceId);
                _ = LoadRecentGamesAsync();
            }
            if (launched > 0)
                RefreshPresenceSoon(TimeSpan.FromSeconds(20));
            Persist();
            Progress(null);
            PushState();
        }
        return new { launched, errors, cancelled = ct.IsCancellationRequested };
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
                    ? L.F("Waiting for Roblox to open {0}… (Roblox is updating to {1})", account.Label, version.Latest)
                    : L.F("Waiting for Roblox to open {0}… (Roblox may be updating)", account.Label));
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

    /// <summary>Gets a sign-in ticket, starts the client and waits for its window to show up (see TrackProcesses).</summary>
    async Task StartSessionAsync(Account account, GameTarget? target, string launchVia, CancellationToken ct = default)
    {
        string ticket = await RobloxApi.GetAuthTicketAsync(account.Cookie, ct);
        var before = RobloxProcesses.GetPlayerPids().ToHashSet();
        var config = RequireVault().Data.Settings.RobloxConfig;
        if (config.Mode != "off" || RobloxGameSettings.HasBaseline)
        {
            bool use = config.Mode == "all" || (config.Mode == "marked" && account.UseRobloxConfig);
            if (RobloxGameSettings.PrepareFor(config, use))
            {
                _robloxSettingsPending = true;
                _robloxSettingsWrittenAt = DateTime.UtcNow;
            }
        }
        RobloxLauncher.Launch(ticket, target, launchVia);
        _lastSessionStart = DateTime.UtcNow;
        _userClosed.Remove(account.Id);
        _pending.Add(new PendingLaunch(account.Id, before, DateTime.UtcNow, target));
        account.LastUsed = DateTime.Now;
        account.CookieValid = true;
    }

    /// <summary>
    /// "Who's playing?": one Roblox at a time, like switching users in Steam. Whatever Roblox is open closes, then the
    /// home screen opens signed in as the chosen account. Asks first when the open one is in a game.
    /// </summary>
    async Task<object?> SwitchToAsync(Guid id, bool force)
    {
        var vault = RequireVault();
        var account = RequireAccount(id);
        var pids = RobloxProcesses.GetPlayerPids();
        if (_running.TryGetValue(account.Id, out var own) && pids.Length == 1 && pids[0] == own.Pid)
        {
            RobloxProcesses.Focus(own.Pid);
            return new { focused = true };
        }
        if (pids.Length > 0 && !force && pids.FirstOrDefault(pid => _logs.StateOf(pid) is { Status: "ingame" or "joining" }) is var playing and > 0)
        {
            var who = _running.Where(r => r.Value.Pid == playing).Select(r => vault.Data.Accounts.FirstOrDefault(x => x.Id == r.Key)).FirstOrDefault();
            return new
            {
                needsConfirm = true,
                message = who is null ? L.T("Roblox is in a game. Switching accounts closes it.") : L.F("{0} is in a game. Switching accounts closes it.", who.Label),
            };
        }
        if (pids.Length > 0)
        {
            CloseAll();
            // Roblox leaves a windowless client behind when its window closes; wait until none has been around for a moment.
            var quietSince = DateTime.UtcNow;
            for (var until = DateTime.UtcNow + BackgroundSweep; DateTime.UtcNow < until; )
            {
                await Task.Delay(250);
                if (RobloxProcesses.GetPlayerPids().Length > 0)
                    quietSince = DateTime.UtcNow;
                else if (DateTime.UtcNow - quietSince > TimeSpan.FromSeconds(2))
                    break;
            }
            _sweepBackgroundUntil = DateTime.UtcNow;
        }
        using var args = JsonDocument.Parse(JsonSerializer.Serialize(new { ids = new[] { id }, appOnly = true, force = true }));
        return await LaunchAsync(args.RootElement);
    }

    object? CancelLaunch()
    {
        _launchCts?.Cancel();
        return null;
    }

    object CloseAccounts(Guid[] ids)
    {
        int closed = 0;
        foreach (var id in ids)
        {
            _rejoinQueue.RemoveAll(r => r.AccountId == id);
            if (!_running.TryGetValue(id, out var info))
                continue;
            _userClosed.Add(id);
            RobloxProcesses.Close(info.Pid);
            closed++;
        }
        return closed;
    }

    object CloseAll()
    {
        foreach (var id in _running.Keys)
            _userClosed.Add(id);
        _rejoinQueue.Clear();
        // Roblox starts a windowless background client when its last window closes; sweep those for a moment
        // so "close all" really leaves nothing running and multi-instance can re-arm.
        _sweepBackgroundUntil = DateTime.UtcNow + BackgroundSweep;
        return RobloxProcesses.CloseAll();
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
    async Task<object?> ArrangeWindowsAsync(int monitor)
    {
        var screens = Screen.AllScreens;
        var screen = monitor >= 0 && monitor < screens.Length ? screens[monitor] : Screen.PrimaryScreen ?? screens[0];
        var tracked = _running
            .OrderBy(r => FindAccount(r.Key)?.Label ?? "", StringComparer.CurrentCultureIgnoreCase)
            .Select(r => r.Value.Pid)
            .ToList();
        var others = RobloxProcesses.GetPlayerPids().Where(p => !tracked.Contains(p)).OrderBy(p => p);
        return await RobloxProcesses.ArrangeGridAsync(tracked.Concat(others).ToList(), screen.WorkingArea);
    }

    // ───────────────────────────── background work ─────────────────────────────

    /// <summary>
    /// Matches launches to the processes they start, keeps window titles in sync, publishes RAM use, runs
    /// auto-rejoin and the resource guard, re-arms multi-instance, and schedules presence refreshes.
    /// </summary>
    void OnProcTimer()
    {
        if (_disposed)
            return;
        try
        {
            TrackProcesses();
        }
        // A timer callback must never take the app down; log and try again on the next tick.
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    void TrackProcesses()
    {
        var now = DateTime.UtcNow;
        var players = RobloxProcesses.GetPlayers();
        var pids = players.Select(p => p.Pid).ToHashSet();
        // One pass over the desktop's windows serves every window check of this tick.
        var windows = RobloxProcesses.VisibleWindows(pids);
        bool HasWindow(int pid) => windows.TryGetValue(pid, out var list) && list.Count > 0;
        if (now < _sweepBackgroundUntil)
        {
            foreach (int pid in pids.Where(p => !HasWindow(p)).ToList())
            {
                RobloxProcesses.Kill(pid);
                pids.Remove(pid);
            }
            players.RemoveAll(p => !pids.Contains(p.Pid));
        }
        bool changed = false;

        foreach (var (id, info) in _running.ToList())
        {
            if (pids.Contains(info.Pid))
                continue;
            _running.Remove(id);
            // The first process may only bootstrap and hand over to a child; keep looking for a while.
            if (now - info.Launch.StartedAt < HandOverWindow)
                _pending.Add(info.Launch);
            else
                OnSessionEnded(id, info.Launch);
            changed = true;
        }

        changed |= _pending.RemoveAll(p => now - p.StartedAt > PendingTimeout) > 0;
        if (_pending.Count > 0)
        {
            var claimed = _running.Values.Select(r => r.Pid).ToHashSet();
            foreach (var launch in _pending.OrderBy(p => p.StartedAt).ToList())
            {
                int pid = pids.Where(p => !launch.Before.Contains(p) && !claimed.Contains(p))
                    .Where(HasWindow)
                    .OrderBy(p => p)
                    .FirstOrDefault();
                if (pid == 0)
                    continue;
                claimed.Add(pid);
                _running[launch.AccountId] = (pid, launch);
                _pending.Remove(launch);
                changed = true;
            }
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
        RejoinFromLogs();
        RunAntiAfk(now, windows);

        if (_vault is { Data.Settings.RenameWindows: true })
        {
            foreach (var (id, info) in _running)
            {
                if (FindAccount(id) is { } account && windows.TryGetValue(info.Pid, out var list))
                    RobloxProcesses.SetWindowTitle(list, $"Roblox - {account.Label}");
            }
        }

        if (_vault is { Data.Settings.MultiInstance: true } && !MultiInstance.Armed && pids.Count == 0)
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

        if (Prefs.CheckUpdates && _updater.Supported && now >= _nextUpdateCheck)
        {
            _nextUpdateCheck = now + UpdateCheckInterval;
            _ = _updater.CheckAsync();
        }

        if (_vault is null)
            return;
        PersistIfDue(now);
        var memory = SystemMemory.Read();
        EmitMetrics(players, memory);
        GuardResources(players, memory, now);
        RunDueRejoin(now);
        if (!_presenceBusy && now >= _nextPresence)
            _ = RefreshPresenceAsync();
    }

    /// <summary>Live RAM per client and for the PC, sent apart from the state so the page can patch numbers in place.</summary>
    void EmitMetrics(List<RobloxProcesses.PlayerProcess> players, MemoryStatus memory)
    {
        var workingSets = players.ToDictionary(p => p.Pid, p => p.WorkingSet);
        string json = JsonSerializer.Serialize(new
        {
            accounts = _running.Select(r => new { id = r.Key, ramMb = workingSets.GetValueOrDefault(r.Value.Pid) / MB }),
            robloxCount = players.Count,
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
            Toast(L.F("Your PC is using {0}% of its RAM. Close some Roblox windows if things get slow.", memory.LoadPercent), "warn");
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

    void OnSessionEnded(Guid accountId, PendingLaunch launch)
    {
        _offlineStrikes.Remove(accountId);
        if (_userClosed.Remove(accountId))
            return;
        if (FindAccount(accountId) is not { AutoRejoin: true } account || launch.Target is null)
            return;

        var history = _rejoinHistory.TryGetValue(accountId, out var h) ? h : _rejoinHistory[accountId] = new List<DateTime>();
        history.RemoveAll(t => DateTime.UtcNow - t > RejoinWindow);
        if (history.Count >= MaxRejoins)
        {
            // Something keeps failing (ban, broken game, bad connection); stop instead of looping.
            account.AutoRejoin = false;
            Persist();
            PushState();
            Toast(L.F("Auto-rejoin for {0} was turned off after {1} tries in 15 minutes.", account.Label, MaxRejoins), "warn");
            return;
        }
        history.Add(DateTime.UtcNow);
        // Rejoin the same game or private server rather than the exact server, which may be gone.
        _rejoinQueue.RemoveAll(r => r.AccountId == accountId);
        _rejoinQueue.Add(new RejoinRequest(accountId, launch.Target with { JobId = null }, DateTime.UtcNow + RejoinDelay));
        Toast(L.F("{0} left the game — rejoining in {1}s…", account.Label, (int)RejoinDelay.TotalSeconds));
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
                || FindAccount(id) is not { AutoRejoin: true } account || info.Launch.Target is null)
                continue;
            _closingForRejoin.Add(info.Pid);
            Toast(L.F("{0} seems disconnected — restarting Roblox…", account.Label));
            RobloxProcesses.Close(info.Pid);
        }
    }

    /// <summary>
    /// Every <see cref="AntiAfk.Interval"/> per Roblox window (counted from joining a server or its last input), and
    /// only while nobody is using the PC, nudges every Roblox window on the PC that is in a server — also ones opened
    /// outside Bloxkeep. A window the person is playing in gets their input anyway.
    /// </summary>
    void RunAntiAfk(DateTime now, Dictionary<int, List<IntPtr>> windows)
    {
        foreach (int gone in _lastInput.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _lastInput.Remove(gone);
        _idleKickReported.RemoveWhere(pid => !windows.ContainsKey(pid));
        if (_vault is not { Data.Settings.AntiAfk: true } || windows.Count == 0)
            return;
        ReportIdleKicks();
        // A window seen for the first time has been idle at least since it joined its server (or since now).
        foreach (int pid in windows.Keys)
            _lastInput.TryAdd(pid, _logs.StateOf(pid) is { Status: "ingame" } joined ? joined.At : now);
        if (_nudging)
            return;

        bool personHere = AntiAfk.PersonActive();
        int foreground = RobloxProcesses.ForegroundPid();
        if (personHere && windows.ContainsKey(foreground))
            _lastInput[foreground] = now;
        if (personHere)
            return;

        var due = windows
            .Where(w => w.Value.Count > 0 && _logs.StateOf(w.Key)?.Status is null or "ingame" && now - LastInput(w.Key) >= AntiAfk.Interval)
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
            var reached = await AntiAfk.NudgeAsync(clients.Select(c => c.Window).ToList(), () => _vault is { Data.Settings.AntiAfk: true });
            foreach (var (pid, window) in clients)
            {
                if (reached.Contains(window))
                    _lastInput[pid] = DateTime.UtcNow;
            }
        }
        finally
        {
            _nudging = false;
        }
    }

    /// <summary>Says so when an account is kicked for idling although anti-AFK is on.</summary>
    void ReportIdleKicks()
    {
        foreach (var (id, info) in _running)
        {
            if (_logs.StateOf(info.Pid) is { Status: "disconnected", Reason: IdleKick } && _idleKickReported.Add(info.Pid)
                && FindAccount(id) is { } account)
                Toast(L.F("{0} was kicked for being AFK. Anti-AFK doesn't work while the PC is locked.", account.Label), "warn");
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
        if (vault is null || account is not { AutoRejoin: true }
            || _running.ContainsKey(account.Id) || _pending.Any(p => p.AccountId == account.Id))
            return;
        _rejoining = true;
        try
        {
            if (vault.Data.Settings.MultiInstance && !MultiInstance.Armed && RobloxProcesses.GetPlayerPids().Length == 0)
                MultiInstance.Enable();
            await StartSessionAsync(account, request.Target, vault.Data.Settings.LaunchVia);
            Persist();
            PushState();
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException && request.NetworkRetries < MaxNetworkRetries)
        {
            // No connection (often right after sleep): try again shortly. This is not counted as a rejoin.
            _rejoinQueue.Add(request with { Due = DateTime.UtcNow + NetworkRetryDelay, NetworkRetries = request.NetworkRetries + 1 });
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
            Toast(L.F("Couldn't rejoin {0}: {1}", account.Label, ex.Message), "error");
        }
        finally
        {
            _rejoining = false;
        }
    }

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
            if ((account is not null && !reliable.Contains(account.UserId)) || _logs.StateOf(info.Pid) is not null)
                continue;
            if (account is not { AutoRejoin: true } || info.Launch.Target is null || DateTime.UtcNow - info.Launch.StartedAt < DisconnectGrace
                || !_presence.TryGetValue(account.UserId, out var presence) || presence.Type is 2 or 4)
            {
                _offlineStrikes.Remove(id);
                continue;
            }
            int strikes = _offlineStrikes[id] = _offlineStrikes.GetValueOrDefault(id) + 1;
            if (strikes < 2)
                continue;
            _offlineStrikes.Remove(id);
            Toast(L.F("{0} seems disconnected — restarting Roblox…", account.Label));
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
            _owner.BeginInvoke(new Action(PushState));
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
        if (_updater.Status != "ready")
            return null;
        if (_launchCts is not null)
            throw new UserError(L.T("Wait until Bloxkeep finishes opening Roblox."));
        SaveOpenSessions();
        _updater.RestartToUpdate();
        return null;
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
        if (Prefs.LastVersion != current)
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
    bool Settling => DateTime.UtcNow - _resumedAt < ResumeGrace || !_networkUp;

    /// <summary>
    /// Kept up to date by Windows' notification: asking directly lists every network adapter, too slow for a check
    /// that runs on each process tick.
    /// </summary>
    volatile bool _networkUp = NetworkInterface.GetIsNetworkAvailable();

    void OnNetworkAvailabilityChanged(object? sender, NetworkAvailabilityEventArgs e) => _networkUp = e.IsAvailable;

    /// <summary>
    /// After sleep every client has lost its connection and the network takes a moment to come back. Disconnect
    /// detection and auto-rejoin wait for it, so the clients aren't all restarted into a dead network at once.
    /// </summary>
    void OnPowerModeChanged(object? sender, PowerModeChangedEventArgs e)
    {
        if (e.Mode != PowerModes.Resume || _disposed)
            return;
        try
        {
            _owner.BeginInvoke(new Action(OnResumed));
        }
        catch (InvalidOperationException)
        {
            // The window is closing.
        }
    }

    void OnResumed()
    {
        _resumedAt = DateTime.UtcNow;
        _offlineStrikes.Clear();
        _nextPresence = _resumedAt + ResumeGrace;
        if (_nextUpdateCheck > _resumedAt + TimeSpan.FromMinutes(2))
            _nextUpdateCheck = _resumedAt + TimeSpan.FromMinutes(2);
        Resumed?.Invoke();
        if (_vault is not null && (_rejoinQueue.Count > 0 || _running.Keys.Any(id => FindAccount(id) is { AutoRejoin: true })))
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
        if (vault is null || _presenceBusy)
            return null;
        var accounts = vault.Data.Accounts.Where(a => a.UserId > 0).ToList();
        _nextPresence = DateTime.UtcNow + PresenceInterval;
        if (accounts.Count == 0)
            return null;

        _presenceBusy = true;
        try
        {
            var viewer = accounts.FirstOrDefault(a => a.CookieValid != false && a.Cookie.Length > 0 && a.Realm == Realm.Global);
            var result = await RobloxApi.GetPresenceAsync(viewer?.Cookie, accounts.Select(a => a.UserId).ToList());

            // Privacy settings can hide an account from the others. Accounts with a client open here are asked with
            // their own cookie (auto-rejoin relies on it) unless that client's log is followed, which already says
            // what it is doing; others only when their game is hidden.
            var askOwn = accounts.Where(a => a != viewer && a.CookieValid != false && a.Cookie.Length > 0
                                             && !(_running.TryGetValue(a.Id, out var run) && _logs.StateOf(run.Pid) is not null)
                                             && (a.Realm == Realm.Vng || result.TryGetValue(a.UserId, out var p)
                                                 && (_running.ContainsKey(a.Id) ? p.Type != 2 || p.PlaceId is null : p is { Type: 2, PlaceId: null })))
                .ToList();
            askOwn = askOwn.Where(a => _running.ContainsKey(a.Id)).Concat(askOwn.Where(a => !_running.ContainsKey(a.Id)).Take(10)).ToList();
            var reliable = result.Where(r => r.Value.Type == 2).Select(r => r.Key).ToHashSet();
            if (viewer is not null)
                reliable.Add(viewer.UserId);
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
                    }
                    finally
                    {
                        gate.Release();
                    }
                }));
            }

            await LoadGamesAsync(result.Values
                .Where(p => (p.RootPlaceId ?? p.PlaceId) > 0)
                .Select(p => ((p.RootPlaceId ?? p.PlaceId)!.Value, p.UniverseId ?? 0))
                .Concat(_running.Values.Select(r => r.Launch).Concat(_pending)
                    .Where(l => l.Target is { PlaceId: > 0 })
                    .Select(l => (l.Target!.PlaceId, 0L)))
                .ToList());

            if (_vault != vault)
                return null;
            _presence.Clear();
            foreach (var (userId, presence) in result)
                _presence[userId] = presence;
            _presenceReliable.Clear();
            _presenceReliable.UnionWith(reliable);
            DetectDisconnectedSessions(reliable);
            PushStateIfChanged();
        }
        catch (Exception ex) when (RobloxApi.IsExpectedError(ex))
        {
            _nextPresence = DateTime.UtcNow + PresenceInterval * 2;
        }
        finally
        {
            _presenceBusy = false;
        }
        return null;
    }

    // ───────────────────────────── settings ─────────────────────────────

    object? SetSettings(JsonElement a)
    {
        var s = RequireVault().Data.Settings;
        if (OptStr(a, "lastPlaceInput") is { } place) s.LastPlaceInput = place.Trim();
        if (OptStr(a, "lastJobId") is { } job) s.LastJobId = job.Trim();
        // Sent while typing in the game box: the page already shows it, so just remember it.
        if (a.ValueKind == JsonValueKind.Object && a.EnumerateObject().Any()
            && a.EnumerateObject().All(p => p.Name is "lastPlaceInput" or "lastJobId"))
        {
            PersistSoon();
            return null;
        }
        if (OptBool(a, "renameWindows") is { } rename) s.RenameWindows = rename;
        if (OptStr(a, "launchVia") is { } via && RobloxLauncher.LaunchOptions.Contains(via))
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
                throw new UserError(L.F("Couldn't turn it off: {0}", ex.Message));
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
        NetworkChange.NetworkAvailabilityChanged -= OnNetworkAvailabilityChanged;
        _updater.Changed -= OnUpdaterChanged;
        _procTimer.Dispose();
        if (_saveDue is not null)
            Persist();
        SaveOpenSessions();
        // A downloaded update is installed once the app has closed; the next start is the new version.
        _updater.ApplyOnExit();
        _launchCts?.Cancel();
        _vault?.Dispose();
        _vault = null;
    }
}
