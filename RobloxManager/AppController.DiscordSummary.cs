using System.Globalization;

namespace RobloxManager;

/// <summary>
/// Discord's play-time summary: one message with an overview (totals against the period before, drops with Roblox's
/// codes, the longest stay, the most windows at once, the top games), a picture drawn here (play time by hour or day,
/// coloured by game, and a row per account with its avatar; see <see cref="SummaryCard"/>), and an embed per preset
/// that played, with its accounts. Play is grouped by the preset whose window it was (<see cref="PlaySession.PresetId"/>).
/// </summary>
public sealed partial class AppController
{
    const int MaxPresetEmbeds = 6, MaxCardAccounts = 12, MaxBlockAccounts = 15;
    const string SummaryPicture = "bloxkeep-summary.png";

    /// <summary>The part of a stay inside the period, and the game it counts for ("u&lt;universe&gt;", else "p&lt;place&gt;").</summary>
    sealed record StayPart(PlaySession Session, DateTime From, DateTime To, string Game)
    {
        public double Seconds => (To - From).TotalSeconds;
    }

    /// <param name="days">1 (a day), 7 or 30.</param>
    /// <param name="dueAt">When a daily or weekly summary was due (local): the period ends there, also when it goes out late.</param>
    /// <param name="local">Now, for telling whether it is late.</param>
    /// <param name="today">One day is today so far (after a scheduled preset), not the 24 hours before.</param>
    /// <param name="preset">After that scheduled preset's stop: only its play, under its name.</param>
    /// <param name="kind">"daily", "weekly", "both", "preset" or "now": which summary the footer says it is.</param>
    DiscordMessage SummaryMessage(DiscordSettings d, int days, DateTime? dueAt = null, DateTime? local = null, bool today = false,
        LaunchPreset? preset = null, string kind = "now")
    {
        var vault = RequireVault();
        days = days is 7 or 30 ? days : 1;
        bool hourly = days == 1, late = dueAt is { } due && local is { } at && at - due > DiscordLate;
        var now = DateTime.UtcNow;
        // A day is the 24 hours up to the summary's time (one at midnight covers the day that just ended); after a
        // scheduled preset, today so far; 7 or 30 days, whole days up to now.
        var to = dueAt is { } end && end.ToUniversalTime() < now ? end.ToUniversalTime() : now;
        var from = today ? DateTime.Today.ToUniversalTime() : hourly ? to.AddDays(-1) : to.ToLocalTime().Date.AddDays(1 - days).ToUniversalTime();

        var only = preset is null && d.SummaryAccounts.Count > 0 ? d.SummaryAccounts.ToHashSet() : null;
        // A preset's own: what its windows played (not its accounts' play opened otherwise).
        bool Counts(PlaySession s) => preset is not null ? s.PresetId == preset.Id : only is null || only.Contains(s.AccountId);
        var parts = Stays(vault, from, to, Counts);
        double total = parts.Sum(p => p.Seconds);
        double before = Stays(vault, from - (to - from), from, Counts).Sum(p => p.Seconds);
        var drops = vault.Data.PlaySessions.Where(s => Counts(s) && s.EndReason == "disconnected" && s.EndedAt >= from && s.EndedAt <= to).ToList();
        var parts2 = d.SummaryParts;

        // Games, most played first; the first few get a colour (the rest share grey).
        var games = parts.GroupBy(p => p.Game)
            .Select(g => (Key: g.Key, Seconds: g.Sum(p => p.Seconds), Stay: g.OrderByDescending(p => p.Session.GameName.Length).First().Session))
            .OrderByDescending(g => g.Seconds).ToList();
        bool showGames = parts2.Contains("games");
        var colour = games.Select((g, i) => (g.Key, i)).ToDictionary(x => x.Key, x => !showGames ? 0 : x.i < SummaryCard.GameColors.Length ? x.i : -1);
        string StayGame(PlaySession s) => s.GameName.Length > 0 ? s.GameName : GameName(s.PlaceId, null);
        string? StayIcon(PlaySession s) => GameIcon(s.PlaceId, s.UniverseId);

        var accounts = parts.GroupBy(p => p.Session.AccountId).Select(g => new
        {
            Id = g.Key,
            Seconds = g.Sum(p => p.Seconds),
            Stays = g.Select(p => p.Session).Distinct().Count(),
            Drops = drops.Count(s => s.AccountId == g.Key),
            Games = g.GroupBy(p => p.Game).Select(x => (Game: colour[x.Key], Seconds: x.Sum(p => p.Seconds))).ToList(),
            Label = g.Select(p => p.Session.AccountLabel).LastOrDefault(l => l is { Length: > 0 }),
        }).OrderByDescending(a => a.Seconds).ToList();
        string NameOf(Guid id, string? label) => FindAccount(id) is { } account ? DiscordName(account)
            : !d.HideNames && label is { Length: > 0 } ? label : DT("Deleted account");

        // The title says the period ("in the last 24 hours" only when it is that: a late one says how long); the footer
        // says which summary it is; a summary after a scheduled preset has the preset above the title.
        string title = days switch
        {
            7 => DT(late ? "Play time for 7 days" : "Play time in the last 7 days"),
            30 => DT(late ? "Play time for 30 days" : "Play time in the last 30 days"),
            _ => DT(today ? "Play time today" : late ? "Play time for one day" : "Play time in the last 24 hours"),
        };
        string footer = kind switch
        {
            "daily" => DT("Bloxkeep · Daily summary"), "weekly" => DT("Bloxkeep · Weekly summary"),
            "both" => DT("Bloxkeep · Daily and weekly summary"), _ => DT("Bloxkeep · Summary"),
        };

        var lines = new List<string>();
        var fields = new List<DiscordField>();
        var presetGroups = preset is null
            ? parts.GroupBy(p => p.Session.PresetId is { } id && PresetFor(id) is not null ? id : (Guid?)null).ToList()
            : [];
        int presetsPlayed = presetGroups.Count(g => g.Key is not null);
        if (total > 0)
        {
            int peak = 0, open = 0;
            DateTime peakAt = from;
            foreach (var (when, step) in parts.SelectMany(p => new[] { (p.From, 1), (p.To, -1) }).OrderBy(e => e.Item1).ThenBy(e => e.Item2))
            {
                open += step;
                if (open > peak)
                    (peak, peakAt) = (open, when);
            }
            var summaryLine = new List<string> { $"**{accounts.Count}** " + DT(accounts.Count == 1 ? "account played" : "accounts played") };
            if (presetsPlayed > 0)
                summaryLine.Add($"**{presetsPlayed}** " + DT(presetsPlayed == 1 ? "preset" : "presets"));
            lines.Add(string.Join(" · ", summaryLine));

            // Six short facts, three to a row: the totals first, then the records.
            if (parts2.Contains("total"))
                fields.Add(new(DT("Play time"), $"**{Duration((long)total)}**\n{Compared(total, before)}", true));
            int stays = parts.Select(p => p.Session).Distinct().Count();
            if (parts2.Contains("sessions"))
                fields.Add(new(DT("Sessions"), $"**{stays}**\n" + DF("about {0} each", Duration((long)(total / Math.Max(1, stays)))), true));
            if (parts2.Contains("kicks"))
            {
                string codes = string.Join(" · ", drops.Where(s => s.ReasonCode is not null).GroupBy(s => s.ReasonCode!.Value)
                    .OrderByDescending(g => g.Count()).Take(3).Select(g => $"`{g.Key}` ×{g.Count()}"));
                fields.Add(new(DT("Disconnects"), $"**{drops.Count}**" + (codes.Length > 0 ? $"\n{DT("Roblox codes")}: {codes}" : ""), true));
            }
            int of = (preset?.AccountIds ?? (IEnumerable<Guid>?)only)?.Count(id => FindAccount(id) is not null) ?? vault.Data.Accounts.Count;
            fields.Add(new(DT("Accounts"), $"**{accounts.Count}**/{of} " + DT("played"), true));
            var longest = parts.OrderByDescending(p => p.Seconds).First();
            fields.Add(new(DT("Longest stay"),
                $"**{Duration((long)longest.Seconds)}**\n{DiscordWebhook.Escape(NameOf(longest.Session.AccountId, longest.Session.AccountLabel))} · {DiscordWebhook.Escape(StayGame(longest.Session))}", true));
            fields.Add(new(DT("Most at once"), $"**{peak}** " + DT(peak == 1 ? "window" : "windows") + $"\n<t:{Unix(peakAt)}:t>", true));
            if (showGames)
            {
                var top = games.Take(5).Select((g, i) =>
                    $"**{i + 1}.** {DiscordWebhook.GameLink(StayGame(g.Stay), g.Stay.PlaceId)} · {Duration((long)g.Seconds)} · {g.Seconds / total:0%}");
                fields.Add(new(DT("Top games"), string.Join("\n", top)));
            }
            // Without a preset embed below, the accounts are listed here (the picture shows them too).
            if (parts2.Contains("accounts") && (preset is not null || presetsPlayed == 0))
                fields.Add(new(DF(accounts.Count == 1 ? "{0} account" : "{0} accounts", accounts.Count),
                    AccountBlock(accounts.Select(a => (NameOf(a.Id, a.Label), a.Seconds)).ToList())));
        }
        else
            lines.Add(DT("Nothing played in this period."));
        // The period in each reader's own time, the accounts it covers, and why it comes late, in small text.
        lines.Add($"-# <t:{Unix(from)}:f> → <t:{Unix(to)}:f>" + (only is not null ? " · " + DF(only.Count == 1 ? "{0} chosen account" : "{0} chosen accounts", only.Count) : ""));
        if (late)
            lines.Add("-# " + DF("Sent late: it was due {0}.", $"<t:{Unix(dueAt!.Value.ToUniversalTime())}:f>"));
        var (shown, text) = Threes(fields, string.Join("\n", lines));

        var card = total <= 0 ? null : new SummaryCardData
        {
            ChartTitle = DT(hourly || today ? "Activity by hour" : "Activity by day"),
            TotalText = Duration((long)total),
            Buckets = Buckets(parts, from, to, hourly || today, colour),
            AccountsTitle = parts2.Contains("accounts") ? DT("By account") : "",
            Accounts = !parts2.Contains("accounts") ? new() : accounts.Take(MaxCardAccounts).Select(a =>
            {
                string name = NameOf(a.Id, a.Label);
                string detail = DF(a.Stays == 1 ? "{0} stay" : "{0} stays", a.Stays) + (a.Drops > 0 ? " · " + DF(a.Drops == 1 ? "{0} drop" : "{0} drops", a.Drops) : "");
                return new SummaryCardAccount
                {
                    Name = name, Detail = detail, Time = Duration((long)a.Seconds), Seconds = a.Seconds, Parts = a.Games,
                    // A hidden name has no picture either: the avatar would tell who it is.
                    AvatarUrl = !d.HideNames && FindAccount(a.Id) is { } account ? _avatarUrls.GetValueOrDefault(account.UserId) : null,
                    Initials = Initials(name),
                    Hue = Hue(FindAccount(a.Id)?.Username is { Length: > 0 } username ? username : name),
                };
            }).ToList(),
            MoreAccounts = parts2.Contains("accounts") && accounts.Count > MaxCardAccounts ? DF("+{0} more accounts", accounts.Count - MaxCardAccounts) : null,
            Games = showGames ? games.Select(g => StayGame(g.Stay)).ToList() : new(),
            OtherGames = DT("Other games"),
        };

        // An embed per preset that played (and one for play outside presets, when some did): its own figures and accounts.
        var more = new List<DiscordMessage>();
        foreach (var group in presetGroups.Where(g => g.Key is not null).OrderByDescending(g => g.Sum(p => p.Seconds)).Take(MaxPresetEmbeds)
                     .Concat(presetsPlayed > 0 ? presetGroups.Where(g => g.Key is null) : []))
        {
            var own = PresetFor(group.Key);
            var groupAccounts = group.GroupBy(p => p.Session.AccountId)
                .Select(g => (Name: NameOf(g.Key, g.Select(p => p.Session.AccountLabel).LastOrDefault(l => l is { Length: > 0 })), Seconds: g.Sum(p => p.Seconds)))
                .OrderByDescending(a => a.Seconds).ToList();
            double seconds = group.Sum(p => p.Seconds);
            int groupStays = group.Select(p => p.Session).Distinct().Count(), groupDrops = drops.Count(s => s.PresetId == group.Key);
            var topGame = group.GroupBy(p => p.Game).OrderByDescending(g => g.Sum(p => p.Seconds)).First().First().Session;
            var embedFields = new List<DiscordField>();
            if (parts2.Contains("total"))
                embedFields.Add(new(DT("Play time"), $"**{Duration((long)seconds)}**", true));
            if (parts2.Contains("sessions"))
                embedFields.Add(new(DT("Sessions"), $"**{groupStays}**", true));
            if (parts2.Contains("kicks"))
                embedFields.Add(new(DT("Disconnects"), $"**{groupDrops}**", true));
            if (parts2.Contains("accounts"))
                embedFields.Add(new(DF(groupAccounts.Count == 1 ? "{0} account" : "{0} accounts", groupAccounts.Count), AccountBlock(groupAccounts)));
            long? presetPlace = own is not null ? PresetPlaceId(own) : null;
            string about = own is null ? DT("Opened by hand, or before Bloxkeep 1.0.5 noted presets.")
                : string.Join(" · ", new[]
                {
                    own.PlaceInput.Length > 0
                        ? DiscordWebhook.GameLink(own.GameName.Length > 0 ? own.GameName : StayGame(topGame), presetPlace ?? topGame.PlaceId)
                        : DT("Roblox home"),
                    own.Schedule is { Enabled: true } schedule ? DF("Scheduled at {0}", schedule.Time) : DT("Run by hand"),
                });
            var (presetFields, presetText) = Threes(embedFields, about);
            more.Add(new DiscordMessage(own is null ? DT("Not from a preset") : DiscordWebhook.Escape(own.Name), presetText ?? "", presetFields,
                Color: own is null ? DiscordWebhook.Grey : DiscordWebhook.Blue)
            {
                Thumbnail = (presetPlace is long place ? GameIcon(place, 0) : null) ?? StayIcon(topGame),
            });
        }

        return new DiscordMessage(title, text ?? "", shown, footer, DiscordWebhook.Blue, to)
        {
            Author = preset?.Name,
            Thumbnail = (games.Count > 0 ? StayIcon(games[0].Stay) : null) ?? DiscordWebhook.Logo,
            Card = card,
            More = more,
        };
    }

    /// <summary>Each stay's part inside [from, to), with the game it counts for (places of one game together).</summary>
    List<StayPart> Stays(Vault vault, DateTime from, DateTime to, Func<PlaySession, bool> counts)
    {
        var universeOf = new Dictionary<long, long>();
        foreach (var s in vault.Data.PlaySessions)
        {
            if (s.UniverseId > 0)
                universeOf[s.PlaceId] = s.UniverseId;
        }
        var list = new List<StayPart>();
        foreach (var s in vault.Data.PlaySessions)
        {
            if (!counts(s) || s.StartedAt >= to)
                continue;
            var end = s.EndedAt is { } ended && ended < to ? ended : to;
            var start = s.StartedAt > from ? s.StartedAt : from;
            if (end <= start)
                continue;
            long universe = s.UniverseId > 0 ? s.UniverseId : universeOf.GetValueOrDefault(s.PlaceId) is > 0 and var u ? u : _games.GetValueOrDefault(s.PlaceId)?.UniverseId ?? 0;
            list.Add(new StayPart(s, start, end, universe > 0 ? $"u{universe}" : $"p{s.PlaceId}"));
        }
        return list;
    }

    /// <summary>The picture's columns: whole clock hours over the period (or days), each split by game.</summary>
    List<SummaryCardBucket> Buckets(List<StayPart> parts, DateTime from, DateTime to, bool hourly, Dictionary<string, int> colour)
    {
        // The app runs in invariant-globalization mode (RobloxManager.csproj): asking for "en-US" or "vi-VN" throws there.
        // The invariant culture's month names and AM/PM are English, which is what the English labels want.
        var culture = CultureInfo.InvariantCulture;
        var localFrom = from.ToLocalTime();
        var start = hourly ? new DateTime(localFrom.Year, localFrom.Month, localFrom.Day, localFrom.Hour, 0, 0, DateTimeKind.Local) : localFrom.Date;
        var buckets = new List<SummaryCardBucket>();
        for (var at = start; at.ToUniversalTime() < to && buckets.Count < 32; at = hourly ? at.AddHours(1) : at.AddDays(1))
        {
            var a = at.ToUniversalTime();
            var b = (hourly ? at.AddHours(1) : at.AddDays(1)).ToUniversalTime();
            var split = parts.Select(p => (p.Game, Seconds: Math.Max(0, ((p.To < b ? p.To : b) - (p.From > a ? p.From : a)).TotalSeconds)))
                .Where(x => x.Seconds > 0).GroupBy(x => colour.GetValueOrDefault(x.Game, -1))
                .Select(g => (Game: g.Key, Seconds: g.Sum(x => x.Seconds))).ToList();
            string label = hourly ? (DiscordLanguage == "vi" ? $"{at.Hour}h" : at.ToString("h tt", culture).ToLowerInvariant().Replace(" ", ""))
                : at.ToString(DiscordLanguage == "vi" ? "dd/MM" : "MMM d", culture);
            buckets.Add(new SummaryCardBucket(label, split));
        }
        return buckets;
    }

    /// <summary>"▲ 35 min more than before" (the period as long before this one), and the like.</summary>
    string Compared(double now, double before)
    {
        if (before <= 0)
            return DT("Nothing the period before");
        long diff = (long)Math.Abs(now - before);
        if (diff < 60)
            return DT("Same as before");
        return now > before ? "▲ " + DF("{0} more than before", Duration(diff)) : "▼ " + DF("{0} less than before", Duration(diff));
    }

    /// <summary>Accounts as a table in a code block: name, h:mm, and a bar against the most played one.</summary>
    string AccountBlock(List<(string Name, double Seconds)> accounts)
    {
        double most = Math.Max(1, accounts.Count > 0 ? accounts.Max(a => a.Seconds) : 1);
        var rows = accounts.Take(MaxBlockAccounts).Select(a =>
        {
            string name = a.Name.Replace('`', 'ˋ');
            name = name.Length > 16 ? name[..15] + "…" : name.PadRight(16);
            long minutes = (long)(a.Seconds / 60);
            int blocks = (int)Math.Round(10 * a.Seconds / most);
            return $"{name} {minutes / 60,2}:{minutes % 60:00}  {new string('█', blocks)}{new string('░', 10 - blocks)}";
        }).ToList();
        if (accounts.Count > MaxBlockAccounts)
            rows.Add(DF("and {0} more", accounts.Count - MaxBlockAccounts));
        return DiscordWebhook.Cut("```\n" + string.Join("\n", rows) + "\n```", DiscordWebhook.MaxFieldValue);
    }

    static long Unix(DateTime utc) => new DateTimeOffset(DateTime.SpecifyKind(utc, DateTimeKind.Utc)).ToUnixTimeSeconds();

    static string Initials(string name)
    {
        var letters = name.Where(char.IsLetterOrDigit).Take(2).ToArray();
        return letters.Length == 0 ? "?" : new string(letters).ToUpperInvariant();
    }

    /// <summary>The page's colour for an account without a picture (the same hash as app.js's hue()).</summary>
    static int Hue(string text)
    {
        uint h = 7;
        foreach (char c in text)
            h = unchecked(h * 31 + c);
        return (int)(h % 360);
    }
}
