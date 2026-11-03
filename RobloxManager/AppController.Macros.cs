using System.Drawing;
using System.Drawing.Imaging;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Macros (Beta): the Macros tab's commands, the runs' state for the page, what starts a macro by itself (Ctrl+Alt+1…9,
/// a repeat, joining or rejoining a game) and recording one. Playing is MacroPlayer.cs's, on the real desktop
/// (MacroDesk.cs): one window at a time, brought forward through <see cref="InputTurn"/> like anti-AFK's nudge.
/// </summary>
public sealed partial class AppController
{
    /// <summary>Ctrl+Alt+0 stops every macro and a recording; Ctrl+Alt+1…9 are ids 11…19; F8 (alone) ends a recording.</summary>
    public const int HotkeyStopMacros = 10, HotkeyMacroFirst = 11, HotkeyEndRecording = 20;
    const int MaxMacroImport = 4 * 1024 * 1024;
    const string MacroFileType = ".bloxkeepmacro";
    /// <summary>A macro that starts by itself waits this long after the join (or after Bloxkeep opened) for the game to settle.</summary>
    static readonly TimeSpan MacroSettle = TimeSpan.FromSeconds(30);
    static readonly TimeSpan MacroPushEvery = TimeSpan.FromMilliseconds(400);

    MacroPlayer? _macroPlayer;
    MacroDesk? _macroDesk;
    MacroRecorder? _recorder;
    double? _macroSweep;
    /// <summary>The accounts each macro last ran on (Ctrl+Alt+digit of a macro for the selected accounts, with no window in front).</summary>
    readonly Dictionary<Guid, Guid[]> _macroLastAccounts = new();
    readonly Dictionary<Guid, DateTime> _macroIntervalAt = new();
    /// <summary>The join (its log's time) a join or rejoin macro already ran for, per macro and account.</summary>
    readonly Dictionary<(Guid Macro, Guid Account), DateTime> _macroJoinRan = new();
    DateTime _macroPushedAt;
    bool _macroPushWaiting;

    MacroDesk MacroDeskNow => _macroDesk ??= new MacroDesk(MacroClient, () => _robloxInput?.PersonAt, () => _robloxInput is { Working: true });

    MacroPlayer MacroPlayerNow
    {
        get
        {
            if (_macroPlayer is not null)
                return _macroPlayer;
            _macroPlayer = new MacroPlayer(MacroDeskNow);
            WireMacroPlayer(_macroPlayer);
            return _macroPlayer;
        }
    }

    void WireMacroPlayer(MacroPlayer player)
    {
        player.Changed = MacrosChanged;
        // Anti-AFK counts the turn as the window's activity: a nudge right after it would be for nothing.
        player.Played = (_, pid) =>
        {
            _lastInput[pid] = DateTime.UtcNow;
            _afkHeld.Remove(pid);
        };
        player.Rejoin = MacroRejoin;
        player.Alert = MacroAlert;
    }

    /// <summary>A macro's alert: the activity entry, the toast, and Discord (with the window's picture when asked).</summary>
    void MacroAlert(Guid account, Macro macro, string text, byte[]? png)
    {
        if (png is null)
        {
            Notify(text, "warn", "macro", account);
            return;
        }
        NoteNotified(text, "warn", "macro", account);
        Toast(text, "warn");
        _ = SendMacroPictureAsync(account, text, png);
    }

    async Task SendMacroPictureAsync(Guid account, string text, byte[] png)
    {
        if (_disposed || _vault is not { Data.Discord: { Enabled: true, WebhookUrl.Length: > 0 } d })
            return;
        var message = new DiscordMessage(DT("Macro alert"), DiscordWebhook.Escape(text), [], Color: DiscordWebhook.Yellow, Timestamp: DateTime.UtcNow)
        {
            Author = FindAccount(account) is { } acc ? DiscordName(acc) : null,
        };
        try
        {
            await DiscordWebhook.SendAsync(d.WebhookUrl, DiscordWebhook.Build(message, DiscordPing.None, d.Style != "plain", d.Username, d.AvatarUrl, "macro.png"),
                null, wait: false, file: new DiscordFile("macro.png", png));
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    /// <summary>The account's Roblox as Bloxkeep knows it: its process, the frame rate limit it was given, whether its log says it is in a game.</summary>
    (int Pid, int Fps, bool InGame)? MacroClient(Guid accountId) =>
        _running.TryGetValue(accountId, out var run) && !_closingForRejoin.Contains(run.Pid)
            ? (run.Pid, run.Launch.FpsLimit, _logs.StateOf(run.Pid) is null or { Status: "ingame" })
            : null;

    /// <summary>The rejoin step: the window closed and opened again into its server, the way a preset's restart does it.</summary>
    string? MacroRejoin(Guid accountId)
    {
        if (!_running.TryGetValue(accountId, out var run) || run.Launch.Target is null)
            return L.T("Bloxkeep didn't open this window into a game, so it can't rejoin it.");
        if (_closingForRejoin.Contains(run.Pid))
            return null;
        if (FindAccount(accountId) is not { } account || account.CookieValid == false || account.Banned == true)
            return L.T("This account can't be opened again now.");
        if (!MultiInstance.Armed && _running.Count > 1)
            return L.T("Turn on Multi-Roblox to rejoin while other windows are open.");
        _restarts[accountId] = (RestartTarget(run), DateTime.UtcNow);
        _closingForRejoin.Add(run.Pid); // Bloxkeep's own close: neither a drop nor closed by Roblox
        RobloxProcesses.Close(run.Pid, InGameNow(run.Pid));
        return null;
    }

    /// <summary>Runs changed: the page hears it at most every 400 ms (a run's step moves many times a second).</summary>
    void MacrosChanged()
    {
        if (_disposed || _macroPushWaiting)
            return;
        var wait = _macroPushedAt + MacroPushEvery - DateTime.UtcNow;
        if (wait <= TimeSpan.Zero)
        {
            _macroPushedAt = DateTime.UtcNow;
            PushStateIfChanged();
            return;
        }
        _macroPushWaiting = true;
        _ = Task.Delay(wait).ContinueWith(_ =>
        {
            _macroPushWaiting = false;
            _macroPushedAt = DateTime.UtcNow;
            PushStateIfChanged();
        }, TaskScheduler.FromCurrentSynchronizationContext());
    }

    List<Macro> MacroList() => RequireVault().Data.Settings.Macros;

    Macro MacroById(Guid id) => MacroList().FirstOrDefault(m => m.Id == id) ?? throw new UserError(L.T("That macro isn't there any more."));

    object? SaveMacro(JsonElement a)
    {
        var vault = RequireVault();
        if (!Has(a, "macro", JsonValueKind.Object))
            throw new UserError(L.T("That isn't a macro."));
        var macro = ReadMacro(a.GetProperty("macro"));
        var macros = vault.Data.Settings.Macros;
        int index = macro.Id == Guid.Empty ? -1 : macros.FindIndex(m => m.Id == macro.Id);
        if (index < 0 && macros.Count >= MacroRules.MaxMacros)
            throw new UserError(L.F("You can save up to {0} macros.", MacroRules.MaxMacros));
        MacroRules.Check(macro, vault.Data.Accounts.Select(acc => acc.Id).ToHashSet(), macros);
        if (index < 0)
        {
            // A new one keeps an id it came with (a draft tried with "Try it" before its first save).
            if (macro.Id == Guid.Empty)
                macro.Id = Guid.NewGuid();
            macros.Add(macro);
        }
        else
        {
            macros[index] = macro;
            // Runs going on keep the steps they started with; the next run takes these.
        }
        _macroIntervalAt.Remove(macro.Id);
        Persist();
        HotkeysChanged?.Invoke();
        RefreshPanel(macro.Id);
        PushState();
        return macro;
    }

    static Macro ReadMacro(JsonElement json)
    {
        try
        {
            return MacroRules.Read(json);
        }
        catch (JsonException)
        {
            throw new UserError(L.T("That isn't a macro."));
        }
    }

    object? DeleteMacro(Guid id)
    {
        var macros = MacroList();
        if (macros.RemoveAll(m => m.Id == id) == 0)
            return null;
        _macroPlayer?.Stop(id, "byUser");
        ClosePanelWindow(id);
        _macroLastAccounts.Remove(id);
        _macroIntervalAt.Remove(id);
        foreach (var key in _macroJoinRan.Keys.Where(k => k.Macro == id).ToList())
            _macroJoinRan.Remove(key);
        Persist();
        HotkeysChanged?.Invoke();
        PushState();
        return null;
    }

    /// <summary>A copy right after it, named "… (copy)", its hotkey left free and its trigger off.</summary>
    object? DuplicateMacro(Guid id)
    {
        var macros = MacroList();
        var source = MacroById(id);
        if (macros.Count >= MacroRules.MaxMacros)
            throw new UserError(L.F("You can save up to {0} macros.", MacroRules.MaxMacros));
        var copy = MacroRules.Copy(source);
        copy.Id = Guid.NewGuid();
        copy.Hotkey = null;
        copy.Enabled = false;
        copy.Panel.AutoShow = false; // one panel by itself per game
        copy.Name = CopyName(source.Name, macros.Select(m => m.Name));
        macros.Insert(macros.IndexOf(source) + 1, copy);
        Persist();
        PushState();
        return copy;
    }

    /// <summary>"Name (copy)", or "(copy 2)" and on when that is taken, shortened so it fits 60 characters.</summary>
    static string CopyName(string name, IEnumerable<string> taken, int max = MacroRules.MaxName)
    {
        var names = taken.ToHashSet(StringComparer.OrdinalIgnoreCase);
        for (int n = 1; ; n++)
        {
            string suffix = n == 1 ? L.T(" (copy)") : L.F(" (copy {0})", n);
            string candidate = CutText(name, max - suffix.Length).TrimEnd() + suffix;
            if (!names.Contains(candidate))
                return candidate;
        }
    }

    object? MoveMacro(Guid id, int to)
    {
        var macros = MacroList();
        var macro = MacroById(id);
        macros.Remove(macro);
        macros.Insert(Math.Clamp(to, 0, macros.Count), macro);
        Persist();
        PushState();
        return null;
    }

    /// <summary>The accounts a run goes to: those sent (the page's selection), else the macro's own.</summary>
    List<Guid> MacroAccounts(Macro macro, Guid[]? chosen)
    {
        var vault = RequireVault();
        IEnumerable<Guid> ids = chosen is { Length: > 0 } ? chosen
            : macro.Targets switch
            {
                "all" => vault.Data.Accounts.Select(acc => acc.Id).Where(_running.ContainsKey),
                "accounts" => macro.AccountIds,
                _ => [],
            };
        return ids.Where(id => vault.Data.Accounts.Any(acc => acc.Id == id)).Distinct().ToList();
    }

    object? RunMacro(Guid id, Guid[]? accountIds)
    {
        var macro = MacroById(id);
        // Accounts sent, but none (nothing selected): not the macro's own instead. On the selected accounts with none sent
        // (the Macros page with nothing selected): where its panel works, the accounts in its game or the window in front.
        var accounts = accountIds is { Length: 0 } ? [] : MacroAccounts(macro, accountIds);
        if (accounts.Count == 0 && accountIds is null && macro.Targets == "selected")
            accounts = PanelAccounts(macro);
        if (accounts.Count == 0)
            throw new UserError(accountIds is { Length: 0 } || (macro.Targets == "selected" && accountIds is null)
                ? L.T("Select the accounts to run the macro on.")
                : L.T("None of the macro's accounts is there."));
        var open = accounts.Where(_running.ContainsKey).ToList();
        if (open.Count == 0)
            throw new UserError(accounts.Count == 1 ? L.T("This account's Roblox isn't open.") : L.T("None of these accounts has Roblox open."));
        return new { queued = QueueMacro(macro, open) };
    }

    int QueueMacro(Macro macro, List<Guid> accounts)
    {
        _macroLastAccounts[macro.Id] = accounts.ToArray();
        // The steps as saved now: an edit while it runs doesn't change a run halfway.
        int queued = MacroPlayerNow.Queue(MacroRules.Copy(macro), accounts);
        if (queued > 0)
        {
            var fps = accounts.Select(a => _running.TryGetValue(a, out var run) ? run.Launch.FpsLimit : 0).DefaultIfEmpty(0).Max();
            _macroSweep = MacroPlayer.SweepSeconds(macro, MacroPlayerNow.Runs.Count(r => !r.Finished), fps);
        }
        PushState();
        return queued;
    }

    /// <param name="accountId">Only this account's run (the editor's "Stop trying").</param>
    /// <param name="trial">Only Try it's run: the saved macro's own runs, same id, go on.</param>
    object? StopMacros(Guid? id, Guid? accountId = null, bool trial = false)
    {
        _macroPlayer?.Stop(id, "byUser", accountId, trial);
        if (id is null && accountId is null && !trial)
            _ = StopRecordingAsync();
        PushState();
        return null;
    }

    /// <summary>The account's window as it is now, for the editor to cut pictures and pick places from.</summary>
    async Task<object?> MacroPictureAsync(Guid accountId)
    {
        RequireVault();
        using var picture = await MacroPictureOfAsync(accountId);
        using var png = new MemoryStream();
        picture.Save(png, ImageFormat.Png);
        return new { image = "data:image/png;base64," + Convert.ToBase64String(png.ToArray()), w = picture.Width, h = picture.Height };
    }

    /// <summary>The window's client area, taken in its turn (a minimised or hidden one is shown unseen for it).</summary>
    async Task<Bitmap> MacroPictureOfAsync(Guid accountId)
    {
        if (MacroDeskNow.Window(accountId) is not { } window)
            throw new UserError(L.T("This account's Roblox isn't open."));
        Bitmap? picture;
        using (await InputTurn.EnterAsync("picture"))
            picture = await WindowShot.ClientAsync(window.Handle, shown: false);
        return picture ?? throw new UserError(L.T("Couldn't take a picture of this window. Try again."));
    }

    /// <summary>The editor's Try: one check on the window as it is now.</summary>
    async Task<object?> MacroTestAsync(JsonElement a)
    {
        RequireVault();
        MacroTest test;
        try
        {
            test = Has(a, "test", JsonValueKind.Object) ? a.GetProperty("test").Deserialize<MacroTest>(MacroRules.PageJson)! : null!;
        }
        catch (JsonException)
        {
            test = null!;
        }
        MacroRules.CheckTest(test);
        var accountId = Guid.TryParse(Str(a, "accountId"), out var g) ? g : Guid.Empty;
        if (test.Kind == "var")
        {
            double value = _macroPlayer?.Vars.GetValueOrDefault(accountId)?.GetValueOrDefault(test.Name!) ?? 0;
            return new { ok = MacroPlayer.Compare(value, test.Op!, test.Value ?? 0) };
        }
        using var picture = await MacroPictureOfAsync(accountId);
        // Cut from a window of another width (the macro's recordedSize, when the page sends it): scaled to this one's.
        double scale = Has(a, "recordedSize", JsonValueKind.Object) && a.GetProperty("recordedSize").TryGetProperty("w", out var w)
            && w.TryGetInt32(out int width) && width > 0 ? (double)picture.Width / width : 1;
        bool not = test.Not == true;
        switch (test.Kind)
        {
            case "image":
            {
                var gray = MacroVision.ToGray(picture);
                var result = await Task.Run(() => MacroVision.Template(test.Image!, scale) is { } template
                    ? MacroVision.Find(gray, template, MacroVision.Area(test.Region, gray.W, gray.H)) : null);
                bool found = result is { } r && r.Score >= (test.Threshold ?? 0.9);
                return new
                {
                    ok = found != not,
                    found = found ? MacroVision.Fraction(result!.Value.Found, gray.W, gray.H) : null,
                    score = result is { } best ? Math.Round(best.Score, 3) : (double?)null,
                };
            }
            case "text":
            {
                var area = MacroVision.Area(test.Region, picture.Width, picture.Height);
                var read = await MacroDeskNow.ReadAsync(picture, area) ?? throw new UserError(L.T("Windows can't read text on this PC: add a language with text recognition in Settings › Time & language."));
                RectangleF? at = null;
                foreach (var lines in read)
                {
                    if ((at = test.Words is { } words ? MacroOcr.LocateAny(lines, words, words)?.Box : MacroOcr.Locate(lines, test.Text!)) is not null)
                        break;
                }
                string text = string.Join("\n", read.FirstOrDefault()?.Select(l => l.Text) ?? []);
                return new
                {
                    ok = at is not null != not,
                    found = at is { } box ? MacroVision.Fraction(Rectangle.Round(box), picture.Width, picture.Height) : null,
                    text = text.Length > 2000 ? text[..2000] : text,
                };
            }
            default:
                return new { ok = MacroVision.ColourAt(picture, test.X!.Value, test.Y!.Value, test.Colour!, test.Tolerance ?? 24) != not };
        }
    }

    /// <summary>Records the account's window: it comes forward, and what the person does in it after a 3-second countdown becomes steps.</summary>
    object? RecordMacro(Guid accountId)
    {
        RequireVault();
        if (_recorder is not null)
            throw new UserError(L.T("A recording is already going on."));
        if (MacroDeskNow.Window(accountId) is not { } window)
            throw new UserError(L.T("This account's Roblox isn't open."));
        if (window.Elevated)
            throw new UserError(L.T("This Roblox runs as administrator, so Bloxkeep can't record or play into it."));
        // Runs wait while recording: their input would be recorded too, and would fight the person's.
        _macroPlayer?.Stop(null, "recording");
        var recorder = new MacroRecorder(accountId, window.Pid, window.Handle, _robloxInput);
        _recorder = recorder;
        recorder.StopAsked = () => _ = StopRecordingAsync();
        recorder.Changed = MacrosChanged;
        MacroPlayerNow.Recording = true;
        HotkeysChanged?.Invoke(); // F8
        RobloxProcesses.FocusInTurn(window.Pid);
        PushState();
        return null;
    }

    /// <summary>Ends the recording; the page gets its steps as the event macroRecorded.</summary>
    async Task<object?> StopRecordingAsync()
    {
        if (_recorder is not { } recorder)
            return null;
        _recorder = null;
        if (_macroPlayer is not null)
            _macroPlayer.Recording = false;
        HotkeysChanged?.Invoke();
        var steps = await recorder.FinishAsync();
        PushState();
        if (_vault is not null)
            Emit("macroRecorded", new { steps, w = recorder.Client.Width, h = recorder.Client.Height });
        return null;
    }

    /// <summary>The recording dropped without its steps (the vault locked).</summary>
    void DropRecording()
    {
        _recorder?.Dispose();
        _recorder = null;
        if (_macroPlayer is not null)
            _macroPlayer.Recording = false;
    }

    object? ImportMacro()
    {
        RequireVault();
        using var dialog = new OpenFileDialog
        {
            Title = L.T("Add a macro from a file"),
            Filter = L.T("Macros") + $" (*{MacroFileType}; *.rec)|*{MacroFileType};*.rec|" + L.T("Bloxkeep macro") + $" (*{MacroFileType})|*{MacroFileType}|"
                + L.T("TinyTask recording") + " (*.rec)|*.rec",
            InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
        };
        if (Ui.Modal(() => dialog.ShowDialog(_owner)) != DialogResult.OK)
            return null;
        return ForPage(ReadMacroFile(dialog.FileName));
    }

    /// <summary>A macro not saved yet, for the editor: its id empty ("").</summary>
    static System.Text.Json.Nodes.JsonNode ForPage(Macro macro)
    {
        var node = JsonSerializer.SerializeToNode(macro, MacroRules.PageJson)!;
        node["id"] = "";
        return node;
    }

    /// <summary>A macro from a file, not saved: the editor opens it, and Save checks it.</summary>
    static Macro ReadMacroFile(string path)
    {
        var info = new FileInfo(path);
        if (!info.Exists || info.Length > MaxMacroImport)
            throw new UserError(L.T("That file is too big to be a macro."));
        byte[] bytes = File.ReadAllBytes(path);
        string name = CutText(Path.GetFileNameWithoutExtension(path), MacroRules.MaxName);
        if (path.EndsWith(".rec", StringComparison.OrdinalIgnoreCase))
            return MacroRecording.FromTinyTask(bytes, name.Length > 0 ? name : "TinyTask");
        Macro macro;
        try
        {
            using var json = JsonDocument.Parse(bytes);
            macro = ReadMacro(json.RootElement);
        }
        catch (JsonException)
        {
            throw new UserError(L.T("That isn't a macro."));
        }
        macro.Id = Guid.Empty;
        macro.Hotkey = null; // a hotkey may be another macro's here
        macro.Enabled = false;
        macro.AccountIds = new(); // the accounts of someone else's vault
        if (macro.Targets == "accounts")
            macro.Targets = "selected";
        if (macro.Trigger is not null && macro.Trigger.Kind != "manual" && macro.Targets == "selected")
            macro.Targets = "all";
        return macro;
    }

    object? ExportMacro(Guid id)
    {
        var macro = MacroById(id);
        using var dialog = new SaveFileDialog
        {
            Title = L.T("Save the macro as a file"),
            Filter = L.T("Bloxkeep macro") + $" (*{MacroFileType})|*{MacroFileType}",
            FileName = string.Concat(macro.Name.Select(ch => Path.GetInvalidFileNameChars().Contains(ch) ? '-' : ch)) + MacroFileType,
            InitialDirectory = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
        };
        if (Ui.Modal(() => dialog.ShowDialog(_owner)) != DialogResult.OK)
            return null;
        File.WriteAllText(dialog.FileName, MacroFileText(macro));
        return new { saved = Path.GetFileName(dialog.FileName) };
    }

    /// <summary>A macro as its file has it: the page's shape, without this vault's accounts.</summary>
    static string MacroFileText(Macro macro)
    {
        var copy = MacroRules.Copy(macro);
        copy.AccountIds = new();
        copy.Hotkey = null;
        return JsonSerializer.Serialize(copy, new JsonSerializerOptions(MacroRules.PageJson) { WriteIndented = true });
    }

    /// <summary>Ctrl+Alt+digit: the macro holding it runs on its accounts; one for the selected accounts runs on the window
    /// in front when that is an account's, else on those it last ran on.</summary>
    void RunMacroHotkey(int digit)
    {
        if (Open?.Data.Settings.Macros.FirstOrDefault(m => m.Hotkey == digit) is not { } macro)
            return;
        Guid[]? accounts = null;
        if (macro.Targets == "selected")
        {
            int front = RobloxProcesses.ForegroundPid();
            accounts = _running.Where(r => r.Value.Pid == front).Select(r => r.Key).ToArray() is { Length: > 0 } inFront ? inFront
                : _macroLastAccounts.GetValueOrDefault(macro.Id);
            if (accounts is not { Length: > 0 })
            {
                Toast(L.F("Run {0} from Bloxkeep once first: it doesn't know which accounts to use.", macro.Name), "warn");
                return;
            }
        }
        var open = MacroAccounts(macro, accounts).Where(_running.ContainsKey).ToList();
        if (open.Count == 0)
        {
            Toast(L.F("{0}: none of its accounts has Roblox open.", macro.Name), "warn");
            return;
        }
        QueueMacro(macro, open);
    }

    /// <summary>The digits macros hold, for the window to register their Ctrl+Alt hotkeys.</summary>
    public int[] MacroHotkeys() => Open?.Data.Settings.Macros.Where(m => m.Hotkey is >= 1 and <= 9).Select(m => m.Hotkey!.Value).Distinct().Order().ToArray() ?? [];

    public bool RecordingMacro => _recorder is not null;

    /// <summary>Every process tick: macros that start by themselves, a recording past its 10 minutes.</summary>
    void TickMacros(DateTime now)
    {
        if (_recorder is { } recorder && now - recorder.StartsAt > MacroRecorder.MaxLength)
            _ = StopRecordingAsync();
        if (_vault is null || _recorder is not null)
            return;
        // Not before Bloxkeep has watched the windows a while: the windows already in a game when it opened count as just joined.
        if (now - _started < MacroSettle)
            return;
        foreach (var macro in _vault.Data.Settings.Macros)
        {
            if (!macro.Enabled || macro.Trigger.Kind == "manual")
                continue;
            var accounts = MacroAccounts(macro, null);
            if (macro.Trigger.Kind == "interval")
            {
                var every = TimeSpan.FromSeconds(macro.Trigger.EverySeconds ?? 60);
                if (!_macroIntervalAt.TryGetValue(macro.Id, out var last) || last > now)
                {
                    // The first run a settle after it was turned on (or Bloxkeep opened), then every period.
                    _macroIntervalAt[macro.Id] = now - every + MacroSettle;
                    continue;
                }
                if (now - last < every)
                    continue;
                _macroIntervalAt[macro.Id] = now;
                var ready = accounts.Where(id => MacroClient(id) is { InGame: true }).ToList();
                if (ready.Count > 0)
                    QueueMacro(macro, ready);
                continue;
            }
            var due = new List<Guid>();
            foreach (var id in accounts)
            {
                if (!_running.TryGetValue(id, out var run) || _logs.StateOf(run.Pid) is not { Status: "ingame" } state || now - state.At < MacroSettle)
                    continue;
                if (macro.Trigger.Kind == "rejoin" && !run.Launch.Rejoin)
                    continue;
                if (_macroJoinRan.TryGetValue((macro.Id, id), out var ran) && ran == state.At)
                    continue;
                _macroJoinRan[(macro.Id, id)] = state.At;
                due.Add(id);
            }
            if (due.Count > 0)
                QueueMacro(macro, due);
        }
    }

    /// <summary>The vault locked: runs stop, the recording is dropped, variables go.</summary>
    void StopMacrosForLock()
    {
        DropRecording();
        _macroPlayer?.Clear("vaultLocked");
        _macroLastAccounts.Clear();
        _macroIntervalAt.Clear();
        _macroJoinRan.Clear();
        _macroSweep = null;
        HotkeysChanged?.Invoke();
    }

    /// <summary>The PC going to sleep: what plays stops (the game won't be where it was at wake).</summary>
    void StopMacrosForSleep()
    {
        _ = StopRecordingAsync();
        _macroPlayer?.Stop(null, "locked");
    }

    /// <summary>A macro run is going on or waits: an update's restart waits for it.</summary>
    bool MacrosPlaying => _macroPlayer?.Active == true || _recorder is not null;

    object MacroRunsDto() => Open is null || _macroPlayer is null ? Array.Empty<object>() : _macroPlayer.Runs.Select(r => new
    {
        macroId = r.Macro.Id,
        accountId = r.AccountId,
        status = r.Status,
        reason = r.Reason,
        step = r.Finished ? (int?)null : r.Pc,
        // Where an ended run was: the editor marks the step a failed run stopped at.
        lastStep = r.Finished ? r.Pc : (int?)null,
        trial = r.Trial ? true : (bool?)null,
        loops = r.Loops > 0 ? r.Loops : (int?)null,
        startedAt = r.StartedAt,
        endsAt = r.EndsAt,
    }).ToArray();

    /// <summary>The macros on the selected accounts that Run still has somewhere to go with none selected: the accounts in
    /// their game, the Roblox window in front, or those it last ran on (as their panel works on).</summary>
    object MacroCanRunDto() => Open is null ? Array.Empty<Guid>()
        : MacroList().Where(m => m.Targets == "selected" && PanelAccounts(m).Count > 0).Select(m => m.Id).ToArray();

    object? MacroRecordingDto() => Open is not null && _recorder is { } r ? new { accountId = r.AccountId, startsAt = r.StartsAt, steps = r.Steps } : null;

    object MacroVarsDto() => Open is not null && _macroPlayer is not null
        ? _macroPlayer.Vars.ToDictionary(v => v.Key.ToString(), v => v.Value) : new Dictionary<string, Dictionary<string, double>>();

    object? MacroSweepDto() => Open is not null && _macroSweep is { } seconds && _macroPlayer?.Active == true ? new { seconds } : null;
}
