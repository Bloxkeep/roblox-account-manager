using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// Macros (Beta): each macro's panel (PanelWindow.cs, a page beside the game), shown by itself while an account plays its
/// game; the settings the player changes there ("$o1"); the on-game overlay (OverlayWindow.cs) that edits the macro's
/// points and areas on the Roblox window itself and picks a place or colour there; trying a macro not saved yet; "What it
/// sees" (MacroWatch.cs).
/// </summary>
public sealed partial class AppController
{
    readonly Dictionary<Guid, PanelWindow> _panels = new();
    /// <summary>Panels shown by themselves (their game is played), and those the person closed meanwhile (left closed until no account plays it).</summary>
    readonly HashSet<Guid> _panelsAuto = new(), _panelsClosed = new();
    /// <summary>Each macro's last run that ended with an error, and when the panel first saw it ended ("Stopped: …" for 10 minutes).</summary>
    readonly Dictionary<Guid, (MacroRun Run, DateTime SeenAt)> _lastRun = new();
    /// <summary>The values before "Reset all", for its Undo (6 s).</summary>
    readonly Dictionary<Guid, (Dictionary<string, JsonElement?> Values, DateTime Until)> _undoValues = new();
    /// <summary>The last window in front that wasn't a panel: where the keyboard goes back to.</summary>
    IntPtr _lastForeground;
    System.Windows.Forms.Timer? _panelStatePush, _panelFollowLater;
    OverlayWindow? _overlay;
    /// <summary>What the overlay is on: the macro (empty for a pick), the account, and whether the page asked for it.</summary>
    (Guid Macro, Guid Account, bool Page)? _overlayFor;
    /// <summary>The editor's "Try it" going on: its run's macro id and account.</summary>
    (Guid Macro, Guid Account)? _tryRun;

    const int MaxAutoPanels = 3;
    static readonly TimeSpan StoppedShown = TimeSpan.FromMinutes(10), UndoFor = TimeSpan.FromSeconds(6);

    /// <summary>The vault open now, for the panels.</summary>
    internal Vault? OpenVault => Open;

    Macro? OpenMacro(Guid id) => Open?.Data.Settings.Macros.FirstOrDefault(m => m.Id == id);

    PanelWindow? ShownPanel(Guid id) => _panels.TryGetValue(id, out var p) && !p.IsDisposed && p.Visible ? p : null;

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
    /// <param name="fromPanel">From the panel beside the game: saved and told to the main page a moment later, once for a burst.</param>
    internal object? SetOption(Guid id, string name, JsonElement value, bool fromPanel = false)
    {
        var macro = MacroById(id);
        var option = macro.Options.FirstOrDefault(o => o.Name == name) ?? throw new UserError(L.T("That setting isn't there any more."));
        option.Value = MacroOptions.Normalize(option, value) ?? throw new UserError(L.T("That value doesn't fit the setting."));
        Live(id, copy =>
        {
            if (copy.Options.FirstOrDefault(o => o.Name == name) is { } o)
                o.Value = option.Value;
        });
        OptionsChanged(id, fromPanel);
        return null;
    }

    /// <summary>Saved and shown: at once from the page; from the panel, saved within seconds (on lock and exit at the latest) and
    /// the main page told once a quarter second, so holding + doesn't write the vault and redraw the page forty times a second.</summary>
    void OptionsChanged(Guid id, bool fromPanel)
    {
        RefreshPanel(id);
        if (!fromPanel)
        {
            Persist();
            PushState();
            return;
        }
        PersistSoon();
        if (_panelStatePush is null)
        {
            _panelStatePush = new System.Windows.Forms.Timer { Interval = 250 };
            _panelStatePush.Tick += (_, _) =>
            {
                _panelStatePush.Stop();
                PushState();
            };
        }
        if (!_panelStatePush.Enabled)
            _panelStatePush.Start();
    }

    /// <summary>"Reset all": every setting back to how the macro was made (the values before kept 6 s for Undo).</summary>
    internal void ResetOptions(Guid id, bool fromPanel = false)
    {
        var macro = MacroById(id);
        if (fromPanel)
            _undoValues[id] = (macro.Options.ToDictionary(o => o.Name, o => o.Value), DateTime.UtcNow + UndoFor);
        foreach (var o in macro.Options)
            o.Value = o.Default;
        Live(id, copy =>
        {
            foreach (var o in copy.Options)
                o.Value = o.Default;
        });
        OptionsChanged(id, fromPanel);
    }

    /// <summary>"Undo" after "Reset all": the values as they were, if it is still within its 6 s.</summary>
    void UndoReset(Guid id)
    {
        if (!_undoValues.Remove(id, out var undo) || DateTime.UtcNow > undo.Until || OpenMacro(id) is not { } macro)
            return;
        foreach (var o in macro.Options)
        {
            if (undo.Values.TryGetValue(o.Name, out var v))
                o.Value = v is { } was && MacroOptions.Normalize(o, was) is { } fits ? fits : o.Default;
        }
        Live(id, copy =>
        {
            foreach (var o in copy.Options)
                o.Value = macro.Options.FirstOrDefault(x => x.Name == o.Name)?.Value ?? o.Value;
        });
        OptionsChanged(id, fromPanel: true);
    }

    /// <summary>An area moved on the game from the panel ("Move areas on game"), saved at once.</summary>
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
        PersistSoon();
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
    /// game, else the Roblox window that was in front before the panel, else those it last ran on, else the one window open.</summary>
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
        if (_panels.Values.Any(p => !p.IsDisposed && p.IsHandleCreated && p.Handle == PanelNative.GetForegroundWindow()))
            front = PanelNative.PidOf(_lastForeground); // the panel is in front: the game before it
        var inFront = _running.Where(r => r.Value.Pid == front).Select(r => r.Key).ToList();
        if (inFront.Count > 0)
            return inFront;
        var last = (_macroLastAccounts.GetValueOrDefault(macro.Id) ?? []).Where(_running.ContainsKey).ToList();
        if (last.Count > 0)
            return last;
        return _running.Count == 1 ? _running.Keys.ToList() : [];
    }

    /// <summary>The account the panel works on the game with (pick, edit areas): its first, else any window open.</summary>
    Guid PanelAccount(Macro macro) => PanelAccounts(macro).Concat(_running.Keys).FirstOrDefault();

    /// <summary>A panel's message for its row (a setting's name) or its status line (null); a toast when no panel shows.</summary>
    void PanelError(Guid id, string? name, string text)
    {
        if (ShownPanel(id) is { } panel)
            panel.Error(name, text);
        else
            Toast(text, "warn");
    }

    /// <summary>The panel's Run: plays the macro, or stops it when it runs.</summary>
    internal void PanelRun(Guid id)
    {
        if (OpenMacro(id) is not { } macro)
            return;
        if (_macroPlayer?.Runs.Any(r => !r.Finished && !r.Trial && r.Macro.Id == id) == true)
        {
            _macroPlayer.Stop(id, "byUser");
            PushState();
            RefreshPanel(id);
            return;
        }
        if (_recorder is not null)
        {
            PanelError(id, null, L.T("Runs wait while recording."));
            return;
        }
        var accounts = PanelAccounts(macro);
        if (accounts.Count == 0)
        {
            PanelError(id, null, L.F("{0}: no Roblox window to run it in.", macro.Name));
            return;
        }
        _lastRun.Remove(id);
        QueueMacro(macro, accounts);
        RefreshPanel(id);
    }

    /// <summary>What the panel shows now (the page puts it in words): its state, who runs, the counters, since when, the
    /// last error, and what can be done.</summary>
    (string State, object Dto) PanelNow(Macro macro)
    {
        var id = macro.Id;
        var runs = _macroPlayer?.Runs.Where(r => !r.Finished && !r.Trial && r.Macro.Id == id).ToList() ?? [];
        // The last run that stopped on an error, until the next run or 10 minutes.
        var ended = _macroPlayer?.Runs.Where(r => r.Finished && !r.Trial && r.Macro.Id == id).LastOrDefault();
        if (ended is { Status: "failed" } && (!_lastRun.TryGetValue(id, out var seen) || seen.Run != ended))
            _lastRun[id] = (ended, DateTime.UtcNow);
        else if (ended is { Status: not "failed" } && _lastRun.TryGetValue(id, out var was) && was.Run != ended)
            _lastRun.Remove(id);
        if (runs.Count > 0)
            _lastRun.Remove(id);
        var failed = _lastRun.TryGetValue(id, out var last) && DateTime.UtcNow - last.SeenAt < StoppedShown ? last.Run : null;
        bool hasGame = PanelAccounts(macro).Count > 0;
        string state = _recorder is not null ? "recording"
            : runs.Count > 0 ? (runs.Any(r => r.Status == "running") || runs.All(r => r.Status == "waiting") ? "running" : "paused")
            : failed is not null ? "stopped"
            : hasGame ? "ready" : "noGame";
        var vars = _macroPlayer?.Vars ?? new();
        var counters = MacroOptions.Counters(macro).ToDictionary(c => c, c => vars.Values.Sum(d => d.GetValueOrDefault(c)));
        string Name(Guid account) => FindAccount(account) is { } acc ? NameText(acc) : "?";
        bool changed = macro.Options.Any(o => o.Kind != "heading" && !SameValue(o.Value, o.Default));
        return (state, new
        {
            state,
            runs = runs.Select(r => new
            {
                name = Name(r.AccountId),
                status = r.Status == "running" && r.Reason == "recovering" ? "recovering" : r.Status,
                reason = r.Status == "paused" ? ReasonText(r.Reason) : null,
            }),
            counters,
            startedAt = runs.Count > 0 ? runs.Min(r => r.StartedAt) : (DateTime?)null,
            lastError = failed is null ? null : new { text = FailText(failed.Reason), step = failed.Pc },
            editingAreas = EditingAreasOf(id),
            hotkeysOn = Prefs.GlobalHotkeys,
            hasGame,
            hasWindow = _running.Count > 0,
            recording = _recorder is { } rec ? new { account = Name(rec.AccountId) } : null,
            undoable = _undoValues.TryGetValue(id, out var undo) && DateTime.UtcNow < undo.Until,
            changed,
            features = FeaturesNow(macro, runs),
        });
    }

    /// <summary>
    /// While it runs: each feature switched on, by its switch, as the panel says it under the switch: playing (at its
    /// step), due at this safe point, stopped last time (why), its next time (it plays every so often), or at the end of a
    /// round. The run playing it is the one shown, else the first.
    /// </summary>
    Dictionary<string, object>? FeaturesNow(Macro macro, List<MacroRun> runs)
    {
        if (runs.Count == 0 || _macroPlayer is not { } player || macro.Features.Count == 0)
            return null;
        var now = new Dictionary<string, object>();
        foreach (var f in macro.Features.Where(f => MacroPlayer.SwitchOn(macro, f)))
        {
            var run = runs.FirstOrDefault(r => r.Section == f.Id) ?? runs.FirstOrDefault(r => r.Due.Contains(f.Id)) ?? runs[0];
            var record = player.Features.GetValueOrDefault((macro.Id, run.AccountId, f.Id));
            now[f.Option] = run.Section == f.Id ? new { state = "playing", step = run.Pc + 1, of = f.Steps.Count }
                : run.Due.Contains(f.Id) ? new { state = "due" }
                : record?.Failed is { } failed ? new { state = "failed", why = FailText(failed) }
                : f.EveryMs is { } every && record?.LastRan is { } last && DateTime.UtcNow - last < TimeSpan.FromMilliseconds(every)
                    ? new { state = "next", at = last + TimeSpan.FromMilliseconds(every) }
                : (object)new { state = "round" };
        }
        return now;
    }

    static bool SameValue(JsonElement? a, JsonElement? b) => (a, b) switch
    {
        (null, null) => true,
        ({ } x, { } y) => x.GetRawText() == y.GetRawText(),
        _ => false,
    };

    /// <summary>A counter's name as the panel shows it: the maker's label when there is one (panel.counterLabels).</summary>
    static string CounterLabel(Macro macro, string name) =>
        macro.Panel.Extra is { } extra && extra.TryGetValue("counterLabels", out var labels) && labels.ValueKind == JsonValueKind.Object
        && labels.TryGetProperty(name, out var label) && label.ValueKind == JsonValueKind.String && label.GetString() is { Length: > 0 } text ? text : name;

    /// <summary>The page of a macro's panel: the macro (its settings with their values as shown), what it does now, the
    /// theme and the language.</summary>
    internal object PanelMessage(Guid id, bool pill)
    {
        var macro = OpenMacro(id);
        object? dto = macro is null ? null : new
        {
            id = macro.Id,
            name = macro.Name,
            hotkey = macro.Hotkey,
            tabs = macro.Panel.Tabs,
            areasCount = macro.Areas.Count,
            counters = MacroOptions.Counters(macro).Select(c => new { name = c, label = CounterLabel(macro, c) }),
            options = macro.Options.Select(o => PanelOption(o)),
        };
        return new
        {
            macro = dto,
            now = macro is null ? null : PanelNow(macro).Dto,
            theme = Themes.Current.Id,
            customTheme = CustomThemeDto(),
            language = L.Language,
            pill,
        };
    }

    static object PanelOption(MacroOption o)
    {
        var value = o.Kind == "heading" ? (JsonElement?)null : MacroOptions.ValueOf(o);
        bool number = o.Kind == "number";
        return new
        {
            name = o.Name,
            label = o.Label,
            kind = o.Kind,
            tab = o.Tab,
            value,
            choices = o.Choices,
            shown = value is { } v ? MacroOptions.Show(o, v) : "",
            // A number: its range, step and unit as the page shows them while − and + are held (the host's text replaces it after).
            min = number ? o.Min : null,
            max = number ? o.Max : null,
            step = number ? MacroOptions.Step(o) : (double?)null,
            scale = number ? MacroOptions.ShownScale(o) : (double?)null,
            suffix = number ? MacroOptions.Suffix(o) : null,
            minShown = number && o.Min is { } min ? MacroOptions.Show(o, JsonSerializer.SerializeToElement(min)) : null,
            maxShown = number && o.Max is { } max ? MacroOptions.Show(o, JsonSerializer.SerializeToElement(max)) : null,
        };
    }

    /// <summary>A run's reason as the panel says it (the page has its own wording for these).</summary>
    static string ReasonText(string? reason) => reason switch
    {
        "personActive" => L.T("you're using the PC"),
        "locked" => L.T("the PC is locked"),
        "fullscreen" => L.T("a full-screen app is in front"),
        "focusRefused" => L.T("couldn't bring the window forward"),
        "notInGame" => L.T("not in a game"),
        "covered" => L.T("another window covers the game"),
        "busy" => L.T("another window's turn"),
        "recording" => L.T("recording"),
        "imeOn" => L.T("Vietnamese typing is on"),
        _ => L.T("waiting"),
    };

    // ───────────────────────── what the panel's page does ─────────────────────────

    /// <summary>Something clicked on a panel (PanelWindow's page).</summary>
    internal void PanelAct(PanelWindow panel, string what, JsonElement a)
    {
        var id = panel.MacroId;
        string name = Str(a, "name");
        try
        {
            switch (what)
            {
                case "run" or "stop":
                    PanelRun(id);
                    HandBack(panel);
                    break;
                case "set" when a.TryGetProperty("value", out var value):
                    SetOption(id, name, value.Clone(), fromPanel: true);
                    break;
                case "set" when a.TryGetProperty("key", out var key):
                    if (OpenMacro(id)?.Options.FirstOrDefault(o => o.Name == name) is { } ko && MacroOptions.Normalize(ko, key) is null)
                        PanelError(id, name, L.T("Can't use this key"));
                    else
                        SetOption(id, name, key.Clone(), fromPanel: true);
                    break;
                case "nudge" when OpenMacro(id)?.Options.FirstOrDefault(o => o.Name == name && o.Kind == "number") is { } o:
                {
                    int by = a.TryGetProperty("by", out var b) && b.ValueKind == JsonValueKind.Number ? (int)Math.Clamp(b.GetDouble(), -100000, 100000) : 0;
                    if (by != 0)
                        SetOption(id, name, MacroOptions.Nudge(o, by), fromPanel: true);
                    else
                        RefreshPanel(id);
                    break;
                }
                case "typed" when OpenMacro(id)?.Options.FirstOrDefault(o => o.Name == name && o.Kind == "number") is { } o:
                {
                    string text = Str(a, "text");
                    if (MacroOptions.TypedNumber(o, text) is not { } raw)
                    {
                        RefreshPanel(id);
                        PanelError(id, name, L.T("That isn't a number."));
                        break;
                    }
                    SetOption(id, name, JsonSerializer.SerializeToElement(raw), fromPanel: true);
                    // Kept in its range: said why it isn't what was typed.
                    if (o.Max is { } max && raw > max)
                        PanelError(id, name, L.F("Max is {0}", MacroOptions.Show(o, JsonSerializer.SerializeToElement(max))));
                    else if (o.Min is { } min && raw < min)
                        PanelError(id, name, L.F("Min is {0}", MacroOptions.Show(o, JsonSerializer.SerializeToElement(min))));
                    break;
                }
                case "reset":
                    ResetOptions(id, fromPanel: true);
                    break;
                case "undoReset":
                    UndoReset(id);
                    break;
                case "pickColour":
                    PanelPickColour(id, name, panel);
                    break;
                case "pickPoint":
                    PanelPickPoint(id, name);
                    break;
                case "areas":
                    ToggleAreasOnGame(id);
                    break;
                case "hide":
                    panel.Dismiss();
                    break;
                case "snap":
                    panel.Snap();
                    break;
                case "away":
                    HandBack(panel);
                    break;
                case "edit":
                    int? step = a.TryGetProperty("step", out var s) && s.ValueKind == JsonValueKind.Number ? s.GetInt32() : null;
                    if (_owner is ShellForm shell)
                        shell.BringToFront();
                    Emit("editMacro", new { id, step });
                    break;
            }
        }
        catch (UserError ex)
        {
            RefreshPanel(id);
            PanelError(id, string.IsNullOrEmpty(name) ? null : name, ex.Message);
        }
    }

    // ───────────────────────── panel windows ─────────────────────────

    void ShowPanelWindow(Macro macro, bool activate)
    {
        if (_disposed || Open is null)
            return;
        if (!_panels.TryGetValue(macro.Id, out var panel) || panel.IsDisposed)
        {
            var id = macro.Id;
            panel = new PanelWindow(this, id);
            panel.Dismissed += () =>
            {
                _panelsAuto.Remove(id);
                if (OpenMacro(id) is { Panel.AutoShow: true })
                    _panelsClosed.Add(id);
                if (_overlayFor is { Page: false } f && f.Macro == id)
                    CloseOverlay();
                PushState();
            };
            _panels[id] = panel;
        }
        // Shown by hand counts as by hand from then on (an update waits for it).
        panel.AutoShown = !activate && (panel.AutoShown || !panel.Visible);
        panel.Open(activate);
    }

    void HidePanelWindow(Guid id, bool byPerson)
    {
        if (ShownPanel(id) is { } panel)
            panel.HideQuietly();
        _panelsAuto.Remove(id);
        if (byPerson && OpenMacro(id) is { Panel.AutoShow: true })
            _panelsClosed.Add(id);
        if (_overlayFor is { Page: false } f && f.Macro == id)
            CloseOverlay();
    }

    /// <summary>The macro deleted: its panel goes (disposed: a Close would only hide it and keep its browser), with its place.</summary>
    void ClosePanelWindow(Guid id)
    {
        if (_panels.Remove(id, out var panel) && !panel.IsDisposed)
            panel.Dispose();
        _panelsAuto.Remove(id);
        _panelsClosed.Remove(id);
        _lastRun.Remove(id);
        _undoValues.Remove(id);
        if (_overlayFor is { Page: false } f && f.Macro == id)
            CloseOverlay();
        if (Prefs.PanelWindows.Remove(id.ToString()))
            Prefs.Save();
    }

    /// <summary>The panel shows what is true now (values patched on its page, never drawn anew).</summary>
    void RefreshPanel(Guid id)
    {
        if (ShownPanel(id) is { } panel)
            panel.Push();
    }

    object PanelsShownDto() => Open is null ? Array.Empty<Guid>() : _panels.Where(f => !f.Value.IsDisposed && f.Value.Visible).Select(f => f.Key).ToArray();

    /// <summary>The game window a panel goes beside: its account's, in screen pixels.</summary>
    internal Rectangle? PanelGameRect(Guid id)
    {
        if (OpenMacro(id) is not { } macro || PanelAccount(macro) is var account && account == Guid.Empty)
            return null;
        return MacroDeskNow.Window(account) is { } window && window.Client.Width > 0 ? window.Client : null;
    }

    /// <summary>The other panels showing, where they are (a new one goes below them).</summary>
    internal IReadOnlyList<Rectangle> PanelsShownBesides(PanelWindow self) =>
        _panels.Values.Where(p => p != self && !p.IsDisposed && p.Visible).Select(p => p.Bounds).ToList();

    /// <summary>A panel just showed: on top as the window in front says; shown by itself, the keyboard stays where it was.</summary>
    internal void PanelShown(PanelWindow panel)
    {
        PanelsFollowForeground();
        if (panel.AutoShown)
        {
            // The browser starting can take the activation with it (TrayWindow saw it): given straight back.
            GiveBackIfTaken(panel);
            var later = new System.Windows.Forms.Timer { Interval = 400 };
            later.Tick += (_, _) =>
            {
                later.Dispose();
                if (!panel.IsDisposed)
                    GiveBackIfTaken(panel);
            };
            later.Start();
        }
        PushState();
    }

    void GiveBackIfTaken(PanelWindow panel)
    {
        if (panel.Before == IntPtr.Zero && _lastForeground != IntPtr.Zero)
            panel.Before = _lastForeground;
        HandBack(panel);
    }

    /// <summary>A panel took the keyboard: which window had it before (lParam is another program's only within this one).</summary>
    internal void PanelActivated(PanelWindow panel, IntPtr previous)
    {
        var before = previous != IntPtr.Zero && !IsPanel(previous) ? previous : _lastForeground;
        if (before != IntPtr.Zero && !IsPanel(before))
            panel.Before = before;
    }

    bool IsPanel(IntPtr hwnd) => hwnd != IntPtr.Zero && _panels.Values.Any(p => !p.IsDisposed && p.IsHandleCreated && p.Handle == hwnd);

    /// <summary>A Roblox window of an account Bloxkeep runs, shown and not minimised.</summary>
    bool IsRunningGame(IntPtr hwnd)
    {
        if (hwnd == IntPtr.Zero || !PanelNative.IsWindow(hwnd))
            return false;
        int pid = PanelNative.PidOf(hwnd);
        return pid != 0 && _running.Values.Any(r => r.Pid == pid);
    }

    /// <summary>
    /// The keyboard back to the game after Run or Stop, or once the pointer left the panel after a change: only while the
    /// panel is still in front, only to the running account's Roblox window that was in front before it, never unhiding or
    /// restoring it, and only between a macro's turns (a turn busy: nothing, never queued). Typing a number and "Press a
    /// key…" keep it (the page doesn't ask then).
    /// </summary>
    void HandBack(PanelWindow panel)
    {
        if (panel.IsDisposed || !panel.IsHandleCreated || PanelNative.GetForegroundWindow() != panel.Handle)
            return;
        var target = panel.Before;
        if (!IsRunningGame(target) || !PanelNative.IsWindowVisible(target) || PanelNative.IsIconic(target) || RobloxProcesses.IsHidden(target))
            return;
        using var turn = InputTurn.TryEnter("focus");
        if (turn is null)
            return;
        AntiAfk.GiveBack(target);
    }

    /// <summary>
    /// The panels stay on top while the window in front is a running account's Roblox, Bloxkeep's own (the main window,
    /// the overlay, a panel); when another program comes forward they go just below it, never to the bottom. Called by
    /// the foreground hook (AppController.Cores.cs); a window brought forward for a moment by a macro's turn waits 0.7 s.
    /// </summary>
    void PanelsFollowForeground()
    {
        var front = PanelNative.GetForegroundWindow();
        if (front != IntPtr.Zero && !IsPanel(front))
            _lastForeground = front;
        if (!_panels.Values.Any(p => !p.IsDisposed && p.Visible))
            return;
        if (WindowsInTurn)
        {
            if (_panelFollowLater is null)
            {
                _panelFollowLater = new System.Windows.Forms.Timer { Interval = 700 };
                _panelFollowLater.Tick += (_, _) =>
                {
                    _panelFollowLater.Stop();
                    PanelsFollowForeground();
                };
            }
            _panelFollowLater.Stop();
            _panelFollowLater.Start();
            return;
        }
        // A message box of Bloxkeep's open: the panels stay lowered under it (Ui.Modal), never raised over it again.
        if (Ui.ModalsOpen > 0)
        {
            if (front != IntPtr.Zero && !IsPanel(front))
                foreach (var panel in _panels.Values.Where(p => !p.IsDisposed && p.Visible))
                    panel.Layer(false, front);
            return;
        }
        bool onTop = front == IntPtr.Zero || IsPanel(front) || PanelNative.PidOf(front) == Environment.ProcessId || IsRunningGame(front);
        foreach (var panel in _panels.Values.Where(p => !p.IsDisposed && p.Visible))
            panel.Layer(onTop, front);
    }

    /// <summary>Every process tick: panels shown and hidden by their game, and what they show.</summary>
    void TickPanels()
    {
        if (Open is not { } vault)
            return;
        // No macro and no panel: nothing to show, hide or refresh (the usual case), so no lists built every tick.
        if (vault.Data.Settings.Macros.Count == 0 && _panels.Count == 0 && _panelsAuto.Count == 0)
            return;
        var inGame = _running.Where(r => _logs.StateOf(r.Value.Pid) is { Status: "ingame", PlaceId: > 0 }).ToList();
        var places = inGame.Select(r => _logs.StateOf(r.Value.Pid)!.PlaceId!.Value).ToHashSet();
        var playing = inGame.Select(r => r.Key).ToHashSet();
        var (show, hide) = MacroOptions.AutoShow(vault.Data.Settings.Macros, places, _panelsAuto, _panelsClosed,
            id => ShownPanel(id) is not null, m => PanelAccounts(m).Any(playing.Contains), MaxAutoPanels);
        foreach (var id in hide)
            HidePanelWindow(id, byPerson: false);
        foreach (var id in show)
        {
            if (OpenMacro(id) is { } macro && !(_panels.TryGetValue(id, out var p) && p.Suspended))
                ShowPanelWindow(macro, activate: false);
        }
        if (show.Count > 0 || hide.Count > 0)
            PushState();
        foreach (var panel in _panels.Values.Where(p => !p.IsDisposed && p.Visible))
            panel.Push();
    }

    /// <summary>Settings › Appearance changed: every panel in the new theme at once.</summary>
    internal void ApplyThemeToPanels()
    {
        foreach (var panel in _panels.Values.Where(p => !p.IsDisposed))
            panel.Push();
    }

    /// <summary>The vault locked: every panel goes (disposed, with its browser), with the overlay, the try and the watch.</summary>
    void ClosePanelsForLock()
    {
        foreach (var panel in _panels.Values.Where(p => !p.IsDisposed).ToList())
            panel.Dispose();
        _panels.Clear();
        _panelsAuto.Clear();
        _panelsClosed.Clear();
        _lastRun.Clear();
        _undoValues.Clear();
        _panelStatePush?.Stop();
        _tryRun = null;
        CloseOverlay(tellPage: false);
        StopWatch();
    }

    static class PanelNative
    {
        [DllImport("user32.dll")]
        public static extern IntPtr GetForegroundWindow();

        [DllImport("user32.dll")]
        static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);

        [DllImport("user32.dll")]
        public static extern bool IsWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        public static extern bool IsWindowVisible(IntPtr hWnd);

        [DllImport("user32.dll")]
        public static extern bool IsIconic(IntPtr hWnd);

        public static int PidOf(IntPtr hWnd)
        {
            if (hWnd == IntPtr.Zero)
                return 0;
            GetWindowThreadProcessId(hWnd, out uint pid);
            return (int)pid;
        }
    }

    // ───────────────────────── on the game ─────────────────────────

    static List<OverlayWindow.Area> OverlayAreas(Macro m) => (m.Areas ?? new()).Where(a => a is not null).Select(a => new OverlayWindow.Area(a.Name, a.Label is { Length: > 0 } label ? label : a.Name,
        ColorTranslator.FromHtml(a.Colour is { Length: 7 } c && c[0] == '#' && c[1..].All(Uri.IsHexDigit) ? c : MacroOptions.AreaColour),
        MacroOptions.Rect(a.Rect) ?? new MacroRect { X = 0.25, Y = 0.25, W = 0.5, H = 0.5 })).ToList();

    /// <summary>The steps' own areas (where a picture, words or a colour are looked for, written on the step rather than
    /// named in Areas), named "step:N" or "test:N" for the step's own or its If's; amber, to tell them from named areas.</summary>
    static List<OverlayWindow.Area> StepAreas(Macro m)
    {
        var list = new List<OverlayWindow.Area>();
        for (int i = 0; i < m.Steps.Count; i++)
        {
            var s = m.Steps[i];
            if (MacroOptions.Rect(s?.Region) is { } own)
                list.Add(new($"step:{i}", L.F("Step {0}", i + 1), ColorTranslator.FromHtml("#f59e0b"), own));
            if (MacroOptions.Rect(s?.Test?.Region) is { } test)
                list.Add(new($"test:{i}", L.F("Step {0}", i + 1), ColorTranslator.FromHtml("#f59e0b"), test));
        }
        return list;
    }

    /// <summary>"step:N" / "test:N" of <see cref="StepAreas"/>: the step and whether it is its If's area.</summary>
    static (int Step, bool Test)? StepAreaOf(string name) =>
        name.Split(':') is [var kind and ("step" or "test"), var n] && int.TryParse(n, out int i) ? (i, kind == "test") : null;

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
        string title = draft.Name ?? ""; // the page says "Editing …" (or "New macro")
        var dots = AreaGeometry.Dots(draft);
        var areas = OverlayAreas(draft).Concat(StepAreas(draft)).ToList();
        if (_overlay is { IsDisposed: false, Kind: OverlayWindow.Mode.Edit } open && _overlayFor is { Page: true } on && on.Account == account)
        {
            open.Set(dots, areas, title);
            return null;
        }
        CloseOverlay();
        var overlay = new OverlayWindow(window.Handle, OverlayWindow.Mode.Edit, title) { CustomTheme = CustomThemeDto() };
        overlay.Set(dots, areas);
        overlay.DotMoved += (step, x, y) => Emit("onGameEdit", new { step, x, y });
        overlay.AreaChanged += (area, rect) =>
        {
            if (StepAreaOf(area) is { } at)
                Emit("onGameEdit", new { region = at.Step, test = at.Test, rect });
            else
                Emit("onGameEdit", new { area, rect });
        };
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
        var overlay = new OverlayWindow(window.Handle, OverlayWindow.Mode.Pick, "", picture) { CustomTheme = CustomThemeDto() };
        overlay.Picked += (x, y, colour) => picked.TrySetResult((x, y, colour));
        void Gone()
        {
            if (_overlay == overlay)
                (_overlay, _overlayFor) = (null, null);
            picked.TrySetResult(null);
        }
        overlay.FormClosed += (_, _) => Gone();
        // Ended before it had a window (waiting for a macro's turn, picked again, the vault locked): WinForms then
        // disposes it without FormClosed, and the pick would never answer.
        overlay.Disposed += (_, _) => Gone();
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
                        SetOption(id, name, JsonSerializer.SerializeToElement(colour), fromPanel: true);
                    }
                    catch (UserError ex)
                    {
                        PanelError(id, name, ex.Message);
                    }
                }, CustomThemeDto());
                return;
            }
            if (await PickOnGameAsync(account) is { } p)
                SetOption(id, name, JsonSerializer.SerializeToElement(p.Colour), fromPanel: true);
        }
        catch (UserError ex)
        {
            PanelError(id, name, ex.Message);
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    /// <summary>A panel's "Pick spot": a click on the game of its account (a picture of it held still meanwhile).</summary>
    internal async void PanelPickPoint(Guid id, string name)
    {
        try
        {
            if (OpenMacro(id) is not { } macro)
                return;
            var account = PanelAccount(macro);
            if (account == Guid.Empty || MacroDeskNow.Window(account) is null)
            {
                PanelError(id, name, L.T("Open the game in an account first"));
                return;
            }
            if (await PickOnGameAsync(account) is { } p)
                SetOption(id, name, MacroOptions.Point(p.X, p.Y), fromPanel: true);
        }
        catch (UserError ex)
        {
            PanelError(id, name, ex.Message);
        }
        catch (Exception ex)
        {
            Log.Write(ex);
        }
    }

    internal bool EditingAreasOf(Guid id) => _overlay is { IsDisposed: false, Kind: OverlayWindow.Mode.Areas } && _overlayFor is { } f && f.Macro == id;

    /// <summary>The panel's "Move areas on game": the macro's areas over its account's window to move and size (saved as they
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
            PanelError(id, null, L.T("Open the game in an account first"));
            return;
        }
        CloseOverlay();
        var overlay = new OverlayWindow(window.Handle, OverlayWindow.Mode.Areas, macro.Name) { CustomTheme = CustomThemeDto() };
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

    /// <summary>The game brought forward in its turn (between a macro's turns), then the overlay over it once its page
    /// has started, taking the keyboard so Esc and Enter are its own, not the game's.</summary>
    static async void ShowOverlay(OverlayWindow overlay, int pid)
    {
        try
        {
            using (await InputTurn.EnterAsync("focus"))
                RobloxProcesses.Focus(pid);
            if (overlay.IsDisposed)
                return;
            await overlay.OpenAsync();
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
        if (_macroPlayer?.BusyWith(account, draft.Id) is { } other)
            throw new UserError(L.F("{0} is running on this account. Stop it first.", other.Name));
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

    public List<(Guid Id, string Name, bool Shown, string State)> TrayMacros() => Open is null || _screenLocked ? []
        : Open.Data.Settings.Macros.Select(m => (m.Id, m.Name, ShownPanel(m.Id) is not null, PanelNow(m).State)).ToList();

    /// <summary>"Macros ›": a macro's panel shown or hidden.</summary>
    public void TrayTogglePanel(Guid id)
    {
        if (OpenMacro(id) is not { } macro)
            return;
        if (ShownPanel(id) is not null)
            HidePanelWindow(id, byPerson: true);
        else
        {
            _panelsClosed.Remove(id);
            ShowPanelWindow(macro, activate: true);
        }
        PushState();
    }
}
