using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace RobloxManager;

/// <summary>
/// Statistics › Activity: what Bloxkeep saw happen, newest first. Accounts opened and auto-rejoined, games joined and
/// left, drops (with Roblox's reason), windows closed (by anyone, or by Roblox when another one opened), and what the
/// notifications report (schedules, sign-ins to renew, launch problems, updates). Noted at a few central points
/// rather than everywhere: <see cref="StartSessionAsync"/>, a stay's start and end (AppController.Stats.cs),
/// <see cref="OnSessionEnded"/> and <see cref="Notify"/>. The last 1000 stay in the vault, since they name accounts;
/// for backups they are bookkeeping, like play time (see ContentHash): they never make one due by themselves.
/// </summary>
public sealed partial class AppController
{
    const int MaxActivity = 1000;
    const int ActivityPage = 100;
    /// <summary>The kinds of event the page filters by (see <see cref="ActivityGroup"/>).</summary>
    static readonly string[] ActivityGroups = ["play", "drops", "schedule", "signin", "updates"];
    /// <summary>
    /// Notification types not noted: a drop has entries of its own (disconnected, closed, rejoined), and low-RAM
    /// warnings would only bury the rest.
    /// </summary>
    static readonly HashSet<string> NotNoted = ["drop", "afk", "ram"];
    /// <summary>Noted while Bloxkeep was locked (a window closed meanwhile): added once a vault is open again.</summary>
    readonly List<ActivityEntry> _activityWhileLocked = new();
    /// <summary>The Bloxkeep update last noted as ready to install, so each is noted once.</summary>
    string? _updateNoted;

    /// <summary>Adds an entry; the account's and game's names are kept as they are now, for after a deletion.</summary>
    /// <param name="text">A sentence of its own (a notification's), kept in both languages.</param>
    void NoteActivity(string kind, Guid account = default, long? placeId = null, int? code = null, DateTime? at = null, string? text = null, string? tone = null)
    {
        var entry = new ActivityEntry
        {
            At = at ?? DateTime.UtcNow,
            Kind = kind,
            AccountId = account == default ? null : account,
            AccountLabel = account == default ? null : FindAccount(account)?.Label,
            PlaceId = placeId > 0 ? placeId : null,
            GameName = placeId > 0 && _games.GetValueOrDefault(placeId.Value)?.Name is { Length: > 0 } name ? name : null,
            Code = code,
            Tone = tone,
        };
        if (text is not null)
            (entry.Text, entry.TextVi, entry.Names) = ActivitySentences(text);
        if (_vault is not { } vault)
        {
            if (_activityWhileLocked.Count < 200)
                _activityWhileLocked.Add(entry);
            return;
        }
        AddLockedActivity(vault);
        AddActivity(vault.Data.Activity, entry);
        PersistSoon();
    }

    /// <summary>The placeholder for an account in an activity sentence (see <see cref="ActivityEntry.Names"/>).</summary>
    const char NameStart = '\uE000', NameEnd = '\uE001';
    static readonly Regex NamePlaceholder = new("\uE000([0-9]{1,3})\uE001", RegexOptions.CultureInvariant);

    /// <summary>
    /// A notification's sentence in English and in Vietnamese. One that L.F made with account names (<see cref="Name"/>,
    /// <see cref="Names"/>) gets a placeholder for each instead, so the log keeps no name: one hidden at the time would
    /// stay "Account 1" for good (another account once the list changes), and one shown then would show while hidden.
    /// </summary>
    (string English, string Vietnamese, List<ActivityName>? Names) ActivitySentences(string text)
    {
        if (L.Made(text) is not { } made || !made.Args.Any(arg => arg is Shown))
        {
            var (english, vietnamese) = L.Both(text);
            return (english, vietnamese, null);
        }
        var names = new List<ActivityName>();
        string Placeholder(Account account)
        {
            int i = names.FindIndex(n => n.Id == account.Id);
            if (i < 0)
            {
                i = names.Count;
                names.Add(new ActivityName { Id = account.Id, Label = account.Label });
            }
            return $"{NameStart}{i}{NameEnd}";
        }
        string In(string language)
        {
            var style = _nameStyle;
            _nameStyle = (false, language); // the words that come with the names (" and 2 more") in this language
            try
            {
                var args = made.Args.Select(arg => arg is Shown shown ? shown.Write(Placeholder) : arg).ToArray();
                return string.Format(L.T(made.English, language), args);
            }
            finally
            {
                _nameStyle = style;
            }
        }
        return (In("en"), In("vi"), names);
    }

    static void AddActivity(List<ActivityEntry> list, ActivityEntry entry)
    {
        list.Add(entry);
        if (list.Count > MaxActivity)
            list.RemoveRange(0, list.Count - MaxActivity);
    }

    /// <summary>
    /// What was noted while locked joins the vault now open, for its own accounts only: another vault may have been
    /// opened (a restored backup), whose list mustn't get this one's accounts.
    /// </summary>
    void AddLockedActivity(Vault vault)
    {
        if (_activityWhileLocked.Count == 0)
            return;
        foreach (var entry in _activityWhileLocked)
        {
            if (entry.AccountId is { } id)
            {
                if (FindAccount(id) is not { } account)
                    continue;
                entry.AccountLabel ??= account.Label;
            }
            AddActivity(vault.Data.Activity, entry);
        }
        _activityWhileLocked.Clear();
        PersistSoon();
    }

    /// <summary>A notification's event (see <see cref="Notify"/>); a scheduled preset's names a preset, not an account.</summary>
    void NoteNotified(string text, string kind, string type, Guid account)
    {
        if (!NotNoted.Contains(type))
            NoteActivity(type, FindAccount(account) is not null ? account : default, text: text, tone: kind);
    }

    /// <summary>A client ended for good (not handed over to another process): closed by anyone, by Roblox, or for its preset's restart.</summary>
    void NoteSessionEnd(Guid accountId, PendingLaunch launch, RobloxLogWatcher.ClientState? lastLog) =>
        NoteActivity(_closedByRoblox.Contains(accountId) ? "closedByRoblox" : _restarts.ContainsKey(accountId) ? "restarted" : "closed",
            accountId, lastLog?.PlaceId ?? launch.Target?.PlaceId);

    /// <summary>A Bloxkeep update finished downloading: noted once per version.</summary>
    void NoteUpdateReady()
    {
        if (_updater.Status != "ready" || _updater.NewVersion is not { } version || version == _updateNoted || WhatsNew.IsQuietUpdate(version, _updater.CurrentVersion))
            return;
        _updateNoted = version;
        NoteActivity("updateReady", text: L.F("Bloxkeep {0} is ready to install: it installs when Bloxkeep restarts.", version), tone: "info");
    }

    /// <summary>Kinds of event as the page's filter has them; a notification type added later counts with the problems.</summary>
    static string ActivityGroup(string kind) => kind switch
    {
        "opened" or "rejoined" or "joined" or "left" or "closed" or "restarted" or "reopened" => "play",
        "signin" => "signin",
        "robloxUpdated" or "updateReady" => "updates",
        _ when kind.StartsWith("schedule", StringComparison.Ordinal) => "schedule",
        _ => "drops",
    };

    static string ActivityTone(ActivityEntry e) => e.Tone ?? e.Kind switch
    {
        "opened" or "rejoined" or "joined" or "reopened" => "ok",
        "disconnected" or "signedOut" or "closedByRoblox" => "warn",
        _ => "info",
    };

    /// <summary>The entries the page's filters leave, newest first (ones noted at the same moment: the later first).</summary>
    List<ActivityEntry> ShownActivity(Vault vault, JsonElement a)
    {
        AddLockedActivity(vault);
        Guid? account = Guid.TryParse(OptStr(a, "account"), out var id) ? id : null;
        string? group = OptStr(a, "group") is { } g && ActivityGroups.Contains(g) ? g : null;
        return vault.Data.Activity
            .Select((e, i) => (e, i))
            .Where(x => (account is null || x.e.AccountId == account) && (group is null || ActivityGroup(x.e.Kind) == group))
            .OrderByDescending(x => x.e.At).ThenByDescending(x => x.i)
            .Select(x => x.e)
            .ToList();
    }

    /// <summary>{account?, group?, limit?}: a page of the list, how many there are, and the accounts it names (for the filter).</summary>
    object ActivityList(JsonElement a)
    {
        var vault = RequireVault();
        var shown = ShownActivity(vault, a);
        int limit = (int)Math.Clamp(Long(a, "limit"), ActivityPage, MaxActivity);
        var page = shown.Take(limit).ToList();
        if (NameActivityGames(vault, page))
            PersistSoon();
        var names = ActivityNames(vault);
        return new
        {
            total = shown.Count,
            entries = page.Select(e => new
            {
                at = e.At,
                kind = e.Kind,
                group = ActivityGroup(e.Kind),
                tone = ActivityTone(e),
                accountId = e.AccountId,
                account = ActivityAccount(e),
                text = ActivitySentence(e, names),
            }),
            accounts = vault.Data.Activity
                .Where(e => e.AccountId is not null)
                .GroupBy(e => e.AccountId!.Value)
                .Select(g => (Id: g.Key, Label: ActivityAccount(g.LastOrDefault(e => e.AccountLabel is not null) ?? g.Last())!,
                    Place: FindAccount(g.Key) is { } account ? vault.Data.Accounts.IndexOf(account) : int.MaxValue))
                .OrderBy(x => Prefs.HideNames ? x.Place : 0).ThenBy(x => x.Label, StringComparer.CurrentCultureIgnoreCase)
                .Select(x => new { id = x.Id, label = x.Label })
                .ToList(),
        };
    }

    /// <summary>
    /// Who an entry is about: the account's name now (its name then, once deleted), or "Account 3" while Settings ›
    /// Appearance hides names (its place in the list, as everywhere else in Bloxkeep).
    /// </summary>
    string? ActivityAccount(ActivityEntry e)
    {
        if (e.AccountId is not { } id)
            return null;
        var account = FindAccount(id);
        if (!Prefs.HideNames)
            return account?.Label ?? e.AccountLabel ?? L.T("Deleted account");
        return account is null ? L.T("Deleted account") : HiddenName(account);
    }

    string HiddenName(Account account) => string.Format(L.T("Account {0}"), (_vault?.Data.Accounts.IndexOf(account) ?? 0) + 1);

    /// <summary>
    /// While names are hidden, swaps them inside sentences too (a notification's names its accounts): whole words,
    /// names of three characters or more, longest first.
    /// </summary>
    Func<string, string> ActivityNames(Vault vault)
    {
        if (!Prefs.HideNames)
            return text => text;
        var byName = new Dictionary<string, Account>(StringComparer.Ordinal);
        foreach (var account in vault.Data.Accounts)
        {
            foreach (string name in new[] { account.Label, account.Username })
            {
                if (name.Length >= 3)
                    byName.TryAdd(name, account);
            }
        }
        if (byName.Count == 0)
            return text => text;
        var pattern = new Regex($@"(?<![\p{{L}}\p{{N}}_])(?:{string.Join("|", byName.Keys.OrderByDescending(n => n.Length).Select(Regex.Escape))})(?![\p{{L}}\p{{N}}_])",
            RegexOptions.CultureInvariant);
        return text => pattern.Replace(text, m => HiddenName(byName[m.Value]));
    }

    /// <summary>
    /// Entries of a place that wasn't looked up when they were noted get its name from what is known now: the lookup
    /// cache, a stay at that place, or the recent games.
    /// </summary>
    /// <returns>true when a name was filled in (worth saving).</returns>
    bool NameActivityGames(Vault vault, List<ActivityEntry> entries)
    {
        bool named = false;
        Dictionary<long, string>? known = null;
        foreach (var e in entries)
        {
            if (e.GameName is not null || e.PlaceId is not { } placeId)
                continue;
            known ??= vault.Data.PlaySessions.Where(s => s.GameName.Length > 0).Select(s => (s.PlaceId, s.GameName))
                .Concat(vault.Data.Settings.RecentGames.Concat(vault.Data.Settings.PinnedGames).Where(g => g.Name.Length > 0).Select(g => (g.PlaceId, GameName: g.Name)))
                .GroupBy(p => p.PlaceId).ToDictionary(p => p.Key, p => p.Last().GameName);
            if ((_games.GetValueOrDefault(placeId)?.Name is { Length: > 0 } cached ? cached : known.GetValueOrDefault(placeId)) is { } name)
            {
                e.GameName = name;
                named = true;
            }
        }
        return named;
    }

    /// <summary>
    /// The entry's sentence as the page and the export show it: the accounts it names as they are called now, or
    /// "Account 3" while names are hidden (<paramref name="names"/> swaps names an older entry holds itself).
    /// </summary>
    string ActivitySentence(ActivityEntry e, Func<string, string> names)
    {
        string text = names(ActivityText(e));
        if (e.Names is { Count: > 0 } named)
            return NamePlaceholder.Replace(text, m => int.TryParse(m.Groups[1].Value, out int i) && i < named.Count ? ActivityNameNow(named[i]) : "");
        // An entry from before the placeholders, about an account renamed or deleted since: its name then is swapped too.
        if (Prefs.HideNames && e.AccountLabel is { Length: >= 3 } label && ActivityAccount(e) is { } now && now != label)
            text = Regex.Replace(text, $@"(?<![\p{{L}}\p{{N}}_]){Regex.Escape(label)}(?![\p{{L}}\p{{N}}_])", _ => now, RegexOptions.CultureInvariant);
        return text;
    }

    /// <summary>An account a sentence names, as it is called now; its name then once deleted (unless names are hidden).</summary>
    string ActivityNameNow(ActivityName n) => FindAccount(n.Id) is { } account
        ? Prefs.HideNames ? HiddenName(account) : account.Label
        : Prefs.HideNames || n.Label.Length == 0 ? L.T("Deleted account") : n.Label;

    /// <summary>The entry as a sentence in the app's language (the account is shown beside it).</summary>
    string ActivityText(ActivityEntry e)
    {
        if (e.Text is not null)
            return L.Language == "vi" && e.TextVi is { Length: > 0 } vi ? vi : e.Text;
        string game = e.GameName ?? (e.PlaceId is { } id ? _games.GetValueOrDefault(id)?.Name is { Length: > 0 } name ? name : L.F("Place {0}", id) : "");
        string text = ActivityKindText(e, game);
        // A game named "Adopt Me!" closes the sentence itself: no "Adopt Me!." after it.
        return game.Length > 0 && game[^1] is '!' or '?' or '.' or '…' && text.EndsWith(game + ".", StringComparison.Ordinal) ? text[..^1] : text;
    }

    string ActivityKindText(ActivityEntry e, string game)
    {
        return e.Kind switch
        {
            "opened" => game.Length > 0 ? L.F("Opened Roblox to play {0}.", game) : L.T("Opened Roblox on its home screen."),
            "rejoined" => game.Length > 0 ? L.F("Auto-rejoin opened Roblox again to play {0}.", game) : L.T("Auto-rejoin opened Roblox again."),
            "restarted" => L.T("Closed for its preset's planned restart."),
            "reopened" => game.Length > 0 ? L.F("Opened again after its planned restart, to play {0}.", game) : L.T("Opened again after its planned restart."),
            "joined" => L.F("Joined {0}.", game),
            "left" => L.F("Left {0}.", game),
            "disconnected" when e.Code is { } code => L.F("Disconnected from {0}: {1}.", game, KickReason(code)),
            "disconnected" => L.F("Disconnected from {0}.", game),
            "signedOut" => L.T("Signed out in the Roblox window."),
            "closed" => L.T("Roblox window closed."),
            "closedByRoblox" => L.T("Roblox closed this window when another one opened."),
            _ => e.Kind,
        };
    }

    /// <summary>Roblox's disconnect codes in plain words, as the page words them (KICK in app.js).</summary>
    static string KickReason(int code) => code switch
    {
        IdleKick => L.T("kicked for being idle"),
        267 => L.T("kicked by the game"),
        SameAccountElsewhere or 276 => L.T("signed in on another device"),
        279 => L.T("couldn't join the server"),
        int lost when LostConnection(lost) => L.T("connection lost"),
        286 => L.T("no access to the private server any more"),
        259 or 280 => L.T("Roblox needs an update"),
        264 or 265 => L.T("already in another game"),
        ServerShutdown => L.T("server shut down"),
        _ => L.F("Roblox code {0}", code),
    };

    /// <summary>
    /// {account?, group?}: the entries the page shows, all of them, into a CSV file the person picks (newest first, the
    /// times in this PC's time zone). Returns how many, or null when the dialog was cancelled.
    /// </summary>
    object? ExportActivity(JsonElement a)
    {
        var vault = RequireVault();
        var shown = ShownActivity(vault, a);
        if (shown.Count == 0)
            throw new UserError(L.T("There is nothing to export."));
        string path;
        using (var dialog = new SaveFileDialog
               {
                   Title = L.T("Export the activity log"),
                   Filter = L.T("CSV file") + " (*.csv)|*.csv",
                   FileName = $"Bloxkeep-activity-{DateTime.Now:yyyy-MM-dd}.csv",
                   DefaultExt = "csv",
                   AddExtension = true,
                   OverwritePrompt = true,
               })
        {
            if (Ui.Modal(() => dialog.ShowDialog(_owner)) != DialogResult.OK)
                return null;
            path = dialog.FileName;
        }
        if (_vault != vault)
            throw new UserError(L.T("Bloxkeep was locked."));
        NameActivityGames(vault, shown);
        var names = ActivityNames(vault);
        var csv = new StringBuilder();
        CsvRow(csv, L.T("Time"), L.T("Account"), L.T("Kind"), L.T("Event"), L.T("Game"));
        foreach (var e in shown)
        {
            string game = e.PlaceId is { } id ? e.GameName ?? _games.GetValueOrDefault(id)?.Name ?? id.ToString(Inv) : "";
            CsvRow(csv, e.At.ToLocalTime().ToString("yyyy-MM-dd HH:mm:ss", Inv), ActivityAccount(e) ?? "", ActivityGroupName(ActivityGroup(e.Kind)),
                ActivitySentence(e, names), game);
        }
        try
        {
            // With a byte order mark, so Excel reads the names and the Vietnamese as UTF-8.
            File.WriteAllText(path, csv.ToString(), new UTF8Encoding(encoderShouldEmitUTF8Identifier: true));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            throw new UserError(L.F("Couldn't save the file: {0}", ErrorText(ex)));
        }
        return shown.Count;
    }

    static string ActivityGroupName(string group) => group switch
    {
        "play" => L.T("Games"),
        "schedule" => L.T("Schedules"),
        "signin" => L.T("Sign-in"),
        "updates" => L.T("Updates"),
        _ => L.T("Disconnects and problems"),
    };

    /// <summary>
    /// One CSV line. A cell a spreadsheet would take for a formula (an account or game named "=…") starts with an
    /// apostrophe, so opening the file never runs anything.
    /// </summary>
    static void CsvRow(StringBuilder csv, params string[] cells)
    {
        for (int i = 0; i < cells.Length; i++)
        {
            string cell = cells[i];
            if (cell.Length > 0 && cell[0] is '=' or '+' or '-' or '@' or '\t' or '\r')
                cell = "'" + cell;
            if (cell.AsSpan().IndexOfAny(",\"\r\n") >= 0)
                cell = "\"" + cell.Replace("\"", "\"\"") + "\"";
            if (i > 0)
                csv.Append(',');
            csv.Append(cell);
        }
        csv.Append("\r\n");
    }
}
