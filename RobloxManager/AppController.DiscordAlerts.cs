using System.Globalization;

namespace RobloxManager;

/// <summary>
/// Discord's alerts (see docs/discord-style.md): each event the Windows notifications get becomes a short news title
/// coloured by what it means, the account it is about with its Roblox avatar above it, the game's icon beside it, and
/// the facts (game, time played, Roblox's reason) side by side. Made when the event happens, from the sentence and
/// values the notification was made from: in Discord's language, with names shown or hidden by Discord's switch.
/// Events within a few seconds go out as one message, a field per event.
/// </summary>
public sealed partial class AppController
{
    /// <summary>A stay that ended this recently is the one an event (a drop, a kick) is about.</summary>
    static readonly TimeSpan RecentStay = TimeSpan.FromMinutes(3);

    /// <summary>An event as Discord shows it; <see cref="Author"/> is the account's name (not escaped).</summary>
    sealed record DiscordAlert(string Title, string? Text, IReadOnlyList<DiscordField> Fields, int Color, DateTime At)
    {
        public string? Author { get; init; }
        public string? AuthorIcon { get; init; }
        public string? Thumbnail { get; init; }
    }

    /// <summary>The account an event is about, as Discord shows it, with the game and the stay it was in (when known).</summary>
    sealed record AlertSubject(string Name, string? Icon, long? PlaceId, string? Game, string? GameIcon, TimeSpan? Played, string? Reason);

    /// <summary>
    /// The alert for an event of <paramref name="type"/> about <paramref name="subject"/> (an account, or a preset for a
    /// schedule's), from the sentence and values its notification was made from (<paramref name="made"/>; null when
    /// unknown: then its <paramref name="text"/> as it is).
    /// </summary>
    DiscordAlert AlertFor(string type, Guid subject, (string English, object?[] Args)? made, string text, DateTime now)
    {
        string en = made?.English ?? "";
        object?[] args = made?.Args ?? [];
        string Arg(int i) => i < args.Length ? Written("{" + i + "}", args) : "";
        long Count(int i) => i < args.Length && args[i] is IConvertible n and not string ? n.ToInt64(CultureInfo.InvariantCulture) : 0;
        string? why = made is { } m ? Why(m) : null;
        DiscordAlert Plain(string title, string? description, int color, params DiscordField[] fields) =>
            Threes(new DiscordAlert(title, description, fields, color, now) { Thumbnail = DiscordWebhook.Logo });

        switch (type)
        {
            case "drop" or "rejoinSlow" or "rejoinFail" or "afk" when SubjectOf(subject, now) is { } who:
                var (title, description, color) = type switch
                {
                    "drop" when en.StartsWith("{0} lost connection", StringComparison.Ordinal) => (DT("Lost connection, restarting Roblox"), (string?)null, DiscordWebhook.Yellow),
                    "drop" when en.StartsWith("{0} dropped out of the game, reason unknown", StringComparison.Ordinal) => (DT("Disconnected, reason unknown"), null, DiscordWebhook.Yellow),
                    "drop" when en.StartsWith("{0} was kicked by the game", StringComparison.Ordinal) => (DT("Kicked by the game, rejoining"),
                        why is null ? null : DiscordWebhook.Escape(why), DiscordWebhook.Yellow),
                    "drop" when en.StartsWith("{0}'s server shut down", StringComparison.Ordinal) => (DT("Server shut down, joining another"), null, DiscordWebhook.Yellow),
                    "drop" when en.StartsWith("{0} isn't rejoined", StringComparison.Ordinal) => (DT("Disconnected, not rejoining"),
                        why is null ? null : DiscordWebhook.Escape(why), DiscordWebhook.Red),
                    "drop" when en.StartsWith("{0} couldn't get into that server", StringComparison.Ordinal) => (DT("Couldn't get into the server, joining another"), null, DiscordWebhook.Yellow),
                    "drop" when en.StartsWith("{0} couldn't get into the game", StringComparison.Ordinal) => (DT("Couldn't get into the game, trying again"), null, DiscordWebhook.Yellow),
                    "drop" when en.StartsWith("{0} can't get into the game", StringComparison.Ordinal) => (DT("Roblox won't let it in, not trying again"),
                        why is null ? null : DiscordWebhook.Escape(why), DiscordWebhook.Red),
                    "drop" => (DT("Disconnected, rejoining"), null, DiscordWebhook.Yellow),
                    "rejoinSlow" => (DT("Keeps getting disconnected"), DT("Bloxkeep keeps rejoining every 10 seconds."), DiscordWebhook.Red),
                    "rejoinFail" => (DT("Couldn't rejoin"), why is null ? null : DiscordWebhook.Escape(why), DiscordWebhook.Red),
                    _ => (DT("Kicked for being AFK"), why is null ? null : DiscordWebhook.Escape(why), DiscordWebhook.Yellow),
                };
                // A kick says it in its title, with the game's words: no "Reason" field saying it again.
                return AccountAlert(en.StartsWith("{0} was kicked by the game", StringComparison.Ordinal) ? who with { Reason = null } : who, title, description, color, now);
            case "gameBan" when SubjectOf(subject, now) is { } who:
                // The game's words are the description: no "Reason" field saying "Kicked by the game" with them again.
                return AccountAlert(who with { Reason = null }, DT("Banned by the game"), why is null ? DT("Bloxkeep won't rejoin this game for this account.")
                    : DiscordWebhook.Escape(why) + "\n-# " + DT("Bloxkeep won't rejoin this game for this account."), DiscordWebhook.Red, now);
            case "banned" when SubjectOf(subject, now) is { } who:
                return AccountAlert(who, DT("Banned on Roblox"), DT("Bloxkeep won't open this account. Once the ban is over, Check sign-in clears it."),
                    DiscordWebhook.Red, now, facts: false);
            case "banned":
                return Plain(DF("{0} accounts banned on Roblox", Count(0)), DT("Bloxkeep won't open these accounts. Once a ban is over, Check sign-in clears it."),
                    DiscordWebhook.Red, new DiscordField(DT("Accounts"), DiscordWebhook.Escape(Arg(1))));
            case "signin":
                long signedOut = Count(0);
                return Plain(DF(signedOut == 1 ? "{0} account needs signing in again" : "{0} accounts need signing in again", signedOut),
                    DT(signedOut == 1 ? "Roblox signed this account out. Sign in again in Bloxkeep to keep it playing." : "Roblox signed these accounts out. Sign them in again in Bloxkeep to keep them playing."),
                    DiscordWebhook.Yellow, new DiscordField(DT(signedOut == 1 ? "Account" : "Accounts"), DiscordWebhook.Escape(Arg(1))));
            case "ram":
                return Plain(DT("RAM is running low"), DT("Close some Roblox windows if things get slow."), DiscordWebhook.Yellow,
                    [new DiscordField(DT("RAM in use"), $"**{Count(0)}%**", true), .. _running.Count > 0 ? [new DiscordField(DT("Roblox windows"), $"{_running.Count}", true)] : Array.Empty<DiscordField>()]);
            case "launch" when en.StartsWith("Roblox closed", StringComparison.Ordinal):
                return Plain(DF(Count(0) == 1 ? "Roblox closed {0} window" : "Roblox closed {0} windows", Count(0)),
                    DT("Roblox may have blocked running several windows at once. Multi-Roblox may need an update."), DiscordWebhook.Red);
            case "launch" when en.EndsWith("didn't open.", StringComparison.Ordinal):
                return Plain(DF(Count(0) == 1 ? "{0} account didn't open" : "{0} accounts didn't open", Count(0)), DT("Open Bloxkeep to see why."), DiscordWebhook.Red);
            case "schedule" or "scheduleFail" or "scheduleMissed" or "scheduleStop":
                return PresetAlert(type, en, subject, Arg, Count, why, now);
        }
        // An event without words of its own here: the notification's sentence.
        return Plain(EventTitle(type == "banned" ? type : NotifyKind(type) ?? type), DiscordWebhook.Escape(text), DiscordWebhook.Yellow);
    }

    /// <summary>A scheduled preset's alert: started, already open, didn't run (and why), missed, finished.</summary>
    DiscordAlert PresetAlert(string type, string en, Guid presetId, Func<int, string> arg, Func<int, long> count, string? why, DateTime now)
    {
        var preset = PresetFor(presetId);
        long? place = preset is not null ? PresetPlaceId(preset) : null;
        string name = DiscordWebhook.Escape(arg(0));
        string? reason = why is null ? null : DiscordWebhook.Escape(why);
        string Accounts(long n) => DF(n == 1 ? "{0} account" : "{0} accounts", n);
        var fields = new List<DiscordField>();
        var (title, text, color) = type switch
        {
            "schedule" when en.Contains("already open", StringComparison.Ordinal) => (DF("Preset {0} started", name), DT("Its accounts were already open."), DiscordWebhook.Grey),
            "schedule" => (DF("Preset {0} started", name), (string?)null, DiscordWebhook.Green),
            "scheduleFail" when en.Contains("didn't open", StringComparison.Ordinal) =>
                (DF(count(1) == 1 ? "Preset {0}: {1} account didn't open" : "Preset {0}: {1} accounts didn't open", name, count(1)), DiscordWebhook.Escape(arg(2)), DiscordWebhook.Red),
            "scheduleFail" => (DF("Preset {0} didn't run", name), reason, DiscordWebhook.Red),
            // The preset's own time, as the person set it (not a moment a timestamp could show).
            "scheduleMissed" => (DF("Preset {0} didn't run", name), (reason is null ? "" : reason + "\n") + "-# " + DF("It was due at {0}.", arg(1)), DiscordWebhook.Yellow),
            _ => (DF("Preset {0} finished", name), DF(count(1) == 1 ? "Closing its {0} account." : "Closing its {0} accounts.", count(1)), DiscordWebhook.Blue),
        };
        if (type == "schedule" && text is null)
            fields.Add(new(DT("Opened"), Accounts(count(1)), true));
        if (place is long game && type is "schedule" or "scheduleStop")
            fields.Add(new(DT("Game"), DiscordWebhook.GameLink(GameName(game, preset?.GameName), game), true));
        return Threes(new DiscordAlert(title, text, fields, color, now) { Thumbnail = (place is long p ? GameIcon(p, 0) : null) ?? DiscordWebhook.Logo });
    }

    /// <summary>The alert about one account: its name and avatar above the title, its game's icon beside it, the game, time played and Roblox's reason.</summary>
    DiscordAlert AccountAlert(AlertSubject who, string title, string? text, int color, DateTime at, bool facts = true)
    {
        var fields = new List<DiscordField>();
        if (facts && who.PlaceId is long place && who.Game is { } game)
            fields.Add(new(DT("Game"), DiscordWebhook.GameLink(game, place), true));
        if (facts && who.Played is { } played && played >= TimeSpan.FromMinutes(1))
            fields.Add(new(DT("Played"), Duration((long)played.TotalSeconds), true));
        if (facts && who.Reason is { } reason)
            fields.Add(new(DT("Reason"), reason, true));
        return Threes(new DiscordAlert(title, text, fields, color, at)
        {
            Author = who.Name, AuthorIcon = who.Icon, Thumbnail = (facts ? who.GameIcon : null) ?? DiscordWebhook.Logo,
        });
    }

    /// <summary>
    /// Discord puts three short fields to a row: one that would sit alone on the last row goes to a small line under
    /// the description instead.
    /// </summary>
    static DiscordAlert Threes(DiscordAlert alert)
    {
        var (fields, text) = Threes(alert.Fields, alert.Text);
        return alert with { Fields = fields, Text = text };
    }

    static (List<DiscordField> Fields, string? Text) Threes(IReadOnlyList<DiscordField> fields, string? text)
    {
        var list = fields.ToList();
        int inline = list.Count(f => f.Inline);
        if (inline % 3 != 1 || list.LastOrDefault(f => f.Inline) is not { } lone)
            return (list, text);
        list.Remove(lone);
        return (list, (text is { Length: > 0 } ? text + "\n" : "") + $"-# {lone.Name}: {lone.Value.Replace("\n", " · ")}");
    }

    AlertSubject? SubjectOf(Guid id, DateTime now)
    {
        if (FindAccount(id) is not { } account || _vault is not { } vault)
            return null;
        // A hidden name has no picture either: the avatar would tell who it is.
        string? icon = !vault.Data.Discord.HideNames && _avatarUrls.GetValueOrDefault(account.UserId) is { } url && DiscordWebhook.IsRobloxPicture(url) ? url : null;
        // The stay it was in: still open, or one that ended just now (a drop ends it right before the event).
        var stays = vault.Data.PlaySessions;
        var stay = _playing.GetValueOrDefault(id) ?? stays.LastOrDefault(s => s.AccountId == id && s.EndedAt is { } end && now - end < RecentStay);
        bool running = _running.TryGetValue(id, out var run);
        var log = running ? _logs.StateOf(run.Pid) : null;
        // The game: where its window is, else the stay's, the rejoin's, or the last one it played within the hour.
        long? place = log?.PlaceId ?? (running ? run.Launch.Target?.PlaceId : null) ?? stay?.PlaceId
            ?? _rejoinQueue.FirstOrDefault(r => r.AccountId == id)?.Target.PlaceId
            ?? stays.LastOrDefault(s => s.AccountId == id && now - (s.EndedAt ?? now) < TimeSpan.FromHours(1))?.PlaceId;
        if (place is <= 0)
            place = null;
        int? code = log is { Status: "disconnected" } ? log.Reason : stay is { EndReason: "disconnected" } ? stay.ReasonCode : null;
        var same = stay is not null && stay.PlaceId == place ? stay : null;
        return new AlertSubject(DiscordName(account), icon, place,
            place is long p ? GameName(p, same?.GameName) : null,
            place is long g ? GameIcon(g, same?.UniverseId ?? 0) : null,
            stay is null ? null : (stay.EndedAt ?? now) - stay.StartedAt,
            // The game's own words with its kick, under Roblox's reason.
            code is int c ? KickText(c) + (log?.KickMessage is { Length: > 0 } said ? "\n" + DiscordWebhook.Escape(said) : "") : null);
    }

    /// <summary>A game's name: as Roblox gave it, else as noted with the stay or preset, else "Place 123".</summary>
    string GameName(long placeId, string? noted) =>
        _games.GetValueOrDefault(placeId)?.Name is { Length: > 0 } name ? name : noted is { Length: > 0 } ? noted : DF("Place {0}", placeId);

    /// <summary>A game's icon from Roblox's image servers, when it is known.</summary>
    string? GameIcon(long placeId, long universeId) => IconOf(placeId, universeId) is { } icon && DiscordWebhook.IsRobloxPicture(icon) ? icon : null;

    /// <summary>Roblox's disconnect code in plain words (as KickReason words them), with the code: "Connection lost `277`".</summary>
    string KickText(int code) =>
        KickWords(code) is { } words ? $"{Capital(words)} `{code}`" : Capital(DF("Roblox code {0}", $"`{code}`"));

    /// <summary>Roblox's disconnect code in Discord's words; null for a code without any.</summary>
    string? KickWords(int code) => code switch
        {
            IdleKick => DT("kicked for being idle"),
            267 => DT("kicked by the game"),
            SameAccountElsewhere or 276 => DT("signed in on another device"),
            279 => DT("couldn't join the server"),
            int lost when LostConnection(lost) => DT("connection lost"),
            286 => DT("no access to the private server any more"),
            259 or 280 => DT("Roblox needs an update"),
            264 or 265 => DT("already in another game"),
            ServerShutdown => DT("server shut down"),
            _ => null,
        };

    /// <summary>A sentence's values written out the way Discord's messages want them: names shown or hidden by its switch.</summary>
    string Written(string format, object?[] args)
    {
        bool hide = _vault?.Data.Discord.HideNames == true;
        _nameStyle = (hide, DiscordLanguage);
        try
        {
            string text = string.Format(format, args);
            return hide ? HideNamesIn(text) : text;
        }
        finally
        {
            _nameStyle = null;
        }
    }

    /// <summary>The event's sentence after its first ": " (why it happened), in Discord's language: "Bloxkeep was locked."</summary>
    string? Why((string English, object?[] Args) made)
    {
        string sentence = L.T(made.English, DiscordLanguage);
        int at = sentence.IndexOf(": ", StringComparison.Ordinal);
        if (at < 0)
            return null;
        string why = Written(sentence[(at + 2)..], made.Args).Trim();
        return why.Length == 0 ? null : Capital(why);
    }

    static string Capital(string text) => text.Length == 0 ? text : char.ToUpperInvariant(text[0]) + text[1..];

    /// <summary>How much an alert asks of the reader: a message of several takes the colour of the one that asks most.</summary>
    static int Urgency(int color) => color switch
    {
        DiscordWebhook.Red => 4,
        DiscordWebhook.Yellow => 3,
        DiscordWebhook.Green => 2,
        DiscordWebhook.Blue => 1,
        _ => 0,
    };

    /// <summary>One message for the events of a few seconds: one alert as it is, several as a field each.</summary>
    DiscordMessage EventsMessage(IReadOnlyList<DiscordAlert> alerts)
    {
        string footer = DT("Bloxkeep · Alerts");
        if (alerts.Count == 1)
        {
            var a = alerts[0];
            return new DiscordMessage(a.Title, a.Text ?? "", a.Fields, footer, a.Color, a.At)
            {
                Author = a.Author, AuthorIcon = a.AuthorIcon, Thumbnail = a.Thumbnail,
            };
        }
        // All about one account: it goes above the title once, not into every field.
        string? one = alerts.Select(a => a.Author).Distinct().Count() == 1 ? alerts[0].Author : null;
        // A field each: its text (a field has no small line: a side note joins it), then its facts on one line, the
        // game and Roblox's reason as they are, the others named; facts the field before gave about the same account
        // aren't repeated.
        string game = DT("Game"), reason = DT("Reason");
        (string? Author, string Facts)? before = null;
        var fields = alerts.Select(a =>
        {
            string name = (one is null && a.Author is { } author ? DiscordWebhook.Escape(author) + " · " : "") + a.Title;
            var lines = new List<string>();
            if (a.Text is { Length: > 0 } text)
                lines.Add((text.StartsWith("-# ", StringComparison.Ordinal) ? text[3..] : text).Replace("\n-# ", " · "));
            string facts = string.Join(" · ", a.Fields.Select(f => (f.Name == game || f.Name == reason ? "" : f.Name + ": ") + f.Value.Replace('\n', ' ')));
            if (facts.Length > 0 && before != (a.Author, facts))
                lines.Add(facts);
            before = (a.Author, facts);
            if (lines.Count == 0)
                lines.Add($"<t:{Unix(a.At)}:t>");
            return new DiscordField(name, string.Join("\n", lines));
        }).ToList();
        return new DiscordMessage(DF("{0} alerts", alerts.Count), "", fields, footer, alerts.MaxBy(a => Urgency(a.Color))!.Color, alerts.Max(a => a.At))
        {
            Author = one, AuthorIcon = one is null ? null : alerts[0].AuthorIcon, Thumbnail = DiscordWebhook.Logo,
        };
    }
}
