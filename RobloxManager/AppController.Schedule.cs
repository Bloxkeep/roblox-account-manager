using System.Globalization;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Scheduled presets and the launch order of accounts.
///
/// A preset can run by itself at a time on chosen weekdays (or once, on a date) and close the accounts it opened after
/// some hours or at a time. The process timer checks once per minute of local wall-clock time: a DST change or a
/// clock set by hand only ever skips a minute or shows one twice, and a preset runs at most once per scheduled minute
/// and never twice within a day of the PC's own uptime clock. Minutes that passed while Bloxkeep wasn't running are
/// not caught up. The run asks nobody anything: a launch that would need a confirmation (Multi-Roblox off, low RAM)
/// is skipped and said so.
/// </summary>
public sealed partial class AppController
{
    static readonly CultureInfo Inv = CultureInfo.InvariantCulture;
    /// <summary>A preset that just ran can't run again before this much uptime (a day minus the biggest clock shift).</summary>
    static readonly TimeSpan MinScheduleGap = TimeSpan.FromHours(22);
    const double MaxStopHours = 168;

    /// <summary>A scheduled run whose stop is still to come: it closes only the accounts the run opened.</summary>
    sealed class ScheduledRun
    {
        public required Guid PresetId;
        public required string Name;
        public required List<Guid> Opened;
        /// <summary>Uptime (<see cref="Environment.TickCount64"/>, ms) to stop at, for "after N hours".</summary>
        public long? StopAtTick;
        /// <summary>Local minute ("yyyy-MM-dd HH:mm") to stop at, for "at a time".</summary>
        public string? StopAtMinute;
    }

    readonly List<ScheduledRun> _scheduledRuns = new();
    /// <summary>Presets whose run stopped while the vault was locked, so the vault still lists it (see <see cref="ResumeSchedules"/>).</summary>
    readonly HashSet<Guid> _stoppedWhileLocked = new();
    /// <summary>Uptime at which each preset last started by schedule.</summary>
    readonly Dictionary<Guid, long> _scheduledStartTick = new();
    string _lastScheduleMinute = "";
    /// <summary>The local time of the last new-minute check; default after an unlock (the lock's own report covers that gap).</summary>
    DateTime _lastScheduleCheck;
    /// <summary>Runs due while the vault is locked (noted when it locked): each is reported once as missed.</summary>
    List<(Guid Id, string Name, string Minute)> _lockedSchedules = new();
    /// <summary>Preset and minute of each run reported as missed while locked, so the unlock doesn't report it again.</summary>
    readonly HashSet<(Guid, string)> _missedReported = new();
    /// <summary>Since when runs could have been missed: the app's start, then each lock.</summary>
    DateTime _lockedSince = DateTime.Now;
    /// <summary>Accounts a scheduled stop is closing right now: a start in the same minute waits for them to go.</summary>
    readonly HashSet<Guid> _stopClosing = new();
    /// <summary>How long a scheduled start waits for a manual launch to finish before giving up.</summary>
    static readonly TimeSpan ScheduleLaunchWait = TimeSpan.FromMinutes(5);
    /// <summary>How long a scheduled start waits for the clients its own stop (the same minute) is closing.</summary>
    static readonly TimeSpan ScheduleStopWait = TimeSpan.FromSeconds(20);

    static string MinuteKey(DateTime local) => local.ToString("yyyy-MM-dd HH:mm", Inv);

    static bool TryTime(string text, out TimeSpan time) =>
        TimeSpan.TryParseExact(text, @"hh\:mm", Inv, out time);

    /// <summary>The form's "schedule" object checked; none when the page sent null (no schedule set up), the old one when it sent nothing.</summary>
    static PresetSchedule? ParseSchedule(JsonElement a, PresetSchedule? old)
    {
        if (Has(a, "schedule", JsonValueKind.Null))
            return null;
        if (!Has(a, "schedule", JsonValueKind.Object))
            return old;
        var sc = a.GetProperty("schedule");
        var s = new PresetSchedule
        {
            Enabled = Bool(sc, "enabled"),
            Time = Str(sc, "time").Trim(),
            Date = Str(sc, "date").Trim(),
            Days = Has(sc, "days", JsonValueKind.Array)
                ? sc.GetProperty("days").EnumerateArray()
                    .Where(d => d.ValueKind == JsonValueKind.Number && d.TryGetInt32(out int n) && n is >= 0 and <= 6)
                    .Select(d => d.GetInt32()).Distinct().Order().ToList()
                : new(),
            Stop = Str(sc, "stop") is "after" or "at" ? Str(sc, "stop") : "never",
            StopHours = Has(sc, "stopHours", JsonValueKind.Number) ? Math.Clamp(sc.GetProperty("stopHours").GetDouble(), 0, MaxStopHours) : 0,
            StopTime = Str(sc, "stopTime").Trim(),
            LastRun = old?.LastRun ?? "",
        };
        if (s.Date.Length > 0)
            s.Days.Clear(); // once, on that date
        if (!s.Enabled)
            return s; // kept as typed, for when it is turned on again
        if (!TryTime(s.Time, out _))
            throw new UserError(L.T("Enter the start time as HH:mm."));
        if (s.Date.Length > 0)
        {
            if (!DateTime.TryParseExact(s.Date, "yyyy-MM-dd", Inv, DateTimeStyles.None, out var date))
                throw new UserError(L.T("Pick the date to run on."));
            if (NextRun(s) is null)
                throw new UserError(L.T("That date and time have already passed."));
        }
        else if (s.Days.Count == 0)
            throw new UserError(L.T("Pick at least one weekday, or a date."));
        if (s.Stop == "after" && s.StopHours <= 0)
            throw new UserError(L.F("Enter how many hours to play (up to {0}).", MaxStopHours));
        if (s.Stop == "at" && !TryTime(s.StopTime, out _))
            throw new UserError(L.T("Enter the stop time as HH:mm."));
        return s;
    }

    static object? ScheduleDto(PresetSchedule? s) =>
        s is null ? null : new { s.Enabled, s.Time, s.Days, s.Date, s.Stop, s.StopHours, s.StopTime };

    /// <summary>The next minute this schedule starts at (local time), or null when it never will.</summary>
    static DateTime? NextRun(PresetSchedule? s)
    {
        if (s is not { Enabled: true } || !TryTime(s.Time, out var time))
            return null;
        var now = DateTime.Now;
        var thisMinute = new DateTime(now.Year, now.Month, now.Day, now.Hour, now.Minute, 0, DateTimeKind.Local);
        if (s.Date.Length > 0)
        {
            if (!DateTime.TryParseExact(s.Date, "yyyy-MM-dd", Inv, DateTimeStyles.AssumeLocal, out var date))
                return null;
            var at = date.Add(time);
            return at >= thisMinute && s.LastRun != MinuteKey(at) ? at : null;
        }
        for (int d = 0; d <= 7; d++)
        {
            var at = now.Date.AddDays(d).Add(time);
            if (s.Days.Contains((int)at.DayOfWeek) && at >= thisMinute && s.LastRun != MinuteKey(at))
                return at;
        }
        return null;
    }

    static bool DueAt(PresetSchedule s, DateTime local) =>
        s.Time == local.ToString("HH\\:mm", Inv)
        && (s.Date.Length > 0 ? s.Date == local.ToString("yyyy-MM-dd", Inv) : s.Days.Contains((int)local.DayOfWeek));

    /// <summary>Every process-timer tick: stops that are due, then (once per wall-clock minute) starts that are due.</summary>
    void CheckSchedules()
    {
        var local = DateTime.Now;
        string minute = MinuteKey(local);
        long tick = Environment.TickCount64;
        // A stop whose minute passed while the PC slept still happens, late rather than never.
        foreach (var run in _scheduledRuns.Where(r => r.StopAtTick is { } at ? tick >= at : string.CompareOrdinal(minute, r.StopAtMinute) >= 0).ToList())
            StopScheduledRun(run);
        if (minute == _lastScheduleMinute)
            return;
        _lastScheduleMinute = minute;
        var lastCheck = _lastScheduleCheck;
        _lastScheduleCheck = local;
        if (_vault is not { } vault)
        {
            // Said once its minute is over: an unlock within the minute still runs it (and nothing is reported).
            foreach (var missed in _lockedSchedules.Where(m => string.CompareOrdinal(m.Minute, minute) < 0).ToList())
            {
                _missedReported.Add((missed.Id, missed.Minute));
                Notify(L.F("{0} didn't run at {1}: Bloxkeep was locked.", missed.Name, missed.Minute[11..]), "warn", "scheduleMissed", missed.Id);
            }
            _lockedSchedules.RemoveAll(m => string.CompareOrdinal(m.Minute, minute) < 0);
            return;
        }
        _lockedSchedules.Clear();
        // The timer didn't tick for a while (the PC slept, or hung): a run due in between is said to be missed, once.
        if (lastCheck != default && local - lastCheck > TimeSpan.FromSeconds(90))
            ReportScheduleGap(vault, lastCheck, local);
        var due = new List<LaunchPreset>();
        foreach (var preset in vault.Data.Settings.Presets)
        {
            if (preset.Schedule is not { Enabled: true } s || !DueAt(s, local) || s.LastRun == minute)
                continue;
            if (_scheduledStartTick.TryGetValue(preset.Id, out long last) && tick - last < MinScheduleGap.TotalMilliseconds)
                continue; // the clock was set back: it ran already
            s.LastRun = minute;
            _scheduledStartTick[preset.Id] = tick;
            if (s.Date.Length > 0)
                s.Enabled = false; // a one-off has had its turn
            due.Add(preset);
        }
        if (due.Count == 0)
            return;
        PersistSoon();
        PushState(); // the next run time changed
        _ = RunScheduledAsync(due);
    }

    /// <summary>Presets due the same minute run one after another: a second launch can't start while one is going.</summary>
    async Task RunScheduledAsync(List<LaunchPreset> due)
    {
        foreach (var preset in due)
        {
            try
            {
                await RunScheduledOneAsync(preset);
            }
            catch (Exception ex)
            {
                Log.Write(ex);
            }
        }
    }

    HashSet<Guid> OpenOrOpening() => _running.Keys.Concat(_pending.Select(p => p.AccountId)).ToHashSet();

    async Task RunScheduledOneAsync(LaunchPreset preset)
    {
        var vault = _vault;
        string name = preset.Name;
        var startedAt = DateTime.Now; // the scheduled minute, give or take: a stop "at" counts from here, not from when the launch ends
        // A manual launch under way would refuse this one ("already launching"): it is waited for, within reason.
        for (var until = DateTime.UtcNow + ScheduleLaunchWait; _launchCts is not null && DateTime.UtcNow < until && _vault == vault;)
            await Task.Delay(1000);
        // A stop of this preset that fired this same minute (stop time = start time) is still closing its clients:
        // they leave the open list within seconds; waited for, so the start opens them again instead of skipping them.
        for (var until = DateTime.UtcNow + ScheduleStopWait; preset.AccountIds.Any(id => _stopClosing.Contains(id) && _running.ContainsKey(id)) && DateTime.UtcNow < until && _vault == vault;)
            await Task.Delay(500);
        _stopClosing.ExceptWith(preset.AccountIds);
        if (_vault != vault)
            return;
        // The newest run owns this preset's stop: a pending one (every day at 20:00, stop after 24 hours — the old
        // stop falls after the new start) is taken over, its accounts still open counted as this run's, so the old
        // stop can't close everything minutes after the new start with nothing reopening.
        var taken = _scheduledRuns.Where(r => r.PresetId == preset.Id).ToList();
        _scheduledRuns.RemoveAll(r => r.PresetId == preset.Id);
        var before = OpenOrOpening();
        bool started = false;
        try
        {
            LaunchResult result;
            try
            {
                result = await StartPresetAsync(preset.Id, force: false);
            }
            catch (Exception ex) when (ex is UserError or RobloxApiException || RobloxApi.IsExpectedError(ex))
            {
                Notify(L.F("Scheduled preset {0} didn't run: {1}", name, ErrorText(ex)), "warn", "scheduleFail", preset.Id);
                return;
            }
            if (result.NeedsConfirm)
            {
                // The question Play would ask (Multi-Roblox not ready, low RAM) isn't answered for the person.
                string why = (result.Message ?? "").Replace("\n\n" + L.T("Open anyway?"), "").Replace("\n\n", " ");
                Notify(L.F("Scheduled preset {0} didn't run: {1}", name, why), "warn", "scheduleFail", preset.Id);
                return;
            }
            if (result.Cancelled)
                return; // Stop was pressed (or Bloxkeep locked): the person is there and saw it
            started = true;
            if (result.Launched > 0)
                Notify(L.P(result.Launched, "Scheduled preset {0} started: {1} account opened.", "Scheduled preset {0} started: {1} accounts opened.", name, result.Launched), "ok", "schedule", preset.Id);
            else if (result.Errors is not { Count: > 0 })
                Notify(L.F("Scheduled preset {0}: its accounts were already open.", name), "info", "schedule", preset.Id);
            // Its own type: the "started" notice a moment earlier would otherwise hold this one back for five minutes.
            if (result.Errors is { Count: > 0 } errors)
                Notify(L.P(errors.Count, "Scheduled preset {0}: {1} account didn't open. {2}", "Scheduled preset {0}: {1} accounts didn't open. {2}", name, errors.Count, errors[0]), "warn", "scheduleFail", preset.Id);
        }
        finally
        {
            // However the launch ended (an error half way, Stop pressed, a lock, something unexpected), what it opened
            // gets its stop. Nothing new opened by a start that didn't happen: the old stop still applies.
            ArmScheduledStop(preset, vault, startedAt, before, taken, started);
        }
    }

    /// <summary>
    /// The stop of a scheduled run that just started. Only what this run opened is closed at its stop (plus what the
    /// run it took over had opened and is still open); accounts that were already open otherwise stay.
    /// </summary>
    void ArmScheduledStop(LaunchPreset preset, Vault? vault, DateTime startedAt, HashSet<Guid> before, List<ScheduledRun> taken, bool started)
    {
        bool Open(Guid id) => _running.ContainsKey(id) || _pending.Any(p => p.AccountId == id);
        var fresh = preset.AccountIds.Where(id => !before.Contains(id) && Open(id)).ToList();
        if (!started && fresh.Count == 0)
        {
            _scheduledRuns.AddRange(taken);
            return;
        }
        var opened = fresh.Concat(taken.SelectMany(r => r.Opened).Where(Open)).Distinct().ToList();
        // Locked meanwhile, the run is kept in memory and written to the vault at the next unlock; another vault
        // unlocked meanwhile has other accounts.
        if (preset.Schedule is not { } s || s.Stop == "never" || opened.Count == 0 || _vault is not null && _vault != vault)
            return;
        var run = new ScheduledRun { PresetId = preset.Id, Name = preset.Name, Opened = opened };
        if (s.Stop == "after")
            run.StopAtTick = Environment.TickCount64 + (long)(s.StopHours * 3_600_000);
        else if (s.Stop == "at" && TryTime(s.StopTime, out var time))
        {
            var at = startedAt.Date.Add(time);
            if (at <= startedAt)
                at = at.AddDays(1); // a stop at or before the start time means the next day (22:00 → 02:00)
            run.StopAtMinute = MinuteKey(at);
        }
        else
            return;
        _scheduledRuns.Add(run);
        if (NoteScheduledRuns())
            PersistSoon();
    }

    void StopScheduledRun(ScheduledRun run)
    {
        _scheduledRuns.Remove(run);
        // Locked, the vault can't be told; the next unlock mustn't arm this stop again from what it still holds.
        if (_vault is null)
            _stoppedWhileLocked.Add(run.PresetId);
        if (NoteScheduledRuns())
            PersistSoon();
        // A client waiting to rejoin (auto-rejoin's wait can be minutes long) or to restart is called off too.
        var ids = run.Opened.Where(id => _running.ContainsKey(id) || _pending.Any(p => p.AccountId == id) || _rejoinQueue.Any(r => r.AccountId == id)).ToArray();
        if (ids.Length == 0)
            return; // closed by hand meanwhile
        _stopClosing.UnionWith(ids);
        CloseAccounts(ids, force: false); // gracefully, and marked as closed on purpose: no auto-rejoin
        Notify(L.P(ids.Length, "Scheduled preset {0} finished: closing {1} account.", "Scheduled preset {0} finished: closing {1} accounts.", run.Name, ids.Length), "info", "scheduleStop", run.PresetId);
        PushState();
    }

    /// <summary>
    /// The account's client is gone for good (closed by the person, a drop auto-rejoin won't undo, never showed up):
    /// no scheduled stop owns it any more, so a client the person opens for it by hand later isn't closed by one.
    /// Through an auto-rejoin the stop keeps it.
    /// </summary>
    void ForgetScheduledOpen(Guid accountId)
    {
        bool changed = false;
        foreach (var run in _scheduledRuns)
            changed |= run.Opened.Remove(accountId);
        if (changed && NoteScheduledRuns())
            PersistSoon();
    }

    /// <summary>
    /// Writes the runs whose stop is still to come into the vault, so an exit, update or crash before the stop doesn't
    /// leave their clients open for good: the next unlock arms them again (see <see cref="ResumeSchedules"/>).
    /// </summary>
    /// <returns>true when the list changed, i.e. the vault needs saving.</returns>
    bool NoteScheduledRuns()
    {
        if (_vault is not { } vault)
            return false;
        long tick = Environment.TickCount64;
        var now = DateTime.UtcNow;
        var runs = _scheduledRuns.Select(r => new ScheduledRunState
        {
            PresetId = r.PresetId,
            Name = r.Name,
            Opened = r.Opened.ToList(),
            StopAt = r.StopAtTick is { } at ? now.AddMilliseconds(at - tick) : null,
            StopAtMinute = r.StopAtMinute,
        }).ToList();
        var noted = vault.Data.ScheduledRuns;
        if (noted.Count == runs.Count && noted.Zip(runs).All(p => p.First.PresetId == p.Second.PresetId && p.First.StopAtMinute == p.Second.StopAtMinute
                && p.First.Opened.SequenceEqual(p.Second.Opened) && (p.First.StopAt is null) == (p.Second.StopAt is null)
                // An "after" stop is written as a time; one re-read on each save would move it by milliseconds.
                && (p.First.StopAt is not { } a || p.Second.StopAt is not { } b || (a - b).Duration() < TimeSpan.FromSeconds(5))))
            return false;
        vault.Data.ScheduledRuns = runs;
        return true;
    }

    /// <summary>
    /// After unlocking: the runs the last run of the app wrote (its clients were picked up again by
    /// <see cref="AdoptOpenSessions"/>) are armed again, also when their preset was deleted since (its clients are still
    /// open); what the app ended in memory meanwhile (a stop fired while locked) wins. The current minute may run
    /// again (a lock hid it), and runs missed since the app started or the vault locked are reported once.
    /// </summary>
    void ResumeSchedules(Vault vault)
    {
        long tick = Environment.TickCount64;
        var now = DateTime.UtcNow;
        foreach (var saved in vault.Data.ScheduledRuns)
        {
            if (_scheduledRuns.Any(r => r.PresetId == saved.PresetId) || _stoppedWhileLocked.Contains(saved.PresetId))
                continue;
            if (saved.StopAt is null && saved.StopAtMinute is null)
                continue;
            var preset = vault.Data.Settings.Presets.FirstOrDefault(p => p.Id == saved.PresetId);
            _scheduledRuns.Add(new ScheduledRun
            {
                PresetId = saved.PresetId,
                Name = preset?.Name ?? (saved.Name.Length > 0 ? saved.Name : L.T("(deleted)")),
                Opened = saved.Opened.Where(id => vault.Data.Accounts.Any(a => a.Id == id)).ToList(),
                StopAtTick = saved.StopAt is { } at ? tick + (long)Math.Max(0, (at - now).TotalMilliseconds) : null,
                StopAtMinute = saved.StopAtMinute,
            });
        }
        _stoppedWhileLocked.Clear();
        if (NoteScheduledRuns())
            PersistSoon();
        // The minute that passed on the lock screen counted as seen: without this the run due right now is skipped
        // (LastRun keeps one that did run from running twice).
        _lastScheduleMinute = "";
        _lastScheduleCheck = default;
        ReportMissedSchedules(vault);
    }

    /// <summary>
    /// Runs due in the minutes strictly between two checks (the current minute still runs as usual), each preset's
    /// latest one said once.
    /// </summary>
    void ReportScheduleGap(Vault vault, DateTime from, DateTime to)
    {
        var fromMinute = new DateTime(from.Year, from.Month, from.Day, from.Hour, from.Minute, 0, from.Kind);
        var toMinute = new DateTime(to.Year, to.Month, to.Day, to.Hour, to.Minute, 0, to.Kind);
        foreach (var preset in vault.Data.Settings.Presets)
        {
            if (preset.Schedule is not { Enabled: true } s || !TryTime(s.Time, out var time))
                continue;
            for (int d = 0; d <= 7; d++)
            {
                var at = toMinute.Date.AddDays(-d).Add(time);
                if (at <= fromMinute)
                    break;
                if (at >= toMinute || !DueAt(s, at) || s.LastRun == MinuteKey(at))
                    continue;
                Notify(L.F("{0} didn't run at {1}: the PC was asleep or busy.", preset.Name, at.ToString("HH\\:mm", Inv)), "warn", "scheduleMissed", preset.Id);
                break;
            }
        }
    }

    /// <summary>Runs due between the app's start (or the lock) and now that didn't happen, each said once.</summary>
    void ReportMissedSchedules(Vault vault)
    {
        var now = DateTime.Now;
        var thisMinute = new DateTime(now.Year, now.Month, now.Day, now.Hour, now.Minute, 0, DateTimeKind.Local);
        foreach (var preset in vault.Data.Settings.Presets)
        {
            if (preset.Schedule is not { Enabled: true } s || !TryTime(s.Time, out var time))
                continue;
            // Only the latest due minute, and only one that is already over: this minute's run is still to come.
            for (int d = 0; d <= 1; d++)
            {
                var at = now.Date.AddDays(-d).Add(time);
                if (at >= thisMinute || !DueAt(s, at))
                    continue;
                string minute = MinuteKey(at);
                if (at >= _lockedSince && s.LastRun != minute && _missedReported.Add((preset.Id, minute)))
                    Notify(L.F("{0} didn't run at {1}: Bloxkeep was locked.", preset.Name, at.ToString("HH\\:mm", Inv)), "warn", "scheduleMissed", preset.Id);
                break;
            }
        }
        _missedReported.Clear();
    }

    /// <summary>Before the vault locks: the runs due next, so each can be reported as missed while it stays locked.</summary>
    void RememberSchedulesForLock()
    {
        _lockedSince = DateTime.Now;
        _lockedSchedules = _vault?.Data.Settings.Presets
            .Select(p => (p.Id, p.Name, Next: NextRun(p.Schedule)))
            .Where(x => x.Next is not null)
            .Select(x => (x.Id, x.Name, MinuteKey(x.Next!.Value)))
            .ToList() ?? new();
    }

    // ───────────────────────────── launch order ─────────────────────────────

    /// <summary>Every account id in launch order: the saved order first, then accounts added since, as listed.</summary>
    static List<Guid> EffectiveLaunchOrder(Vault vault)
    {
        var accounts = vault.Data.Accounts;
        var known = accounts.Select(a => a.Id).ToHashSet();
        var order = vault.Data.Settings.LaunchOrder.Where(known.Contains).Distinct().ToList();
        var seen = order.ToHashSet();
        order.AddRange(accounts.Select(a => a.Id).Where(seen.Add));
        return order;
    }

    List<Guid> LaunchOrderDto() => _vault is { } vault ? EffectiveLaunchOrder(vault) : new();

    /// <summary>The chosen accounts in the global launch order (Play with several selected).</summary>
    List<Account> InLaunchOrder(Vault vault, IEnumerable<Guid> ids)
    {
        var wanted = ids.ToHashSet();
        return EffectiveLaunchOrder(vault).Where(wanted.Contains).Select(FindAccount).OfType<Account>().ToList();
    }

    object? SetLaunchOrder(Guid[] ids)
    {
        var vault = RequireVault();
        var known = vault.Data.Accounts.Select(a => a.Id).ToHashSet();
        var order = ids.Where(known.Contains).ToList();
        order.AddRange(EffectiveLaunchOrder(vault).Where(id => !order.Contains(id))); // anything the page didn't list stays last
        vault.Data.Settings.LaunchOrder = order;
        Persist();
        PushState();
        return null;
    }
}
