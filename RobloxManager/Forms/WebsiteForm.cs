using System.ComponentModel;
using System.Drawing.Drawing2D;

namespace RobloxManager;

/// <summary>
/// Bloxkeep's own browser for www.roblox.com: one window, one tab per account (<see cref="WebsiteTab"/>, each its own
/// private sign-in). Account menu › Open website adds that account's tab or goes back to it; the + at the end of the
/// tabs (and Add account › Open roblox.com) opens a blank one to sign in on the site. Closing the window closes every tab.
/// </summary>
public sealed class WebsiteForm : Form
{
    /// <summary>Windows 11's icon font, or Windows 10's.</summary>
    internal static readonly string IconFont = FontFamily.Families.Any(f => f.Name == "Segoe Fluent Icons") ? "Segoe Fluent Icons" : "Segoe MDL2 Assets";

    readonly Themes.Theme _theme = Themes.Current;
    readonly Panel _strip, _pages;
    readonly Button _newTab;
    readonly ToolTip _tips = new();
    readonly List<(WebsiteTab Tab, TabChip Chip)> _tabs = new();
    WebsiteTab? _selected;
    bool _closingAll, _done;

    /// <summary>Opens a blank tab in this window (the + button, Ctrl+T); set by the app, which keeps the list of tabs.</summary>
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public Action? NewBlankTab { get; set; }

    public IReadOnlyList<WebsiteTab> Tabs => _tabs.Select(t => t.Tab).ToList();
    public WebsiteTab? SelectedTab => _selected;

    public WebsiteForm()
    {
        Text = "Roblox";
        Icon = Ui.AppIcon;
        BackColor = _theme.Background;
        StartPosition = FormStartPosition.Manual;
        MinimumSize = new Size(LogicalToDeviceUnits(480), LogicalToDeviceUnits(360));
        KeyPreview = true;

        _strip = new Panel { Dock = DockStyle.Top, Height = LogicalToDeviceUnits(38), BackColor = _theme.Background };
        _newTab = new Button
        {
            Text = "",
            Font = new Font(IconFont, 9f),
            FlatStyle = FlatStyle.Flat,
            BackColor = _theme.Background,
            ForeColor = _theme.Text2,
            Cursor = Cursors.Hand,
            TabStop = false,
            Size = new Size(LogicalToDeviceUnits(30), LogicalToDeviceUnits(28)),
            AccessibleName = L.T("New tab"),
        };
        _newTab.FlatAppearance.BorderSize = 0;
        _newTab.FlatAppearance.MouseOverBackColor = Blend(_theme.Background, _theme.Surface, .6);
        _tips.SetToolTip(_newTab, L.T("New tab, to sign in on roblox.com (Ctrl+T)"));
        _newTab.Click += (_, _) => NewBlankTab?.Invoke();
        _strip.Controls.Add(_newTab);
        _strip.Resize += (_, _) => LayoutChips();
        _pages = new Panel { Dock = DockStyle.Fill, BackColor = _theme.Background };
        Controls.Add(_pages);
        Controls.Add(_strip);
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Ui.UseThemedTitleBar(this, _theme);
    }

    /// <summary>Opens beside the main window, centred on its screen.</summary>
    public void PlaceNear(Form owner)
    {
        var area = Screen.FromControl(owner).WorkingArea;
        var size = new Size(Math.Min(LogicalToDeviceUnits(1280), area.Width), Math.Min(LogicalToDeviceUnits(860), area.Height));
        Bounds = new Rectangle(area.Left + (area.Width - size.Width) / 2, area.Top + (area.Height - size.Height) / 2, size.Width, size.Height);
    }

    /// <summary>Adds a tab at the end and shows it.</summary>
    public void AddTab(WebsiteTab tab)
    {
        var chip = new TabChip(_theme) { Title = tab.Title };
        _tips.SetToolTip(chip, tab.Title);
        chip.Picked += () => SelectTab(tab);
        chip.CloseClicked += () => _ = CloseTabAsync(tab);
        tab.TitleChanged += () =>
        {
            chip.Title = tab.Title;
            _tips.SetToolTip(chip, tab.Title);
            if (_selected == tab)
                SyncTitle();
        };
        tab.CloseRequested += () => _ = CloseTabAsync(tab);
        tab.Visible = false;
        _tabs.Add((tab, chip));
        _pages.Controls.Add(tab);
        _strip.Controls.Add(chip);
        LayoutChips();
        SelectTab(tab);
    }

    public void SelectTab(WebsiteTab tab)
    {
        if (!_tabs.Any(t => t.Tab == tab))
            return;
        _selected = tab;
        foreach (var (other, chip) in _tabs)
        {
            chip.Selected = other == tab;
            if (other != tab)
                other.Visible = false;
        }
        tab.Visible = true;
        tab.BringToFront();
        SyncTitle();
        if (ContainsFocus)
            tab.FocusPage();
    }

    void SyncTitle() => Text = _selected is { } tab && tab.Title != "Roblox" ? L.F("{0} — Roblox", tab.Title) : "Roblox";

    /// <summary>Closes one tab (after a last read of a saved account's session); the window goes with its last tab.</summary>
    public async Task CloseTabAsync(WebsiteTab tab)
    {
        int index = _tabs.FindIndex(t => t.Tab == tab);
        if (index < 0)
            return;
        var chip = _tabs[index].Chip;
        _tabs.RemoveAt(index);
        _strip.Controls.Remove(chip);
        chip.Dispose();
        if (_selected == tab)
        {
            _selected = null;
            if (_tabs.Count > 0)
                SelectTab(_tabs[Math.Min(index, _tabs.Count - 1)].Tab);
        }
        LayoutChips();
        tab.Visible = false;
        await tab.CloseAsync();
        _pages.Controls.Remove(tab);
        tab.Dispose();
        if (_tabs.Count == 0 && !_closingAll && !IsDisposed)
        {
            _done = true;
            Close();
        }
    }

    /// <summary>Closes every tab at once, without the last read of the sessions (the vault is locking, or the app quitting).</summary>
    public void CloseNow()
    {
        _closingAll = _done = true;
        foreach (var (tab, _) in _tabs.ToList())
            tab.CloseNow();
        _tabs.Clear();
        Close();
    }

    protected override async void OnFormClosing(FormClosingEventArgs e)
    {
        base.OnFormClosing(e);
        if (e.Cancel || _done || _tabs.Count == 0)
            return;
        // Each saved account's session is read once more before its tab goes, a moment at most.
        e.Cancel = true;
        _done = _closingAll = true;
        Hide();
        try
        {
            await Task.WhenAll(_tabs.Select(t => t.Tab.CloseAsync()).ToList());
        }
        finally
        {
            _tabs.Clear();
            if (!IsDisposed)
                Close();
        }
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        foreach (var (tab, _) in _tabs.ToList())
            tab.CloseNow();
        _tabs.Clear();
        _tips.Dispose();
        base.OnFormClosed(e);
    }

    /// <summary>Ctrl+T a blank tab, Ctrl+W closes this one, Ctrl+Tab / Ctrl+Shift+Tab (or Ctrl+PgDn / PgUp) the next or previous.</summary>
    protected override bool ProcessCmdKey(ref Message msg, Keys keyData)
    {
        switch (keyData)
        {
            case Keys.Control | Keys.T:
                NewBlankTab?.Invoke();
                return true;
            case Keys.Control | Keys.W when _selected is { } tab:
                _ = CloseTabAsync(tab);
                return true;
            case Keys.Control | Keys.Tab or Keys.Control | Keys.PageDown:
                Step(1);
                return true;
            case Keys.Control | Keys.Shift | Keys.Tab or Keys.Control | Keys.PageUp:
                Step(-1);
                return true;
        }
        return base.ProcessCmdKey(ref msg, keyData);
    }

    void Step(int by)
    {
        if (_tabs.Count < 2 || _selected is null)
            return;
        int i = _tabs.FindIndex(t => t.Tab == _selected);
        SelectTab(_tabs[(i + by + _tabs.Count) % _tabs.Count].Tab);
        _selected?.FocusPage();
    }

    /// <summary>Tabs share the strip's width (wide when few, narrower when many), with + right after the last one.</summary>
    void LayoutChips()
    {
        int pad = LogicalToDeviceUnits(8), gap = LogicalToDeviceUnits(4), height = LogicalToDeviceUnits(32);
        int room = _strip.ClientSize.Width - pad * 2 - _newTab.Width - gap;
        int count = Math.Max(1, _tabs.Count);
        int width = Math.Clamp((room - gap * (count - 1)) / count, LogicalToDeviceUnits(72), LogicalToDeviceUnits(220));
        int x = pad, top = _strip.ClientSize.Height - height;
        foreach (var (_, chip) in _tabs)
        {
            chip.Bounds = new Rectangle(x, top, width, height);
            x += width + gap;
        }
        _newTab.Location = new Point(Math.Min(x, _strip.ClientSize.Width - pad - _newTab.Width), top + (height - _newTab.Height) / 2);
    }

    static Color Blend(Color a, Color b, double t) =>
        Color.FromArgb((int)(a.R + (b.R - a.R) * t), (int)(a.G + (b.G - a.G) * t), (int)(a.B + (b.B - a.B) * t));

    /// <summary>A tab in the strip: its title, and ✕ to close it (shown on the chosen tab and under the pointer).</summary>
    internal sealed class TabChip : Control
    {
        readonly Themes.Theme _theme;
        bool _selected, _hover, _overClose;
        public event Action? Picked;
        public event Action? CloseClicked;

        public TabChip(Themes.Theme theme)
        {
            _theme = theme;
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint | ControlStyles.ResizeRedraw, true);
            Cursor = Cursors.Hand;
            AccessibleRole = AccessibleRole.PageTab;
        }

        [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
        public string Title
        {
            get => Text;
            set
            {
                Text = value;
                AccessibleName = value;
                Invalidate();
            }
        }

        [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
        public bool Selected
        {
            get => _selected;
            set
            {
                _selected = value;
                Invalidate();
            }
        }

        Rectangle CloseBox
        {
            get
            {
                int size = LogicalToDeviceUnits(20);
                return new Rectangle(Width - size - LogicalToDeviceUnits(8), (Height - size) / 2, size, size);
            }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            var g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Color back = _selected ? _theme.Surface : _hover ? Blend(_theme.Background, _theme.Surface, .55) : _theme.Background;
            int r = LogicalToDeviceUnits(9);
            using (var path = new GraphicsPath())
            {
                // Rounded on top only: the chosen tab runs into its address bar below.
                path.AddArc(0, 0, r * 2, r * 2, 180, 90);
                path.AddArc(Width - r * 2 - 1, 0, r * 2, r * 2, 270, 90);
                path.AddLine(Width - 1, Height, 0, Height);
                path.CloseFigure();
                using var brush = new SolidBrush(back);
                g.FillPath(brush, path);
            }
            bool showClose = _selected || _hover;
            int left = LogicalToDeviceUnits(12), right = showClose ? CloseBox.Left - LogicalToDeviceUnits(4) : Width - LogicalToDeviceUnits(10);
            TextRenderer.DrawText(g, Text, Font, new Rectangle(left, 0, Math.Max(0, right - left), Height),
                _selected ? _theme.Text : _theme.Text2, TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix | TextFormatFlags.SingleLine);
            if (showClose)
            {
                var box = CloseBox;
                if (_overClose)
                {
                    using var hot = new SolidBrush(Blend(back, _theme.Text2, .22));
                    g.FillEllipse(hot, box);
                }
                using var glyph = new Font(IconFont, 7.5f);
                TextRenderer.DrawText(g, "", glyph, box, _theme.Text2, TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPadding);
            }
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            base.OnMouseMove(e);
            bool over = CloseBox.Contains(e.Location);
            if (!_hover || over != _overClose)
            {
                _hover = true;
                _overClose = over;
                Invalidate();
            }
        }

        protected override void OnMouseLeave(EventArgs e)
        {
            base.OnMouseLeave(e);
            _hover = _overClose = false;
            Invalidate();
        }

        protected override void OnMouseUp(MouseEventArgs e)
        {
            base.OnMouseUp(e);
            if (e.Button == MouseButtons.Middle || (e.Button == MouseButtons.Left && (_selected || _hover) && CloseBox.Contains(e.Location)))
                CloseClicked?.Invoke();
            else if (e.Button == MouseButtons.Left)
                Picked?.Invoke();
        }
    }
}
