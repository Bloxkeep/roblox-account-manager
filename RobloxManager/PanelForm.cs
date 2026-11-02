using System.Globalization;
using System.Text.Json;

namespace RobloxManager;

/// <summary>
/// A macro's panel (MacroPanel.cs): a small always-on-top window next to the game, plain WinForms in the app's theme like
/// <see cref="MiniForm"/>, made from the macro itself; nobody designs it. A big Run/Stop with the macro's hotkey, one
/// status line (who runs or waits, the counters, the run time), then the settings the maker let the player change, in
/// order: a number with − and + (typed when its value is clicked), a colour with Pick (on the game), a switch, a key with
/// Change, a choice from our own list. "Edit areas" when the macro has areas (moved and sized on the game), "Reset" puts
/// the settings back. Every value saves as it changes. Minimise folds it to a small pill; ✕ or Esc hides it. Every
/// button is reached with Tab (a focus ring shows where). Its place is kept per macro in <see cref="Prefs.PanelWindows"/>.
/// </summary>
sealed class PanelForm : Form
{
    const int Width0 = 300, Pad = 10, RowH = 32, Gap = 6, RunH = 44, PillW = 260, PillH = 44, MaxH = 640;
    const int WM_SYSCOMMAND = 0x0112, SC_MINIMIZE = 0xF020;

    readonly AppController _app;
    readonly Panel _body = new BufferedPanel { Dock = DockStyle.Fill, AutoScroll = true, TabStop = false };
    readonly Panel _pill = new() { Dock = DockStyle.Fill, Visible = false, TabStop = false };
    readonly Label _pillText = new() { AutoEllipsis = true, TextAlign = ContentAlignment.MiddleLeft, UseMnemonic = false };
    readonly PanelButton _pillRun = new(), _pillOpen = new();
    readonly System.Windows.Forms.Timer _clock = new() { Interval = 1000 };
    readonly ToolTip _tip = new();
    Themes.Theme _theme = Themes.Current;
    PanelButton? _run;
    Label? _status;
    bool _activate = true, _pillMode;
    /// <summary>A key setting waiting for its new key.</summary>
    (string Name, PanelButton Button)? _capturing;
    /// <summary>The tab shown (when the macro has tabs); null: the first.</summary>
    string? _tab;
    Size _beforePill;

    public Guid MacroId { get; }
    public event Action? Dismissed;

    public PanelForm(AppController app, Guid macroId)
    {
        _app = app;
        MacroId = macroId;
        Icon = Ui.AppIcon;
        Font = new Font("Segoe UI", 9.5f);
        FormBorderStyle = FormBorderStyle.FixedSingle;
        MaximizeBox = false;
        MinimizeBox = true; // folds it to the pill
        ShowInTaskbar = false;
        TopMost = true;
        KeyPreview = true;
        StartPosition = FormStartPosition.Manual;
        ClientSize = new Size(Px(Width0), Px(200));
        Ui.NoteOnTop(this);
        _pill.Controls.AddRange([_pillText, _pillRun, _pillOpen]);
        _pillRun.Click += (_, _) => _app.PanelRun(MacroId);
        _pillOpen.Text = "▴";
        _pillOpen.AccessibleName = L.T("Show the panel");
        _pillOpen.Click += (_, _) => SetPill(false);
        _pill.Resize += (_, _) => LayoutPill();
        Controls.AddRange([_body, _pill]);
        _clock.Tick += (_, _) => Tick();
    }

    int Px(int logical) => LogicalToDeviceUnits(logical);

    Macro? Macro => _app.OpenVault?.Data.Settings.Macros.FirstOrDefault(m => m.Id == MacroId);

    /// <param name="activate">false when shown by itself (its game started): the game keeps the keyboard.</param>
    public void Open(bool activate)
    {
        _activate = activate;
        Reload(rebuild: true);
        if (!Visible)
            Show();
        else if (activate)
            Activate();
    }

    protected override bool ShowWithoutActivation => !_activate;

    /// <summary>Hidden by the app (its game ended, the macro deleted): not counted as the person closing it.</summary>
    public void HideQuietly()
    {
        SavePlace();
        Hide();
    }

    void Dismiss()
    {
        SavePlace();
        Hide();
        Dismissed?.Invoke();
    }

    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        if (e.CloseReason == CloseReason.UserClosing)
        {
            e.Cancel = true;
            Dismiss();
            return;
        }
        SavePlace();
        base.OnFormClosing(e);
    }

    protected override void WndProc(ref Message m)
    {
        // Minimise: the pill, not the taskbar (it has no button there).
        if (m.Msg == WM_SYSCOMMAND && ((int)m.WParam & 0xFFF0) == SC_MINIMIZE)
        {
            SetPill(true);
            return;
        }
        base.WndProc(ref m);
    }

    protected override void OnVisibleChanged(EventArgs e)
    {
        base.OnVisibleChanged(e);
        if (Visible)
            _clock.Start();
        else
            _clock.Stop();
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Ui.UseThemedTitleBar(this, _theme);
    }

    protected override void OnLoad(EventArgs e)
    {
        base.OnLoad(e);
        var area = (Screen.PrimaryScreen ?? Screen.AllScreens[0]).WorkingArea;
        if (Prefs.PanelWindows.TryGetValue(MacroId.ToString(), out var saved) && Parse(saved) is { } at
            && Screen.AllScreens.Any(s => s.WorkingArea.IntersectsWith(new Rectangle(at.X + Width / 4, at.Y + 2, Math.Max(1, Width / 2), Px(24)))))
            Location = Ui.Inside(new Rectangle(at, Size), Screen.FromPoint(at).WorkingArea, 8);
        else
            Location = new Point(area.Right - Width - Px(16), area.Top + Px(80));
    }

    static Point? Parse(string text)
    {
        var p = text.Split(',');
        return p.Length >= 2 && int.TryParse(p[0], out int x) && int.TryParse(p[1], out int y) ? new Point(x, y) : null;
    }

    void SavePlace()
    {
        if (!Visible || WindowState != FormWindowState.Normal)
            return;
        Prefs.PanelWindows[MacroId.ToString()] = string.Create(CultureInfo.InvariantCulture, $"{Left},{Top}");
        Prefs.Save();
    }

    protected override void OnMove(EventArgs e)
    {
        base.OnMove(e);
        if (Visible && !_pillMode)
            _moved = true;
    }

    bool _moved;

    protected override void OnResizeEnd(EventArgs e)
    {
        base.OnResizeEnd(e);
        if (_moved)
            SavePlace();
        _moved = false;
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (_capturing is { } cap)
        {
            e.Handled = e.SuppressKeyPress = true;
            _capturing = null;
            if (e.KeyCode == Keys.Escape)
            {
                Build();
                return;
            }
            if (MacroKeys.FromVk((ushort)e.KeyCode) is not { } key)
            {
                cap.Button.Text = L.T("Not that key: try another");
                _capturing = cap;
                return;
            }
            Try(() => _app.SetOption(MacroId, cap.Name, JsonSerializer.SerializeToElement(key.Name)));
            return;
        }
        if (e.KeyCode == Keys.Escape)
        {
            e.Handled = true;
            Dismiss();
            return;
        }
        base.OnKeyDown(e);
    }

    protected override void OnKeyUp(KeyEventArgs e)
    {
        // Alt pressed alone in "Change" would open the window menu.
        if (_capturing is not null)
            e.Handled = e.SuppressKeyPress = true;
        base.OnKeyUp(e);
    }

    protected override bool ProcessDialogKey(Keys keyData)
    {
        // Tab, the arrows and Alt are keys to take while "Change" waits for one.
        if (_capturing is not null && (keyData & Keys.KeyCode) is Keys.Tab or Keys.Up or Keys.Down or Keys.Left or Keys.Right or Keys.Menu)
        {
            OnKeyDown(new KeyEventArgs(keyData));
            return true;
        }
        return base.ProcessDialogKey(keyData);
    }

    /// <summary>A change refused: said in the status line, the panel drawn as it is.</summary>
    void Try(Action change)
    {
        try
        {
            change();
        }
        catch (UserError ex)
        {
            Build();
            if (_status is not null)
            {
                _status.Text = ex.Message;
                _status.ForeColor = _theme.Bad;
            }
        }
    }

    // ───────────────────────── building ─────────────────────────

    public void Reload(bool rebuild)
    {
        if (Macro is not { } macro)
        {
            HideQuietly();
            return;
        }
        Text = macro.Name + "  ·  Beta";
        if (rebuild && _capturing is null)
            Build();
        Tick();
    }

    void Build()
    {
        if (Macro is not { } macro || _pillMode)
            return;
        _theme = Themes.Current;
        BackColor = _body.BackColor = _pill.BackColor = _theme.Background;
        ForeColor = _theme.Text;
        _pillText.ForeColor = _theme.Text;
        _pillRun.Apply(_theme);
        _pillOpen.Apply(_theme);
        if (IsHandleCreated)
            Ui.UseThemedTitleBar(this, _theme);
        SuspendLayout();
        foreach (Control c in _body.Controls.Cast<Control>().ToList())
            c.Dispose();
        _body.Controls.Clear();
        int tab = 0, width = Px(Width0), inner = width - Px(Pad * 2), y = Px(Pad);
        _run = Button("", () => _app.PanelRun(MacroId), tab++);
        _run.Font = new Font(Font.FontFamily, 11f, FontStyle.Bold);
        _run.Bounds = new Rectangle(Px(Pad), y, inner, Px(RunH));
        _body.Controls.Add(_run);
        y = _run.Bottom + Px(Gap);
        _status = new Label
        {
            Bounds = new Rectangle(Px(Pad), y, inner, Px(36)), ForeColor = _theme.Text2, AutoEllipsis = true, UseMnemonic = false,
            TextAlign = ContentAlignment.TopLeft,
        };
        _body.Controls.Add(_status);
        y = _status.Bottom + Px(Gap);
        var tabs = macro.Panel.Tabs;
        if (tabs.Count >= 2)
        {
            // The tabs, in rows of up to three: each shows its own settings.
            if (_tab is not null && !tabs.Contains(_tab))
                _tab = null;
            int perRow = Math.Min(3, tabs.Count), tw = (inner - Px(Gap) * (perRow - 1)) / perRow;
            for (int i = 0; i < tabs.Count; i++)
            {
                string name = tabs[i];
                var b = Button(name, () => { _tab = name; Build(); }, tab++);
                b.Selected = name == (_tab ?? tabs[0]);
                b.AccessibleName = L.F("Tab {0}", name);
                b.Bounds = new Rectangle(Px(Pad) + i % perRow * (tw + Px(Gap)), y + i / perRow * (Px(28) + Px(4)), tw, Px(28));
                _body.Controls.Add(b);
            }
            y += (tabs.Count + perRow - 1) / perRow * (Px(28) + Px(4)) + Px(Gap);
        }
        foreach (var option in MacroOptions.OnTab(macro, _tab))
        {
            if (option.Kind == "heading")
            {
                // A title over the settings below it, a little apart from the ones above.
                var title = Words(option.Label, new Rectangle(Px(Pad), y + Px(4), inner, Px(22)), _theme.Text2);
                title.Font = new Font(Font, FontStyle.Bold);
                _body.Controls.Add(title);
                y = title.Bottom + Px(2);
                continue;
            }
            var row = Row(option, new Rectangle(Px(Pad), y, inner, Px(RowH)), ref tab);
            _body.Controls.Add(row);
            y = row.Bottom + Px(Gap);
        }
        var bottom = new List<PanelButton>();
        if (macro.Areas.Count > 0)
            bottom.Add(Button(_app.EditingAreasOf(MacroId) ? L.T("Done") : L.T("Edit areas"), () => _app.ToggleAreasOnGame(MacroId), tab++));
        if (macro.Options.Count > 0)
        {
            var reset = Button(L.T("Reset"), () => Try(() => _app.ResetOptions(MacroId)), tab++);
            reset.AccessibleName = L.T("Put the settings back as they were made");
            _tip.SetToolTip(reset, reset.AccessibleName);
            bottom.Add(reset);
        }
        if (bottom.Count > 0)
        {
            int w = (inner - Px(Gap) * (bottom.Count - 1)) / bottom.Count;
            for (int i = 0; i < bottom.Count; i++)
            {
                bottom[i].Bounds = new Rectangle(Px(Pad) + i * (w + Px(Gap)), y + Px(2), w, Px(30));
                _body.Controls.Add(bottom[i]);
            }
            y += Px(30) + Px(Gap);
        }
        int height = y + Px(Pad - Gap);
        ClientSize = new Size(width + (height > Px(MaxH) ? SystemInformation.VerticalScrollBarWidth : 0), Math.Min(height, Px(MaxH)));
        _body.AutoScrollMinSize = new Size(0, height);
        ResumeLayout();
        Tick();
    }

    /// <summary>One setting's row: its label on the left, its control on the right.</summary>
    Control Row(MacroOption o, Rectangle r, ref int tab)
    {
        var holder = new Panel { Bounds = r, BackColor = _theme.Background, TabStop = false };
        int labelW = (int)(r.Width * 0.45);
        var label = Words(o.Label, new Rectangle(0, 0, labelW - Px(6), r.Height), _theme.Text);
        holder.Controls.Add(label);
        var place = new Rectangle(labelW, Px(2), r.Width - labelW, r.Height - Px(4));
        var value = MacroOptions.ValueOf(o);
        void Set(JsonElement v) => Try(() => _app.SetOption(MacroId, o.Name, v));
        switch (o.Kind)
        {
            case "number":
            {
                int bw = Px(30);
                var minus = Button("−", () => Set(MacroOptions.Nudge(o, -1)), tab++);
                var shown = Button(MacroOptions.Show(o, value), () => { }, tab++);
                var plus = Button("+", () => Set(MacroOptions.Nudge(o, 1)), tab++);
                minus.AccessibleName = L.F("Less {0}", o.Label);
                plus.AccessibleName = L.F("More {0}", o.Label);
                shown.AccessibleName = L.F("{0}: {1}. Press to type it.", o.Label, shown.Text);
                minus.Bounds = new Rectangle(place.X, place.Y, bw, place.Height);
                plus.Bounds = new Rectangle(place.Right - bw, place.Y, bw, place.Height);
                shown.Bounds = new Rectangle(minus.Right + Px(3), place.Y, plus.Left - minus.Right - Px(6), place.Height);
                shown.Click += (_, _) => TypeNumber(holder, shown, o);
                holder.Controls.AddRange([minus, shown, plus]);
                break;
            }
            case "colour":
            {
                string colour = value.ValueKind == JsonValueKind.String ? value.GetString()! : "#ffffff";
                var swatch = Button(colour, () => { }, tab);
                swatch.Tint = ColorTranslator.FromHtml(colour);
                swatch.TabStop = false;
                var pick = Button(L.T("Pick"), () => _app.PanelPickColour(MacroId, o.Name, this), tab++);
                pick.AccessibleName = L.F("Pick {0} on the game", o.Label);
                int half = place.Width / 2;
                swatch.Bounds = new Rectangle(place.X, place.Y, half - Px(2), place.Height);
                pick.Bounds = new Rectangle(place.X + half + Px(2), place.Y, place.Width - half - Px(2), place.Height);
                holder.Controls.AddRange([swatch, pick]);
                break;
            }
            case "check":
            {
                bool on = value.ValueKind == JsonValueKind.Number && value.GetDouble() != 0;
                var toggle = new PanelSwitch { On = on, TabIndex = tab++, AccessibleName = o.Label };
                toggle.Apply(_theme);
                int sw = Px(44);
                toggle.Bounds = new Rectangle(place.Right - sw, place.Y + (place.Height - Px(22)) / 2, sw, Px(22));
                toggle.Toggled += now => Set(JsonSerializer.SerializeToElement(now ? 1 : 0));
                // The label switches it too.
                label.Cursor = Cursors.Hand;
                label.Click += (_, _) => toggle.Flip();
                holder.Controls.Add(toggle);
                break;
            }
            case "key":
            {
                string key = value.ValueKind == JsonValueKind.String ? value.GetString()! : "";
                PanelButton change = null!;
                change = Button(L.T("Change"), () =>
                {
                    _capturing = (o.Name, change);
                    change.Text = L.T("Press a key…");
                    change.Focus();
                }, tab++);
                change.AccessibleName = L.F("Change the key of {0}", o.Label);
                var shown = Words(key, Rectangle.Empty, _theme.Text);
                shown.Font = new Font(Font, FontStyle.Bold);
                int cw = Px(84);
                shown.Bounds = new Rectangle(place.X, 0, place.Width - cw - Px(4), r.Height);
                change.Bounds = new Rectangle(place.Right - cw, place.Y, cw, place.Height);
                holder.Controls.AddRange([shown, change]);
                break;
            }
            case "point":
            {
                var shown = Words(MacroOptions.Show(o, value), Rectangle.Empty, _theme.Text2);
                var set = Button(L.T("Set"), () => _app.PanelPickPoint(MacroId, o.Name), tab++);
                set.AccessibleName = L.F("Set {0} on the game", o.Label);
                _tip.SetToolTip(set, set.AccessibleName);
                int cw = Px(84);
                shown.Bounds = new Rectangle(place.X, 0, place.Width - cw - Px(4), r.Height);
                set.Bounds = new Rectangle(place.Right - cw, place.Y, cw, place.Height);
                holder.Controls.AddRange([shown, set]);
                break;
            }
            case "choice":
            {
                string now = value.ValueKind == JsonValueKind.String ? value.GetString()! : "";
                var pick = Button(now + "  ▾", () => PopUp((o.Choices ?? new()).Select(c => (c, c == now, (Action)(() => Set(JsonSerializer.SerializeToElement(c))))).ToList()), tab++);
                pick.AccessibleName = o.Label + ": " + now;
                pick.Bounds = place;
                holder.Controls.Add(pick);
                break;
            }
            case "list":
            {
                var ticked = new HashSet<string>(value.ValueKind == JsonValueKind.Array ? value.EnumerateArray().Select(w => w.GetString() ?? "") : [],
                    StringComparer.OrdinalIgnoreCase);
                string shown = MacroOptions.Show(o, value);
                var pick = Button(shown + "  ▾", () => TickList(o, ticked), tab++);
                pick.AccessibleName = o.Label + ": " + shown;
                pick.Bounds = place;
                holder.Controls.Add(pick);
                break;
            }
        }
        return holder;
    }

    /// <summary>The number's value turned into a box to type it in (in seconds for seconds); Enter or leaving keeps it, Esc doesn't.</summary>
    void TypeNumber(Panel holder, PanelButton shown, MacroOption o)
    {
        var box = new TextBox
        {
            Text = MacroOptions.Show(o, MacroOptions.ValueOf(o)), BorderStyle = BorderStyle.FixedSingle, BackColor = _theme.Surface, ForeColor = _theme.Text,
            TextAlign = HorizontalAlignment.Center, Bounds = shown.Bounds, AccessibleName = o.Label,
        };
        bool done = false;
        void Finish(bool keep)
        {
            if (done)
                return;
            done = true;
            if (keep && MacroOptions.Typed(o, box.Text) is { } v)
                Try(() => _app.SetOption(MacroId, o.Name, v));
            else
                BeginInvoke(Build);
        }
        box.KeyDown += (_, e) =>
        {
            if (e.KeyCode is Keys.Enter or Keys.Escape)
            {
                e.Handled = e.SuppressKeyPress = true;
                Finish(e.KeyCode == Keys.Enter);
            }
        };
        box.Leave += (_, _) => Finish(true);
        shown.Visible = false;
        holder.Controls.Add(box);
        box.BringToFront();
        box.Focus();
        box.SelectAll();
    }

    Label Words(string text, Rectangle r, Color colour) => new()
    {
        Text = text, Bounds = r, ForeColor = colour, AutoEllipsis = true, UseMnemonic = false, TextAlign = ContentAlignment.MiddleLeft,
    };

    PanelButton Button(string text, Action click, int tabIndex)
    {
        var b = new PanelButton { Text = text, TabIndex = tabIndex };
        b.Apply(_theme);
        b.Click += (_, _) => click();
        return b;
    }

    /// <summary>A list drawn by us under the pointer (no system drop-down), in the theme like the game monitor's menu.</summary>
    void PopUp(List<(string Text, bool Now, Action Run)> choices)
    {
        var menu = new ContextMenuStrip { ShowImageMargin = false, ShowCheckMargin = true, Font = Font };
        menu.Renderer = new ThemedMenu.Renderer(_theme, ThemedMenu.RoundCorners(menu.Handle, _theme));
        foreach (var (text, now, run) in choices)
        {
            var item = new ToolStripMenuItem(text) { Padding = new Padding(4, 4, 20, 4), Checked = now };
            item.Click += (_, _) => BeginInvoke(run);
            menu.Items.Add(item);
        }
        menu.Closed += (_, _) => BeginInvoke(menu.Dispose);
        menu.Show(Cursor.Position);
    }

    /// <summary>A list's words to tick, in a menu that stays open while ticking; kept when it closes.</summary>
    void TickList(MacroOption o, HashSet<string> ticked)
    {
        var all = o.Choices ?? new();
        var menu = new ContextMenuStrip { ShowImageMargin = false, ShowCheckMargin = true, Font = Font };
        menu.Renderer = new ThemedMenu.Renderer(_theme, ThemedMenu.RoundCorners(menu.Handle, _theme));
        var items = all.Select(c => new ToolStripMenuItem(c) { Padding = new Padding(4, 3, 20, 3), Checked = ticked.Contains(c), CheckOnClick = true }).ToList();
        foreach (var item in items)
            item.CheckedChanged += (_, _) => { if (item.Checked) ticked.Add(item.Text!); else ticked.Remove(item.Text!); };
        var every = new ToolStripMenuItem(L.T("Tick all")) { Padding = new Padding(4, 3, 20, 3) };
        every.Click += (_, _) => items.ForEach(i => i.Checked = true);
        var none = new ToolStripMenuItem(L.T("Untick all")) { Padding = new Padding(4, 3, 20, 3) };
        none.Click += (_, _) => items.ForEach(i => i.Checked = false);
        menu.Items.AddRange([every, none, new ToolStripSeparator(), .. items]);
        menu.Closing += (_, e) =>
        {
            if (e.CloseReason == ToolStripDropDownCloseReason.ItemClicked)
                e.Cancel = true; // ticking goes on
        };
        menu.Closed += (_, _) =>
        {
            Try(() => _app.SetOption(MacroId, o.Name, JsonSerializer.SerializeToElement(all.Where(ticked.Contains).ToList())));
            BeginInvoke(menu.Dispose);
        };
        menu.Show(Cursor.Position);
    }

    // ───────────────────────── live ─────────────────────────

    public void Tick()
    {
        if (!Visible || Macro is not { } macro || _app.PanelStatus(MacroId) is not { } now)
            return;
        string hotkey = macro.Hotkey is { } digit ? $"   (Ctrl+Alt+{digit})" : "";
        string run = (now.Running ? L.T("Stop") : L.T("Run")) + hotkey;
        if (_run is not null)
        {
            _run.Text = run;
            _run.Selected = now.Running;
        }
        if (_status is not null)
        {
            if (_status.ForeColor == _theme.Bad && _status.Text != now.Line)
                _status.ForeColor = _theme.Text2; // a refusal is shown one tick
            else
                _status.Text = now.Line;
            _tip.SetToolTip(_status, now.Line);
        }
        _pillRun.Text = now.Running ? L.T("Stop") : L.T("Run");
        _pillText.Text = macro.Name + " · " + now.Line;
    }

    /// <summary>Folded to a small pill (name, status, Run/Stop, ▴), or back.</summary>
    void SetPill(bool on)
    {
        if (on == _pillMode)
            return;
        _pillMode = on;
        if (on)
        {
            SavePlace();
            _beforePill = ClientSize;
            _body.Visible = false;
            _pill.Visible = true;
            ClientSize = new Size(Px(PillW), Px(PillH));
            Tick();
        }
        else
        {
            _pill.Visible = false;
            _body.Visible = true;
            if (_beforePill.Width > 0)
                ClientSize = _beforePill;
            Build();
        }
    }

    void LayoutPill()
    {
        int h = _pill.ClientSize.Height, w = _pill.ClientSize.Width, bw = Px(56), ow = Px(30);
        _pillText.Bounds = new Rectangle(Px(8), 0, w - bw - ow - Px(20), h);
        _pillRun.Bounds = new Rectangle(w - bw - ow - Px(8), Px(6), bw, h - Px(12));
        _pillOpen.Bounds = new Rectangle(w - ow - Px(4), Px(6), ow, h - Px(12));
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _clock.Dispose();
            _tip.Dispose();
        }
        base.Dispose(disposing);
    }

    sealed class BufferedPanel : Panel
    {
        public BufferedPanel() => DoubleBuffered = true;
    }
}

/// <summary>A panel's button: flat in the theme, an optional tint, a "selected" look (Stop while it runs), and a focus
/// ring that shows where Tab is.</summary>
sealed class PanelButton : Button
{
    Themes.Theme? _theme;
    Color? _tint;
    bool _selected;

    public PanelButton()
    {
        FlatStyle = FlatStyle.Flat;
        FlatAppearance.BorderSize = 1;
        UseVisualStyleBackColor = false;
        UseMnemonic = false;
        Cursor = Cursors.Hand;
        AutoEllipsis = true;
    }

    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Color? Tint
    {
        get => _tint;
        set
        {
            _tint = value;
            if (_theme is not null)
                Apply(_theme);
        }
    }

    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public bool Selected
    {
        get => _selected;
        set
        {
            if (_selected == value)
                return;
            _selected = value;
            if (_theme is not null)
                Apply(_theme);
        }
    }

    public void Apply(Themes.Theme theme)
    {
        _theme = theme;
        var back = _tint ?? (_selected ? Drawn.Accent(theme) : theme.Surface);
        BackColor = back;
        // Text readable on its colour.
        ForeColor = _tint is { } t ? (t.GetBrightness() > .55f ? Color.Black : Color.White) : _selected ? Drawn.AccentInk(theme) : theme.Text;
        FlatAppearance.BorderColor = theme.Line;
        FlatAppearance.MouseOverBackColor = Drawn.Mix(back, theme.Text, theme.Dark ? .1f : .07f);
        FlatAppearance.MouseDownBackColor = Drawn.Mix(back, theme.Text, theme.Dark ? .16f : .12f);
        Invalidate();
    }

    protected override bool ShowFocusCues => false;

    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        if (Focused && _theme is { } t)
            FocusRing.Draw(e.Graphics, ClientRectangle, t, DeviceDpi);
    }

    protected override void OnGotFocus(EventArgs e)
    {
        base.OnGotFocus(e);
        Invalidate();
    }

    protected override void OnLostFocus(EventArgs e)
    {
        base.OnLostFocus(e);
        Invalidate();
    }
}

/// <summary>An on/off switch drawn by us: click, Space or Enter flips it; a focus ring shows where Tab is.</summary>
sealed class PanelSwitch : Control
{
    Themes.Theme _theme = Themes.Current;
    bool _on;

    public event Action<bool>? Toggled;

    public PanelSwitch()
    {
        SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint | ControlStyles.Selectable, true);
        TabStop = true;
        Cursor = Cursors.Hand;
        AccessibleRole = AccessibleRole.CheckButton;
    }

    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public bool On
    {
        get => _on;
        set
        {
            _on = value;
            AccessibleDescription = value ? L.T("On") : L.T("Off");
            Invalidate();
        }
    }

    public void Apply(Themes.Theme theme)
    {
        _theme = theme;
        BackColor = theme.Background;
        Invalidate();
    }

    public void Flip()
    {
        On = !On;
        Toggled?.Invoke(On);
    }

    protected override void OnClick(EventArgs e)
    {
        base.OnClick(e);
        Focus();
        Flip();
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (e.KeyCode is Keys.Space or Keys.Enter)
        {
            e.Handled = true;
            Flip();
            return;
        }
        base.OnKeyDown(e);
    }

    protected override void OnGotFocus(EventArgs e)
    {
        base.OnGotFocus(e);
        Invalidate();
    }

    protected override void OnLostFocus(EventArgs e)
    {
        base.OnLostFocus(e);
        Invalidate();
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        var g = e.Graphics;
        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        var r = new Rectangle(1, 1, Width - 3, Height - 3);
        using var track = new System.Drawing.Drawing2D.GraphicsPath();
        int d = r.Height;
        track.AddArc(r.X, r.Y, d, d, 90, 180);
        track.AddArc(r.Right - d, r.Y, d, d, 270, 180);
        track.CloseFigure();
        using (var fill = new SolidBrush(_on ? Drawn.Accent(_theme) : _theme.Surface))
            g.FillPath(fill, track);
        using (var line = new Pen(_on ? Drawn.Accent(_theme) : _theme.Line))
            g.DrawPath(line, track);
        int knob = d - 6;
        var k = new Rectangle(_on ? r.Right - knob - 3 : r.X + 3, r.Y + 3, knob, knob);
        using (var dot = new SolidBrush(_on ? Drawn.AccentInk(_theme) : _theme.Text2))
            g.FillEllipse(dot, k);
        if (Focused)
            FocusRing.Draw(g, ClientRectangle, _theme, DeviceDpi);
    }
}

/// <summary>The ring around the control Tab is on.</summary>
static class FocusRing
{
    public static void Draw(Graphics g, Rectangle bounds, Themes.Theme theme, int dpi)
    {
        using var pen = new Pen(Drawn.Accent(theme), Math.Max(2, dpi / 48));
        var r = bounds;
        r.Inflate(-1, -1);
        g.DrawRectangle(pen, r.X, r.Y, r.Width - 1, r.Height - 1);
    }
}
