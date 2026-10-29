using System.Text.Json;

namespace RobloxManager;

/// <summary>Launch presets: saved accounts, game and server kind, run from the dock or by schedule (AppController.Schedule.cs).
/// Servers other than "any" and a fixed Job ID are picked when the preset runs, so a scheduled run needs no page.</summary>
public sealed partial class AppController
{
    /// <param name="NeedsConfirm">Nothing was opened yet: <paramref name="Message"/> says what to look at before opening anyway.</param>
    /// <param name="Notice">Something the person should know that isn't an error (no server had room).</param>
    /// <param name="Skipped">Accounts left as they were because they were already open where asked.</param>
    /// <param name="Asks">With NeedsConfirm, the warning kinds in the message: the page offers each one's fix, and "Don't ask again" for skippable ones.</param>
    sealed record LaunchResult(bool NeedsConfirm = false, string? Message = null, int Launched = 0, List<string>? Errors = null,
        bool Cancelled = false, string? Notice = null, List<string>? Skipped = null, List<string>? Asks = null, List<Guid>? Refused = null);

    const int MaxPresets = 50;
    const int MaxPresetName = 60;
    static readonly string[] PresetServers = ["any", "most", "fewest", "ping", "random", "job", "follow"];
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
        p.FollowPlayer,
        p.DelaySeconds,
        schedule = ScheduleDto(p.Schedule),
        nextRun = NextRun(p.Schedule),
        // Only from what is already known: the Presets tab asks for the missing ones (presetIcons).
        iconUrl = PresetPlaceId(p) is long place ? _games.GetValueOrDefault(place)?.IconUrl : null,
        lastRun = p.LastRunAt,
        p.AntiAfk,
        p.AutoRejoin,
        p.RejoinWith,
        p.Profile,
        p.RestartHours,
        p.Arrange,
        // The link stays in the app, like Settings' one: the page only gets its masked form.
        discord = p.DiscordWebhook.Length > 0 ? DiscordWebhook.Mask(p.DiscordWebhook) : null,
        // Its windows open now (or being opened again by auto-rejoin), for the Presets tab's Close.
        running = PresetWindows(p.Id).Length,
    };

    Guid[] PresetWindows(Guid presetId) =>
        _running.Where(r => r.Value.Launch.PresetId == presetId).Select(r => r.Key)
            .Concat(_pending.Where(l => l.PresetId == presetId).Select(l => l.AccountId))
            .Concat(_rejoinQueue.Where(r => r.PresetId == presetId).Select(r => r.AccountId))
            .Distinct().ToArray();

    /// <summary>The Presets tab's Close: the windows this preset opened, gracefully and on purpose (no auto-rejoin).</summary>
    object ClosePresetWindows(Guid presetId)
    {
        RequireVault();
        var ids = PresetWindows(presetId);
        return ids.Length == 0 ? 0 : CloseAccounts(ids, force: false);
    }

    /// <summary>The game's place id, or null without a game or for a share link (its place is only known when it opens).</summary>
    static long? PresetPlaceId(LaunchPreset p) =>
        p.PlaceInput.Length > 0 && GameTarget.TryParse(p.PlaceInput, "", out var target, out _) && target!.ShareCode is null && target.PlaceId > 0
            ? target.PlaceId : null;

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
        // Without a game there's no server to choose; following a player needs none (their game is used).
        string requested = Str(a, "server");
        string server = requested == "follow" ? "follow" : place.Length == 0 ? "any" : PresetServers.Contains(requested) ? requested : "any";
        string job = server == "job" ? Str(a, "jobId").Trim() : "";
        if (server == "job" && job.Length == 0)
            throw new UserError(L.T("Enter the server's Job ID."));
        // "Rejoin into": only with a game, and not for a preset that follows a player (its rejoins follow that player).
        string rejoinWith = Str(a, "rejoinWith").Trim();
        if (place.Length == 0 || server == "follow" || !(rejoinWith == "any" || Guid.TryParse(rejoinWith, out var with) && ids.Contains(with)))
            rejoinWith = "";
        int? delay = Has(a, "delaySeconds", JsonValueKind.Number) ? (int)Math.Clamp(Long(a, "delaySeconds"), 0, 120) : null;
        string profile = Str(a, "profile") == AfkProfile ? AfkProfile : "";
        // A restart opens the same game again (or follows again); a preset without a game has nothing to restart.
        int restartHours = (place.Length > 0 || server == "follow") && RestartChoices.Contains((int)Long(a, "restartHours")) ? (int)Long(a, "restartHours") : 0;
        var schedule = ParseSchedule(a, vault.Data.Settings.Presets.FirstOrDefault(p => p.Id == Id(a))?.Schedule);
        // Its own Discord channel: absent keeps the one saved, "" goes back to Settings' one, a link replaces it.
        string? discord = OptStr(a, "discordWebhook")?.Trim();
        if (discord is { Length: > 0 })
            discord = DiscordWebhook.TryNormalize(discord, out string normalized) ? normalized
                : throw new UserError(L.T("That isn't a Discord webhook link. It looks like https://discord.com/api/webhooks/… (Channel settings › Integrations › Webhooks › Copy Webhook URL)."));

        // Checked now, so a typo shows while saving rather than when the preset runs.
        RobloxUser? followed = null;
        if (server == "follow")
        {
            string who = Str(a, "followPlayer").Trim().TrimStart('@').Trim();
            followed = await LookUpPlayerAsync(who) ?? throw new UserError(L.F("Roblox has no player called {0}.", who));
        }
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
        preset.FollowPlayer = followed?.Name ?? "";
        preset.FollowUserId = followed?.Id ?? 0;
        preset.DelaySeconds = delay;
        preset.AntiAfk = OptBool(a, "antiAfk");
        preset.AutoRejoin = OptBool(a, "autoRejoin");
        preset.RejoinWith = rejoinWith;
        preset.Profile = profile;
        preset.RestartHours = restartHours;
        preset.Arrange = OptBool(a, "arrange");
        if (discord is not null && discord != preset.DiscordWebhook)
        {
            preset.DiscordWebhook = discord;
            DiscordRouteChanged(preset.Id); // what waited for the old link isn't sent to it
        }
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

    /// <summary>At most <paramref name="max"/> UTF-16 units, cut between whole text elements (never inside an emoji or accent).</summary>
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

    /// <summary>A copy right after the original (and its earlier copies), named "… (copy)" or numbered. Its schedule comes along
    /// off, so the two don't start the same accounts at the same minute.</summary>
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
            RejoinWith = source.RejoinWith,
            Profile = source.Profile,
            RestartHours = source.RestartHours,
            Arrange = source.Arrange,
            DiscordWebhook = source.DiscordWebhook,
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

    /// <summary>The Presets tab's schedule switch; turning it on checks it like the form (a passed one-off can't be turned on).</summary>
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

    /// <summary>Opens the preset's remaining accounts in order through the dock's Play launch (same waits, warnings, Stop, progress).</summary>
    async Task<LaunchResult> StartPresetAsync(Guid id, bool force)
    {
        var vault = RequireVault();
        var preset = vault.Data.Settings.Presets.FirstOrDefault(p => p.Id == id) ?? throw new UserError(L.T("Preset not found."));
        var accounts = preset.AccountIds.Distinct().Select(FindAccount).OfType<Account>().ToList();
        if (accounts.Count == 0)
            throw new UserError(L.T("This preset has no accounts left."));
        CheckCanLaunch(accounts);
        GameTarget? target = null;
        if (preset.Server == "follow")
            target = await PresetFollowTargetAsync(vault, preset, accounts);
        else if (preset.PlaceInput.Length > 0
            && !GameTarget.TryParse(preset.PlaceInput, preset.Server == "job" ? preset.JobId : "", out target, out string error))
            throw new UserError(error);

        // A link that already names a server (or a private one) keeps it.
        string server = preset.Server;
        bool pick = target is { JobId: null, LinkCode: null, ShareCode: null, AccessCode: null } && server is "most" or "fewest" or "ping" or "random";
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

    /// <summary>Where a follow-a-player preset opens: the player's server now, as Roblox shows it to the preset's accounts (failing
    /// them, the other signed-in ones); rejoins follow too. When Roblox won't show one (offline, website, Studio, hidden), the
    /// preset doesn't run and says why.</summary>
    async Task<GameTarget> PresetFollowTargetAsync(Vault vault, LaunchPreset preset, IReadOnlyList<Account> accounts)
    {
        if (preset.FollowUserId <= 0)
            throw new UserError(L.T("This preset joins a player who isn't saved with it. Edit the preset and type the player's name again."));
        string player = preset.FollowPlayer.Length > 0 ? preset.FollowPlayer : preset.FollowUserId.ToString();
        var viewers = accounts.Concat(vault.Data.Accounts.Where(other => !accounts.Contains(other))).Where(CanAskPresence).ToList();
        if (viewers.Count == 0)
            throw new UserError(L.T("None of the selected accounts is signed in, so Bloxkeep can't ask Roblox where this player is. Sign them in again first."));
        Progress(L.F("Looking for {0}…", player));
        var (seen, failed) = await WherePlayerIsAsync(preset.FollowUserId, viewers, vault);
        if (seen is null && failed is not null)
            throw new UserError(failed is RobloxApiException { CookieInvalid: true }
                ? L.T("Roblox didn't accept the selected accounts' sign-in. Check their sign-in, then try again.")
                : ErrorText(failed));
        return (seen is null ? "offline" : PlayerStatus(seen)) switch
        {
            "found" => new GameTarget(seen!.PlaceId!.Value, seen.JobId) { Follow = new FollowedPlayer(preset.FollowUserId, player) },
            "hidden" => throw new UserError(L.F("{0} is in a game, but Roblox doesn't show which server to your accounts (their privacy settings), so this preset can't join them.", player)),
            "studio" => throw new UserError(L.F("{0} is in Roblox Studio, not in a game, so this preset can't join them yet.", player)),
            "website" => throw new UserError(L.F("{0} is on the Roblox website, not in a game, so this preset can't join them yet.", player)),
            _ => throw new UserError(L.F("{0} is offline, or hides being online, so this preset can't join them yet.", player)),
        };
    }

    /// <summary>A public server with room for all accounts: fullest or emptiest (Roblox's order), lowest ping over two pages, or
    /// random. With none, or no answer, Roblox picks as usual.</summary>
    async Task<(GameTarget Target, string? Note)> PickServerAsync(GameTarget target, string choice, int party, CancellationToken ct)
    {
        if (party == 0)
            return (target, null); // every account is already open; nothing will join
        Progress(L.T("Finding a server…"));
        var servers = new List<ServerInfo>();
        bool failed = false;
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
            failed = true;
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
        // Nothing loaded says nothing about room in the servers: said as what happened.
        string note = failed && servers.Count == 0 ? L.T("Couldn't load the server list, so Roblox picks a server.")
            : L.F("No server has room for {0} accounts right now, so Roblox picks one.", party);
        Toast(note, "warn");
        return (target, note);
    }

    // ───────── options of the windows a preset opened ─────────
    // A client remembers the preset that opened it (PendingLaunch.PresetId, kept through rejoins, restarts and saved sessions).
    // Options are read from the preset as it is now, so edits apply at once; a deleted preset or Play follows the usual settings.

    LaunchPreset? PresetFor(Guid? id) =>
        id is Guid preset ? _vault?.Data.Settings.Presets.FirstOrDefault(p => p.Id == preset) : null;

    static bool AutoRejoinFor(LaunchPreset? preset, Account account) => preset?.AutoRejoin ?? account.AutoRejoin;

    static bool AntiAfkFor(LaunchPreset? preset, AppSettings settings) => preset?.AntiAfk ?? settings.AntiAfk;

    bool RejoinOn(Account account, Guid? presetId) => AutoRejoinFor(PresetFor(presetId), account);

    /// <summary>A preset's "Rejoin into" (<see cref="LaunchPreset.RejoinWith"/>): the server its chosen account, or any other window
    /// it opened, plays in now per that window's log. Servers Roblox calls full, ended or closed to this account, or a game that banned
    /// it, are passed over; none left: null, and the rejoin goes to any server of its game.</summary>
    async Task<GameTarget?> TeammateTargetAsync(Account account, RejoinRequest request, CancellationToken ct)
    {
        if (PresetFor(request.PresetId) is not { RejoinWith.Length: > 0 } preset || request.Target.IsPrivate || request.Target.ShareCode is not null)
            return null;
        Guid? only = Guid.TryParse(preset.RejoinWith, out var chosen) ? chosen : null;
        var servers = _running
            .Where(r => r.Key != account.Id && (only is Guid one ? r.Key == one : r.Value.Launch.PresetId == preset.Id))
            .OrderBy(r => r.Value.Launch.StartedAt)
            .Select(r => _logs.StateOf(r.Value.Pid))
            .Where(s => s is { Status: "ingame", PlaceId: not null, JobId.Length: > 0 })
            .Select(s => new GameTarget(s!.PlaceId!.Value, s.JobId))
            .Distinct().ToList();
        foreach (var target in servers)
        {
            if (GameBanOf(account, target.PlaceId) is not null)
                continue;
            var answer = await CheckJoinAsync(account, target, "Rejoining another account's server", ct);
            ct.ThrowIfCancellationRequested();
            if (answer is not ({ Full: true } or { Ended: true } or { Refused: true }))
                return target;
        }
        return null;
    }

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

    /// <summary>A light-AFK preset's window is minimised once in its game (Roblox draws less then). Only once, so a window brought
    /// back up stays up; one in use at that moment is left alone.</summary>
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

    /// <summary>Windows maximised for Settings' "Maximise" (by process id), and at which point of starting up.</summary>
    readonly Dictionary<int, string> _maximised = new();
    /// <summary>When each window Bloxkeep opened was first seen (by process id): one whose client writes no log is maximised a while after.</summary>
    readonly Dictionary<int, DateTime> _windowFirstSeen = new();
    /// <summary>Roblox gives its window its size as it finishes starting up: a window not in a game yet is left this long first.</summary>
    static readonly TimeSpan MaximiseWithoutLog = TimeSpan.FromSeconds(10);

    /// <summary>Settings' "Maximise" for windows Bloxkeep opened this run: once up a few seconds or in a game, and again on joining if
    /// Roblox resized it. Maximised, not fullscreen, so windows don't fight over Roblox's one fullscreen switch. Not light-AFK or
    /// tiled presets' windows, nor one the person minimised; not while they're busy in another window (Windows brings a maximised
    /// one to the front).</summary>
    void MaximiseNewWindows(DateTime now, Dictionary<int, List<IntPtr>> windows)
    {
        foreach (int gone in _maximised.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _maximised.Remove(gone);
        foreach (int gone in _windowFirstSeen.Keys.Where(pid => !windows.ContainsKey(pid)).ToList())
            _windowFirstSeen.Remove(gone);
        if (_vault is not { Data.Settings.MaximizeOnOpen: true })
            return;
        int? foreground = null;
        foreach (var run in _running.Values)
        {
            // Only windows opened since Bloxkeep started: ones picked up after a restart were sized by the person already.
            if (run.Launch.StartedAt < _started || !windows.TryGetValue(run.Pid, out var list) || list.Count == 0)
                continue;
            if (PresetFor(run.Launch.PresetId) is { } preset && (preset.Profile == AfkProfile || preset.Arrange == true))
                continue;
            if (!_windowFirstSeen.TryGetValue(run.Pid, out var seen))
                _windowFirstSeen[run.Pid] = seen = now;
            // In a game, or up a while (Roblox's home screen, still loading, or a client that writes no log).
            string step = _logs.StateOf(run.Pid)?.Status == "ingame" ? "ingame" : now - seen >= MaximiseWithoutLog ? "shown" : "";
            if (step.Length == 0 || (_maximised.TryGetValue(run.Pid, out var done) && (done == step || done == "ingame")))
                continue;
            if ((foreground ??= RobloxProcesses.ForegroundPid()) != run.Pid && AntiAfk.PersonActive())
                continue; // the next tick: not over what the person is doing
            RobloxProcesses.Maximise(RobloxProcesses.MainWindowOf(list));
            _maximised[run.Pid] = step;
        }
    }

    /// <summary>Restarts under way: the account's client is closing, to open again into this target (see <see cref="QueueRejoin"/>).</summary>
    readonly Dictionary<Guid, (GameTarget Target, DateTime At)> _restarts = new();
    /// <summary>At least this long after the last client started, so a preset's windows (opened together) aren't all reopened at once.</summary>
    static readonly TimeSpan RestartSpacing = TimeSpan.FromMinutes(1);
    /// <summary>A client asked to close for a restart that is still there after this is left alone (tried again later).</summary>
    static readonly TimeSpan RestartCloseTimeout = TimeSpan.FromMinutes(1);

    /// <summary>A preset's "restart every N hours": a client in its game that ran that long is closed and reopened through auto-rejoin's
    /// path, one at a time (menu or error screens are auto-rejoin's). A window in use while the person is at the PC waits; one merely
    /// in front of an idle PC is restarted. A client that couldn't be reopened (Multi-Roblox not ready with other windows, a signed-out
    /// or banned account) is left running.</summary>
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
            // Open before Bloxkeep started and its log not found yet: it may be on an error screen, auto-rejoin's.
            if (_logs.StateOf(run.Pid) is null && run.Launch.StartedAt < _started && now - _started < RobloxLogWatcher.SearchLimit)
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

    /// <summary>Where a restart goes: the client's current server while it's the game it was opened into (a preset's accounts stay
    /// together), else the same game or private server. A game picked in the window since is the one it goes back into.</summary>
    GameTarget RestartTarget((int Pid, PendingLaunch Launch) run)
    {
        var target = run.Launch.Target!;
        if (_logs.StateOf(run.Pid)?.ChosenPlace is { } chosen && chosen != target.PlaceId)
            target = new GameTarget(chosen) { Follow = target.Follow };
        return !target.IsPrivate && _logs.StateOf(run.Pid) is { Status: "ingame", JobId: { } job, PlaceId: { } place } && place == target.PlaceId
            ? target with { JobId = job }
            : target with { JobId = null };
    }
}
