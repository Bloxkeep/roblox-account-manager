using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Launch presets: saved accounts, game and kind of server, run from the dock or by schedule (AppController.Schedule.cs).
/// Servers other than "any" and a fixed Job ID are picked here when the preset runs, so a scheduled run needs no page.
/// </summary>
public sealed partial class AppController
{
    /// <param name="NeedsConfirm">Nothing was opened yet: <paramref name="Message"/> asks whether to open anyway.</param>
    /// <param name="Notice">Something the person should know that isn't an error (no server had room).</param>
    /// <param name="Skipped">Accounts left as they were because they were already open where asked.</param>
    sealed record LaunchResult(bool NeedsConfirm = false, string? Message = null, int Launched = 0, List<string>? Errors = null,
        bool Cancelled = false, string? Notice = null, List<string>? Skipped = null);

    const int MaxPresets = 50;
    const int MaxPresetName = 60;
    static readonly string[] PresetServers = ["any", "most", "fewest", "ping", "random", "job"];
    /// <summary><see cref="LaunchPreset.Profile"/> for the light AFK setup (<see cref="RobloxGameSettings.LightAfk"/>).</summary>
    const string AfkProfile = "afk";
    /// <summary>The choices the form offers for <see cref="LaunchPreset.RestartHours"/> (0 = never).</summary>
    static readonly int[] RestartChoices = [2, 4, 6, 12];

    object PresetDto(LaunchPreset p) => new
    {
        id = p.Id,
        p.Name,
        p.AccountIds,
        p.PlaceInput,
        p.GameName,
        p.Server,
        p.JobId,
        p.DelaySeconds,
        schedule = ScheduleDto(p.Schedule),
        nextRun = NextRun(p.Schedule),
        // Only from what is already known: the Presets tab asks for the missing ones (presetIcons).
        iconUrl = PresetPlaceId(p) is long place ? _games.GetValueOrDefault(place)?.IconUrl : null,
        lastRun = p.LastRunAt,
        p.AntiAfk,
        p.AutoRejoin,
        p.Profile,
        p.RestartHours,
    };

    /// <summary>The game's place id, or null without a game or for a share link (its place is only known when it opens).</summary>
    static long? PresetPlaceId(LaunchPreset p) =>
        p.PlaceInput.Length > 0 && GameTarget.TryParse(p.PlaceInput, "", out var target, out _) && target!.ShareCode is null && target.PlaceId > 0
            ? target.PlaceId : null;

    /// <summary>Adds a preset, or replaces the one with the given id. Returns its id.</summary>
    async Task<object?> SavePresetAsync(JsonElement a)
    {
        var vault = RequireVault();
        string name = Str(a, "name").Trim();
        if (name.Length == 0)
            throw new UserError(L.T("Give the preset a name."));
        if (name.Length > MaxPresetName)
            name = CutText(name, MaxPresetName).TrimEnd();
        var ids = Ids(a).Where(id => vault.Data.Accounts.Any(acc => acc.Id == id)).ToList();
        if (ids.Count == 0)
            throw new UserError(L.T("Select at least one account."));
        string place = Str(a, "placeInput").Trim();
        // Without a game the preset just opens Roblox, so there is no server to choose.
        string server = place.Length == 0 ? "any" : PresetServers.Contains(Str(a, "server")) ? Str(a, "server") : "any";
        string job = server == "job" ? Str(a, "jobId").Trim() : "";
        if (server == "job" && job.Length == 0)
            throw new UserError(L.T("Enter the server's Job ID."));
        int? delay = Has(a, "delaySeconds", JsonValueKind.Number) ? (int)Math.Clamp(Long(a, "delaySeconds"), 0, 120) : null;
        string profile = Str(a, "profile") == AfkProfile ? AfkProfile : "";
        // A restart opens the same game again; a preset without one only opens Roblox, so there is nothing to restart.
        int restartHours = place.Length > 0 && RestartChoices.Contains((int)Long(a, "restartHours")) ? (int)Long(a, "restartHours") : 0;
        var schedule = ParseSchedule(a, vault.Data.Settings.Presets.FirstOrDefault(p => p.Id == Id(a))?.Schedule);

        // Checked now, so a typo shows while saving rather than when the preset runs.
        string gameName = "";
        if (place.Length > 0)
        {
            if (!GameTarget.TryParse(place, job, out var target, out string error))
                throw new UserError(error);
            if (target!.ShareCode is null && await GetGameInfoAsync(target.PlaceId) is { } info)
                gameName = info.Name;
        }
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));

        var presets = vault.Data.Settings.Presets;
        var preset = presets.FirstOrDefault(p => p.Id == Id(a));
        if (preset is null)
        {
            if (presets.Count >= MaxPresets)
                throw new UserError(L.F("You can save up to {0} presets.", MaxPresets));
            preset = new LaunchPreset();
            presets.Add(preset);
        }
        preset.Name = name;
        preset.AccountIds = ids;
        preset.PlaceInput = place;
        preset.GameName = gameName;
        preset.Server = server;
        preset.JobId = job;
        preset.DelaySeconds = delay;
        preset.AntiAfk = OptBool(a, "antiAfk");
        preset.AutoRejoin = OptBool(a, "autoRejoin");
        preset.Profile = profile;
        preset.RestartHours = restartHours;
        // A schedule changed today (another time, re-enabled) may run today: the "not twice within a day" guard
        // against a clock set back is for the schedule as it was.
        var old = preset.Schedule;
        if (schedule is not null && (old is null || old.Enabled != schedule.Enabled || old.Time != schedule.Time || old.Date != schedule.Date || !old.Days.SequenceEqual(schedule.Days)))
            _scheduledStartTick.Remove(preset.Id);
        preset.Schedule = schedule;
        ScheduleChanged(preset);
        Persist();
        PushState();
        return preset.Id;
    }

    /// <summary>
    /// At most <paramref name="max"/> UTF-16 units, cut between whole characters as shown (never in the middle of an
    /// emoji, or between a letter and its accent).
    /// </summary>
    static string CutText(string text, int max)
    {
        if (text.Length <= max)
            return text;
        int length = 0;
        var elements = System.Globalization.StringInfo.GetTextElementEnumerator(text);
        while (elements.MoveNext() && length + elements.GetTextElement().Length <= max)
            length += elements.GetTextElement().Length;
        return text[..length];
    }

    object? DeletePreset(Guid id)
    {
        RequireVault().Data.Settings.Presets.RemoveAll(p => p.Id == id);
        Persist();
        PushState();
        return null;
    }

    /// <summary>
    /// A copy placed right after the original (and its earlier copies), named "… (copy)" (numbered when taken). Its schedule comes along switched
    /// off, so the two don't start the same accounts at the same minute. Returns the copy's id.
    /// </summary>
    object? DuplicatePreset(Guid id)
    {
        var presets = RequireVault().Data.Settings.Presets;
        int index = presets.FindIndex(p => p.Id == id);
        if (index < 0)
            throw new UserError(L.T("Preset not found."));
        if (presets.Count >= MaxPresets)
            throw new UserError(L.F("You can save up to {0} presets.", MaxPresets));
        var source = presets[index];
        var names = presets.Select(p => p.Name).ToHashSet(StringComparer.OrdinalIgnoreCase);
        string name = "";
        var copyNames = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        for (int n = 1; n <= MaxPresets + 1; n++)
        {
            string suffix = n == 1 ? L.T(" (copy)") : L.F(" (copy {0})", n);
            // The original's name is shortened, not the suffix, so a 60-character name still reads as a copy.
            string candidate = CutText(source.Name, MaxPresetName - suffix.Length).TrimEnd() + suffix;
            copyNames.Add(candidate);
            if (name.Length == 0 && !names.Contains(candidate))
                name = candidate;
        }
        // After the copies already made of it, so "(copy 2)" comes under "(copy)", not above it.
        int at = index + 1;
        while (at < presets.Count && copyNames.Contains(presets[at].Name))
            at++;
        var s = source.Schedule;
        var copy = new LaunchPreset
        {
            Name = name,
            AccountIds = source.AccountIds.ToList(),
            PlaceInput = source.PlaceInput,
            GameName = source.GameName,
            Server = source.Server,
            JobId = source.JobId,
            DelaySeconds = source.DelaySeconds,
            AntiAfk = source.AntiAfk,
            AutoRejoin = source.AutoRejoin,
            Profile = source.Profile,
            RestartHours = source.RestartHours,
            Schedule = s is null ? null : new PresetSchedule
            {
                Enabled = false,
                Time = s.Time,
                Days = s.Days.ToList(),
                Date = s.Date,
                Stop = s.Stop,
                StopHours = s.StopHours,
                StopTime = s.StopTime,
            },
        };
        presets.Insert(at, copy);
        Persist();
        PushState();
        return copy.Id;
    }

    /// <summary>The dock menu and the Presets tab list presets in this order; any the page didn't send stay, last.</summary>
    object? SetPresetOrder(Guid[] ids)
    {
        var presets = RequireVault().Data.Settings.Presets;
        var byId = presets.ToDictionary(p => p.Id);
        var order = ids.Distinct().Where(byId.ContainsKey).Select(id => byId[id]).ToList();
        order.AddRange(presets.Where(p => !order.Contains(p)));
        presets.Clear();
        presets.AddRange(order);
        Persist();
        PushState();
        return null;
    }

    /// <summary>
    /// The Presets tab's switch: the saved schedule on or off as it is. Turning it on checks it like the form does
    /// (a one-off whose date has passed can't be turned on).
    /// </summary>
    object? SetPresetSchedule(Guid id, bool enabled)
    {
        var preset = RequireVault().Data.Settings.Presets.FirstOrDefault(p => p.Id == id) ?? throw new UserError(L.T("Preset not found."));
        if (preset.Schedule is not { } s)
            throw new UserError(L.T("This preset has no schedule. Edit it to add one."));
        if (s.Enabled != enabled)
        {
            var form = JsonSerializer.SerializeToElement(new
            {
                schedule = new { enabled, time = s.Time, days = s.Days, date = s.Date, stop = s.Stop, stopHours = s.StopHours, stopTime = s.StopTime },
            });
            preset.Schedule = ParseSchedule(form, s);
            _scheduledStartTick.Remove(preset.Id); // as in the form: a schedule turned on again may run today
            ScheduleChanged(preset);
            Persist();
        }
        PushState();
        return null;
    }

    /// <summary>Looks up the games of presets whose icon isn't known yet (the Presets tab shows them), then pushes the state.</summary>
    async Task<object?> PresetIconsAsync()
    {
        var places = RequireVault().Data.Settings.Presets.Select(PresetPlaceId).OfType<long>()
            .Where(id => _games.GetValueOrDefault(id)?.IconUrl is null).Distinct().Select(id => (id, 0L)).ToList();
        if (places.Count == 0)
            return null;
        await LoadGamesAsync(places);
        if (_vault is not null)
            PushStateIfChanged();
        return null;
    }

    /// <summary>The page's command: the same run, typed for the dispatcher (the schedule calls <see cref="StartPresetAsync"/>).</summary>
    async Task<object?> RunPresetAsync(Guid id, bool force) => await StartPresetAsync(id, force);

    /// <summary>
    /// Opens the preset's accounts that still exist, in its order, through the same launch as the dock's Play button
    /// (same waits, warnings, Stop button and progress text).
    /// </summary>
    async Task<LaunchResult> StartPresetAsync(Guid id, bool force)
    {
        var vault = RequireVault();
        var preset = vault.Data.Settings.Presets.FirstOrDefault(p => p.Id == id) ?? throw new UserError(L.T("Preset not found."));
        // Accounts deleted since the preset was saved are left out.
        var accounts = preset.AccountIds.Distinct().Select(FindAccount).OfType<Account>().ToList();
        if (accounts.Count == 0)
            throw new UserError(L.T("This preset has no accounts left."));
        CheckCanLaunch(accounts);
        GameTarget? target = null;
        if (preset.PlaceInput.Length > 0
            && !GameTarget.TryParse(preset.PlaceInput, preset.Server == "job" ? preset.JobId : "", out target, out string error))
            throw new UserError(error);

        // A link that already names a server (or a private one) keeps it.
        string server = preset.Server;
        bool pick = target is { JobId: null, LinkCode: null, ShareCode: null } && server is "most" or "fewest" or "ping" or "random";
        string? notice = null;
        var result = await LaunchAccountsAsync(vault, accounts, target, force, preset.DelaySeconds ?? vault.Data.Settings.LaunchDelaySeconds,
            pick
                ? async (t, party, ct) =>
                {
                    var (picked, note) = await PickServerAsync(t, server, party, ct);
                    notice = note;
                    return picked;
                }
                : null,
            preset.Id);
        // Shown on the Presets tab; a run that opened nothing (all already open, or all failed) doesn't count.
        if (result.Launched > 0 && _vault == vault)
        {
            preset.LastRunAt = DateTime.UtcNow;
            PersistSoon();
            PushState();
        }
        return result with { Notice = notice };
    }

    /// <summary>
    /// A public server with room for all the accounts: the fullest or emptiest (Roblox's own order), the lowest average
    /// ping over two pages, or one at random. With none, or no answer from Roblox, Roblox picks as it normally does.
    /// </summary>
    async Task<(GameTarget Target, string? Note)> PickServerAsync(GameTarget target, string choice, int party, CancellationToken ct)
    {
        if (party == 0)
            return (target, null); // every account is already open; nothing will join
        Progress(L.T("Finding a server…"));
        var servers = new List<ServerInfo>();
        try
        {
            var (first, next) = await RobloxApi.GetServersAsync(target.PlaceId, choice == "fewest", null, ct);
            servers.AddRange(first);
            if (choice == "ping" && next is not null)
                servers.AddRange((await RobloxApi.GetServersAsync(target.PlaceId, false, next, ct)).Servers);
        }
        catch (Exception ex) when (!ct.IsCancellationRequested && RobloxApi.IsExpectedError(ex))
        {
            // Rate-limited or offline: whatever loaded is used, else Roblox picks.
        }
        var fits = servers.Where(s => s.MaxPlayers - s.Playing >= party).ToList();
        var chosen = choice switch
        {
            "ping" => fits.Where(s => s.Ping > 0).MinBy(s => s.Ping) ?? fits.FirstOrDefault(),
            "random" => fits.Count > 0 ? fits[Random.Shared.Next(fits.Count)] : null,
            _ => fits.FirstOrDefault(),
        };
        if (chosen is not null)
            return (target with { JobId = chosen.JobId }, null);
        string note = L.F("No server has room for {0} accounts right now, so Roblox picks one.", party);
        Toast(note, "warn");
        return (target, note);
    }

    // ───────── options of the windows a preset opened ─────────
    // A client remembers the preset that opened it (PendingLaunch.PresetId, kept through auto-rejoins and restarts and
    // saved with the open sessions). Its options are read from the preset as it is now, so an edit applies at once; a
    // deleted preset, or a client opened by Play, follows the usual settings.

    LaunchPreset? PresetFor(Guid? id) =>
        id is Guid preset ? _vault?.Data.Settings.Presets.FirstOrDefault(p => p.Id == preset) : null;

    /// <summary>Auto-rejoin for a client: its preset's option when it sets one, else the account's own switch.</summary>
    static bool AutoRejoinFor(LaunchPreset? preset, Account account) => preset?.AutoRejoin ?? account.AutoRejoin;

    /// <summary>Anti-AFK for a client: its preset's option when it sets one, else Settings.</summary>
    static bool AntiAfkFor(LaunchPreset? preset, AppSettings settings) => preset?.AntiAfk ?? settings.AntiAfk;

    bool RejoinOn(Account account, Guid? presetId) => AutoRejoinFor(PresetFor(presetId), account);

    /// <summary>Anti-AFK for a Roblox window here: the preset's that opened it, else Settings (also windows opened outside Bloxkeep).</summary>
    bool AntiAfkOn(int pid)
    {
        if (_vault is not { } vault)
            return false;
        foreach (var run in _running.Values)
        {
            if (run.Pid == pid)
                return AntiAfkFor(PresetFor(run.Launch.PresetId), vault.Data.Settings);
        }
        return vault.Data.Settings.AntiAfk;
    }

    /// <summary>Windows of a light-AFK preset already minimised once in their game (by process id).</summary>
    readonly HashSet<int> _afkMinimised = new();

    /// <summary>
    /// A light-AFK preset's window is minimised once its log says it's in the game (Roblox draws less while minimised).
    /// Only once, so a window the person brings back up stays up; one they're using at that moment is left alone.
    /// </summary>
    void MinimiseAfkWindows(Dictionary<int, List<IntPtr>> windows)
    {
        _afkMinimised.RemoveWhere(pid => !windows.ContainsKey(pid));
        if (_vault is null)
            return;
        int? foreground = null;
        foreach (var run in _running.Values)
        {
            if (_afkMinimised.Contains(run.Pid) || _logs.StateOf(run.Pid) is not { Status: "ingame" }
                || !windows.TryGetValue(run.Pid, out var list) || list.Count == 0 || PresetFor(run.Launch.PresetId)?.Profile != AfkProfile)
                continue;
            _afkMinimised.Add(run.Pid);
            if ((foreground ??= RobloxProcesses.ForegroundPid()) == run.Pid && AntiAfk.PersonActive())
                continue;
            RobloxProcesses.Minimise(RobloxProcesses.MainWindowOf(list));
        }
    }

    /// <summary>Restarts under way: the account's client is closing, to open again into this target (see <see cref="QueueRejoin"/>).</summary>
    readonly Dictionary<Guid, (GameTarget Target, DateTime At)> _restarts = new();
    /// <summary>At least this long after the last client started, so a preset's windows (opened together) aren't all reopened at once.</summary>
    static readonly TimeSpan RestartSpacing = TimeSpan.FromMinutes(1);
    /// <summary>A client asked to close for a restart that is still there after this is left alone (tried again later).</summary>
    static readonly TimeSpan RestartCloseTimeout = TimeSpan.FromMinutes(1);

    /// <summary>
    /// A preset's "restart every N hours": a client of it that has run that long is closed and opened again through
    /// auto-rejoin's path, one client at a time. Only a client in its game: one in the menu or on an error screen is
    /// auto-rejoin's to handle. A window in front while the person is at the PC is being used, so it waits for a later
    /// tick; one that is merely in front of an idle PC (the newest window takes the focus) is restarted like the rest.
    /// A client that couldn't be opened again (Multi-Roblox not ready with other windows open, a signed-out or banned
    /// account) is left running instead of being closed for good.
    /// </summary>
    void RestartDueClients(DateTime now)
    {
        foreach (var (id, restart) in _restarts.ToList())
        {
            if (now - restart.At > RestartCloseTimeout || now < restart.At)
                _restarts.Remove(id);
        }
        if (_vault is null || _running.Count == 0 || _restarts.Count > 0 || Settling || _rejoining || _launchCts is not null
            || _pending.Count > 0 || _rejoinQueue.Any(r => r.Restart) || now < _sweepBackgroundUntil || now - _lastSessionStart < RestartSpacing)
            return;
        int? foreground = null;
        foreach (var (id, run) in _running.OrderBy(r => r.Value.Launch.StartedAt).ToList())
        {
            if (PresetFor(run.Launch.PresetId) is not { RestartHours: > 0 } preset || run.Launch.Target is null
                || now - run.Launch.StartedAt < TimeSpan.FromHours(preset.RestartHours) || _closingForRejoin.Contains(run.Pid)
                || _logs.StateOf(run.Pid) is { Status: not "ingame" } || FindAccount(id) is not { } account)
                continue;
            if ((foreground ??= RobloxProcesses.ForegroundPid()) == run.Pid && AntiAfk.PersonActive())
                continue;
            // Auto-rejoin's path would refuse to open it again (see RejoinAsync): it would stay closed.
            if (account.CookieValid == false || account.Banned == true
                || (!MultiInstance.Armed && _running.Count > 1))
                continue;
            _restarts[id] = (RestartTarget(run), now);
            _closingForRejoin.Add(run.Pid); // Bloxkeep's own close: neither a drop nor closed by Roblox
            Toast(L.F("Restarting {0}'s Roblox (its preset restarts it every {1} h)…", Name(account), preset.RestartHours));
            RobloxProcesses.Close(run.Pid, InGameNow(run.Pid));
            return;
        }
    }

    /// <summary>
    /// Where a restart goes: back into the server the client is in while that is the game it was opened into (accounts
    /// a preset opened together stay together), else the same game or private server, as auto-rejoin does.
    /// </summary>
    GameTarget RestartTarget((int Pid, PendingLaunch Launch) run)
    {
        var target = run.Launch.Target!;
        return target.LinkCode is null && _logs.StateOf(run.Pid) is { Status: "ingame", JobId: { } job, PlaceId: { } place } && place == target.PlaceId
            ? target with { JobId = job }
            : target with { JobId = null };
    }
}
