using System.Runtime.InteropServices;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Settings › Discord: a channel webhook the person pastes gets a play-time summary (daily, weekly, and/or when a
/// scheduled preset finishes), an optional status message edited in place, and the same events as the Windows
/// notifications. Opt-in and off by default; the link lives in the vault. Messages wait in a short in-memory queue
/// (events within 10 s go out as one), which follows Discord's rate limits and retries with a growing delay while
/// offline. Nothing is sent, or kept waiting, while Bloxkeep is locked. Only account names, game names, times,
/// counts, process ids and RAM ever go out, with Roblox's own picture links of the games and of the accounts (not
/// while names are hidden); never cookies, passwords or the link itself. How they look: docs/discord-style.md.
/// </summary>
public sealed partial class AppController
{
    static readonly TimeSpan DiscordBatch = TimeSpan.FromSeconds(10);
    /// <summary>A summary for a scheduled preset's stop waits for its clients to close, so their stays are counted whole.</summary>
    static readonly TimeSpan DiscordAfterStop = TimeSpan.FromSeconds(30);
    static readonly TimeSpan DiscordMaxBackoff = TimeSpan.FromMinutes(10);
    const int DiscordQueueMax = 50;
    const int MaxBoardAccounts = 24;
    static readonly string[] DiscordParts = ["total", "accounts", "games", "sessions", "kicks"];
    static readonly int[] DiscordBoardMinutes = [2, 5, 10, 15, 30, 60];

    /// <param name="Kind">"events", "summary" or "board" (at most one board update waits; a newer one replaces it).</param>
    /// <param name="DailyDay">The daily (or weekly) summary's due day ("yyyy-MM-dd"), noted as sent only once it went out.</param>
    /// <param name="Route">The preset whose own webhook it goes to (<see cref="LaunchPreset.DiscordWebhook"/>); null for Settings' one.</param>
    sealed record DiscordItem(string Kind, DiscordMessage Message, DiscordPing Ping, string? DailyDay = null, string? WeeklyDay = null, Guid? Route = null);

    /// <summary>A summary sent more than this after its time says it is late.</summary>
    static readonly TimeSpan DiscordLate = TimeSpan.FromMinutes(10);

    readonly List<DiscordItem> _discordQueue = new();
    /// <summary>Events waiting to be sent together: their kind, alert, ping, and the preset whose webhook they go to (null: Settings' one).</summary>
    readonly List<(string Kind, DiscordAlert Alert, DiscordPing Ping, Guid? Route)> _discordEvents = new();
    /// <summary>When each event type was last queued per account (like the Windows notifications: a rejoin loop can't spam).</summary>
    readonly Dictionary<string, DateTime> _discordNotified = new();
    /// <summary>The vault the queue belongs to: after a lock (or another vault) whatever waited is dropped.</summary>
    Vault? _discordVault;
    DateTime _discordBatchDue, _discordNextTry, _discordNextBoard;
    DateTime? _discordLastSent;
    /// <summary>Summaries after a scheduled preset's stop, by preset: each one covers that preset's accounts.</summary>
    readonly Dictionary<Guid, DateTime> _discordSummariesDue = new();
    /// <summary>
    /// A status message whose id came back after the vault locked; handed to the vault at the next unlock. The link is
    /// a secret of the vault: only its hash (<see cref="WebhookKey"/>) stays in memory while locked.
    /// </summary>
    (string UrlKey, string Id)? _discordBoardPosted;
    /// <summary>Summaries that went out after the vault locked (their days); noted as sent at the next unlock.</summary>
    readonly List<(string UrlKey, string? DailyDay, string? WeeklyDay)> _discordSummariesSent = new();
    string? _discordError;
    bool _discordSending;
    int _discordFailures, _discordDropped;

    /// <summary>The language Discord messages are written in.</summary>
    string DiscordLanguage => _vault?.Data.Discord.Language switch { "en" => "en", "vi" => "vi", _ => L.Language };

    string DT(string english) => L.T(english, DiscordLanguage);
    string DF(string english, params object?[] args) => string.Format(DT(english), args);

    object? DiscordDto()
    {
        if (_vault is null)
            return null;
        var d = _vault.Data.Discord;
        return new
        {
            d.Enabled,
            hasWebhook = d.WebhookUrl.Length > 0,
            webhookHint = DiscordWebhook.Mask(d.WebhookUrl),
            d.Stopped,
            d.Username,
            d.AvatarUrl,
            d.Language,
            d.Style,
            d.HideNames,
            summary = new
            {
                daily = d.SummaryDaily, dailyTime = d.SummaryDailyTime, weekly = d.SummaryWeekly, weekday = d.SummaryWeekday,
                weeklyTime = d.SummaryWeeklyTime, afterSchedule = d.SummaryAfterSchedule, days = d.SummaryDays,
                parts = d.SummaryParts, accounts = d.SummaryAccounts,
            },
            board = new { enabled = d.Board, minutes = d.BoardMinutes },
            events = d.Events.Where(NotifyKinds.Contains).ToArray(),
            pings = d.Pings,
            quiet = new { enabled = d.Quiet, from = d.QuietFrom, to = d.QuietTo },
            lastSent = _discordLastSent,
            error = _discordError,
            queued = _discordQueue.Count + (_discordEvents.Count > 0 ? 1 : 0),
        };
    }

    // ───────────────────────────── settings ─────────────────────────────

    /// <summary>Any of the Discord fields; only the ones sent change. Each is checked before anything is kept.</summary>
    object? SetDiscord(JsonElement a)
    {
        var vault = RequireVault();
        var d = vault.Data.Discord;
        var local = DateTime.Now;
        string Time(string name, string current)
        {
            if (OptStr(a, name) is not { } text)
                return current;
            if (!TryTime(text, out _))
                throw new UserError(L.T("Enter the time as HH:mm."));
            return text;
        }
        string dailyTime = Time("summaryDailyTime", d.SummaryDailyTime), weeklyTime = Time("summaryWeeklyTime", d.SummaryWeeklyTime);
        string quietFrom = Time("quietFrom", d.QuietFrom), quietTo = Time("quietTo", d.QuietTo);
        string? username = OptStr(a, "username")?.Trim();
        if (username is not null)
        {
            if (username.Length == 0)
                username = "Bloxkeep";
            // Discord refuses these names for a webhook.
            if (username.Length > DiscordWebhook.MaxUsername || username.Contains("discord", StringComparison.OrdinalIgnoreCase) || username.Contains("clyde", StringComparison.OrdinalIgnoreCase))
                throw new UserError(L.T("Discord doesn't allow that name (up to 80 characters, without \"discord\" or \"clyde\")."));
        }
        string? avatar = OptStr(a, "avatarUrl")?.Trim();
        if (avatar is { Length: > 0 } && (avatar.Length > 2048 || !Uri.TryCreate(avatar, UriKind.Absolute, out var uri) || uri.Scheme != Uri.UriSchemeHttps || avatar.Any(char.IsWhiteSpace)))
            throw new UserError(L.T("The picture link must start with https://."));
        string? language = OptStr(a, "language");
        if (language is not (null or "app" or "en" or "vi"))
            throw new UserError(L.T("Unknown language."));
        string? style = OptStr(a, "style");
        if (style is not (null or "embed" or "plain"))
            throw new UserError(L.T("Unknown message style."));
        List<string>? parts = null;
        if (Has(a, "summaryParts", JsonValueKind.Array))
        {
            parts = a.GetProperty("summaryParts").EnumerateArray().Select(e => e.ValueKind == JsonValueKind.String ? e.GetString()! : "")
                .Where(DiscordParts.Contains).Distinct().ToList();
            if (parts.Count == 0)
                throw new UserError(L.T("Pick at least one thing for the summary."));
        }
        int? days = Has(a, "summaryDays", JsonValueKind.Number) ? (int)Long(a, "summaryDays") : null;
        if (days is not (null or 1 or 7 or 30))
            throw new UserError(L.T("Unknown period."));
        int? weekday = Has(a, "summaryWeekday", JsonValueKind.Number) ? (int)Long(a, "summaryWeekday") : null;
        if (weekday is < 0 or > 6)
            throw new UserError(L.T("Unknown weekday."));
        int? minutes = Has(a, "boardMinutes", JsonValueKind.Number) ? (int)Long(a, "boardMinutes") : null;
        if (minutes is not null && !DiscordBoardMinutes.Contains(minutes.Value))
            throw new UserError(L.T("Update the status message every 2 to 60 minutes."));
        // {event: {kind, enabled}} and {ping: {kind, value}}: one switch or one ping choice at a time.
        (string Kind, bool On)? ev = null;
        if (Has(a, "event", JsonValueKind.Object))
        {
            var e = a.GetProperty("event");
            string kind = Str(e, "kind");
            if (!NotifyKinds.Contains(kind))
                throw new UserError(L.T("Unknown notification kind."));
            ev = (kind, Bool(e, "enabled"));
        }
        (string Kind, string Value)? ping = null;
        if (Has(a, "ping", JsonValueKind.Object))
        {
            var p = a.GetProperty("ping");
            string kind = Str(p, "kind"), value = Str(p, "value").Trim();
            if (kind != "summary" && !NotifyKinds.Contains(kind))
                throw new UserError(L.T("Unknown notification kind."));
            if (DiscordPing.Parse(value) is null)
                throw new UserError(value.StartsWith("role:", StringComparison.Ordinal)
                    ? L.T("A role ID is 17 to 20 digits. Copy it with Developer Mode on: right-click the role › Copy Role ID.")
                    : L.T("A user ID is 17 to 20 digits. Copy it with Developer Mode on: right-click the person › Copy User ID."));
            ping = (kind, value);
        }
        if (OptBool(a, "enabled") is true && d.WebhookUrl.Length == 0)
            throw new UserError(L.T("Paste a webhook link first."));

        // Everything checked: apply.
        bool switchedOn = false;
        if (OptBool(a, "enabled") is { } enabled)
        {
            switchedOn = enabled && !d.Enabled;
            d.Enabled = enabled;
            if (enabled)
                d.Stopped = null;
        }
        d.Username = username ?? d.Username;
        d.AvatarUrl = avatar ?? d.AvatarUrl;
        d.Language = language ?? d.Language;
        d.Style = style ?? d.Style;
        if (OptBool(a, "hideNames") is { } hideNames)
            d.HideNames = hideNames;
        // A summary switched on (or moved), or sending switched on, starts at its next time rather than going out now
        // for a time already past: the latest one due counts as sent.
        string LatestDay(string time, int? day) => LastSummaryDue(local, time, day) is { } at ? DayKey(at) : "";
        if (OptBool(a, "summaryDaily") is { } daily)
            d.SummaryDaily = daily;
        if (switchedOn || OptBool(a, "summaryDaily") is true || OptStr(a, "summaryDailyTime") is not null)
            d.LastDaily = LatestDay(dailyTime, null);
        d.SummaryDailyTime = dailyTime;
        if (OptBool(a, "summaryWeekly") is { } weekly)
            d.SummaryWeekly = weekly;
        d.SummaryWeekday = weekday ?? d.SummaryWeekday;
        if (switchedOn || OptBool(a, "summaryWeekly") is true || OptStr(a, "summaryWeeklyTime") is not null || weekday is not null)
            d.LastWeekly = LatestDay(weeklyTime, d.SummaryWeekday);
        d.SummaryWeeklyTime = weeklyTime;
        if (OptBool(a, "summaryAfterSchedule") is { } afterSchedule)
            d.SummaryAfterSchedule = afterSchedule;
        d.SummaryDays = days ?? d.SummaryDays;
        d.SummaryParts = parts ?? d.SummaryParts;
        if (Has(a, "summaryAccounts", JsonValueKind.Array))
        {
            var known = vault.Data.Accounts.Select(x => x.Id).ToHashSet();
            d.SummaryAccounts = a.GetProperty("summaryAccounts").EnumerateArray()
                .Select(e => e.ValueKind == JsonValueKind.String && Guid.TryParse(e.GetString(), out var g) ? g : Guid.Empty)
                .Where(known.Contains).Distinct().ToList();
        }
        if (OptBool(a, "board") is { } board)
        {
            d.Board = board;
            _discordNextBoard = DateTime.UtcNow; // the first update right away
        }
        d.BoardMinutes = minutes ?? d.BoardMinutes;
        if (ev is { } e2)
        {
            d.Events.Remove(e2.Kind);
            if (e2.On)
                d.Events.Add(e2.Kind);
        }
        if (ping is { } p2)
        {
            if (p2.Value is "" or "none")
                d.Pings.Remove(p2.Kind);
            else
                d.Pings[p2.Kind] = p2.Value;
        }
        if (OptBool(a, "quiet") is { } quiet)
            d.Quiet = quiet;
        d.QuietFrom = quietFrom;
        d.QuietTo = quietTo;
        if (!d.Enabled)
            ResetDiscordQueue();
        PersistSoon();
        PushState();
        return null;
    }

    /// <summary>A new link (checked strictly, kept as discord.com/api/webhooks/{id}/{token}), or "" to forget it.</summary>
    object? SetDiscordWebhook(string url)
    {
        var d = RequireVault().Data.Discord;
        url = url.Trim();
        if (url.Length == 0)
        {
            d.WebhookUrl = "";
            d.Enabled = false;
        }
        else if (DiscordWebhook.TryNormalize(url, out string normalized))
            d.WebhookUrl = normalized;
        else
            throw new UserError(L.T("That isn't a Discord webhook link. It looks like https://discord.com/api/webhooks/… (Channel settings › Integrations › Webhooks › Copy Webhook URL)."));
        // Another webhook (or none): the old status message isn't this one's to edit, and nothing queued goes to it.
        d.BoardMessageId = "";
        d.Stopped = null;
        _discordError = null;
        _discordLastSent = null;
        ResetDiscordQueue();
        PersistSoon();
        PushState();
        return null;
    }

    /// <summary>The whole link, when the person asks to see it (like a cookie's Reveal).</summary>
    object? RevealDiscordWebhook() => RequireVault().Data.Discord.WebhookUrl;

    // ───────────────────────────── what goes out ─────────────────────────────

    /// <summary>
    /// From <see cref="Notify"/>, right after the L.F that made <paramref name="text"/>: an event of a kind left on
    /// under Discord joins the next message, made again from the same sentence and values (see AppController.DiscordAlerts.cs).
    /// </summary>
    void NotifyDiscord(string text, string type, Guid account)
    {
        var made = L.Made(text);
        if (_disposed || _vault is not { Data.Discord: { Enabled: true, WebhookUrl.Length: > 0 } d } vault)
            return;
        var now = DateTime.UtcNow;
        SyncDiscordVault(vault, now);
        // A schedule's message names its preset (account is the preset's id then); an account's, the preset whose
        // window it is (opened by it, or about to rejoin for it).
        Guid? preset = type.StartsWith("schedule", StringComparison.Ordinal) ? account : PresetOfAccount(account);
        if (type == "scheduleStop" && d.SummaryAfterSchedule)
            _discordSummariesDue[account] = now + DiscordAfterStop;
        if (NotifyKind(type) is not { } kind || !d.Events.Contains(kind))
            return;
        string key = $"{type}:{account}";
        // A time ahead of now (the clock was set back) doesn't hold the event back.
        if (_discordNotified.TryGetValue(key, out var last) && last <= now && now - last < NotifyRepeat)
            return;
        if (_discordNotified.Count > 256)
        {
            foreach (string stale in _discordNotified.Where(n => now - n.Value >= NotifyRepeat).Select(n => n.Key).ToList())
                _discordNotified.Remove(stale);
        }
        _discordNotified[key] = now;
        if (_discordEvents.Count == 0)
            _discordBatchDue = now + DiscordBatch;
        // A ban goes with the sign-in events (their switch and ping) under a title of its own.
        _discordEvents.Add((type == "banned" ? type : kind, AlertFor(type, account, made, DiscordText(text), now), PingFor(d, kind), RouteFor(preset)));
    }

    /// <summary>The preset whose window the account's is: opened by it, being opened, or waiting to rejoin for it.</summary>
    Guid? PresetOfAccount(Guid account) =>
        _running.TryGetValue(account, out var run) && run.Launch.PresetId is { } id ? id
            : _rejoinQueue.FirstOrDefault(r => r.AccountId == account)?.PresetId
              ?? _pending.FirstOrDefault(l => l.AccountId == account)?.PresetId;

    /// <summary>A preset with a webhook of its own; null (Settings' webhook) for any other.</summary>
    Guid? RouteFor(Guid? preset) => PresetFor(preset) is { DiscordWebhook.Length: > 0 } p ? p.Id : null;

    /// <summary>Where a message goes: its preset's own webhook, or Settings' one (also once the preset's is removed).</summary>
    string UrlFor(DiscordSettings d, Guid? route) => PresetFor(route) is { DiscordWebhook.Length: > 0 } p ? p.DiscordWebhook : d.WebhookUrl;

    /// <summary>A preset's own webhook changed: what waited for the old one isn't sent to it.</summary>
    void DiscordRouteChanged(Guid preset)
    {
        _discordQueue.RemoveAll(i => i.Route == preset);
        _discordEvents.RemoveAll(e => e.Route == preset);
    }

    /// <summary>The ping chosen for a kind of message, or none during the quiet hours.</summary>
    static DiscordPing PingFor(DiscordSettings d, string kind)
    {
        var ping = DiscordPing.Parse(d.Pings.GetValueOrDefault(kind)) ?? DiscordPing.None;
        return ping.Any && InQuietHours(d, DateTime.Now) ? DiscordPing.None : ping;
    }

    static bool InQuietHours(DiscordSettings d, DateTime local)
    {
        if (!d.Quiet || !TryTime(d.QuietFrom, out var from) || !TryTime(d.QuietTo, out var to) || from == to)
            return false;
        var now = local.TimeOfDay;
        return from < to ? now >= from && now < to : now >= from || now < to; // across midnight (23:00–07:00)
    }

    string EventTitle(string kind) => kind switch
    {
        "drop" => DT("Account dropped"),
        "schedule" => DT("Scheduled preset"),
        "signin" => DT("Sign-in needs renewing"),
        "banned" => DT("Account banned"),
        "ram" => DT("Low RAM"),
        _ => DT("Launch problem"),
    };

    string Duration(long seconds)
    {
        long minutes = seconds / 60;
        return minutes < 60 ? DF("{0} min", minutes) : DF("{0} h {1} min", minutes / 60, minutes % 60);
    }

    static string DayKey(DateTime local) => local.ToString("yyyy-MM-dd", Inv);

    /// <summary>The latest local time at or before <paramref name="local"/> a daily (no weekday) or weekly summary was due.</summary>
    internal static DateTime? LastSummaryDue(DateTime local, string time, int? weekday)
    {
        if (!TryTime(time, out var at))
            return null;
        var due = local.Date + at;
        if (due > local)
            due = due.AddDays(-1);
        if (weekday is int w)
        {
            while ((int)due.DayOfWeek != w)
                due = due.AddDays(-1);
        }
        return due;
    }

    /// <summary>
    /// When a summary is due and neither sent (<paramref name="last"/> is its last sent day) nor waiting in the queue;
    /// null otherwise. A day before the last sent one (the clock set back) isn't due again.
    /// </summary>
    DateTime? DueSummary(bool on, string time, int? weekday, string last, DateTime local, Func<DiscordItem, string?> queuedDay)
    {
        if (!on || LastSummaryDue(local, time, weekday) is not { } at)
            return null;
        string day = DayKey(at);
        return string.CompareOrdinal(day, last) > 0 && !_discordQueue.Any(i => queuedDay(i) == day) ? at : null;
    }

    /// <summary>An account on the status board: its name as Discord shows it, game, connection, time playing, PID and RAM.</summary>
    /// <param name="Status">"ingame", "joining", "menu", "disconnected" or "signedout" (see <see cref="Connection"/>).</param>
    sealed record BoardRow(string Name, long? PlaceId, string? Game, string Status, long Seconds, int Pid, long Ram);

    /// <summary>The status message: every account with a Roblox window open here, its game, time playing, PID and RAM.</summary>
    DiscordMessage BoardMessage(DiscordSettings d, List<RobloxProcesses.PlayerProcess> players)
    {
        var ram = players.ToDictionary(p => p.Pid, p => p.WorkingSet);
        var now = DateTime.UtcNow;
        var rows = _running.Select(r => (Account: FindAccount(r.Key), r.Key, r.Value.Pid, r.Value.Launch))
            .Where(r => r.Account is not null).OrderBy(r => r.Account!.Label, StringComparer.CurrentCultureIgnoreCase)
            .Select(r =>
            {
                long? placeId = _logs.StateOf(r.Pid)?.PlaceId ?? r.Launch.Target?.PlaceId;
                var since = _playing.GetValueOrDefault(r.Key)?.StartedAt ?? r.Launch.StartedAt;
                return new BoardRow(DiscordName(r.Account!), placeId, placeId is long id ? GameName(id, null) : null, Connection(r.Account!, r.Pid, r.Launch),
                    (long)Math.Max(0, (now - since).TotalSeconds), r.Pid, ram.GetValueOrDefault(r.Pid));
            }).ToList();
        return BoardMessage(d.BoardMinutes, rows, now);
    }

    /// <summary>
    /// The status message from its rows: how many are in a game in the title (green when all are, yellow when one is
    /// cut off), the others' state and Roblox's RAM below it, a field per account.
    /// </summary>
    DiscordMessage BoardMessage(int minutes, List<BoardRow> rows, DateTime now)
    {
        string footer = DF("Bloxkeep · Board · updates every {0} min", minutes);
        if (rows.Count == 0)
            return new DiscordMessage(DT("Nobody is playing"), DT("No Roblox window is open."), [], footer, DiscordWebhook.Grey, now) { Thumbnail = DiscordWebhook.Logo };
        string Status(string status, bool start) => (status, start) switch
        {
            ("ingame", true) => DT("In game"), ("ingame", false) => DT("in game"),
            ("joining", true) => DT("Joining"), ("joining", false) => DT("joining"),
            ("menu", true) => DT("In the menu"), ("menu", false) => DT("in the menu"),
            ("signedout", true) => DT("Signed out"), ("signedout", false) => DT("signed out"),
            (_, true) => DT("Disconnected"), _ => DT("disconnected"),
        };
        var fields = rows.Take(MaxBoardAccounts).Select(r => new DiscordField(DiscordWebhook.Escape(r.Name),
            (r.PlaceId is long id ? DiscordWebhook.GameLink(r.Game ?? DF("Place {0}", id), id) : DT("No game"))
            + $"\n{Status(r.Status, true)} · {Duration(r.Seconds)}\nPID `{r.Pid}`" + (r.Ram > 0 ? $" · {r.Ram / MB} MB" : ""), true)).ToList();
        if (rows.Count > MaxBoardAccounts)
            fields.Add(new("…", DF("and {0} more", rows.Count - MaxBoardAccounts), true));
        int inGame = rows.Count(r => r.Status == "ingame");
        bool cutOff = rows.Any(r => r.Status is "disconnected" or "signedout");
        string title = inGame == rows.Count ? DF(rows.Count == 1 ? "{0} account in game" : "{0} accounts in game", rows.Count)
            : DF("{0} of {1} accounts in game", inGame, rows.Count);
        // The ones not in a game, by state ("1 joining · 1 disconnected"), and what Roblox uses of the RAM.
        var lines = new List<string>();
        var others = rows.Where(r => r.Status != "ingame").GroupBy(r => r.Status)
            .OrderBy(g => g.Key switch { "joining" => 2, "menu" => 3, "signedout" => 1, _ => 0 })
            .Select(g => $"**{g.Count()}** {Status(g.Key, false)}").ToList();
        if (others.Count > 0)
            lines.Add(string.Join(" · ", others));
        long ram = rows.Sum(r => r.Ram);
        if (ram > 0)
            lines.Add("-# " + DF("Roblox is using {0} of RAM", ram >= 1024 * MB ? (ram / (1024.0 * MB)).ToString("0.0", Inv) + " GB" : $"{ram / MB} MB"));
        // No logo beside the accounts: their three columns get the whole width.
        return new DiscordMessage(title, string.Join("\n", lines), fields, footer,
            cutOff ? DiscordWebhook.Yellow : inGame == rows.Count ? DiscordWebhook.Green : DiscordWebhook.Blue, now);
    }

    /// <summary>The status message's last words when Bloxkeep locks or closes: it would say "3 in game" for good otherwise.</summary>
    DiscordMessage FinalBoardMessage(bool closing) =>
        new(closing ? DT("Bloxkeep was closed") : DT("Bloxkeep is locked"),
            closing ? DT("This board updates again when you open Bloxkeep and unlock it.") : DT("This board updates again when you unlock Bloxkeep."),
            [], DT("Bloxkeep · Board"), DiscordWebhook.Grey, DateTime.UtcNow)
        {
            Thumbnail = DiscordWebhook.Logo,
        };

    /// <summary>"Send test": Bloxkeep can post here; a preset's says its messages come here.</summary>
    DiscordMessage TestMessage(LaunchPreset? preset, bool forPreset) =>
        new(DT("Bloxkeep is connected"),
            forPreset ? DT("Bloxkeep can post to this channel. This preset's messages will come here.")
                : DT("Bloxkeep can post to this channel. Choose what it sends under Settings › Discord."),
            [], DT("Bloxkeep · Test"), DiscordWebhook.Green, DateTime.UtcNow)
        {
            Author = preset?.Name is { Length: > 0 } name ? name : null,
            Thumbnail = DiscordWebhook.Logo,
        };

    void EnqueueDiscord(DiscordItem item)
    {
        if (item.Kind == "board")
            _discordQueue.RemoveAll(i => i.Kind == "board");
        _discordQueue.Add(item);
        // Offline for long: the oldest go, and the next message says how many.
        while (_discordQueue.Count > DiscordQueueMax)
        {
            if (_discordQueue[0].Kind != "board")
                _discordDropped++;
            _discordQueue.RemoveAt(0);
        }
    }

    void ResetDiscordQueue()
    {
        _discordQueue.Clear();
        _discordEvents.Clear();
        _discordSummariesDue.Clear();
        _discordDropped = _discordFailures = 0;
        _discordNextTry = DateTime.MinValue;
    }

    // ───────────────────────────── sending ─────────────────────────────

    /// <summary>Each process check while unlocked: due events, summaries and status updates join the queue; one message goes out at a time.</summary>
    void TickDiscord(DateTime now, List<RobloxProcesses.PlayerProcess> players)
    {
        if (_vault is not { } vault)
            return;
        var d = vault.Data.Discord;
        SyncDiscordVault(vault, now);
        if (!d.Enabled || d.WebhookUrl.Length == 0)
            return;
        if (_discordEvents.Count > 0 && now >= _discordBatchDue)
        {
            // One message per channel: Settings' webhook and each preset's own.
            foreach (var group in _discordEvents.GroupBy(e => e.Route).ToList())
            {
                var ping = group.Aggregate(DiscordPing.None, (all, e) => all.Merge(e.Ping));
                EnqueueDiscord(new DiscordItem("events", EventsMessage(group.Select(e => e.Alert).ToList()), ping, Route: group.Key));
            }
            _discordEvents.Clear();
        }
        // A summary is due from its time on until it went out (noted only then): one missed while locked, asleep, closed
        // or offline goes out once at the next chance, marked late; only the latest one missed is sent.
        var local = DateTime.Now;
        var dailyAt = DueSummary(d.SummaryDaily, d.SummaryDailyTime, null, d.LastDaily, local, i => i.DailyDay);
        var weeklyAt = DueSummary(d.SummaryWeekly, d.SummaryWeeklyTime, d.SummaryWeekday, d.LastWeekly, local, i => i.WeeklyDay);
        // The daily one covering 7 days at the weekly one's time would say the same twice: one message for both.
        if (dailyAt is { } both && weeklyAt == both && d.SummaryDays == 7)
        {
            EnqueueDiscord(new DiscordItem("summary", SummaryMessage(d, 7, both, local, kind: "both"), PingFor(d, "summary"), DayKey(both), DayKey(both)));
        }
        else
        {
            if (dailyAt is { } day)
                EnqueueDiscord(new DiscordItem("summary", SummaryMessage(d, d.SummaryDays, day, local, kind: "daily"), PingFor(d, "summary"), DailyDay: DayKey(day)));
            if (weeklyAt is { } week)
                EnqueueDiscord(new DiscordItem("summary", SummaryMessage(d, 7, week, local, kind: "weekly"), PingFor(d, "summary"), WeeklyDay: DayKey(week)));
        }
        // After a scheduled preset: today so far, for its accounts, to its own webhook when it has one.
        foreach (var (presetId, after) in _discordSummariesDue.Where(x => now >= x.Value).ToList())
        {
            _discordSummariesDue.Remove(presetId);
            var preset = PresetFor(presetId);
            if (preset is not null)
                EnqueueDiscord(new DiscordItem("summary", SummaryMessage(d, 1, today: true, preset: preset, kind: "preset"), PingFor(d, "summary"), Route: RouteFor(presetId)));
        }
        if (d.Board && now >= _discordNextBoard)
        {
            _discordNextBoard = now + TimeSpan.FromMinutes(Math.Max(2, d.BoardMinutes));
            EnqueueDiscord(new DiscordItem("board", BoardMessage(d, players), DiscordPing.None)); // edits never ping anyway
        }
        if (!_discordSending && _discordQueue.Count > 0 && now >= _discordNextTry)
            _ = SendQueuedAsync(vault, _discordQueue[0]);
    }

    /// <summary>Unlocked (again), or another vault: nothing queued before the lock goes out now.</summary>
    void SyncDiscordVault(Vault vault, DateTime now)
    {
        if (ReferenceEquals(_discordVault, vault))
            return;
        _discordVault = vault;
        ResetDiscordQueue();
        _discordNextBoard = now + TimeSpan.FromSeconds(20);
        // What went out while the vault was locking: this vault edits that status message rather than posting another,
        // and counts those summaries as sent.
        var d = vault.Data.Discord;
        if (_discordBoardPosted is { } posted)
        {
            _discordBoardPosted = null;
            if (WebhookKey(d.WebhookUrl) == posted.UrlKey && d.BoardMessageId.Length == 0)
            {
                d.BoardMessageId = posted.Id;
                PersistSoon();
            }
        }
        string key = WebhookKey(d.WebhookUrl);
        foreach (var (_, daily, weekly) in _discordSummariesSent.Where(s => s.UrlKey == key))
            NoteSummarySent(d, daily, weekly);
        _discordSummariesSent.Clear();
    }

    /// <summary>A webhook link's hash: enough to tell it is the same link, without keeping the link itself.</summary>
    static string WebhookKey(string url) =>
        Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(url)));

    /// <summary>The clock was set back: the waits move back with it, and an event noted "later" doesn't mute the next ones.</summary>
    void MoveDiscordBack(TimeSpan back)
    {
        _discordBatchDue = MovedBack(_discordBatchDue, back);
        _discordNextTry = MovedBack(_discordNextTry, back);
        _discordNextBoard = MovedBack(_discordNextBoard, back);
        foreach (var preset in _discordSummariesDue.Keys.ToList())
            _discordSummariesDue[preset] = MovedBack(_discordSummariesDue[preset], back);
        foreach (string key in _discordNotified.Keys.ToList())
            _discordNotified[key] = MovedBack(_discordNotified[key], back);
    }

    async Task SendQueuedAsync(Vault vault, DiscordItem item)
    {
        try
        {
            await SendQueuedCoreAsync(vault, item);
        }
        // Fire and forget from the timer: never takes the app down. Only the type is logged (no message could hold the link).
        catch (Exception ex)
        {
            _discordSending = false;
            Log.Note($"Discord send failed: {ex.GetType().Name}");
            // Not tried again on every tick (it may have gone out before the failure): a minute later at the soonest.
            var retry = DateTime.UtcNow + TimeSpan.FromMinutes(1);
            if (_discordNextTry < retry)
                _discordNextTry = retry;
        }
    }

    async Task SendQueuedCoreAsync(Vault vault, DiscordItem item)
    {
        var d = vault.Data.Discord;
        var message = item.Message;
        int dropped = item.Kind == "board" ? 0 : _discordDropped;
        if (dropped > 0)
            message = WithDropped(message, dropped);
        bool board = item.Kind == "board";
        string? edit = board && d.BoardMessageId.Length > 0 ? d.BoardMessageId : null;
        string url = UrlFor(d, item.Route);
        _discordSending = true;
        DiscordResult result;
        try
        {
            var picture = await SummaryPictureAsync(message);
            var payload = DiscordWebhook.Build(message, item.Ping, d.Style != "plain", d.Username, d.AvatarUrl, picture?.Name);
            result = await DiscordWebhook.SendAsync(url, payload, edit, wait: board, file: picture);
        }
        finally
        {
            _discordSending = false;
        }
        if (_disposed)
            return;
        if (_vault != vault || UrlFor(d, item.Route) != url)
        {
            // Locked while the first status message or a summary went out: the next unlock edits that message instead
            // of leaving it behind, and doesn't send the summary again.
            if (_vault is null && result.Outcome == DiscordOutcome.Sent)
            {
                if (board && edit is null && result.MessageId is { } posted)
                    _discordBoardPosted = (WebhookKey(url), posted);
                if (item.DailyDay is not null || item.WeeklyDay is not null)
                    _discordSummariesSent.Add((WebhookKey(url), item.DailyDay, item.WeeklyDay));
            }
            return;
        }
        // Kept even when a newer update replaced this one meanwhile, so that one edits this message instead of posting another.
        if (board && result.Outcome == DiscordOutcome.Sent && result.MessageId is { } id && id != d.BoardMessageId)
        {
            d.BoardMessageId = id;
            PersistSoon();
        }
        // A summary is noted as sent only now that it went out (or Discord will never take it), so one dropped from
        // the queue (a lock, sending switched off) is sent again later.
        if (result.Outcome is DiscordOutcome.Sent or DiscordOutcome.Rejected)
            NoteSummarySent(d, item);
        // Sending switched off meanwhile: this answer is about a queue that is gone.
        if (!d.Enabled)
            return;
        // A newer status update replaced this one meanwhile (or the queue was emptied): Discord's answer still counts
        // for what goes out next (a rate limit, being offline, a webhook that is gone).
        bool current = _discordQueue.Count > 0 && ReferenceEquals(_discordQueue[0], item);
        var now = DateTime.UtcNow;
        switch (result.Outcome)
        {
            case DiscordOutcome.Sent:
                if (current)
                {
                    _discordQueue.RemoveAt(0);
                    _discordDropped -= dropped;
                }
                _discordFailures = 0;
                _discordNextTry = now + result.Wait;
                _discordLastSent = now;
                _discordError = null;
                break;
            case DiscordOutcome.MessageGone:
                // Someone deleted the status message: a new one is posted (and its id kept) right away.
                d.BoardMessageId = "";
                PersistSoon();
                break;
            case DiscordOutcome.RateLimited:
                _discordNextTry = now + result.Wait;
                break;
            case DiscordOutcome.Offline:
                _discordFailures++;
                var wait = TimeSpan.FromSeconds(15 * Math.Pow(2, Math.Min(_discordFailures - 1, 6)));
                _discordNextTry = now + (wait < DiscordMaxBackoff ? wait : DiscordMaxBackoff);
                _discordError = DiscordOfflineText(result);
                break;
            case DiscordOutcome.Gone when item.Route is { } gone && PresetFor(gone) is { } preset:
                // Only that preset's webhook is gone: it goes, and its messages go to Settings' webhook again.
                preset.DiscordWebhook = "";
                DiscordRouteChanged(gone);
                if (current)
                    _discordQueue.Remove(item);
                PersistSoon();
                _discordError = L.F("Discord says the webhook of preset {0} no longer exists, so it was removed. Its messages go to the channel in Settings again.", preset.Name);
                break;
            case DiscordOutcome.Gone:
                StopDiscord(d);
                break;
            default:
                // Discord refused this one message (it never will take it): it goes, the rest follow.
                if (current)
                    _discordQueue.RemoveAt(0);
                _discordError = L.F("Discord refused a message: {0}", result.Error);
                break;
        }
        PushState();
    }

    /// <summary>Messages dropped while offline, said in a small line at the end of the next one.</summary>
    DiscordMessage WithDropped(DiscordMessage message, int dropped) => message with
    {
        Text = ((message.Text.Length > 0 ? message.Text + "\n" : "") + "-# " + DF(dropped == 1
            ? "{0} older message couldn't be sent while offline and was dropped." : "{0} older messages couldn't be sent while offline and were dropped.", dropped)),
    };

    /// <summary>A daily or weekly summary went out: its day is noted, never an older one over a newer.</summary>
    void NoteSummarySent(DiscordSettings d, DiscordItem item) => NoteSummarySent(d, item.DailyDay, item.WeeklyDay);

    void NoteSummarySent(DiscordSettings d, string? dailyDay, string? weeklyDay)
    {
        bool changed = false;
        if (dailyDay is { } day && string.CompareOrdinal(day, d.LastDaily) > 0)
        {
            d.LastDaily = day;
            changed = true;
        }
        if (weeklyDay is { } week && string.CompareOrdinal(week, d.LastWeekly) > 0)
        {
            d.LastWeekly = week;
            changed = true;
        }
        if (changed)
            PersistSoon();
    }

    /// <summary>
    /// Locking or closing: the status message would go on saying "N accounts open" for good. One last edit says why
    /// it stopped; best effort, given a few seconds at most, past the queue (which is dropped with the lock).
    /// </summary>
    /// <returns>The edit on its way, or null when there is no status message to edit.</returns>
    Task? FinalDiscordBoard(bool closing)
    {
        if (_vault?.Data.Discord is not { Enabled: true, Board: true, WebhookUrl.Length: > 0, BoardMessageId.Length: > 0 } d)
            return null;
        var payload = DiscordWebhook.Build(FinalBoardMessage(closing), DiscordPing.None, d.Style != "plain", d.Username, d.AvatarUrl);
        string url = d.WebhookUrl, id = d.BoardMessageId;
        // On a worker: the exit waits for it, which must not need the UI thread to finish.
        return Task.Run(async () =>
        {
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
            await DiscordWebhook.SendAsync(url, payload, id, wait: false, cts.Token);
        });
    }

    /// <summary>Discord says the webhook no longer exists (deleted, or its link reset): sending stops until a new link.</summary>
    void StopDiscord(DiscordSettings d)
    {
        d.Enabled = false;
        d.Stopped = "gone";
        d.BoardMessageId = "";
        ResetDiscordQueue();
        _discordError = null;
        PersistSoon();
        Toast(L.T("Discord says the webhook no longer exists, so Bloxkeep stopped sending to it. Paste a new link in Settings › Discord."), "warn");
    }

    static string DiscordOfflineText(DiscordResult result) =>
        result.Error is { } error ? L.F("Couldn't reach Discord: {0}", error) : L.T("Couldn't reach Discord. Check your internet connection.");

    /// <summary>
    /// The preset form's "Send test": to the link typed in it, or to the one the preset has. Nothing is saved, and a
    /// webhook that doesn't exist is only said, not removed.
    /// </summary>
    async Task<object?> SendPresetDiscordTestAsync(Guid presetId, string? typed)
    {
        var vault = RequireVault();
        var d = vault.Data.Discord;
        string url;
        if (typed is { Length: > 0 })
            url = DiscordWebhook.TryNormalize(typed, out string normalized) ? normalized
                : throw new UserError(L.T("That isn't a Discord webhook link. It looks like https://discord.com/api/webhooks/… (Channel settings › Integrations › Webhooks › Copy Webhook URL)."));
        else
            url = PresetFor(presetId)?.DiscordWebhook is { Length: > 0 } saved ? saved : throw new UserError(L.T("Paste a webhook link first."));
        var message = TestMessage(PresetFor(presetId), forPreset: true);
        var result = await DiscordWebhook.SendAsync(url, DiscordWebhook.Build(message, DiscordPing.None, d.Style != "plain", d.Username, d.AvatarUrl), null, wait: true);
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));
        return result.Outcome switch
        {
            DiscordOutcome.Sent => null,
            DiscordOutcome.Gone or DiscordOutcome.MessageGone => throw new UserError(L.T("Discord says this webhook doesn't exist: it was deleted, or the link is wrong. Paste a new link.")),
            DiscordOutcome.RateLimited => throw new UserError(L.F("Discord asks to wait {0} s before the next message.", Math.Ceiling(result.Wait.TotalSeconds))),
            DiscordOutcome.Offline => throw new UserError(DiscordOfflineText(result)),
            _ => throw new UserError(L.F("Discord refused the message: {0}", result.Error)),
        };
    }

    /// <summary>A summary's picture (see <see cref="SummaryCard"/>), or none: the message goes without it rather than not at all.</summary>
    static async Task<DiscordFile?> SummaryPictureAsync(DiscordMessage message)
    {
        if (message.Card is not { } card)
            return null;
        try
        {
            return new DiscordFile(SummaryPicture, await SummaryCard.RenderAsync(card));
        }
        catch (Exception ex) when (ex is ArgumentException or ExternalException or InvalidOperationException or OutOfMemoryException)
        {
            Log.Write(ex);
            return null;
        }
    }

    /// <summary>"Send test" and "Send summary now": straight out, past the queue, with the answer shown at once.</summary>
    async Task<object?> SendDiscordNowAsync(bool summary)
    {
        var vault = RequireVault();
        var d = vault.Data.Discord;
        if (d.WebhookUrl.Length == 0)
            throw new UserError(L.T("Paste a webhook link first."));
        var message = summary ? SummaryMessage(d, d.SummaryDays) : TestMessage(null, forPreset: false);
        var picture = await SummaryPictureAsync(message);
        var result = await DiscordWebhook.SendAsync(d.WebhookUrl, DiscordWebhook.Build(message, summary ? PingFor(d, "summary") : DiscordPing.None, d.Style != "plain", d.Username, d.AvatarUrl, picture?.Name),
            null, wait: true, file: picture);
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));
        switch (result.Outcome)
        {
            case DiscordOutcome.Sent:
                _discordLastSent = DateTime.UtcNow;
                _discordError = null;
                d.Stopped = null;
                PushState();
                return null;
            case DiscordOutcome.Gone or DiscordOutcome.MessageGone:
                if (d.Enabled)
                    StopDiscord(d);
                else
                {
                    d.Stopped = "gone";
                    PersistSoon();
                }
                PushState();
                throw new UserError(L.T("Discord says this webhook doesn't exist: it was deleted, or the link is wrong. Paste a new link."));
            case DiscordOutcome.RateLimited:
                throw new UserError(L.F("Discord asks to wait {0} s before the next message.", Math.Ceiling(result.Wait.TotalSeconds)));
            case DiscordOutcome.Offline:
                throw new UserError(DiscordOfflineText(result));
            default:
                throw new UserError(L.F("Discord refused the message: {0}", result.Error));
        }
    }
}
