using System.Drawing;
using System.Drawing.Imaging;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Macros (Beta): each macro's panel (PanelForm.cs), shown by itself while an account plays its game; the settings the
/// player changes there ("$o1"); the on-game overlay (GameOverlay.cs) that edits the macro's points and areas on the
/// Roblox window itself and picks a place or colour there; trying a macro not saved yet; "What it sees" (MacroWatch.cs).
/// </summary>
public sealed partial class AppController
{
    readonly Dictionary<Guid, PanelForm> _panelForms = new();
    /// <summary>Panels shown by themselves (their game is played), and those the person closed meanwhile (left closed until no account plays it).</summary>
    readonly HashSet<Guid> _panelsAuto = new(), _panelsClosed = new();
    GameOverlay? _overlay;
    /// <summary>What the overlay is on: the macro (empty for a pick), the account, and whether the page asked for it.</summary>
    (Guid Macro, Guid Account, bool Page)? _overlayFor;
    /// <summary>The editor's "Try it" going on: its run's macro id and account.</summary>
    (Guid Macro, Guid Account)? _tryRun;

    /// <summary>The vault open now, for the panels.</summary>
    internal Vault? OpenVault => Open;

    Macro? OpenMacro(Guid id) => Open?.Data.Settings.Macros.FirstOrDefault(m => m.Id == id);

    // ───────────────────────── commands ─────────────────────────

    object? ShowPanel(Guid id, bool show)
    {
        var macro = MacroById(id);
        if (show)
        {
            _panelsClosed.Remove(id);
            ShowPanelWindow(macro, activate: true);
        }
        else
            HidePanelWindow(id, byPerson: true);
        PushState();
        return null;
    }

    /// <summary>A setting's value from the page or the panel, saved at once; runs going on read it from their next step.</summary>
    internal object? SetOption(Guid id, string name, JsonElement value)
    {
        var macro = MacroById(id);
        var option = macro.Options.FirstOrDefault(o => o.Name == name) ?? throw new UserError(L.T("That setting isn't there any more."));
        option.Value = MacroOptions.Normalize(option, value) ?? throw new UserError(L.T("That value doesn't fit the setting."));
        Live(id, copy =>
        {
            if (copy.Options.FirstOrDefault(o => o.Name == name) is { } o)
                o.Value = option.Value;
        });
        Persist();
        RefreshPanel(id);
        PushState();
        return null;
    }

    /// <summary>"Reset": every setting back to how the macro was made.</summary>
    internal void ResetOptions(Guid id)
    {
        var macro = MacroById(id);
        foreach (var o in macro.Options)
            o.Value = o.Default;
        Live(id, copy =>
        {
            foreach (var o in copy.Options)
                o.Value = o.Default;
        });
        Persist();
        RefreshPanel(id);
        PushState();
    }

    /// <summary>An area moved on the game from the panel ("Edit areas"), saved at once.</summary>
    internal void SetArea(Guid id, string name, MacroRect rect)
    {
        var macro = MacroById(id);
        if (macro.Areas.FirstOrDefault(a => a.Name == name) is not { } area || MacroOptions.Rect(rect) is not { } fits)
            return;
        area.Rect = fits;
        Live(id, copy =>
        {
            if (copy.Areas.FirstOrDefault(a => a.Name == name) is { } c)
                c.Rect = fits;
        });
        Persist();
        PushState();
    }

    /// <summary>The copies runs going on play (a run keeps the steps it started with, but follows its settings).</summary>
    void Live(Guid id, Action<Macro> change)
    {
        foreach (var run in _macroPlayer?.Runs.Where(r => !r.Finished && !r.Trial && r.Macro.Id == id) ?? [])
            change(run.Macro);
    }

    object? MacroTemplatesDto() => MacroTemplates.All();

    // ───────────────────────── running from a panel ─────────────────────────

    /// <summary>The accounts a panel's Run plays on: the macro's own, or for the selected accounts those in its panel's
    /// game, else the window in front, else those it last ran on.</summary>
    List<Guid> PanelAccounts(Macro macro)
    {
        if (macro.Targets == "accounts")
            return macro.AccountIds.Where(_running.ContainsKey).ToList();
        if (macro.Targets == "all")
            return _running.Keys.ToList();
        var inGame = macro.Panel.PlaceId is { } place
            ? _running.Where(r => _logs.StateOf(r.Value.Pid) is { Status: "ingame" } st && st.PlaceId == place).Select(r => r.Key).ToList() : [];
        if (inGame.Count > 0)
            return inGame;
        int front = RobloxProcesses.ForegroundPid();
        var inFront = _running.Where(r => r.Value.Pid == front).Select(r => r.Key).ToList();
        if (inFront.Count > 0)
            return inFront;
        return (_macroLastAccounts.GetValueOrDefault(macro.Id) ?? []).Where(_running.ContainsKey).ToList();
    }

    /// <summary>The account the panel works on the game with (pick, edit areas): its first, else any window open.</summary>
    Guid PanelAccount(Macro macro) => PanelAccounts(macro).Concat(_running.Keys).FirstOrDefault();

    /// <summary>The panel's Run: plays the macro, or stops it when it runs.</summary>
    internal void PanelRun(Guid id)
    {
        if (OpenMacro(id) is not { } macro)
            return;
        if (_macroPlayer?.Runs.Any(r => !r.Finished && !r.Trial && r.Macro.Id == id) == true)
        {
            _macroPlayer.Stop(id, "byUser");
            PushState();
            RefreshPanel(id, rebuild: false);
            return;
        }
        var accounts = PanelAccounts(macro);
        if (accounts.Count == 0)
        {
            Toast(L.F("{0}: no Roblox window to run it in.", macro.Name), "warn");
            return;
        }
        QueueMacro(macro, accounts);
        RefreshPanel(id, rebuild: false);
    }

    /// <summary>What a panel shows: its status line (who runs or waits, the counters, the run time), and whether it runs.</summary>
    internal sealed record PanelNow(string Line, bool Running);

    internal PanelNow? PanelStatus(Guid id)
    {
        if (OpenMacro(id) is not { } macro)
            return null;
        var runs = _macroPlayer?.Runs.Where(r => !r.Finished && !r.Trial && r.Macro.Id == id).ToList() ?? [];
        var parts = new List<string>();
        if (_recorder is not null)
            parts.Add(L.T("Recording…"));
        else if (runs.Count == 0)
            parts.Add(L.T("Idle"));
        else
        {
            foreach (var run in runs.Take(3))
            {
                string name = FindAccount(run.AccountId) is { } acc ? NameText(acc) : "?";
                parts.Add(run.Status switch
                {
                    "running" => run.Reason == "recovering" ? L.F("{0} gets unstuck", name) : L.F("{0} runs", name),
                    "paused" => L.F("{0} paused: {1}", name, ReasonText(run.Reason)),
                    _ => L.F("{0} waits its turn", name),
                });
            }
            if (runs.Count > 3)
                parts.Add(L.F("{0} more", runs.Count - 3));
        }
        var vars = _macroPlayer?.Vars ?? new();
        foreach (var counter in MacroOptions.Counters(macro))
        {
            double sum = vars.Values.Sum(d => d.GetValueOrDefault(counter));
            parts.Add($"{counter} {sum.ToString("0.##", System.Globalization.CultureInfo.InvariantCulture)}");
        }
        if (runs.Count > 0)
        {
            var t = DateTime.UtcNow - runs.Min(r => r.StartedAt);
            parts.Add($"{(int)t.TotalHours}:{t.Minutes:00}:{t.Seconds:00}");
        }
        return new PanelNow(string.Join(" · ", parts), runs.Count > 0);
    }

    /// <summary>A run's reason as the panel says it (the page has its own wording for these).</summary>
    static string ReasonText(string? reason) => reason switch
    {
        "personActive" => L.T("you're using the PC"),
        "locked" => L.T("the PC is locked"),
        "fullscreen" => L.T("a full-screen app is in front"),
        "focusRefused" => L.T("Windows wouldn't bring the game forward"),
        "notInGame" => L.T("not in a game"),
        "covered" => L.T("another window is over the game"),
        "busy" => L.T("another window's turn"),
        "recording" => L.T("recording"),
        "imeOn" => L.T("a Vietnamese keyboard is on"),
        _ => L.T("waiting"),
    };

    // ───────────────────────── panel windows ─────────────────────────

    void ShowPanelWindow(Macro macro, bool activate)
    {
        if (_disposed || Open is null)
            return;
        if (!_panelForms.TryGetValue(macro.Id, out var form) || form.IsDisposed)
        {
            var id = macro.Id;
            form = new PanelForm(this, id);
            form.Dismissed += () =>
            {
                _panelsAuto.Remove(id);
                if (OpenMacro(id) is { Panel.AutoShow: true })
                    _panelsClosed.Add(id);
                PushState();
            };
            _panelForms[id] = form;
        }
        form.Open(activate);
    }

    void HidePanelWindow(Guid id, bool byPerson)
    {
        if (_panelForms.TryGetValue(id, out var form) && !form.IsDisposed && form.Visible)
            form.HideQuietly();
        _panelsAuto.Remove(id);
        if (byPerson && OpenMacro(id) is { Panel.AutoShow: true })
            _panelsClosed.Add(id);
        if (_overlayFor is { Page: false } f && f.Macro == id)
            CloseOverlay();
    }

    /// <summary>The macro deleted: its panel goes, with its place.</summary>
    void ClosePanelWindow(Guid id)
    {
        if (_panelForms.Remove(id, out var form) && !form.IsDisposed)
            form.Close();
        _panelsAuto.Remove(id);
        _panelsClosed.Remove(id);
        if (_overlayFor is { Page: false } f && f.Macro == id)
            CloseOverlay();
        if (Prefs.PanelWindows.Remove(id.ToString()))
            Prefs.Save();
    }

    void RefreshPanel(Guid id, bool rebuild = true)
    {
        if (_panelForms.TryGetValue(id, out var form) && !form.IsDisposed && form.Visible)
            form.Reload(rebuild);
    }

    object PanelsShownDto() => Open is null ? Array.Empty<Guid>() : _panelForms.Where(f => !f.Value.IsDisposed && f.Value.Visible).Select(f => f.Key).ToArray();

    /// <summary>Every process tick: panels shown and hidden by their game, their status lines.</summary>
    void TickPanels()
    {
        if (Open is not { } vault)
            return;
        var places = _running.Values.Select(r => _logs.StateOf(r.Pid)).Where(st => st is { Status: "ingame", PlaceId: > 0 }).Select(st => st!.PlaceId!.Value).ToHashSet();
        var (show, hide) = MacroOptions.AutoShow(vault.Data.Settings.Macros, places, _panelsAuto, _panelsClosed,
            id => _panelForms.TryGetValue(id, out var f) && !f.IsDisposed && f.Visible);
        foreach (var id in hide)
            HidePanelWindow(id, byPerson: false);
        foreach (var id in show)
        {
            if (OpenMacro(id) is { } macro)
                ShowPanelWindow(macro, activate: false);
        }
        if (show.Count > 0 || hide.Count > 0)
            PushState();
        foreach (var form in _panelForms.Values.Where(f => !f.IsDisposed && f.Visible))
            form.Tick();
    }

    /// <summary>The vault locked: every panel goes, with the overlay, the try and the watch.</summary>
    void ClosePanelsForLock()
    {
        foreach (var form in _panelForms.Values.Where(f => !f.IsDisposed).ToList())
            form.Close();
        _panelForms.Clear();
        _panelsAuto.Clear();
        _panelsClosed.Clear();
        _tryRun = null;
        CloseOverlay(tellPage: false);
        StopWatch();
    }

    // ───────────────────────── on the game ─────────────────────────

    static List<GameOverlay.Area> OverlayAreas(Macro m) => (m.Areas ?? new()).Where(a => a is not null).Select(a => new GameOverlay.Area(a.Name, a.Label is { Length: > 0 } label ? label : a.Name,
        ColorTranslator.FromHtml(a.Colour is { Length: 7 } c && c[0] == '#' && c[1..].All(Uri.IsHexDigit) ? c : MacroOptions.AreaColour),
        MacroOptions.Rect(a.Rect) ?? new MacroRect { X = 0.25, Y = 0.25, W = 0.5, H = 0.5 })).ToList();

    /// <summary>
    /// The editor's "Edit on game": the draft's points and areas over the account's window, the page told of each drag
    /// (onGameEdit) and calling again with the new draft; accountId null ends it.
    /// </summary>
    object? EditOnGameCommand(JsonElement a)
    {
        RequireVault();
        Guid account = Guid.TryParse(Str(a, "accountId"), out var g) ? g : Guid.Empty;
        if (account == Guid.Empty)
        {
            if (_overlayFor is { Page: true })
                CloseOverlay(tellPage: false);
            return null;
        }
        Macro draft;
        if (Has(a, "macro", JsonValueKind.Object))
            draft = ReadMacro(a.GetProperty("macro"));
        else
            draft = MacroById(Id(a));
        if (MacroDeskNow.Window(account) is not { } window)
            throw new UserError(L.T("This account's Roblox isn't open."));
        string title = L.F("Editing {0}", draft.Name is { Length: > 0 } name ? name : L.T("New macro"));
        var dots = AreaGeometry.Dots(draft);
        var areas = OverlayAreas(draft);
        if (_overlay is { IsDisposed: false, Kind: GameOverlay.Mode.Edit } open && _overlayFor is { Page: true } on && on.Account == account)
        {
            open.Set(dots, areas, title);
            return null;
        }
        CloseOverlay();
        var overlay = new GameOverlay(window.Handle, GameOverlay.Mode.Edit, title);
        overlay.Set(dots, areas);
        overlay.DotMoved += (step, x, y) => Emit("onGameEdit", new { step, x, y });
        overlay.AreaChanged += (area, rect) => Emit("onGameEdit", new { area, rect });
        overlay.Done += () =>
        {
            if (_overlay == overlay)
                (_overlay, _overlayFor) = (null, null);
            Emit("onGameEdit", new { done = true });
        };
        (_overlay, _overlayFor) = (overlay, (draft.Id, account, true));
        ShowOverlay(overlay, window.Pid); // the game in view under the overlay
        return null;
    }

    /// <summary>"Pick on game": the next click on the account's window (caught by the overlay, nothing reaches the game)
    /// gives its place and colour; Esc gives null.</summary>
    async Task<object?> PickOnGameCommandAsync(JsonElement a)
    {
        RequireVault();
        Guid account = Guid.TryParse(Str(a, "accountId"), out var g) ? g : Guid.Empty;
        if (account == Guid.Empty)
            throw new UserError(L.T("Choose the account whose game to pick on."));
        return await PickOnGameAsync(account) is { } p ? new { x = p.X, y = p.Y, colour = p.Colour } : null;
    }

    internal async Task<(double X, double Y, string Colour)?> PickOnGameAsync(Guid account)
    {
        if (MacroDeskNow.Window(account) is not { } window)
            throw new UserError(L.T("This account's Roblox isn't open."));
        var picture = await MacroPictureOfAsync(account);
        CloseOverlay();
        var picked = new TaskCompletionSource<(double, double, string)?>(TaskCreationOptions.RunContinuationsAsynchronously);
        var overlay = new GameOverlay(window.Handle, GameOverlay.Mode.Pick, L.T("Click the place on the game. Esc cancels."), picture);
        overlay.Picked += (x, y, colour) => picked.TrySetResult((x, y, colour));
        overlay.FormClosed += (_, _) =>
        {
            if (_overlay == overlay)
                (_overlay, _overlayFor) = (null, null);
            picked.TrySetResult(null);
        };
        (_overlay, _overlayFor) = (overlay, (Guid.Empty, account, false));
        ShowOverlay(overlay, window.Pid);
        return await picked.Task;
    }

    /// <summary>A panel's colour "Pick": on the game of its account, or on the screen when no game is open.</summary>
    internal async void PanelPickColour(Guid id, string name, Form panel)
    {
        try
        {
            if (OpenMacro(id) is not { } macro)
                return;
            var account = PanelAccount(macro);
            if (account == Guid.Empty)
            {
                ScreenPicker.Pick(panel, colour =>
                {
                    try
                    {
                        SetOption(id, name, JsonSerializer.SerializeToElement(colour));
                    }
                    catch (UserError ex)
                    {
                        Toast(ex.Message, "warn");
                    }
                });
                return;
            }
            if (await PickOnGameAsync(account) is { } p)
                SetOption(id, name, JsonSerializer.SerializeToElement(p.Colour));
        }
        catch (UserError ex)
        {
            Toast(ex.Message, "warn");
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    /// <summary>A panel's point "Set": a click on the game of its account (a picture of it held still meanwhile).</summary>
    internal async void PanelPickPoint(Guid id, string name)
    {
        try
        {
            if (OpenMacro(id) is not { } macro)
                return;
            var account = PanelAccount(macro);
            if (account == Guid.Empty || MacroDeskNow.Window(account) is null)
            {
                Toast(L.T("Open the game in an account first: the point is set on its window."), "warn");
                return;
            }
            if (await PickOnGameAsync(account) is { } p)
                SetOption(id, name, MacroOptions.Point(p.X, p.Y));
        }
        catch (UserError ex)
        {
            Toast(ex.Message, "warn");
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    internal bool EditingAreasOf(Guid id) => _overlay is { IsDisposed: false, Kind: GameOverlay.Mode.Areas } && _overlayFor is { } f && f.Macro == id;

    /// <summary>The panel's "Edit areas": the macro's areas over its account's window to move and size (saved as they
    /// change); again, Done or Esc ends it.</summary>
    internal void ToggleAreasOnGame(Guid id)
    {
        if (EditingAreasOf(id))
        {
            CloseOverlay();
            RefreshPanel(id);
            return;
        }
        if (OpenMacro(id) is not { } macro || macro.Areas.Count == 0)
            return;
        var account = PanelAccount(macro);
        if (account == Guid.Empty || MacroDeskNow.Window(account) is not { } window)
        {
            Toast(L.T("Open the game in an account first: the areas are moved on its window."), "warn");
            return;
        }
        CloseOverlay();
        var overlay = new GameOverlay(window.Handle, GameOverlay.Mode.Areas, L.F("Areas of {0}", macro.Name));
        overlay.Set([], OverlayAreas(macro));
        overlay.AreaChanged += (area, rect) => SetArea(id, area, rect);
        overlay.Done += () =>
        {
            if (_overlay == overlay)
                (_overlay, _overlayFor) = (null, null);
            RefreshPanel(id);
        };
        (_overlay, _overlayFor) = (overlay, (id, account, false));
        ShowOverlay(overlay, window.Pid);
        RefreshPanel(id);
    }

    /// <summary>The game brought forward in its turn (between a macro's turns), then the overlay over it, taking the
    /// keyboard so Esc and Enter are its own, not the game's.</summary>
    static async void ShowOverlay(GameOverlay overlay, int pid)
    {
        try
        {
            using (await InputTurn.EnterAsync("focus"))
                RobloxProcesses.Focus(pid);
            if (overlay.IsDisposed)
                return;
            overlay.Show();
            overlay.Activate();
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    /// <summary>The overlay goes; the page hears its editing ended unless it asked for that itself.</summary>
    void CloseOverlay(bool tellPage = true)
    {
        var overlay = _overlay;
        var was = _overlayFor;
        (_overlay, _overlayFor) = (null, null);
        if (overlay is null || overlay.IsDisposed)
            return;
        overlay.End();
        if (tellPage && was is { Page: true })
            Emit("onGameEdit", new { done = true });
        if (was is { Page: false } p && p.Macro != Guid.Empty)
            RefreshPanel(p.Macro);
    }

    // ───────────────────────── trying a draft ─────────────────────────

    /// <summary>The editor's "Try it": the macro as it is in the editor, once, on one account; "What it sees" follows it
    /// (macroSeen) until it ends, its last frame saying how.</summary>
    object? MacroTryRunCommand(JsonElement a)
    {
        var vault = RequireVault();
        if (!Has(a, "macro", JsonValueKind.Object))
            throw new UserError(L.T("That isn't a macro."));
        Guid account = Guid.TryParse(Str(a, "accountId"), out var g) ? g : Guid.Empty;
        if (account == Guid.Empty)
            throw new UserError(L.T("Choose the account to try it on."));
        if (!_running.ContainsKey(account))
            throw new UserError(L.T("This account's Roblox isn't open."));
        var draft = ReadMacro(a.GetProperty("macro"));
        // Once, by hand, on this account: nothing of how it starts by itself.
        draft.Hotkey = null;
        draft.Enabled = false;
        draft.Trigger = new MacroTrigger();
        draft.Targets = "selected";
        draft.AccountIds = new();
        if (string.IsNullOrWhiteSpace(draft.Name))
            draft.Name = L.T("New macro");
        MacroRules.Check(draft, vault.Data.Accounts.Select(acc => acc.Id).ToHashSet(), []);
        if (draft.Id == Guid.Empty)
            draft.Id = _tryRun is { } last && OpenMacro(last.Macro) is null ? last.Macro : Guid.NewGuid();
        if (_tryRun is { } previous)
            _macroPlayer?.Stop(previous.Macro, "byUser", previous.Account, trialOnly: true);
        if (_macroPlayer?.Runs.Any(r => !r.Finished && !r.Trial && r.Macro.Id == draft.Id && r.AccountId == account) == true)
            throw new UserError(L.T("It's running on this account already. Stop it first."));
        int queued = MacroPlayerNow.Queue(draft, [account], trial: true);
        _tryRun = (draft.Id, account);
        _watchMacro = draft;
        StartWatch(account, draft.Id, page: true);
        PushState();
        return new { queued, macroId = draft.Id };
    }

    /// <summary>How the try ended, in words; null while it goes on.</summary>
    string? TryEnded()
    {
        if (_tryRun is not { } t)
            return null;
        var run = _macroPlayer?.Runs.LastOrDefault(r => r.Trial && r.Macro.Id == t.Macro && r.AccountId == t.Account);
        if (run is { Finished: false })
            return null;
        return run?.Status switch
        {
            "done" => L.T("Tried: it ran to the end."),
            "failed" => L.F("Tried: it stopped ({0}).", FailText(run.Reason)),
            _ => L.T("Tried: stopped."),
        };
    }

    static string FailText(string? reason) => reason switch
    {
        "timeout" => L.T("a wait ran out of time"),
        "notFound" => L.T("what it looks for wasn't there"),
        "missingValue" => L.T("a setting or area is missing"),
        "windowGone" => L.T("the window closed"),
        "maxTime" => L.T("it ran its longest time"),
        "stuck" => L.T("it got stuck"),
        _ => ReasonText(reason),
    };

    // ───────────────────────── what it sees ─────────────────────────

    System.Windows.Forms.Timer? _watchTimer;
    (Guid Account, Guid Macro, bool Page)? _watching;
    /// <summary>The editor's macro as it is now (not saved yet), looked at in place of the saved one.</summary>
    Macro? _watchMacro;
    bool _watchBusy;
    DateTime _watchReadAt;
    List<MacroWatch.Box> _watchText = new();

    /// <summary>The editor's Watch panel: the page event macroSeen about 5 times a second, on the macro being edited
    /// (macro) or the saved one (macroId). accountId null (or empty) stops it.</summary>
    object? MacroWatchCommand(JsonElement a)
    {
        RequireVault();
        Guid account = Guid.TryParse(Str(a, "accountId"), out var id) ? id : Guid.Empty;
        Guid macroId = Guid.TryParse(Str(a, "macroId"), out var m) ? m : Guid.Empty;
        Macro? edited = null;
        if (account != Guid.Empty && Has(a, "macro", JsonValueKind.Object))
        {
            try
            {
                edited = MacroRules.Read(a.GetProperty("macro"));
            }
            catch (Exception ex) when (ex is UserError or JsonException)
            {
                edited = null; // half-edited: the saved one, if any
            }
        }
        if (account == Guid.Empty || edited is null && macroId == Guid.Empty)
        {
            StopWatch();
            return null;
        }
        if (edited is null)
            MacroById(macroId);
        _tryRun = null;
        _watchMacro = edited;
        StartWatch(account, edited?.Id is { } e && e != Guid.Empty ? e : macroId, page: true);
        return null;
    }

    void StartWatch(Guid account, Guid macro, bool page)
    {
        _watching = (account, macro, page);
        _watchText = new();
        _watchReadAt = DateTime.MinValue;
        if (_watchTimer is null)
        {
            _watchTimer = new System.Windows.Forms.Timer { Interval = 200 };
            _watchTimer.Tick += async (_, _) => await WatchTickAsync();
        }
        _watchTimer.Start();
    }

    void StopWatch()
    {
        _watching = null;
        _watchMacro = null;
        _watchTimer?.Stop();
    }

    async Task WatchTickAsync()
    {
        if (_watchBusy || _watching is not { } w || Open is null)
            return;
        var macro = _watchMacro ?? Open.Data.Settings.Macros.FirstOrDefault(m => m.Id == w.Macro);
        if (MacroDeskNow.Window(w.Account) is not { } window || macro is null)
        {
            if (w.Page)
                Emit("macroSeen", new { accountId = w.Account, image = (string?)null, w = 0, h = 0, boxes = Array.Empty<object>(), numbers = Array.Empty<object>(),
                    note = macro is null ? L.T("That macro isn't there any more.") : L.T("This account's Roblox isn't open.") });
            StopWatch();
            _tryRun = null;
            return;
        }
        string? ended = TryEnded();
        // Between a macro's turns only: a picture shows a minimised window for a moment, as a turn does.
        if (InputTurn.TryEnter("picture") is not { } lease)
            return;
        _watchBusy = true;
        try
        {
            Bitmap? picture;
            using (lease)
                picture = await WindowShot.ClientAsync(window.Handle, shown: false);
            if (picture is null)
                return;
            using (picture)
            {
                var run = _macroPlayer?.Runs.LastOrDefault(r => r.Macro.Id == macro.Id && r.AccountId == w.Account)
                    ?? new MacroRun { Macro = macro, AccountId = w.Account, Jumps = [] };
                var seen = await Task.Run(() => MacroWatch.Look(picture, macro, name => MacroPlayerNow.Value(run, name)));
                // Text is read at most once a second (it takes a while).
                if (DateTime.UtcNow - _watchReadAt > TimeSpan.FromSeconds(1) && MacroWatch.TextSteps(macro).Any())
                {
                    _watchReadAt = DateTime.UtcNow;
                    _watchText = await MacroWatch.ReadTextAsync(picture, macro, MacroDeskNow);
                }
                seen.Boxes.AddRange(_watchText);
                if (_watching is not { } still || still != w)
                    return;
                using var small = MacroWatch.Scaled(picture, 640);
                using var jpeg = new MemoryStream();
                small.Save(jpeg, ImageFormat.Jpeg);
                Emit("macroSeen", new
                {
                    accountId = w.Account,
                    image = "data:image/jpeg;base64," + Convert.ToBase64String(jpeg.ToArray()),
                    w = picture.Width,
                    h = picture.Height,
                    boxes = seen.Boxes,
                    numbers = seen.Numbers,
                    note = ended ?? MacroWatch.Note(macro, seen),
                });
                if (ended is not null)
                {
                    StopWatch();
                    _tryRun = null;
                }
            }
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
        finally
        {
            _watchBusy = false;
        }
    }

    // ───────────────────────── tray ─────────────────────────

    public List<(Guid Id, string Name, bool Shown)> TrayMacros() => Open is null || _screenLocked ? []
        : Open.Data.Settings.Macros.Select(m => (m.Id, m.Name, _panelForms.TryGetValue(m.Id, out var f) && !f.IsDisposed && f.Visible)).ToList();

    /// <summary>"Macros ›": a macro's panel shown or hidden.</summary>
    public void TrayTogglePanel(Guid id)
    {
        if (OpenMacro(id) is not { } macro)
            return;
        if (_panelForms.TryGetValue(id, out var f) && !f.IsDisposed && f.Visible)
            HidePanelWindow(id, byPerson: true);
        else
        {
            _panelsClosed.Remove(id);
            ShowPanelWindow(macro, activate: true);
        }
        PushState();
    }
}
