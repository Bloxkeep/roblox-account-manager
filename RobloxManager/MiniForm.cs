using System.Runtime.InteropServices;

namespace RobloxManager;

/// <summary>
/// A small always-on-top window with the accounts running now — name, game, RAM and process id, with Show and
/// Close for each — for keeping an eye on things while playing or working in another program. Plain WinForms drawn
/// in the app's theme rather than a second browser (the point is that it costs next to nothing), so it keeps
/// working while the main window is hidden in the tray and its browser released. Esc hides it; its place and
/// height are remembered in <see cref="Prefs"/>.
/// </summary>
sealed class MiniForm : Form
{
    const int LogicalWidth = 300, MinHeight = 200, MaxHeight = 4000, Pad = 12, ButtonW = 72, ButtonH = 24, FootH = 52;
    const TextFormatFlags TextFlags = TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix | TextFormatFlags.SingleLine
        | TextFormatFlags.Left | TextFormatFlags.NoPadding;

    readonly AppController _app;
    readonly Action _openApp;
    readonly System.Windows.Forms.Timer _timer = new() { Interval = 2000 };
    readonly Panel _list = new() { Dock = DockStyle.Fill, AutoScroll = true, TabStop = false };
    readonly Label _empty = new() { Dock = DockStyle.Fill, TextAlign = ContentAlignment.MiddleCenter, Padding = new Padding(16) };
    readonly Panel _foot = new() { Dock = DockStyle.Bottom, TabStop = false };
    readonly FlatButton _closeAll = new() { TabIndex = 1000 }, _open = new() { TabIndex = 1001 };
    readonly List<Row> _rows = new();
    readonly Font _font = new("Segoe UI", 9.5f);
    Font _nameFont, _metaFont;
    Themes.Theme _theme = Themes.Current;
    bool _activate = true, _failed;

    /// <summary>Raised after the person hid the window (Esc, its close button).</summary>
    public event Action? Dismissed;

    public MiniForm(AppController app, Action openApp)
    {
        _app = app;
        _openApp = openApp;
        Text = Ui.Title;
        Icon = Ui.AppIcon;
        Font = _font;
        (_nameFont, _metaFont) = MakeFonts();
        FormBorderStyle = FormBorderStyle.Sizable;
        MaximizeBox = false;
        MinimizeBox = false;
        ShowInTaskbar = false;
        TopMost = true;
        Ui.NoteOnTop(this);
        KeyPreview = true;
        StartPosition = FormStartPosition.Manual;
        ClientSize = new Size(Px(LogicalWidth), Px(360));
        // Only the height can be changed: the rows are laid out for this width. (A maximum height of 0 would not
        // mean "any" to Windows' resize limits, so a height taller than any screen stands in.)
        MinimumSize = new Size(Width, Px(MinHeight));
        MaximumSize = new Size(Width, Px(MaxHeight));
        Bounds = DefaultPlace();

        _closeAll.Click += (_, _) => CloseAllClicked();
        _open.Click += (_, _) => _openApp();
        _foot.Height = Px(FootH);
        _foot.Resize += (_, _) => LayoutFoot();
        _foot.Controls.AddRange([_closeAll, _open]);
        _list.Controls.Add(_empty);
        _list.Resize += (_, _) => Relayout();
        // The fill panel first, so the footer (docked after it) takes its strip off the bottom.
        Controls.AddRange([_list, _foot]);
        _timer.Tick += (_, _) => Tick();
        ApplyTheme();
        LayoutFoot();
    }

    int Px(int logical) => LogicalToDeviceUnits(logical);

    int Logical(int pixels) => (int)Math.Round(pixels * 96.0 / DeviceDpi);

    /// <summary>Sized in pixels for this window's own dpi: the form's font follows a screen change by itself, these must be made again.</summary>
    (Font Name, Font Meta) MakeFonts() =>
        (new Font("Segoe UI Semibold", 9.5f * DeviceDpi / 72f, GraphicsUnit.Pixel), new Font("Segoe UI", 8.5f * DeviceDpi / 72f, GraphicsUnit.Pixel));

    /// <summary>Shows the window (or brings it up again); the page's state follows through the owner.</summary>
    /// <param name="activate">false when reopened at start, so the lock screen keeps the keyboard.</param>
    public void Open(bool activate)
    {
        _activate = activate;
        Prefs.MiniOpen = true;
        Prefs.Save();
        if (!Visible)
            Show();
        else if (activate)
            Activate();
        RefreshRows();
    }

    /// <summary>Hides the window at the person's request; forgotten as open, unlike a hide for the app's exit.</summary>
    public void Dismiss()
    {
        if (!Visible)
            return;
        SaveBounds();
        Prefs.MiniOpen = false;
        Prefs.Save();
        Hide();
        Dismissed?.Invoke();
    }

    protected override bool ShowWithoutActivation => !_activate;

    protected override void OnVisibleChanged(EventArgs e)
    {
        base.OnVisibleChanged(e);
        if (Visible)
        {
            RefreshRows();
            _timer.Start();
        }
        else
            _timer.Stop();
    }

    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        // The title bar's X hides, like Esc; the app's own exit closes it for real.
        if (e.CloseReason == CloseReason.UserClosing)
        {
            e.Cancel = true;
            Dismiss();
            return;
        }
        SaveBounds();
        Prefs.Save();
        base.OnFormClosing(e);
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (e.KeyCode == Keys.Escape)
        {
            e.Handled = true;
            Dismiss();
            return;
        }
        base.OnKeyDown(e);
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Frame();
    }

    protected override void OnLoad(EventArgs e)
    {
        base.OnLoad(e);
        // The saved place only now, with the window real: moved to a screen with another scale it then gets a
        // real dpi change (set before the handle existed, it would keep the main screen's scale there). The height
        // is saved in logical pixels, so it is the same height on whichever screen.
        if (SavedPlace(out var saved))
        {
            Location = saved.Location;
            int tallest = Math.Min(MaximumSize.Height, Screen.FromPoint(saved.Location).WorkingArea.Height);
            Height = Math.Max(Math.Min(Px(saved.Height), tallest), MinimumSize.Height);
        }
    }

    protected override void OnResizeEnd(EventArgs e)
    {
        base.OnResizeEnd(e);
        SaveBounds();
        Prefs.Save();
    }

    protected override void OnDpiChanged(DpiChangedEventArgs e)
    {
        base.OnDpiChanged(e);
        // Moved to a screen with another scale: the fixed width, the fonts and the rows follow it.
        MaximumSize = Size.Empty;
        MinimumSize = Size.Empty;
        Width = Px(LogicalWidth) + (Width - ClientSize.Width);
        MinimumSize = new Size(Width, Px(MinHeight));
        MaximumSize = new Size(Width, Px(MaxHeight));
        var old = (_nameFont, _metaFont);
        (_nameFont, _metaFont) = MakeFonts();
        old._nameFont.Dispose();
        old._metaFont.Dispose();
        _foot.Height = Px(FootH);
        LayoutFoot();
        Relayout();
        Invalidate(true);
    }

    // ───────────────────────────── place ─────────────────────────────

    /// <summary>A corner of the main screen, for a first open or when the saved place is on no screen any more.</summary>
    Rectangle DefaultPlace()
    {
        var area = (Screen.PrimaryScreen ?? Screen.AllScreens[0]).WorkingArea;
        return new Rectangle(area.Right - Width - Px(16), area.Bottom - Height - Px(16), Width, Height);
    }

    /// <summary>The saved place (its height in logical pixels), unless no screen shows its title bar any more (a monitor unplugged).</summary>
    bool SavedPlace(out Rectangle r)
    {
        if (!TryParse(Prefs.MiniWindow, out r))
            return false;
        // The middle half of the title bar must be somewhere it can be grabbed.
        var grip = new Rectangle(r.X + r.Width / 4, r.Y + 2, r.Width / 2, Px(24));
        return Screen.AllScreens.Any(s => s.WorkingArea.IntersectsWith(grip));
    }

    static bool TryParse(string? saved, out Rectangle r)
    {
        r = default;
        var parts = saved?.Split(',');
        if (parts is not { Length: 4 } || !int.TryParse(parts[0], out int x) || !int.TryParse(parts[1], out int y)
            || !int.TryParse(parts[2], out int w) || !int.TryParse(parts[3], out int h) || w <= 0 || h <= 0)
            return false;
        r = new Rectangle(x, y, w, h);
        return true;
    }

    /// <summary>Noted, not written: the callers save prefs once they are done.</summary>
    void SaveBounds()
    {
        if (Visible && WindowState == FormWindowState.Normal)
            Prefs.MiniWindow = $"{Left},{Top},{LogicalWidth},{Logical(Height)}";
    }

    // ───────────────────────────── look ─────────────────────────────

    [DllImport("dwmapi.dll")]
    static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    /// <summary>Title bar, border and corners like the main window and the tray menu (Windows 11 does the rounding).</summary>
    void Frame()
    {
        Ui.UseThemedTitleBar(this, _theme);
        const int DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWA_BORDER_COLOR = 34, DWMWCP_ROUND = 2;
        int round = DWMWCP_ROUND;
        DwmSetWindowAttribute(Handle, DWMWA_WINDOW_CORNER_PREFERENCE, ref round, sizeof(int));
        int border = ColorTranslator.ToWin32(_theme.Line);
        DwmSetWindowAttribute(Handle, DWMWA_BORDER_COLOR, ref border, sizeof(int));
    }

    /// <summary>A new theme from Settings, or Windows switching between light and dark.</summary>
    public void ApplyTheme()
    {
        _theme = Themes.Current;
        BackColor = _list.BackColor = _foot.BackColor = _empty.BackColor = _theme.Background;
        _empty.ForeColor = _theme.Text2;
        _closeAll.Apply(_theme);
        _open.Apply(_theme);
        _closeAll.ForeColor = _theme.Bad;
        foreach (var row in _rows)
            row.Apply(_theme);
        if (IsHandleCreated)
            Frame();
        Invalidate(true);
    }

    static void SetText(Control control, string text)
    {
        if (control.Text != text)
            control.Text = text;
    }

    void LayoutFoot()
    {
        int pad = Px(Pad), h = Px(30), w = (_foot.ClientSize.Width - pad * 3) / 2, y = (_foot.ClientSize.Height - h) / 2;
        _closeAll.SetBounds(pad, y, w, h);
        _open.SetBounds(pad * 2 + w, y, w, h);
    }

    // ───────────────────────────── rows ─────────────────────────────

    /// <summary>A refresh that fails is logged (the first of a run of failures), not shown: a box every 2 s would pile up, and the next tick tries again.</summary>
    void Tick()
    {
        try
        {
            RefreshRows();
            _failed = false;
        }
        catch (Exception ex)
        {
            if (!_failed)
                Log.Write(ex);
            _failed = true;
        }
    }

    /// <summary>
    /// Every two seconds while visible: the rows are matched to the clients open now, texts taken fresh (the
    /// language can change). A row stays with its account (not its place in the list): when one above it goes
    /// its row moves up rather than taking over the next account's texts under a hand about to click Close.
    /// </summary>
    void RefreshRows()
    {
        if (!Visible || IsDisposed)
            return;
        var clients = _app.Unlocked ? _app.RunningClients() : [];
        SetText(_closeAll, L.T("Close all"));
        SetText(_open, L.T("Open Bloxkeep"));
        // Locked, nothing is known about the clients (the count would be from before the lock).
        _closeAll.Enabled = _app.Unlocked && (clients.Count > 0 || _app.RobloxCount > 0);
        var byId = _rows.ToDictionary(r => r.ClientId);
        var rows = new List<Row>(clients.Count);
        for (int i = 0; i < clients.Count; i++)
        {
            if (!byId.Remove(clients[i].Id, out var row))
            {
                row = new Row(this);
                row.Apply(_theme);
                _list.Controls.Add(row);
            }
            row.Set(clients[i], i);
            rows.Add(row);
        }
        foreach (var gone in byId.Values)
        {
            _list.Controls.Remove(gone);
            gone.Dispose();
        }
        _rows.Clear();
        _rows.AddRange(rows);
        SetText(_empty, _app.Unlocked ? L.T("No Roblox window is open.") : L.T("Unlock Bloxkeep to see the accounts."));
        _empty.Visible = clients.Count == 0;
        Relayout();
    }

    /// <summary>Three lines of text, or the two buttons, whichever is taller.</summary>
    int RowHeight()
    {
        int lines = TextRenderer.MeasureText("Ag", _nameFont).Height + TextRenderer.MeasureText("Ag", Font).Height
            + TextRenderer.MeasureText("Ag", _metaFont).Height + Px(2);
        return Math.Max(lines, Px(ButtonH) * 2 + Px(6)) + Px(8) * 2;
    }

    void Relayout()
    {
        int rowH = RowHeight();
        // The scrollbar first (it narrows the rows), then the rows in the scrolled position.
        _list.AutoScrollMinSize = new Size(0, _rows.Count * rowH);
        int width = _list.ClientSize.Width, y = _list.AutoScrollPosition.Y;
        foreach (var row in _rows)
        {
            row.PlaceAt(y, width, rowH);
            y += rowH;
        }
    }

    void CloseAllClicked()
    {
        // The same question the page asks, with its "Don't ask again".
        if (!Prefs.SkipConfirm.Contains("closeAll"))
        {
            var (yes, remember) = Ui.Modal(() => Dialog.ConfirmRemember(this, L.T("Close ALL Roblox running on this PC (including Roblox in the system tray)?"),
                L.T("Close all"), L.T("Cancel"), danger: true));
            if (!yes)
                return;
            if (remember)
                _app.SkipConfirm("closeAll");
        }
        _app.CloseAllClients();
        RefreshRows();
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _timer.Dispose();
            _nameFont.Dispose();
            _metaFont.Dispose();
            // A screen change swaps in a scaled font and leaves the first one behind.
            if (!ReferenceEquals(Font, _font))
                Font.Dispose();
            _font.Dispose();
        }
        base.Dispose(disposing);
    }

    static Color Mix(Color a, Color b, float amount) => Color.FromArgb(
        (int)(a.R + (b.R - a.R) * amount), (int)(a.G + (b.G - a.G) * amount), (int)(a.B + (b.B - a.B) * amount));

    static string Ram(long mb) => mb <= 0 ? "…" : mb >= 1024 ? $"{mb / 1024.0:0.0} GB" : $"{mb} MB";

    /// <summary>One client: its texts drawn here, its two buttons real controls so Tab reaches them.</summary>
    sealed class Row : Control
    {
        /// <summary>No click this soon after the row appeared or moved: it was meant for what was there before.</summary>
        const int SettleMs = 500;
        /// <summary>After Close, the buttons stay off this long; Roblox normally ends well within it, and the row with it.</summary>
        const int CloseWaitMs = 10_000;

        readonly MiniForm _owner;
        readonly FlatButton _show = new() { TabIndex = 0 }, _close = new() { TabIndex = 1 };
        AppController.MiniClient? _client;
        Themes.Theme _theme = Themes.Current;
        long _movedAt = Environment.TickCount64, _closedAt;

        public Row(MiniForm owner)
        {
            _owner = owner;
            TabStop = false;
            SetStyle(ControlStyles.OptimizedDoubleBuffer | ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.ResizeRedraw, true);
            _show.Click += (_, _) =>
            {
                if (_client is { } c && Settled)
                    RobloxProcesses.Focus(c.Pid);
            };
            _close.Click += (_, _) =>
            {
                if (_client is not { } c || !Settled || _closedAt != 0)
                    return;
                // Off until the client is gone (a second click would force-kill a game mid-way), saying why.
                _closedAt = Environment.TickCount64;
                _show.Enabled = _close.Enabled = false;
                SetText(_close, L.T("Closing…"));
                _owner._app.CloseClient(c.Id);
            };
            Controls.AddRange([_show, _close]);
        }

        public Guid ClientId => _client?.Id ?? Guid.Empty;

        bool Settled => Environment.TickCount64 - _movedAt >= SettleMs;

        /// <summary>In the window's dpi, as the row heights and fonts are (a row's own may lag while it is being made or moved between screens).</summary>
        int Px(int logical) => _owner is null ? LogicalToDeviceUnits(logical) : _owner.Px(logical);

        public void Apply(Themes.Theme theme)
        {
            _theme = theme;
            BackColor = theme.Background;
            _show.Apply(theme);
            _close.Apply(theme);
            _close.ForeColor = theme.Bad;
            Invalidate();
        }

        public void Set(AppController.MiniClient client, int index)
        {
            TabIndex = index;
            SetText(_show, L.T("Show"));
            if (_closedAt != 0 && Environment.TickCount64 - _closedAt >= CloseWaitMs)
            {
                // The close didn't take (Roblox asked something, or hung): the buttons come back.
                _closedAt = 0;
                _show.Enabled = _close.Enabled = true;
            }
            SetText(_close, _closedAt != 0 ? L.T("Closing…") : L.T("Close"));
            if (_client == client)
                return;
            _client = client;
            Invalidate();
        }

        /// <summary>Placed by the owner's layout; a row that moved under the pointer ignores clicks for a moment.</summary>
        public void PlaceAt(int y, int width, int height)
        {
            if (Top != y)
                _movedAt = Environment.TickCount64;
            SetBounds(0, y, width, height);
        }

        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            int w = Px(ButtonW), h = Px(ButtonH), gap = Px(6);
            int x = Width - Px(Pad) - w, y = (Height - (h * 2 + gap)) / 2;
            _show.SetBounds(x, y, w, h);
            _close.SetBounds(x, y + h + gap, w, h);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            var g = e.Graphics;
            g.Clear(_theme.Background);
            if (_client is not { } c)
                return;
            int pad = Px(Pad);
            // Long names and game titles end in "…" before the buttons.
            var line = new Rectangle(pad, Px(8), Math.Max(0, _show.Left - Px(10) - pad), 0);
            Line(g, ref line, c.Label, _owner._nameFont, _theme.Text);
            Line(g, ref line, c.Game, Font, _theme.Text2);
            Line(g, ref line, $"PID {c.Pid}  ·  {Ram(c.RamMb)}", _owner._metaFont, _theme.Text2);
            using var pen = new Pen(_theme.Line);
            g.DrawLine(pen, pad, Height - 1, Width - pad, Height - 1);
        }

        static void Line(Graphics g, ref Rectangle r, string text, Font font, Color color)
        {
            r.Height = TextRenderer.MeasureText(g, "Ag", font).Height;
            TextRenderer.DrawText(g, text, font, r, color, TextFlags);
            r.Y += r.Height + 1;
        }
    }

    /// <summary>A plain button in the theme's colours: a flat surface with a thin line, no gradient.</summary>
    sealed class FlatButton : Button
    {
        Themes.Theme? _theme;

        public FlatButton()
        {
            FlatStyle = FlatStyle.Flat;
            FlatAppearance.BorderSize = 1;
            UseVisualStyleBackColor = false;
            Cursor = Cursors.Hand;
        }

        public void Apply(Themes.Theme theme)
        {
            _theme = theme;
            BackColor = theme.Surface;
            ForeColor = theme.Text;
            FlatAppearance.BorderColor = theme.Line;
            FlatAppearance.MouseOverBackColor = Mix(theme.Surface, theme.Text, theme.Dark ? .1f : .07f);
            FlatAppearance.MouseDownBackColor = Mix(theme.Surface, theme.Text, theme.Dark ? .16f : .12f);
            Invalidate();
        }

        /// <summary>Off, drawn here: Windows' own greyed text ignores the theme (and shows nearly nothing on a dark one).</summary>
        protected override void OnPaint(PaintEventArgs e)
        {
            if (Enabled || _theme is not { } t)
            {
                base.OnPaint(e);
                return;
            }
            var g = e.Graphics;
            g.Clear(Mix(t.Surface, t.Background, .6f));
            using var pen = new Pen(Mix(t.Line, t.Background, .4f));
            g.DrawRectangle(pen, 0, 0, Width - 1, Height - 1);
            TextRenderer.DrawText(g, Text, Font, ClientRectangle, Mix(t.Text2, t.Background, .4f),
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix | TextFormatFlags.SingleLine);
        }
    }
}
